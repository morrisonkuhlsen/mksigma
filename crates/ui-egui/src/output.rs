//! The Output Viewer: tables, notes and warnings in the order procedures produced them.

use mksigma_engine::output::{Cell, Item, Output, Table};

pub(crate) fn output_view(ui: &mut egui::Ui, output: &mut Output) {
    ui.horizontal(|ui| {
        ui.strong("Output");
        if !output.items.is_empty() && ui.small_button("Clear").clicked() {
            output.items.clear();
        }
    });
    egui::ScrollArea::vertical().id_salt("output").stick_to_bottom(true).auto_shrink([false, false]).show(ui, |ui| {
        for (i, item) in output.items.iter().enumerate() {
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
                // Text reads left to right; numbers line up on the right.
                let layout = if matches!(cell, Cell::Text(_)) {
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
