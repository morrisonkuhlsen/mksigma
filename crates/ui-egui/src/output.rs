//! The Output Viewer: tables, notes, warnings and charts in the order procedures produced them.
//! Tables can be pivoted (rows and columns swapped); charts are drawn from the same layout the
//! HTML export uses, so both look alike. Bars, bins, boxes and the value of a frequency table's
//! row can be clicked to cross-filter the other items; the engine runs them again.

use std::collections::{HashMap, HashSet};

use mksigma_engine::core::{Measure, Value, VarType};
use mksigma_engine::output::chart::{self, Anchor, Shape, Stroke, TextSize};
use mksigma_engine::output::{Category, Cell, Chart, Item, Table};
use mksigma_engine::{Selection, Session, Slicer};

/// What the viewer remembers between frames.
#[derive(Debug, Default)]
pub struct OutputState {
    /// Items shown transposed, by index.
    pub transposed: HashSet<usize>,
    /// Slicers added in the viewer whose values aren't picked yet, by variable.
    pub pending: Vec<String>,
    /// A range being typed in a slicer, by variable, until Apply.
    pub ranges: HashMap<String, (f64, f64)>,
}

/// What the user asked for in the viewer this frame.
pub(crate) enum OutputAction {
    None,
    Clear,
    /// A category of an item was clicked.
    Select(usize, Category),
    /// A slicer changed: the `SLICE` command to run.
    Syntax(String),
    ClearFilter,
}

/// What the Output is exported as.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    Html,
    Pdf,
}

pub(crate) fn output_view(ui: &mut egui::Ui, session: &Session, state: &mut OutputState) -> OutputAction {
    let mut action = OutputAction::None;
    let output = &session.output;
    let t = crate::theme::Tokens::get(ui.ctx());
    ui.horizontal(|ui| {
        crate::icons::show(ui, crate::icons::Icon::Table, 14.0);
        ui.label(egui::RichText::new("Output").strong());
        if !output.items.is_empty() {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.small_button("Clear").on_hover_text("Remove every item from the Output").clicked() {
                    state.transposed.clear();
                    action = OutputAction::Clear;
                }
            });
        }
    });
    if let Some(syntax) = slicers(ui, session, state) {
        action = OutputAction::Syntax(syntax);
    }
    let selected = session.cross_filter();
    if let Some(category) = selected {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(format!("Filtered by {}", category.label)).color(ui.visuals().hyperlink_color));
            if ui.small_button("Clear filter").on_hover_text("Show every item on all the cases").clicked() {
                action = OutputAction::ClearFilter;
            }
        });
    }
    ui.add_space(4.0);
    ui.painter().hline(ui.max_rect().x_range(), ui.cursor().top(), egui::Stroke::new(1.0, t.border));
    ui.add_space(8.0);
    if output.items.is_empty() {
        ui.add_space(40.0);
        ui.vertical_centered(|ui| {
            ui.label(egui::RichText::new("No output yet").color(t.text_dim).size(15.0));
            ui.label(egui::RichText::new("Pick a procedure on the Analyze or Graphs tab, or run syntax.").color(t.text_dim));
        });
    }
    egui::ScrollArea::vertical().id_salt("output").stick_to_bottom(true).auto_shrink([false, false]).show(ui, |ui| {
        for (i, item) in output.items.iter().enumerate() {
            // Only the item the selection came from shows it; the others are already filtered.
            let shown = selected.filter(|_| session.is_filter_source(i));
            match item {
                Item::Table(t) => {
                    let flipped = state.transposed.contains(&i);
                    let clicked = table(ui, i, t, flipped, shown);
                    if clicked.pivot {
                        if flipped {
                            state.transposed.remove(&i);
                        } else {
                            state.transposed.insert(i);
                        }
                    }
                    if let Some(category) = clicked.select {
                        action = OutputAction::Select(i, category);
                    }
                }
                Item::Text(s) => {
                    ui.small(s);
                }
                Item::Warning(s) => {
                    ui.colored_label(ui.visuals().warn_fg_color, s);
                }
                Item::Chart(c) => {
                    if let Some(category) = draw_chart(ui, c, shown) {
                        action = OutputAction::Select(i, category);
                    }
                }
            }
            ui.add_space(12.0);
        }
    });
    action
}

/// The slicer strip: a chip per slicer (click it to pick values or a range) and a button to
/// add one. Returns the `SLICE` command a change asks for.
fn slicers(ui: &mut egui::Ui, session: &Session, state: &mut OutputState) -> Option<String> {
    let t = crate::theme::Tokens::get(ui.ctx());
    let ds = &session.dataset;
    state.pending.retain(|v| ds.variable(v).is_some() && !session.slicers().iter().any(|s| &s.variable == v));
    let mut syntax = None;
    ui.add_space(4.0);
    ui.horizontal_wrapped(|ui| {
        crate::icons::show(ui, crate::icons::Icon::Filter, 14.0);
        ui.label(egui::RichText::new("Slicers").color(t.text_dim));
        let names: Vec<String> = session.slicers().iter().map(|s| s.variable.clone()).chain(state.pending.iter().cloned()).collect();
        for name in names {
            let current = session.slicers().iter().find(|s| s.variable == name);
            let text = current.map_or_else(|| format!("{name}: all"), |s| s.describe(ds));
            let chip = ui.add(
                egui::Button::new(egui::RichText::new(text).color(if current.is_some() { t.accent } else { t.text_dim })).selected(current.is_some()),
            );
            egui::Popup::from_toggle_button_response(&chip).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
                if let Some(s) = slicer_editor(ui, session, state, &name, current) {
                    syntax = Some(s);
                }
            });
        }
        let add = ui.small_button("+ Slicer").on_hover_text("Show only some cases in every item");
        egui::Popup::menu(&add).show(|ui| {
            egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
                for var in ds.variables() {
                    let taken = session.slicers().iter().any(|s| s.variable == var.name) || state.pending.contains(&var.name);
                    let label = var.label.as_deref().map_or_else(|| var.name.clone(), |l| format!("{} ({l})", var.name));
                    if ui.add_enabled(!taken, egui::Button::new(label).frame(false)).clicked() {
                        state.pending.push(var.name.clone());
                    }
                }
            });
        });
    });
    syntax
}

/// Picks a slicer's values (checkboxes with counts) or, for a Scale variable with many values,
/// its range. Returns the `SLICE` command for a change.
fn slicer_editor(ui: &mut egui::Ui, session: &Session, state: &mut OutputState, name: &str, current: Option<&Slicer>) -> Option<String> {
    const SHOWN: usize = 50;
    let (_, var) = session.dataset.variable(name)?;
    let values = session.values_of(name, SHOWN + 1);
    let by_values = var.var_type != VarType::Numeric || (var.measure != Measure::Scale && values.len() <= SHOWN) || values.len() <= 12;
    let mut syntax = None;
    ui.strong(var.display_name());
    if by_values {
        let checked: Vec<Value> = match current.map(|s| &s.selection) {
            Some(Selection::Values(v)) => v.clone(),
            _ => Vec::new(),
        };
        egui::ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
            for (value, label, count) in values.iter().take(SHOWN) {
                let mut on = checked.contains(value);
                if ui.checkbox(&mut on, format!("{label}   ({count:.0})")).changed() {
                    // Kept in the values' order, whatever order they were ticked in.
                    let next: Vec<Value> =
                        values.iter().map(|(v, _, _)| v).filter(|v| if *v == value { on } else { checked.contains(v) }).cloned().collect();
                    syntax = Some(if next.is_empty() {
                        format!("SLICE {name} OFF.")
                    } else {
                        Slicer { variable: name.to_string(), selection: Selection::Values(next) }.syntax()
                    });
                }
            }
        });
        if values.len() > SHOWN {
            ui.label(egui::RichText::new(format!("The first {SHOWN} values")).small());
        }
    } else if let Some((lo, hi)) = session.range_of(name) {
        let start = match current.map(|s| &s.selection) {
            Some(Selection::Range { low, high }) => (low.unwrap_or(lo), high.unwrap_or(hi)),
            _ => (lo, hi),
        };
        let range = state.ranges.entry(name.to_string()).or_insert(start);
        let speed = ((hi - lo) / 200.0).max(0.01);
        ui.horizontal(|ui| {
            ui.label("From");
            ui.add(egui::DragValue::new(&mut range.0).speed(speed));
            ui.label("to");
            ui.add(egui::DragValue::new(&mut range.1).speed(speed));
        });
        if ui.button("Apply").clicked() {
            let (a, b) = *range;
            syntax = Some(Slicer { variable: name.to_string(), selection: Selection::Range { low: Some(a.min(b)), high: Some(a.max(b)) } }.syntax());
        }
    }
    ui.separator();
    if ui.button("Remove slicer").clicked() {
        state.ranges.remove(name);
        if current.is_some() {
            syntax = Some(format!("SLICE {name} OFF."));
        } else {
            state.pending.retain(|v| v != name);
        }
        ui.close();
    }
    syntax
}

/// What was clicked in a table.
struct TableClick {
    pivot: bool,
    select: Option<Category>,
}

/// Shows a table, transposed when `flipped`. A row that stands for a category can be selected
/// at its first text cell (the value of a frequency table's row), or at its label if it has none.
fn table(ui: &mut egui::Ui, index: usize, t: &Table, flipped: bool, selected: Option<&Category>) -> TableClick {
    let mut click = TableClick { pivot: false, select: None };
    ui.horizontal(|ui| {
        ui.strong(&t.title);
        click.pivot = ui.small_button("pivot").on_hover_text("Swap rows and columns").clicked();
    });
    // Where each row of `t` is selected: a cell's column, or `None` for the row label.
    let key = |row: usize| t.rows.get(row).and_then(|(_, cells)| cells.iter().position(|c| matches!(c, Cell::Text(_))));
    let shown = if flipped { t.transposed() } else { t.clone() };
    let mut select = |ui: &mut egui::Ui, row: usize, text: &str, strong: bool| {
        let Some(category) = t.category(row) else {
            if strong {
                ui.strong(text);
            } else {
                ui.label(text);
            }
            return;
        };
        let on = selected == Some(category);
        if ui.selectable_label(on, text).on_hover_text(format!("Filter the other items by {}", category.label)).clicked() {
            click.select = Some(category.clone());
        }
    };
    egui::Grid::new(("output-table", index)).striped(true).show(ui, |ui| {
        ui.label("");
        for (c, name) in shown.columns.iter().enumerate() {
            if flipped && key(c).is_none() {
                select(ui, c, name, true);
            } else {
                ui.strong(name);
            }
        }
        ui.end_row();
        for (r, (label, cells)) in shown.rows.iter().enumerate() {
            if !flipped && key(r).is_none() {
                select(ui, r, label, false);
            } else {
                ui.label(label);
            }
            for (c, cell) in cells.iter().enumerate() {
                let (row, col) = if flipped { (c, r) } else { (r, c) };
                // Text reads left to right; numbers line up on the right.
                let layout = if matches!(cell, Cell::Text(_)) {
                    egui::Layout::left_to_right(egui::Align::Center)
                } else {
                    egui::Layout::right_to_left(egui::Align::Center)
                };
                ui.with_layout(layout, |ui| {
                    if key(row) == Some(col) {
                        select(ui, row, &cell.render(), false);
                    } else {
                        ui.label(cell.render());
                    }
                });
            }
            ui.end_row();
        }
    });
    click
}

/// Draws a chart; returns the category of the bin, bar or box clicked, if it has one. With
/// `selected` among its categories, the others are dimmed.
fn draw_chart(ui: &mut egui::Ui, c: &Chart, selected: Option<&Category>) -> Option<Category> {
    // As wide as the pane allows, within limits; 5:8 proportions.
    let width = ui.available_width().clamp(320.0, 640.0);
    let size = (width, (width * 0.625).round());
    let shapes = chart::layout(c, f64::from(size.0), f64::from(size.1));
    let (rect, response) = ui.allocate_exact_size(egui::vec2(size.0, size.1), egui::Sense::click());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Other, true, c.title()));
    let part_at = |pos: egui::Pos2| chart::part_at(&shapes, f64::from(pos.x - rect.min.x)).filter(|p| c.category(*p).is_some());
    let hovered = response.hover_pos().and_then(part_at);
    let clicked = if response.clicked() { response.interact_pointer_pos().and_then(part_at) } else { None };
    if let Some(category) = hovered.and_then(|p| c.category(p)) {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        response.on_hover_text(format!("Filter the other items by {}", category.label));
    }
    let dim = selected.filter(|s| (0..).map_while(|p| c.category(p)).any(|k| k == *s));

    let painter = ui.painter_at(rect);
    let visuals = ui.visuals();
    let text_color = visuals.text_color();
    let weak = visuals.weak_text_color();
    let grid = visuals.widgets.noninteractive.bg_stroke.color;
    let bar = crate::theme::Tokens::get(ui.ctx()).accent;
    let curve = egui::Color32::from_rgb(234, 112, 40);
    let at = |x: f64, y: f64| rect.min + egui::vec2(x as f32, y as f32);
    for shape in shapes.iter().cloned() {
        match shape {
            Shape::Line { from, to, stroke } => {
                let (width, color) = match stroke {
                    Stroke::Axis => (1.0, text_color),
                    Stroke::Grid => (1.0, grid),
                    Stroke::Curve => (2.0, curve),
                    Stroke::Median => (2.5, text_color),
                };
                painter.line_segment([at(from.0, from.1), at(to.0, to.1)], egui::Stroke::new(width, color));
            }
            Shape::Rect { x, y, width, height, part } => {
                let r = egui::Rect::from_min_size(at(x, y), egui::vec2(width as f32, height as f32));
                let category = part.and_then(|p| c.category(p));
                let fill = match dim {
                    Some(selected) if category != Some(selected) => bar.gamma_multiply(0.25),
                    _ if part.is_some() && part == hovered => bar.gamma_multiply(0.8),
                    _ => bar,
                };
                painter.rect_filled(r, 0.0, fill);
                let edge =
                    if dim.is_some() && category == dim { egui::Stroke::new(2.0, text_color) } else { egui::Stroke::new(1.0, visuals.panel_fill) };
                painter.rect_stroke(r, 0.0, edge, egui::StrokeKind::Inside);
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
    clicked.and_then(|p| c.category(p)).cloned()
}
