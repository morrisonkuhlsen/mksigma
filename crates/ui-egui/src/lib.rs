//! MK Sigma egui frontend: a File menu, Data View and Variable View (both editable), a syntax
//! editor with the session history, and the Output Viewer. It is thin: it reads the session and
//! acts through `Session`; nothing below this crate depends on egui. Platform services (the
//! native file dialog) come in through `Services`, so tests drive the UI without them.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use std::path::{Path, PathBuf};

use mksigma_engine::Session;

mod edit;
mod output;
mod views;

pub use edit::{Editing, Field, Target};
pub use output::{ExportFormat, OutputState};
pub use views::ValueLabelsDialog;

use edit::{Commit, syntax_for, value_labels_syntax};
use views::DialogOutcome;

/// A file dialog: shows it (with a suggested name, for saving) and returns the chosen path, or
/// `None` when cancelled.
pub type PickOpen = Box<dyn Fn() -> Option<PathBuf>>;
pub type PickSave = Box<dyn Fn(&str) -> Option<PathBuf>>;

/// What the app provides from the platform.
#[derive(Default)]
pub struct Services {
    /// Shows an open-file dialog; `None` when cancelled.
    pub pick_open: Option<PickOpen>,
    /// Shows a save-file dialog suggesting a file name; `None` when cancelled.
    pub pick_save: Option<PickSave>,
    /// Seconds since 1970, for the saved file's creation time.
    pub now: Option<Box<dyn Fn() -> i64>>,
    /// Shows a save dialog for exporting the Output, suggesting a file name whose extension
    /// (`.html` or `.pdf`) says the format.
    pub pick_export: Option<PickSave>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    Data,
    Variables,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Status {
    Info(String),
    Warning(String),
    Error(String),
}

pub struct SigmaApp {
    pub session: Session,
    pub view: View,
    /// Show value labels instead of values in Data View.
    pub value_labels: bool,
    pub syntax: String,
    pub status: Option<Status>,
    /// Name of the open file, shown in the window title.
    pub file_name: Option<String>,
    /// Where Save writes: the file opened or last saved, if it came from disk.
    pub file_path: Option<PathBuf>,
    /// The cell being edited, if any.
    pub editing: Option<Editing>,
    pub value_labels_dialog: Option<ValueLabelsDialog>,
    /// Which Output tables are pivoted.
    pub output_state: OutputState,
    services: Services,
    title: String,
}

impl Default for SigmaApp {
    fn default() -> Self {
        Self::new(Services::default())
    }
}

pub(crate) const ROW_HEIGHT: f32 = 20.0;

/// An undo label short enough for a menu.
fn short(label: &str) -> String {
    let mut s: String = label.chars().take(40).collect();
    if label.chars().count() > 40 {
        s.push('…');
    }
    s
}

impl SigmaApp {
    pub fn new(services: Services) -> Self {
        let (session, status) = match Session::with_sample() {
            Ok(s) => (s, Some(Status::Info("Sample dataset. Use File > Open… or drop a .sav file on the window.".into()))),
            Err(e) => (Session::new(), Some(Status::Error(e.to_string()))),
        };
        Self {
            session,
            view: View::Data,
            value_labels: false,
            syntax: "FREQUENCIES VARIABLES=age /FORMAT=NOTABLE /STATISTICS.\nDESCRIPTIVES VARIABLES=age income score.".to_string(),
            status,
            file_name: None,
            file_path: None,
            editing: None,
            value_labels_dialog: None,
            output_state: OutputState::default(),
            services,
            title: String::new(),
        }
    }

    /// Opens a system file from disk.
    pub fn open_path(&mut self, path: &Path) {
        match std::fs::read(path) {
            Ok(bytes) => {
                let name = path.file_name().map_or_else(|| path.display().to_string(), |n| n.to_string_lossy().into_owned());
                if self.open_bytes(&name, &bytes) {
                    self.file_path = Some(path.to_path_buf());
                }
            }
            Err(e) => self.status = Some(Status::Error(format!("{}: {e}", path.display()))),
        }
    }

    /// Writes the Output as a self-contained HTML page or a PDF document.
    pub fn export(&mut self, path: &Path, format: ExportFormat) {
        let title = self.file_name.as_deref().map_or_else(|| "MK Sigma Output".to_string(), |f| format!("{f} — Output"));
        let bytes = match format {
            ExportFormat::Html => self.session.output.to_html(&title).into_bytes(),
            ExportFormat::Pdf => self.session.output.to_pdf(&title),
        };
        match std::fs::write(path, bytes) {
            Ok(()) => self.status = Some(Status::Info(format!("Exported the Output to {}", path.display()))),
            Err(e) => self.status = Some(Status::Error(format!("{}: {e}", path.display()))),
        }
    }

    fn export_dialog(&mut self, format: ExportFormat) {
        let stem = self.file_name.as_deref().and_then(|f| f.rsplit_once('.').map(|(s, _)| s.to_string())).unwrap_or_else(|| "output".to_string());
        let extension = match format {
            ExportFormat::Html => "html",
            ExportFormat::Pdf => "pdf",
        };
        if let Some(path) = self.services.pick_export.as_ref().and_then(|pick| pick(&format!("{stem}.{extension}"))) {
            self.export(&path, format);
        }
    }

    /// Saves to the current file, or asks where when there is none.
    pub fn save(&mut self) {
        match self.file_path.clone() {
            Some(path) => self.save_to(&path),
            None => self.save_as(),
        }
    }

    pub fn save_as(&mut self) {
        let suggested = self.file_name.clone().unwrap_or_else(|| "Untitled.sav".to_string());
        if let Some(path) = self.services.pick_save.as_ref().and_then(|pick| pick(&suggested)) {
            self.save_to(&path);
        }
    }

    /// Writes next to the target and renames over it, so a failed save never leaves a half-written
    /// file in place of the old one.
    pub fn save_to(&mut self, path: &Path) {
        let now = self.services.now.as_ref().map_or(0, |now| now());
        let (bytes, warnings) = match self.session.save_sav(mksigma_engine::timestamp(now)) {
            Ok(written) => written,
            Err(e) => {
                self.status = Some(Status::Error(e.to_string()));
                return;
            }
        };
        let temporary = path.with_extension("sav.saving");
        let written = std::fs::write(&temporary, &bytes).and_then(|()| std::fs::rename(&temporary, path));
        let name = path.file_name().map_or_else(|| path.display().to_string(), |n| n.to_string_lossy().into_owned());
        match written {
            Ok(()) => {
                let summary = format!("Saved {name}");
                self.status =
                    Some(if warnings.is_empty() { Status::Info(summary) } else { Status::Warning(format!("{summary}. {}", warnings.join("; "))) });
                self.file_name = Some(name);
                self.file_path = Some(path.to_path_buf());
            }
            Err(e) => {
                let _ = std::fs::remove_file(&temporary);
                // The data wasn't written, so it still has unsaved changes.
                self.session.modified = true;
                self.status = Some(Status::Error(format!("{name}: {e}")));
            }
        }
    }

    /// Opens a system file's bytes (a path read, or a file dropped where there is no path).
    /// Returns whether it opened; on failure the open dataset stays.
    pub fn open_bytes(&mut self, name: &str, bytes: &[u8]) -> bool {
        match self.session.open_sav(bytes) {
            Ok(warnings) => {
                let ds = &self.session.dataset;
                let summary = format!("{name}: {} cases, {} variables", ds.n_cases(), ds.variables().len());
                self.status =
                    Some(if warnings.is_empty() { Status::Info(summary) } else { Status::Warning(format!("{summary}. {}", warnings.join("; "))) });
                self.file_name = Some(name.to_string());
                self.file_path = None;
                self.editing = None;
                self.value_labels_dialog = None;
                true
            }
            Err(e) => {
                self.status = Some(Status::Error(format!("{name}: {e}")));
                false
            }
        }
    }

    fn open_dialog(&mut self) {
        if let Some(path) = self.services.pick_open.as_ref().and_then(|pick| pick()) {
            self.open_path(&path);
        }
    }

    fn run_syntax(&mut self) {
        self.status = match self.session.run_syntax(&self.syntax) {
            Ok(()) => None,
            Err(e) => Some(Status::Error(e.to_string())),
        };
    }

    /// Shows an engine error in the status line.
    fn report(&mut self, result: Result<(), mksigma_engine::EngineError>) {
        if let Err(e) = result {
            self.status = Some(Status::Error(e.to_string()));
        }
    }

    pub fn undo(&mut self) {
        let label = self.session.undo_label().map(str::to_string);
        self.editing = None;
        match self.session.undo() {
            Ok(true) => self.status = label.map(|l| Status::Info(format!("Undid {l}"))),
            Ok(false) => {}
            Err(e) => self.status = Some(Status::Error(e.to_string())),
        }
    }

    pub fn redo(&mut self) {
        let label = self.session.redo_label().map(str::to_string);
        self.editing = None;
        match self.session.redo() {
            Ok(true) => self.status = label.map(|l| Status::Info(format!("Redid {l}"))),
            Ok(false) => {}
            Err(e) => self.status = Some(Status::Error(e.to_string())),
        }
    }

    /// Runs syntax generated by an edit; the status line shows what ran, or why it didn't.
    pub fn run_edit(&mut self, syntax: &str) {
        self.status = Some(match self.session.run_syntax(syntax) {
            Ok(()) => Status::Info(syntax.to_string()),
            Err(e) => Status::Error(format!("{e} ({syntax})")),
        });
    }

    fn apply_edit(&mut self, target: Target, text: &str) {
        match target {
            Target::Step(i) => {
                let unchanged = self.session.steps().get(i).and_then(|s| s.syntax()).is_some_and(|s| s.trim() == text.trim());
                if !unchanged {
                    let result = self.session.replace_step(i, text);
                    self.report(result);
                }
            }
            Target::Cell { case, var } => {
                if let Err(e) = self.session.set_cell(case, var, text) {
                    self.status = Some(Status::Error(e.to_string()));
                }
            }
            Target::Variable(i, Field::Name) if i == self.session.dataset.variables().len() => {
                let name = text.trim();
                if !name.is_empty() {
                    self.run_edit(&format!("NUMERIC {name}."));
                }
            }
            Target::Variable(i, field) => {
                let syntax = self.session.dataset.variables().get(i).and_then(|var| syntax_for(var, field, text));
                if let Some(syntax) = syntax {
                    self.run_edit(&syntax);
                }
            }
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        self.handle_input(ui.ctx());
        let title = format!("{}{} — MK Sigma", if self.session.modified { "• " } else { "" }, self.file_name.as_deref().unwrap_or("Untitled"));
        if title != self.title {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.title = title;
        }

        ui.horizontal(|ui| {
            ui.menu_button("File", |ui| {
                if ui.button("Open…").clicked() {
                    ui.close();
                    self.open_dialog();
                }
                if ui.button("Save").clicked() {
                    ui.close();
                    self.save();
                }
                if ui.button("Save As…").clicked() {
                    ui.close();
                    self.save_as();
                }
                ui.separator();
                let has_output = !self.session.output.items.is_empty();
                if ui.add_enabled(has_output, egui::Button::new("Export Output as HTML…")).clicked() {
                    ui.close();
                    self.export_dialog(ExportFormat::Html);
                }
                if ui.add_enabled(has_output, egui::Button::new("Export Output as PDF…")).clicked() {
                    ui.close();
                    self.export_dialog(ExportFormat::Pdf);
                }
            });
            ui.menu_button("Edit", |ui| {
                let undo = self.session.undo_label().map(|l| format!("Undo {}", short(l)));
                if ui.add_enabled(undo.is_some(), egui::Button::new(undo.as_deref().unwrap_or("Undo"))).clicked() {
                    ui.close();
                    self.undo();
                }
                let redo = self.session.redo_label().map(|l| format!("Redo {}", short(l)));
                if ui.add_enabled(redo.is_some(), egui::Button::new(redo.as_deref().unwrap_or("Redo"))).clicked() {
                    ui.close();
                    self.redo();
                }
            });
            ui.separator();
            ui.selectable_value(&mut self.view, View::Data, "Data View");
            ui.selectable_value(&mut self.view, View::Variables, "Variable View");
            if self.view == View::Data {
                ui.separator();
                ui.checkbox(&mut self.value_labels, "Value labels");
            }
        });
        ui.separator();

        let status_height = 28.0;
        let body = (ui.available_height() - status_height).max(100.0);
        // Data and Variable View get 65% of the width, the Output Viewer the rest.
        let gap = 12.0;
        let left_width = ((ui.available_width() - gap) * 0.65).floor();
        let right_width = ui.available_width() - gap - left_width;
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            let top_down = egui::Layout::top_down(egui::Align::Min);
            ui.allocate_ui_with_layout(egui::vec2(left_width, body), top_down, |ui| {
                ui.set_width(left_width);
                ui.set_height(body);
                self.left_pane(ui);
            });
            ui.allocate_ui_with_layout(egui::vec2(right_width, body), top_down, |ui| {
                ui.set_width(right_width);
                ui.set_height(body);
                if let output::OutputAction::Export(format) = output::output_view(ui, &mut self.session.output, &mut self.output_state) {
                    self.export_dialog(format);
                }
            });
        });
        self.status_line(ui);
        self.dialogs(ui.ctx());
    }

    fn handle_input(&mut self, ctx: &egui::Context) {
        let shortcut = |modifiers, key| ctx.input_mut(|i| i.consume_shortcut(&egui::KeyboardShortcut::new(modifiers, key)));
        // The longer shortcut first, so Ctrl+Shift+S isn't taken as Ctrl+S.
        if shortcut(egui::Modifiers::COMMAND | egui::Modifiers::SHIFT, egui::Key::S) {
            self.save_as();
        } else if shortcut(egui::Modifiers::COMMAND, egui::Key::S) {
            self.save();
        }
        if shortcut(egui::Modifiers::COMMAND, egui::Key::O) {
            self.open_dialog();
        }
        // Undo and redo the data only when no text box has the keyboard (they undo their own text).
        if ctx.memory(|m| m.focused().is_none()) {
            if shortcut(egui::Modifiers::COMMAND | egui::Modifiers::SHIFT, egui::Key::Z) || shortcut(egui::Modifiers::COMMAND, egui::Key::Y) {
                self.redo();
            } else if shortcut(egui::Modifiers::COMMAND, egui::Key::Z) {
                self.undo();
            }
        }
        let dropped = ctx.input_mut(|i| std::mem::take(&mut i.raw.dropped_files));
        for file in dropped {
            let path = file.path();
            let name = path.file_name().map_or_else(|| "dropped file".to_string(), |n| n.to_string_lossy().into_owned());
            match file.bytes() {
                Ok(bytes) => {
                    self.open_bytes(&name, &bytes);
                }
                Err(e) => self.status = Some(Status::Error(format!("{name}: {e}"))),
            }
        }
    }

    fn left_pane(&mut self, ui: &mut egui::Ui) {
        let syntax_height = 300.0;
        let table_height = (ui.available_height() - syntax_height).max(120.0);
        let mut edits = Vec::new();
        let mut commits = Vec::new();
        ui.allocate_ui(egui::vec2(ui.available_width(), table_height), |ui| match self.view {
            View::Data => views::data_view(ui, &self.session.dataset, self.value_labels, &mut self.editing, &mut edits, &mut commits),
            View::Variables => views::variable_view(ui, &self.session.dataset, &mut self.editing, &mut edits, &mut commits),
        });
        for (target, text) in edits {
            self.apply_edit(target, &text);
        }
        for commit in commits {
            self.apply_commit(commit);
        }

        ui.separator();
        ui.horizontal(|ui| {
            ui.strong("Syntax");
            if ui.button("▶ Run").clicked() {
                self.run_syntax();
            }
        });
        ui.add(egui::TextEdit::multiline(&mut self.syntax).code_editor().desired_rows(4).desired_width(f32::INFINITY));
        self.steps_panel(ui);
    }

    fn apply_commit(&mut self, commit: Commit) {
        let result = match commit {
            Commit::Syntax(syntax) => {
                self.run_edit(&syntax);
                Ok(())
            }
            Commit::OpenValueLabels(i) => {
                self.value_labels_dialog = self.session.dataset.variables().get(i).map(|v| ValueLabelsDialog::new(i, v));
                Ok(())
            }
            Commit::InsertCase(case) => self.session.insert_case(case),
            Commit::DeleteCase(case) => self.session.delete_case(case),
            Commit::EnableStep(i, enabled) => self.session.set_step_enabled(i, enabled),
            Commit::MoveStep(i, up) => self.session.move_step(i, up),
            Commit::RemoveStep(i) => self.session.remove_step(i),
        };
        self.report(result);
    }

    /// Applied steps, Power Query style: every change to the data since the file was opened.
    /// Switch a step off, click its command to rewrite it, move or remove it; the data is
    /// replayed from the file.
    fn steps_panel(&mut self, ui: &mut egui::Ui) {
        let mut edits = Vec::new();
        let mut commits = Vec::new();
        let mut paste = false;
        let steps = self.session.steps();
        let count = steps.len();
        egui::CollapsingHeader::new(format!("Applied steps ({count})")).id_salt("steps").default_open(true).show(ui, |ui| {
            if steps.iter().any(|s| s.enabled && s.syntax().is_some()) && ui.small_button("Paste steps into syntax").clicked() {
                paste = true;
            }
            egui::ScrollArea::vertical().id_salt("steps-list").max_height(110.0).stick_to_bottom(true).show(ui, |ui| {
                let text_width = (ui.available_width() - 210.0).max(120.0);
                for (i, step) in steps.iter().enumerate() {
                    ui.horizontal(|ui| {
                        let mut enabled = step.enabled;
                        if ui.checkbox(&mut enabled, "").on_hover_text("Apply this step").changed() {
                            commits.push(Commit::EnableStep(i, enabled));
                        }
                        // A step that failed the last replay: red number, the reason on hover.
                        let number = egui::RichText::new(format!("{:>2}", i + 1));
                        match &step.error {
                            Some(error) => {
                                ui.label(number.color(ui.visuals().error_fg_color)).on_hover_text(format!("Skipped: {error}"));
                            }
                            None => {
                                ui.label(number.weak());
                            }
                        }
                        let mut text = egui::RichText::new(step.label()).monospace();
                        if !step.enabled {
                            text = text.weak().strikethrough();
                        } else if step.error.is_some() {
                            text = text.color(ui.visuals().error_fg_color);
                        }
                        if step.syntax().is_some() {
                            let source = step.label();
                            edit::editable(ui, &mut self.editing, &mut edits, Target::Step(i), text, || source, text_width, ROW_HEIGHT, false);
                        } else {
                            ui.add_sized([text_width, ROW_HEIGHT], egui::Label::new(text).truncate());
                        }
                        // Plain words: the default font has no arrow or cross glyphs.
                        if ui.add_enabled(i > 0, egui::Button::new("up").small()).on_hover_text("Move this step earlier").clicked() {
                            commits.push(Commit::MoveStep(i, true));
                        }
                        if ui.add_enabled(i + 1 < count, egui::Button::new("down").small()).on_hover_text("Move this step later").clicked() {
                            commits.push(Commit::MoveStep(i, false));
                        }
                        if ui.small_button("remove").on_hover_text("Remove this step").clicked() {
                            commits.push(Commit::RemoveStep(i));
                        }
                    });
                }
            });
        });
        if paste {
            let lines: Vec<&str> = self.session.steps().iter().filter(|s| s.enabled).filter_map(|s| s.syntax()).collect();
            let text = lines.join("\n");
            if !self.syntax.ends_with('\n') && !self.syntax.is_empty() {
                self.syntax.push('\n');
            }
            self.syntax.push_str(&text);
        }
        for (target, text) in edits {
            self.apply_edit(target, &text);
        }
        for commit in commits {
            self.apply_commit(commit);
        }
    }

    fn dialogs(&mut self, ctx: &egui::Context) {
        let Some(dialog) = self.value_labels_dialog.as_mut() else { return };
        let Some(var) = self.session.dataset.variables().get(dialog.var) else {
            self.value_labels_dialog = None;
            return;
        };
        match views::value_labels_dialog(ctx, dialog, &var.name) {
            DialogOutcome::Open => {}
            DialogOutcome::Cancel => self.value_labels_dialog = None,
            DialogOutcome::Ok => {
                let syntax = value_labels_syntax(var, &dialog.rows);
                self.value_labels_dialog = None;
                self.run_edit(&syntax);
            }
        }
    }

    fn status_line(&self, ui: &mut egui::Ui) {
        ui.separator();
        match &self.status {
            Some(Status::Info(s)) => {
                ui.label(s);
            }
            Some(Status::Warning(s)) => {
                ui.colored_label(ui.visuals().warn_fg_color, s);
            }
            Some(Status::Error(s)) => {
                ui.colored_label(ui.visuals().error_fg_color, s);
            }
            None => {
                ui.label("");
            }
        }
    }
}
