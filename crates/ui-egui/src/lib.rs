//! MK Sigma egui frontend: a File menu, Data View and Variable View, a syntax editor and the
//! Output Viewer. It is thin: it reads the session and acts through `Session`; nothing below
//! this crate depends on egui. Platform services (the native file dialog) come in through
//! `Services`, so tests drive the UI without them.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use std::path::{Path, PathBuf};

use mksigma_engine::Session;
use mksigma_engine::core::{Dataset, Measure, MissingValues, Value, VarType, Variable};
use mksigma_engine::output::{Item, Table};

/// What the app provides from the platform.
#[derive(Default)]
pub struct Services {
    /// Shows an open-file dialog; `None` when cancelled.
    pub pick_open: Option<Box<dyn Fn() -> Option<PathBuf>>>,
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
    services: Services,
    title_dirty: bool,
}

impl Default for SigmaApp {
    fn default() -> Self {
        Self::new(Services::default())
    }
}

const ROW_HEIGHT: f32 = 20.0;

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
            services,
            title_dirty: true,
        }
    }

    /// Opens a system file from disk.
    pub fn open_path(&mut self, path: &Path) {
        match std::fs::read(path) {
            Ok(bytes) => {
                let name = path.file_name().map_or_else(|| path.display().to_string(), |n| n.to_string_lossy().into_owned());
                self.open_bytes(&name, &bytes);
            }
            Err(e) => self.status = Some(Status::Error(format!("{}: {e}", path.display()))),
        }
    }

    /// Opens a system file's bytes (a path read, or a file dropped where there is no path).
    pub fn open_bytes(&mut self, name: &str, bytes: &[u8]) {
        match self.session.open_sav(bytes) {
            Ok(warnings) => {
                let ds = &self.session.dataset;
                let summary = format!("{name}: {} cases, {} variables", ds.n_cases(), ds.variables().len());
                self.status =
                    Some(if warnings.is_empty() { Status::Info(summary) } else { Status::Warning(format!("{summary}. {}", warnings.join("; "))) });
                self.file_name = Some(name.to_string());
                self.title_dirty = true;
            }
            Err(e) => self.status = Some(Status::Error(format!("{name}: {e}"))),
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

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        self.handle_input(ui.ctx());
        if self.title_dirty {
            let title = self.file_name.as_ref().map_or_else(|| "MK Sigma".to_string(), |f| format!("{f} — MK Sigma"));
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Title(title));
            self.title_dirty = false;
        }

        ui.horizontal(|ui| {
            ui.menu_button("File", |ui| {
                if ui.button("Open…").clicked() {
                    ui.close();
                    self.open_dialog();
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
        // Data and Variable View get 60% of the width, the Output Viewer the rest.
        let gap = 12.0;
        let left_width = ((ui.available_width() - gap) * 0.6).floor();
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
                self.output_view(ui);
            });
        });
        self.status_line(ui);
    }

    fn handle_input(&mut self, ctx: &egui::Context) {
        let open = ctx.input_mut(|i| i.consume_shortcut(&egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::O)));
        if open {
            self.open_dialog();
        }
        let dropped = ctx.input_mut(|i| std::mem::take(&mut i.raw.dropped_files));
        for file in dropped {
            let path = file.path();
            let name = path.file_name().map_or_else(|| "dropped file".to_string(), |n| n.to_string_lossy().into_owned());
            match file.bytes() {
                Ok(bytes) => self.open_bytes(&name, &bytes),
                Err(e) => self.status = Some(Status::Error(format!("{name}: {e}"))),
            }
        }
    }

    fn left_pane(&mut self, ui: &mut egui::Ui) {
        let syntax_height = 170.0;
        let table_height = (ui.available_height() - syntax_height).max(120.0);
        ui.allocate_ui(egui::vec2(ui.available_width(), table_height), |ui| match self.view {
            View::Data => data_view(ui, &self.session.dataset, self.value_labels),
            View::Variables => variable_view(ui, &self.session.dataset),
        });
        ui.separator();
        ui.horizontal(|ui| {
            ui.strong("Syntax");
            if ui.button("▶ Run").clicked() {
                self.run_syntax();
            }
        });
        ui.add(egui::TextEdit::multiline(&mut self.syntax).code_editor().desired_rows(6).desired_width(f32::INFINITY));
    }

    fn output_view(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.strong("Output");
            if !self.session.output.items.is_empty() && ui.small_button("Clear").clicked() {
                self.session.output.items.clear();
            }
        });
        egui::ScrollArea::vertical().id_salt("output").stick_to_bottom(true).auto_shrink([false, false]).show(ui, |ui| {
            for (i, item) in self.session.output.items.iter().enumerate() {
                match item {
                    Item::Table(t) => table(ui, i, t),
                    Item::Text(s) => {
                        ui.small(s);
                    }
                    Item::Warning(s) => {
                        ui.colored_label(ui.visuals().warn_fg_color, s);
                    }
                }
                ui.add_space(12.0);
            }
        });
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

/// A cell as Data View shows it: the value label when asked and one exists, else the value in
/// the variable's format.
fn cell_text(var: &Variable, v: &Value, labels: bool) -> String {
    match (labels, var.value_label(v)) {
        (true, Some(label)) => label.to_string(),
        _ => var.format.render(v),
    }
}

/// Columns are as wide as their header or widest visible text, within limits.
fn column_width(ui: &egui::Ui, text: &str) -> f32 {
    let galley = ui.painter().layout_no_wrap(text.to_string(), egui::TextStyle::Body.resolve(ui.style()), egui::Color32::WHITE);
    (galley.size().x + 16.0).clamp(56.0, 220.0)
}

fn data_view(ui: &mut egui::Ui, ds: &Dataset, labels: bool) {
    let vars = ds.variables();
    let n = ds.n_cases();
    let case_width = column_width(ui, &n.to_string()).max(48.0);
    let widths: Vec<f32> = vars.iter().map(|v| column_width(ui, &v.name).max(80.0)).collect();
    egui::ScrollArea::horizontal().id_salt("data-h").auto_shrink([false, false]).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.add_sized([case_width, ROW_HEIGHT], egui::Label::new(""));
            for (v, w) in vars.iter().zip(&widths) {
                ui.add_sized([*w, ROW_HEIGHT], egui::Label::new(egui::RichText::new(&v.name).strong())).on_hover_text(v.display_name());
            }
        });
        ui.separator();
        // Only the visible rows are laid out, so large files stay fast.
        egui::ScrollArea::vertical().id_salt("data-v").auto_shrink([false, false]).show_rows(ui, ROW_HEIGHT, n, |ui, rows| {
            for case in rows {
                ui.horizontal(|ui| {
                    ui.add_sized([case_width, ROW_HEIGHT], egui::Label::new(egui::RichText::new((case + 1).to_string()).weak()));
                    for (i, (var, w)) in vars.iter().zip(&widths).enumerate() {
                        let value = ds.column(i).and_then(|c| c.get(case));
                        let text = value.map_or_else(|| ".".to_string(), |v| cell_text(var, v, labels));
                        let missing = value.is_some_and(|v| var.is_missing(v));
                        let mut rich = egui::RichText::new(text);
                        if missing {
                            rich = rich.weak();
                        }
                        let layout = if var.var_type == VarType::Numeric && !labels {
                            egui::Layout::right_to_left(egui::Align::Center)
                        } else {
                            egui::Layout::left_to_right(egui::Align::Center)
                        };
                        ui.allocate_ui_with_layout(egui::vec2(*w, ROW_HEIGHT), layout, |ui| {
                            ui.set_min_width(*w);
                            ui.add(egui::Label::new(rich).truncate());
                        });
                    }
                });
            }
        });
    });
}

fn missing_text(var: &Variable) -> String {
    let render = |x: f64| var.format.render(&Value::Num(x));
    match &var.missing {
        MissingValues::None => "None".to_string(),
        MissingValues::Discrete(list) => list.iter().map(|v| var.format.render(v)).collect::<Vec<_>>().join(", "),
        MissingValues::Range { low, high, discrete } => {
            let range = format!("{} - {}", render(*low), render(*high));
            discrete.map_or(range.clone(), |d| format!("{range}, {}", render(d)))
        }
    }
}

fn values_text(var: &Variable) -> String {
    match var.value_labels.as_slice() {
        [] => "None".to_string(),
        [(v, label), rest @ ..] => {
            let first = format!("{{{}, {label}}}", var.format.render(v));
            if rest.is_empty() { first } else { format!("{first}…") }
        }
    }
}

fn variable_view(ui: &mut egui::Ui, ds: &Dataset) {
    egui::ScrollArea::both().id_salt("variables").auto_shrink([false, false]).show(ui, |ui| {
        egui::Grid::new("variable-view").striped(true).num_columns(8).show(ui, |ui| {
            for h in ["", "Name", "Type", "Label", "Values", "Missing", "Measure", "Format"] {
                ui.strong(h);
            }
            ui.end_row();
            for (i, var) in ds.variables().iter().enumerate() {
                ui.label(egui::RichText::new((i + 1).to_string()).weak());
                ui.label(&var.name);
                ui.label(match var.var_type {
                    VarType::Numeric => "Numeric".to_string(),
                    VarType::String(w) => format!("String ({w})"),
                });
                ui.label(var.label.as_deref().unwrap_or(""));
                let values = values_text(var);
                let response = ui.label(&values);
                if var.value_labels.len() > 1 {
                    let all: Vec<String> = var.value_labels.iter().map(|(v, l)| format!("{} = {l}", var.format.render(v))).collect();
                    response.on_hover_text(all.join("\n"));
                }
                ui.label(missing_text(var));
                ui.label(match var.measure {
                    Measure::Nominal => "Nominal",
                    Measure::Ordinal => "Ordinal",
                    Measure::Scale => "Scale",
                });
                ui.label(var.format.to_string());
                ui.end_row();
            }
        });
    });
}

fn table(ui: &mut egui::Ui, index: usize, t: &Table) {
    ui.strong(&t.title);
    egui::Grid::new(("output-table", index)).striped(true).show(ui, |ui| {
        ui.label("");
        for c in &t.columns {
            ui.strong(c);
        }
        ui.end_row();
        for (label, cells) in &t.rows {
            ui.label(label);
            for cell in cells {
                let layout = if matches!(cell, mksigma_engine::output::Cell::Text(_)) {
                    egui::Layout::left_to_right(egui::Align::Center)
                } else {
                    egui::Layout::right_to_left(egui::Align::Center)
                };
                ui.with_layout(layout, |ui| {
                    ui.label(cell.render());
                });
            }
            ui.end_row();
        }
    });
}
