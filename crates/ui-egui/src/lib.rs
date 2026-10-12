//! MK Sigma egui frontend: Data View, a syntax editor and the Output Viewer. It is thin: it reads
//! the session and acts through `Session`; nothing below this crate depends on egui.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use mksigma_engine::Session;
use mksigma_engine::core::Value;
use mksigma_engine::output::{Item, Table};

pub struct SigmaApp {
    session: Session,
    syntax: String,
    error: Option<String>,
}

impl Default for SigmaApp {
    fn default() -> Self {
        Self::new()
    }
}

impl SigmaApp {
    pub fn new() -> Self {
        let (session, error) = match Session::with_sample() {
            Ok(s) => (s, None),
            Err(e) => (Session::new(), Some(e.to_string())),
        };
        Self { session, syntax: "DESCRIPTIVES VARIABLES=age income score.".to_string(), error }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("MK Sigma");
        ui.separator();
        ui.columns(2, |cols| {
            if let [left, right] = cols {
                self.data_view(left);
                self.output_view(right);
            }
        });
    }

    fn data_view(&mut self, ui: &mut egui::Ui) {
        ui.strong("Data View");
        let ds = &self.session.dataset;
        egui::ScrollArea::both().id_salt("data").max_height(280.0).show(ui, |ui| {
            egui::Grid::new("data-grid").striped(true).show(ui, |ui| {
                ui.label("");
                for v in ds.variables() {
                    ui.strong(&v.name).on_hover_text(v.display_name());
                }
                ui.end_row();
                for case in 0..ds.n_cases() {
                    ui.label((case + 1).to_string());
                    for i in 0..ds.variables().len() {
                        let text = match ds.column(i).and_then(|c| c.get(case)) {
                            Some(Value::Num(n)) => n.to_string(),
                            Some(Value::Str(s)) => s.clone(),
                            Some(Value::SysMis) | None => ".".to_string(),
                        };
                        ui.label(text);
                    }
                    ui.end_row();
                }
            });
        });

        ui.add_space(12.0);
        ui.strong("Syntax");
        ui.add(egui::TextEdit::multiline(&mut self.syntax).code_editor().desired_rows(6).desired_width(f32::INFINITY));
        if ui.button("▶ Run").clicked() {
            self.error = self.session.run_syntax(&self.syntax).err().map(|e| e.to_string());
        }
        if let Some(e) = &self.error {
            ui.colored_label(ui.visuals().error_fg_color, e);
        }
    }

    fn output_view(&mut self, ui: &mut egui::Ui) {
        ui.strong("Output");
        egui::ScrollArea::vertical().id_salt("output").stick_to_bottom(true).show(ui, |ui| {
            for (i, item) in self.session.output.items.iter().enumerate() {
                match item {
                    Item::Table(t) => table(ui, i, t),
                    Item::Text(s) => {
                        ui.label(s);
                    }
                    Item::Warning(s) => {
                        ui.colored_label(ui.visuals().warn_fg_color, s);
                    }
                }
                ui.add_space(12.0);
            }
        });
    }
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
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(cell.render());
                });
            }
            ui.end_row();
        }
    });
}
