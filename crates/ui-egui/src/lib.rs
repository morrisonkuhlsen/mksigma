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
    services: Services,
    title: String,
}

impl Default for SigmaApp {
    fn default() -> Self {
        Self::new(Services::default())
    }
}

pub(crate) const ROW_HEIGHT: f32 = 20.0;

impl SigmaApp {
    pub fn new(services: Services) -> Self {
        let (session, status) = match Session::with_sample() {
            Ok(s) => (s, Some(Status::Info("Sample dataset. File ▸ Open… or drop a .sav file on the window.".into()))),
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

    /// Runs syntax generated by an edit; the status line shows what ran, or why it didn't.
    pub fn run_edit(&mut self, syntax: &str) {
        self.status = Some(match self.session.run_syntax(syntax) {
            Ok(()) => Status::Info(syntax.to_string()),
            Err(e) => Status::Error(format!("{e} ({syntax})")),
        });
    }

    fn apply_edit(&mut self, target: Target, text: &str) {
        match target {
            Target::Cell { case, var } => {
                if let Err(e) = self.session.set_cell(case, var, text) {
                    self.status = Some(Status::Error(e.to_string()));
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
                output::output_view(ui, &mut self.session.output);
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
        let syntax_height = 200.0;
        let table_height = (ui.available_height() - syntax_height).max(120.0);
        let mut edits = Vec::new();
        let mut commits = Vec::new();
        ui.allocate_ui(egui::vec2(ui.available_width(), table_height), |ui| match self.view {
            View::Data => views::data_view(ui, &self.session.dataset, self.value_labels, &mut self.editing, &mut edits),
            View::Variables => views::variable_view(ui, &self.session.dataset, &mut self.editing, &mut edits, &mut commits),
        });
        for (target, text) in edits {
            self.apply_edit(target, &text);
        }
        for commit in commits {
            match commit {
                Commit::Syntax(syntax) => self.run_edit(&syntax),
                Commit::OpenValueLabels(i) => {
                    self.value_labels_dialog = self.session.dataset.variables().get(i).map(|v| ValueLabelsDialog::new(i, v));
                }
            }
        }

        ui.separator();
        ui.horizontal(|ui| {
            ui.strong("Syntax");
            if ui.button("▶ Run").clicked() {
                self.run_syntax();
            }
        });
        ui.add(egui::TextEdit::multiline(&mut self.syntax).code_editor().desired_rows(5).desired_width(f32::INFINITY));
        let journal = &self.session.journal;
        let mut paste = false;
        egui::CollapsingHeader::new(format!("History ({})", journal.len())).id_salt("history").show(ui, |ui| {
            if !journal.is_empty() && ui.small_button("Paste into syntax").clicked() {
                paste = true;
            }
            egui::ScrollArea::vertical().id_salt("history-list").max_height(90.0).stick_to_bottom(true).show(ui, |ui| {
                for line in journal {
                    ui.monospace(line);
                }
            });
        });
        if paste {
            let history = self.session.journal.join("\n");
            if !self.syntax.ends_with('\n') && !self.syntax.is_empty() {
                self.syntax.push('\n');
            }
            self.syntax.push_str(&history);
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
