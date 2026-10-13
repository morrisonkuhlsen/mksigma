//! The Output Viewer: tables, notes, warnings and charts in the order procedures produced them.
//! Tables can be pivoted (rows and columns swapped); charts are drawn from the same layout the
//! HTML export uses, so both look alike.

use std::collections::HashSet;

use mksigma_engine::output::chart::{self, Anchor, Shape, Stroke, TextSize};
use mksigma_engine::output::{Cell, Chart, Item, Output, Table};

/// What the viewer remembers between frames.
#[derive(Debug, Default)]
pub struct OutputState {
    /// Items shown transposed, by index.
    pub transposed: HashSet<usize>,
}

/// What the user asked for in the viewer this frame.
pub(crate) enum OutputAction {
    None,
    Export,
}

pub(crate) fn output_view(ui: &mut egui::Ui, output: &mut Output, state: &mut OutputState) -> OutputAction {
    let mut action = OutputAction::None;
    ui.horizontal(|ui| {
        ui.strong("Output");
        if !output.items.is_empty() {
            if ui.small_button("Clear").clicked() {
                output.items.clear();
                state.transposed.clear();
            }
            if ui.small_button("Export HTML").on_hover_text("Save the Output as a web page").clicked() {
                action = OutputAction::Export;
            }
        }
    });
    egui::ScrollArea::vertical().id_salt("output").stick_to_bottom(true).auto_shrink([false, false]).show(ui, |ui| {
        for (i, item) in output.items.iter().enumerate() {
            match item {
                Item::Table(t) => {
                    let flipped = state.transposed.contains(&i);
                    let pivot = if flipped { t.transposed() } else { t.clone() };
                    if table(ui, i, &pivot) {
                        if flipped {
                            state.transposed.remove(&i);
                        } else {
                            state.transposed.insert(i);
                        }
                    }
                }
                Item::Text(s) => {
                    ui.small(s);
                }
                Item::Warning(s) => {
                    ui.colored_label(ui.visuals().warn_fg_color, s);
                }
                Item::Chart(c) => draw_chart(ui, c),
            }
            ui.add_space(12.0);
        }
    });
    action
}

/// Shows a table; returns whether its Pivot button was clicked.
fn table(ui: &mut egui::Ui, index: usize, t: &Table) -> bool {
    let mut pivot = false;
    ui.horizontal(|ui| {
        ui.strong(&t.title);
        pivot = ui.small_button("pivot").on_hover_text("Swap rows and columns").clicked();
    });
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
    pivot
}

fn draw_chart(ui: &mut egui::Ui, c: &Chart) {
    // As wide as the pane allows, within limits; 5:8 proportions.
    let width = ui.available_width().clamp(320.0, 640.0);
    let size = (width, (width * 0.625).round());
    let (rect, _) = ui.allocate_exact_size(egui::vec2(size.0, size.1), egui::Sense::hover());
    let painter = ui.painter_at(rect);
    let visuals = ui.visuals();
    let text_color = visuals.text_color();
    let weak = visuals.weak_text_color();
    let grid = visuals.widgets.noninteractive.bg_stroke.color;
    let bar = visuals.selection.bg_fill;
    let curve = egui::Color32::from_rgb(234, 112, 40);
    let at = |x: f64, y: f64| rect.min + egui::vec2(x as f32, y as f32);
    for shape in chart::layout(c, f64::from(size.0), f64::from(size.1)) {
        match shape {
            Shape::Line { from, to, stroke } => {
                let color = match stroke {
                    Stroke::Axis => text_color,
                    Stroke::Grid => grid,
                    Stroke::Curve => curve,
                };
                painter.line_segment([at(from.0, from.1), at(to.0, to.1)], egui::Stroke::new(1.0, color));
            }
            Shape::Rect { x, y, width, height } => {
                let r = egui::Rect::from_min_size(at(x, y), egui::vec2(width as f32, height as f32));
                painter.rect_filled(r, 0.0, bar);
                painter.rect_stroke(r, 0.0, egui::Stroke::new(1.0, visuals.panel_fill), egui::StrokeKind::Inside);
            }
            Shape::Circle { x, y, radius } => {
                painter.circle_filled(at(x, y), radius as f32, bar);
            }
            Shape::Path { points } => {
                painter.add(egui::Shape::line(points.iter().map(|(x, y)| at(*x, *y)).collect(), egui::Stroke::new(2.0, curve)));
            }
            Shape::Text { x, y, text, anchor, size, vertical } => {
                let (font, color) = match size {
                    TextSize::Small => (egui::FontId::proportional(11.0), weak),
                    TextSize::Normal => (egui::FontId::proportional(12.5), text_color),
                    TextSize::Title => (egui::FontId::proportional(14.0), text_color),
                };
                let galley = painter.layout_no_wrap(text, font, color);
                let size = galley.size();
                // Positions are baselines, as in SVG: shift to the galley's top left.
                let dx = match anchor {
                    Anchor::Start => 0.0,
                    Anchor::Middle => size.x / 2.0,
                    Anchor::End => size.x,
                };
                if vertical {
                    let pos = at(x, y) + egui::vec2(-size.y * 0.8, dx);
                    painter.add(egui::epaint::TextShape::new(pos, galley, color).with_angle(-std::f32::consts::FRAC_PI_2));
                } else {
                    painter.galley(at(x, y) - egui::vec2(dx, size.y * 0.8), galley, color);
                }
            }
        }
    }
}
