//! The Output Viewer: tables, notes, warnings and charts in the order procedures produced them.
//! Tables can be pivoted (rows and columns swapped); charts are drawn from the same layout the
//! HTML export uses, so both look alike. Bars, bins, boxes and the value of a frequency table's
//! row can be clicked to cross-filter the other items; the engine runs them again.

use mksigma_i18n::tr;
use std::collections::{HashMap, HashSet};

use mksigma_engine::core::{Measure, Value, VarType};
use mksigma_engine::output::chart::{self, Anchor, Shape, Stroke, TextSize};
use mksigma_engine::output::style::{Family, Style};
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
    /// Showing the dashboard instead of the Output.
    pub dashboard: bool,
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
    /// Pin or unpin an Output item.
    Pin(usize),
    /// Move a dashboard tile earlier (`true`) or later.
    MoveTile(usize, bool),
    ExportDashboard,
    SaveDashboard,
    /// Make a tile take a whole row (`true`) or half of one.
    Wide(usize, bool),
    /// Open the calculation behind an item, by hand.
    Explain(usize),
    /// Show a Learn article.
    Learn(&'static str),
}

/// What the Output is exported as.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    Html,
    Pdf,
    Latex,
}

/// Right-click on an item: copy it as LaTeX, ready to paste into a document.
fn copy_as_latex(response: &egui::Response, item: &Item, style: &Style) {
    response.context_menu(|ui| {
        if ui.button(tr!("out-copy-latex")).on_hover_text(tr!("out-copy-latex-tip")).clicked() {
            ui.ctx().copy_text(mksigma_engine::output::latex::item_to_latex(item, style));
            ui.close();
        }
    });
}

/// A header tab: the Output or the dashboard.
fn pane_tab(ui: &mut egui::Ui, on: bool, icon: crate::icons::Icon, name: &str, tip: &str) -> bool {
    let t = crate::theme::Tokens::get(ui.ctx());
    let text = egui::RichText::new(name).color(if on { t.text } else { t.text_dim });
    let response = ui
        .horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            crate::icons::show(ui, icon, 14.0);
            ui.add(egui::Button::new(if on { text.strong() } else { text }).frame(false))
        })
        .inner;
    if on {
        let r = response.rect;
        ui.painter().rect_filled(
            egui::Rect::from_min_max(egui::pos2(r.left(), r.bottom() + 1.0), egui::pos2(r.right(), r.bottom() + 3.0)),
            1.0,
            t.accent,
        );
    }
    response.on_hover_text(tip).clicked()
}

pub(crate) fn output_view(ui: &mut egui::Ui, session: &Session, state: &mut OutputState) -> OutputAction {
    let mut action = OutputAction::None;
    let output = &session.output;
    let t = crate::theme::Tokens::get(ui.ctx());
    let tiles = session.dashboard();
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 14.0;
        if pane_tab(ui, !state.dashboard, crate::icons::Icon::Table, &tr!("out-output"), &tr!("out-output-tip")) {
            state.dashboard = false;
        }
        let tip = tr!("out-dashboard-tip", n = tiles.len());
        if pane_tab(ui, state.dashboard, crate::icons::Icon::Grid, &tr!("out-dashboard"), &tip) {
            state.dashboard = true;
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if state.dashboard {
                if ui
                    .add_enabled(!tiles.is_empty(), egui::Button::new(tr!("out-export-dashboard")).small())
                    .on_hover_text(tr!("out-export-dashboard-tip"))
                    .clicked()
                {
                    action = OutputAction::ExportDashboard;
                }
                let tip = tr!("out-save-dashboard-tip");
                if ui.add_enabled(!tiles.is_empty(), egui::Button::new(tr!("out-save-dashboard")).small()).on_hover_text(tip).clicked() {
                    action = OutputAction::SaveDashboard;
                }
            } else if !output.items.is_empty() && ui.small_button(tr!("out-clear")).on_hover_text(tr!("clear-output-tip")).clicked() {
                state.transposed.clear();
                action = OutputAction::Clear;
            }
        });
    });
    if let Some(syntax) = slicers(ui, session, state) {
        action = OutputAction::Syntax(syntax);
    }
    let selected = session.cross_filter();
    if let Some(category) = selected {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(tr!("fact-filtered", filters = category.label.as_str())).color(ui.visuals().hyperlink_color));
            if ui.small_button(tr!("out-clear-filter")).on_hover_text(tr!("out-clear-filter-tip")).clicked() {
                action = OutputAction::ClearFilter;
            }
        });
    }
    ui.add_space(4.0);
    ui.painter().hline(ui.max_rect().x_range(), ui.cursor().top(), egui::Stroke::new(1.0, t.border));
    ui.add_space(8.0);
    let empty = |ui: &mut egui::Ui, title: &str, hint: &str| {
        ui.add_space(40.0);
        ui.vertical_centered(|ui| {
            ui.label(egui::RichText::new(title).color(t.text_dim).size(15.0));
            ui.label(egui::RichText::new(hint).color(t.text_dim));
        });
    };
    if state.dashboard {
        if tiles.is_empty() {
            empty(ui, &tr!("out-dashboard-empty"), &tr!("out-dashboard-empty-sub"));
        }
        apply_style(ui, &session.output.style);
        egui::ScrollArea::both().id_salt("dashboard").auto_shrink([false, false]).show(ui, |ui| {
            if let Some(a) = dashboard(ui, session, state, &tiles) {
                action = a;
            }
        });
        return action;
    }
    if output.items.is_empty() {
        empty(ui, &tr!("out-empty"), &tr!("out-empty-sub"));
    }
    apply_style(ui, &session.output.style);
    egui::ScrollArea::both().id_salt("output").stick_to_bottom(true).auto_shrink([false, false]).show(ui, |ui| {
        for (i, item) in output.items.iter().enumerate() {
            if let Some(a) = show_item(ui, session, state, i, item, Some(session.is_pinned(i))) {
                action = a;
            }
            ui.add_space(12.0);
        }
    });
    action
}

/// The pinned items as cards, two columns when there is room. Each card can move or be unpinned.
fn dashboard(ui: &mut egui::Ui, session: &Session, state: &mut OutputState, tiles: &[usize]) -> Option<OutputAction> {
    let two = ui.available_width() >= 860.0;
    // Rows: a wide tile alone, the others two by two (one column when the pane is narrow).
    let mut rows: Vec<(bool, Vec<usize>)> = Vec::new();
    for (k, &i) in tiles.iter().enumerate() {
        let wide = !two || session.is_wide(i);
        match rows.last_mut() {
            Some((false, row)) if !wide && row.len() == 1 => row.push(k),
            _ => rows.push((wide, vec![k])),
        }
    }
    let mut action = None;
    for (wide, row) in rows {
        if wide {
            for k in row {
                action = card(ui, session, state, tiles, k).or(action);
            }
        } else {
            ui.columns(2, |cols| {
                for (col, k) in cols.iter_mut().zip(row) {
                    action = card(col, session, state, tiles, k).or(action.take());
                }
            });
        }
        ui.add_space(10.0);
    }
    action
}

/// Tile `k` of the dashboard: its buttons (move, width, unpin) over the item.
fn card(ui: &mut egui::Ui, session: &Session, state: &mut OutputState, tiles: &[usize], k: usize) -> Option<OutputAction> {
    let t = crate::theme::Tokens::get(ui.ctx());
    let (&i, item) = tiles.get(k).and_then(|i| Some((i, session.output.items.get(*i)?)))?;
    let wide = session.is_wide(i);
    let mut action = None;
    egui::Frame::new().fill(t.paper).stroke(egui::Stroke::new(1.0, t.border)).corner_radius(8).inner_margin(egui::Margin::same(10)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.small_button(tr!("out-unpin")).on_hover_text(tr!("out-unpin-tip")).clicked() {
                    action = Some(OutputAction::Pin(i));
                }
                let (label, tip) = if wide { (tr!("out-narrow"), tr!("out-narrow-tip")) } else { (tr!("out-wide"), tr!("out-wide-tip")) };
                if ui.small_button(label).on_hover_text(tip).clicked() {
                    action = Some(OutputAction::Wide(i, !wide));
                }
                if ui.add_enabled(k + 1 < tiles.len(), egui::Button::new(">").small()).on_hover_text(tr!("out-move-later")).clicked() {
                    action = Some(OutputAction::MoveTile(k, false));
                }
                if ui.add_enabled(k > 0, egui::Button::new("<").small()).on_hover_text(tr!("out-move-earlier")).clicked() {
                    action = Some(OutputAction::MoveTile(k, true));
                }
            });
        });
        if let Some(a) = show_item(ui, session, state, i, item, None) {
            action = Some(a);
        }
    });
    action
}

/// The pin beside an item: filled when it is on the dashboard.
fn pin_button(ui: &mut egui::Ui, pinned: bool) -> bool {
    let t = crate::theme::Tokens::get(ui.ctx());
    let name = if pinned { tr!("out-unpin-from") } else { tr!("out-pin-to") };
    let (rect, response) = ui.allocate_exact_size(egui::vec2(22.0, 20.0), egui::Sense::click());
    response.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, true, pinned, &name));
    if pinned {
        ui.painter().rect_filled(rect, 4.0, t.accent_soft);
    } else if response.hovered() {
        ui.painter().rect_filled(rect, 4.0, t.hover);
    }
    crate::icons::paint(ui.painter(), rect.shrink2(egui::vec2(4.0, 3.0)), crate::icons::Icon::Pin, &t);
    response.on_hover_text(name).clicked()
}

/// One item; `pin` shows the pin with whether the item is pinned.
fn show_item(ui: &mut egui::Ui, session: &Session, state: &mut OutputState, i: usize, item: &Item, pin: Option<bool>) -> Option<OutputAction> {
    // Only the item the selection came from shows it; the others are already filtered.
    let shown = session.cross_filter().filter(|_| session.is_filter_source(i));
    let mut action = None;
    match item {
        Item::Table(t) => {
            let flipped = state.transposed.contains(&i);
            let article = session.command_of(i).and_then(mksigma_learn::for_command);
            let clicked = table(ui, i, t, flipped, shown, pin, session.can_explain(i), article.is_some(), &session.output.style);
            if clicked.pivot {
                if flipped {
                    state.transposed.remove(&i);
                } else {
                    state.transposed.insert(i);
                }
            }
            if clicked.pin {
                action = Some(OutputAction::Pin(i));
            }
            if clicked.explain {
                action = Some(OutputAction::Explain(i));
            }
            if clicked.learn
                && let Some(id) = article
            {
                action = Some(OutputAction::Learn(id));
            }
            if let Some(category) = clicked.select {
                action = Some(OutputAction::Select(i, category));
            }
        }
        Item::Text(s) => {
            ui.small(s);
        }
        Item::Warning(s) => {
            ui.colored_label(ui.visuals().warn_fg_color, s);
        }
        Item::Chart(c) => {
            if let Some(pinned) = pin {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                    if pin_button(ui, pinned) {
                        action = Some(OutputAction::Pin(i));
                    }
                });
            }
            if let Some(category) = draw_chart(ui, c, shown, &session.output.style) {
                action = Some(OutputAction::Select(i, category));
            }
        }
    }
    action
}

/// The Output's text in the style's size and family. Serif has no font on screen (only the
/// interface's own are loaded): it shows in the usual one, and in serif in every export.
fn apply_style(ui: &mut egui::Ui, style: &Style) {
    let (k, family) = text_of(style);
    let text = &mut ui.style_mut().text_styles;
    for (name, size) in [(egui::TextStyle::Body, 13.0), (egui::TextStyle::Button, 13.0), (egui::TextStyle::Small, 11.0)] {
        text.insert(name, egui::FontId::new(size * k, family.clone()));
    }
}

/// The style's scale and font family on screen.
fn text_of(style: &Style) -> (f32, egui::FontFamily) {
    let family = if style.family == Family::Mono { egui::FontFamily::Monospace } else { egui::FontFamily::Proportional };
    (style.scale() as f32, family)
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
        ui.label(egui::RichText::new(tr!("out-slicers")).color(t.text_dim));
        let names: Vec<String> = session.slicers().iter().map(|s| s.variable.clone()).chain(state.pending.iter().cloned()).collect();
        for name in names {
            let current = session.slicers().iter().find(|s| s.variable == name);
            let text = current.map_or_else(|| tr!("out-slicer-all", name = name.as_str()), |s| s.describe(ds));
            let chip = ui.add(
                egui::Button::new(egui::RichText::new(text).color(if current.is_some() { t.accent } else { t.text_dim })).selected(current.is_some()),
            );
            egui::Popup::from_toggle_button_response(&chip).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
                if let Some(s) = slicer_editor(ui, session, state, &name, current) {
                    syntax = Some(s);
                }
            });
        }
        let add = ui.small_button(tr!("out-add-slicer")).on_hover_text(tr!("out-add-slicer-tip"));
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
    // A slicer keeps its kind; a new one is by values unless a Scale variable has many.
    let by_values = match current.map(|s| &s.selection) {
        Some(Selection::Values(_)) => true,
        Some(Selection::Range { .. }) => false,
        None => var.var_type != VarType::Numeric || (var.measure != Measure::Scale && values.len() <= SHOWN) || values.len() <= 12,
    };
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
            ui.label(egui::RichText::new(tr!("out-first-values", n = SHOWN)).small());
        }
    } else if let Some((lo, hi)) = session.range_of(name) {
        let start = match current.map(|s| &s.selection) {
            Some(Selection::Range { low, high }) => (low.unwrap_or(lo), high.unwrap_or(hi)),
            _ => (lo, hi),
        };
        let range = state.ranges.entry(name.to_string()).or_insert(start);
        let speed = ((hi - lo) / 200.0).max(0.01);
        ui.horizontal(|ui| {
            ui.label(tr!("out-from"));
            ui.add(egui::DragValue::new(&mut range.0).speed(speed));
            ui.label(tr!("out-to"));
            ui.add(egui::DragValue::new(&mut range.1).speed(speed));
        });
        if ui.button(tr!("out-apply")).clicked() {
            let (a, b) = *range;
            syntax = Some(Slicer { variable: name.to_string(), selection: Selection::Range { low: Some(a.min(b)), high: Some(a.max(b)) } }.syntax());
        }
    }
    ui.separator();
    if ui.button(tr!("out-remove-slicer")).clicked() {
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
    pin: bool,
    explain: bool,
    learn: bool,
    select: Option<Category>,
}

/// Shows a table, transposed when `flipped`. A row that stands for a category can be selected
/// at its first text cell (the value of a frequency table's row), or at its label if it has none.
#[allow(clippy::too_many_arguments)]
fn table(
    ui: &mut egui::Ui,
    index: usize,
    t: &Table,
    flipped: bool,
    selected: Option<&Category>,
    pin: Option<bool>,
    explain: bool,
    learn: bool,
    style: &Style,
) -> TableClick {
    let mut click = TableClick { pivot: false, pin: false, explain: false, learn: false, select: None };
    ui.horizontal(|ui| {
        let title = ui.add(egui::Label::new(egui::RichText::new(&t.title).strong()).sense(egui::Sense::click()));
        copy_as_latex(&title, &Item::Table(t.clone()), style);
        click.pivot = ui.small_button(tr!("out-pivot")).on_hover_text(tr!("out-pivot-tip")).clicked();
        if explain {
            click.explain = ui.small_button("Σ").on_hover_text(tr!("out-sigma-tip")).clicked();
        }
        if learn {
            click.learn = ui.small_button("?").on_hover_text(tr!("learn-about-tip")).clicked();
        }
        if let Some(pinned) = pin {
            click.pin = pin_button(ui, pinned);
        }
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
        if ui.selectable_label(on, text).on_hover_text(tr!("out-filter-by", category = category.label.as_str())).clicked() {
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
                        select(ui, row, &style.cell(cell), false);
                    } else {
                        ui.label(style.cell(cell));
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
fn draw_chart(ui: &mut egui::Ui, c: &Chart, selected: Option<&Category>, style: &Style) -> Option<Category> {
    // As wide as the pane allows, within limits; 5:8 proportions. A larger style draws the same
    // chart scaled up, text and all, so nothing crowds.
    let (k, family) = text_of(style);
    // In a pane that scrolls sideways the room is what shows, not what could scroll.
    let room = ui.available_width().min(ui.clip_rect().right() - ui.cursor().left() - 4.0);
    let width = room.min(640.0 * k).max(320.0);
    let size = (width, (width * 0.625).round());
    let shapes = chart::layout(c, f64::from(size.0 / k), f64::from(size.1 / k));
    let (rect, response) = ui.allocate_exact_size(egui::vec2(size.0, size.1), egui::Sense::click());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Other, true, c.title()));
    copy_as_latex(&response, &Item::Chart(c.clone()), style);
    let part_at = |pos: egui::Pos2| chart::part_at(&shapes, f64::from((pos.x - rect.min.x) / k)).filter(|p| c.category(*p).is_some());
    let hovered = response.hover_pos().and_then(part_at);
    let clicked = if response.clicked() { response.interact_pointer_pos().and_then(part_at) } else { None };
    if let Some(category) = hovered.and_then(|p| c.category(p)) {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        response.on_hover_text(tr!("out-filter-by", category = category.label.as_str()));
    }
    let dim = selected.filter(|s| (0..).map_while(|p| c.category(p)).any(|k| k == *s));

    let painter = ui.painter_at(rect);
    let visuals = ui.visuals();
    let text_color = visuals.text_color();
    let weak = visuals.weak_text_color();
    let grid = visuals.widgets.noninteractive.bg_stroke.color;
    let rgb = |[r, g, b]: [u8; 3]| egui::Color32::from_rgb(r, g, b);
    let (bar, curve) = style
        .palette
        .colors()
        .map_or((crate::theme::Tokens::get(ui.ctx()).accent, egui::Color32::from_rgb(234, 112, 40)), |(bar, curve)| (rgb(bar), rgb(curve)));
    let at = |x: f64, y: f64| rect.min + egui::vec2(x as f32, y as f32) * k;
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
                let r = egui::Rect::from_min_size(at(x, y), egui::vec2(width as f32, height as f32) * k);
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
                painter.circle_filled(at(x, y), radius as f32 * k, bar);
            }
            Shape::Path { points } => {
                painter.add(egui::Shape::line(points.iter().map(|(x, y)| at(*x, *y)).collect(), egui::Stroke::new(2.0, curve)));
            }
            Shape::Text { x, y, text, anchor, size, vertical } => {
                let (font, color) = match size {
                    TextSize::Small => (egui::FontId::new(11.0 * k, family.clone()), weak),
                    TextSize::Normal => (egui::FontId::new(12.5 * k, family.clone()), text_color),
                    TextSize::Title => (egui::FontId::new(14.0 * k, family.clone()), text_color),
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
