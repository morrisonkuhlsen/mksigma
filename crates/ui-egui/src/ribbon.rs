//! The top of the window, as its sibling Pearl's: a title row (AutoSave, Save, Undo, Redo and more
//! on the left, the logo and the document's name in the middle, search, export and theme on the
//! right), a row of menus (File, then the tabs) and the current tab's toolbar in one slim row,
//! its groups set apart by short light lines. Buttons only report what was asked for; the app
//! acts through the session.

use crate::View;
use crate::icons::{self, Icon};
use crate::output::ExportFormat;
use crate::procedure::Kind;
use crate::suggest::Procedure;
use crate::theme::Tokens;
use mksigma_i18n::tr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tab {
    #[default]
    Home,
    Data,
    Transform,
    Analyze,
    Graphs,
    View,
}

const TABS: [Tab; 6] = [Tab::Home, Tab::Data, Tab::Transform, Tab::Analyze, Tab::Graphs, Tab::View];

impl Tab {
    /// Its name in the interface's language.
    fn name(self) -> String {
        match self {
            Tab::Home => tr!("tab-home"),
            Tab::Data => tr!("tab-data"),
            Tab::Transform => tr!("tab-transform"),
            Tab::Analyze => tr!("tab-analyze"),
            Tab::Graphs => tr!("tab-graphs"),
            Tab::View => tr!("tab-view"),
        }
    }
}

/// What was asked for in the title row or the ribbon this frame.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Action {
    Open,
    Save,
    SaveAs,
    Undo,
    Redo,
    Run,
    Export(ExportFormat),
    ClearOutput,
    InsertCase,
    InsertVariable,
    ToggleLabels,
    View(View),
    Suggest(Procedure),
    Theme(egui::ThemePreference),
    ToggleAutoSave,
    Search,
    SaveDashboard,
    /// Show the File backstage.
    File,
    /// Open a procedure's dialog.
    Dialog(Kind),
    /// Run this syntax (a control that writes a command, such as the Output style's).
    Syntax(String),
    /// Speak this language.
    Language(mksigma_i18n::Lang),
}

/// What the buttons show: what can be undone, whether there is Output, the current view.
pub(crate) struct State<'a> {
    pub title: &'a str,
    pub undo: Option<&'a str>,
    pub redo: Option<&'a str>,
    pub has_output: bool,
    pub value_labels: bool,
    pub view: View,
    pub autosave: bool,
    pub style: mksigma_engine::output::Style,
}

pub(crate) const TITLE_HEIGHT: f32 = 38.0;
/// A control in the title row.
const CONTROL_HEIGHT: f32 = 26.0;
/// The row of menus, the space under it and the toolbar's row.
const MENU_HEIGHT: f32 = 30.0;
const MENU_GAP: f32 = 6.0;
const TOOLBAR_HEIGHT: f32 = 34.0;
/// A toolbar button.
const BUTTON_HEIGHT: f32 = 23.0;
/// The corners of buttons in the bars, as Pearl's.
const RADIUS: f32 = 4.0;

/// An undo label short enough for a tooltip.
fn short(label: &str) -> String {
    let mut s: String = label.chars().take(40).collect();
    if label.chars().count() > 40 {
        s.push('…');
    }
    s
}

/// A frameless icon button of the title row, in one quiet colour, with an accessible name;
/// clicks count only when enabled.
fn icon_button(ui: &mut egui::Ui, icon: Icon, name: &str, enabled: bool) -> egui::Response {
    let t = Tokens::get(ui.ctx());
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(28.0, CONTROL_HEIGHT), if enabled { egui::Sense::click() } else { egui::Sense::hover() });
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, name));
    if enabled && (response.hovered() || response.is_pointer_button_down_on()) {
        ui.painter().rect_filled(rect, RADIUS, if response.is_pointer_button_down_on() { t.pressed } else { t.hover });
    }
    let color = if enabled { t.text_dim } else { t.text_dim.gamma_multiply(0.4) };
    icons::paint_mono(ui.painter(), egui::Rect::from_center_size(rect.center(), egui::vec2(16.0, 16.0)), icon, color, t.window);
    response.on_hover_text(name)
}

/// An on/off switch drawn as a pill with a knob.
fn switch(ui: &mut egui::Ui, on: bool, name: &str) -> egui::Response {
    let t = Tokens::get(ui.ctx());
    let (r, response) = ui.allocate_exact_size(egui::vec2(30.0, 16.0), egui::Sense::click());
    response.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Checkbox, true, on, name));
    let painter = ui.painter();
    // The one rounded control: a switch reads as a switch by its pill and round knob.
    painter.rect_filled(r, 8.0, if on { t.accent } else { egui::Color32::TRANSPARENT });
    painter.rect_stroke(r, 8.0, egui::Stroke::new(1.0, if on { t.accent } else { t.text_dim }), egui::StrokeKind::Inside);
    let knob = if on { egui::pos2(r.right() - 8.0, r.center().y) } else { egui::pos2(r.left() + 8.0, r.center().y) };
    painter.circle_filled(knob, 5.0, if on { crate::theme::on_accent(&t) } else { t.text_dim });
    response
}

/// A filled accent button with a white icon shape and text, for the one main action of a panel.
pub(crate) fn accent_button(ui: &mut egui::Ui, icon: Icon, text: &str) -> egui::Response {
    let t = Tokens::get(ui.ctx());
    let ink = crate::theme::on_accent(&t);
    let galley = ui.painter().layout_no_wrap(text.to_string(), egui::FontId::proportional(13.0), ink);
    let size = egui::vec2(galley.size().x + 36.0, 26.0);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, text));
    let fill = if response.hovered() { t.accent.gamma_multiply(0.85) } else { t.accent };
    ui.painter().rect_filled(rect, 0.0, fill);
    icons::paint_mono(
        ui.painter(),
        egui::Rect::from_center_size(egui::pos2(rect.left() + 15.0, rect.center().y), egui::vec2(12.0, 12.0)),
        icon,
        ink,
        fill,
    );
    ui.painter().galley(egui::pos2(rect.left() + 26.0, rect.center().y - galley.size().y / 2.0), galley, ink);
    response
}

/// The title row: AutoSave, Save, Undo, Redo and more on the left; the logo and the document's
/// name in the middle, cut with "…" so it never runs under the controls; search, export and
/// theme on the right. The row is the window's title bar: drag it to move the window,
/// double-click to maximize.
pub(crate) fn title_bar(ui: &mut egui::Ui, state: &State) -> Option<Action> {
    let t = Tokens::get(ui.ctx());
    let mut action = None;
    let (rect, background) = ui.allocate_exact_size(egui::vec2(ui.available_width(), TITLE_HEIGHT), egui::Sense::click_and_drag());
    if background.drag_started() {
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
    }
    if background.double_clicked() {
        let maximized = ui.ctx().input(|i| i.viewport().maximized.unwrap_or(false));
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
    }
    let mut row =
        ui.new_child(egui::UiBuilder::new().max_rect(rect.shrink2(egui::vec2(8.0, 0.0))).layout(egui::Layout::left_to_right(egui::Align::Center)));
    row.spacing_mut().item_spacing.x = 0.0;
    row.label(egui::RichText::new(tr!("autosave")).size(12.5).color(t.text_dim));
    row.add_space(6.0);
    let tip = if state.autosave { &tr!("autosave-on-tip") } else { &tr!("autosave-off-tip") };
    if switch(&mut row, state.autosave, &tr!("autosave")).on_hover_text(tip).clicked() {
        action = Some(Action::ToggleAutoSave);
    }
    row.add_space(6.0);
    if icon_button(&mut row, Icon::Save, &tr!("save"), true).clicked() {
        action = Some(Action::Save);
    }
    let undo = state.undo.map_or_else(|| tr!("undo"), |l| tr!("undo-named", what = short(l)));
    if icon_button(&mut row, Icon::Undo, &undo, state.undo.is_some()).clicked() && state.undo.is_some() {
        action = Some(Action::Undo);
    }
    let redo = state.redo.map_or_else(|| tr!("redo"), |l| tr!("redo-named", what = short(l)));
    if icon_button(&mut row, Icon::Redo, &redo, state.redo.is_some()).clicked() && state.redo.is_some() {
        action = Some(Action::Redo);
    }
    let more = icon_button(&mut row, Icon::More, &tr!("more-commands"), true);
    egui::Popup::menu(&more).show(|ui| {
        ui.set_min_width(180.0);
        for (name, then) in [
            (tr!("open-ellipsis"), Action::Open),
            (tr!("save"), Action::Save),
            (tr!("save-as-ellipsis"), Action::SaveAs),
            (tr!("run-syntax"), Action::Run),
        ] {
            if ui.button(name).clicked() {
                action = Some(then);
                ui.close();
            }
        }
    });
    let left_end = row.min_rect().right();
    let mut right_start = rect.right();
    row.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        if icon_button(ui, Icon::Search, &tr!("search-tip"), true).clicked() {
            action = Some(Action::Search);
        }
        let export = icon_button(ui, Icon::Share, &tr!("export-output-tip"), state.has_output);
        egui::Popup::menu(&export).show(|ui| {
            ui.set_min_width(170.0);
            for (name, format) in [
                (tr!("export-html-menu"), ExportFormat::Html),
                (tr!("export-pdf-menu"), ExportFormat::Pdf),
                (tr!("export-latex-menu"), ExportFormat::Latex),
            ] {
                if ui.button(name).clicked() {
                    action = Some(Action::Export(format));
                    ui.close();
                }
            }
        });
        let theme = icon_button(ui, Icon::Contrast, &tr!("theme-tip"), true);
        egui::Popup::menu(&theme).show(|ui| {
            ui.set_min_width(120.0);
            for (preference, name) in [
                (egui::ThemePreference::Light, &tr!("theme-light")),
                (egui::ThemePreference::Dark, &tr!("theme-dark")),
                (egui::ThemePreference::System, &tr!("theme-system")),
            ] {
                if ui.button(name).clicked() {
                    action = Some(Action::Theme(preference));
                    ui.close();
                }
            }
        });
        right_start = ui.min_rect().left();
    });
    // The logo and the name in the middle, the name cut with "…" to fit between the two sides.
    const MARK: f32 = 22.0;
    let half = (rect.center().x - left_end).min(right_start - rect.center().x) - 12.0 - MARK;
    let mut job = egui::text::LayoutJob::simple_singleline(state.title.to_string(), egui::FontId::proportional(13.0), t.text);
    job.wrap = egui::text::TextWrapping::truncate_at_width((2.0 * half).max(0.0));
    let galley = ui.painter().layout_job(job);
    let at = rect.center() - galley.size() / 2.0;
    if half > MARK {
        let logo = egui::Rect::from_center_size(egui::pos2(at.x - 13.0, rect.center().y), egui::vec2(18.0, 18.0));
        match crate::logo::texture(ui.ctx()) {
            Some(texture) => {
                let size = texture.size_vec2() * (18.0 / texture.size_vec2().y.max(1.0));
                let logo = egui::Rect::from_center_size(egui::pos2(at.x - 4.0 - size.x / 2.0, rect.center().y), size);
                egui::Image::new((texture.id(), size)).tint(t.accent).paint_at(ui, logo);
            }
            None => icons::paint_mono(ui.painter(), logo, Icon::Sigma, t.accent, t.window),
        }
    }
    // Bold by drawing twice, half a point apart: the built-in fonts have no bold face.
    ui.painter().galley(at, galley.clone(), t.text);
    ui.painter().galley(at + egui::vec2(0.5, 0.0), galley, t.text);
    action
}

/// A word in the row of menus: File or a tab. A quiet fill under the pointer.
fn menu_word(ui: &mut egui::Ui, word: &str, color: egui::Color32, bold: bool) -> egui::Response {
    let t = Tokens::get(ui.ctx());
    let font = egui::FontId::proportional(13.0);
    let galley = ui.painter().layout_no_wrap(word.to_string(), font, color);
    let (rect, response) = ui.allocate_exact_size(egui::vec2(galley.size().x + 18.0, MENU_HEIGHT - 4.0), egui::Sense::click());
    if response.hovered() {
        ui.painter().rect_filled(rect, 0.0, t.hover);
    }
    let at = rect.center() - galley.size() / 2.0;
    if bold {
        ui.painter().galley(at + egui::vec2(0.5, 0.0), galley.clone(), color);
    }
    ui.painter().galley(at, galley, color);
    response
}

/// The row of menus — File in the accent, then the tabs, the current one in bold — and the
/// current tab's toolbar under it unless `collapsed`. A click on the current tab, a double-click
/// on any, or Ctrl+F1 folds the toolbar away; a tab clicked while it is folded brings it back.
/// When the commands' names don't fit the window, the buttons show their icons alone and keep
/// their names in the tooltip.
pub(crate) fn ribbon(ui: &mut egui::Ui, tab: &mut Tab, collapsed: &mut bool, state: &State) -> Option<Action> {
    let t = Tokens::get(ui.ctx());
    let mut action = None;
    let (bar, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), MENU_HEIGHT), egui::Sense::hover());
    let mut row =
        ui.new_child(egui::UiBuilder::new().max_rect(bar.shrink2(egui::vec2(10.0, 0.0))).layout(egui::Layout::left_to_right(egui::Align::Center)));
    row.spacing_mut().item_spacing.x = 0.0;
    let file = menu_word(&mut row, &tr!("file"), t.accent, false);
    file.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, tr!("file")));
    if file.on_hover_text(tr!("file-tip")).clicked() {
        action = Some(Action::File);
    }
    for which in TABS {
        let name = which.name();
        let on = *tab == which;
        let response = menu_word(&mut row, &name, if on { t.text } else { t.text_dim }, on);
        response.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, true, on, &name));
        if response.double_clicked() {
            *collapsed = !*collapsed;
        } else if response.clicked() {
            if on && !*collapsed {
                *collapsed = true;
            } else {
                *tab = which;
                *collapsed = false;
            }
        }
    }
    if *collapsed {
        return action;
    }
    ui.add_space(MENU_GAP);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), TOOLBAR_HEIGHT), egui::Sense::hover());
    let mut row =
        ui.new_child(egui::UiBuilder::new().max_rect(rect.shrink2(egui::vec2(10.0, 0.0))).layout(egui::Layout::left_to_right(egui::Align::Center)));
    row.spacing_mut().item_spacing.x = 2.0;
    // The width the tab's buttons took with their names, the last time they had them.
    let natural_id = egui::Id::new(("toolbar-natural-width", *tab as u8));
    let natural: Option<f32> = row.ctx().data(|d| d.get_temp(natural_id));
    let compact = natural.is_some_and(|w| w > rect.width() - 20.0);
    let start = row.cursor().left();
    if let Some(a) = buttons(&mut row, *tab, state, compact) {
        action = Some(a);
    }
    if !compact {
        let width = row.cursor().left() - start;
        row.ctx().data_mut(|d| d.insert_temp(natural_id, width));
    }
    action
}

/// One tab's buttons, in groups set apart by space; `compact` leaves their names to the tooltip.
fn buttons(ui: &mut egui::Ui, tab: Tab, state: &State, compact: bool) -> Option<Action> {
    let mut action = None;
    let mut go = |ui: &mut egui::Ui, icon: Icon, label: &str, tip: &str, enabled: bool, on: bool, then: Action| {
        if small(ui, icon, label, tip, enabled, on, compact) {
            action = Some(then.clone());
        }
    };
    let suggest = |p| Action::Suggest(p);
    match tab {
        Tab::Home => {
            go(ui, Icon::Open, &tr!("open"), &tr!("open-tip"), true, false, Action::Open);
            go(ui, Icon::Save, &tr!("save"), &tr!("save-tip"), true, false, Action::Save);
            divider(ui);
            go(ui, Icon::Run, &tr!("run-syntax"), &tr!("run-syntax-tip"), true, false, Action::Run);
            divider(ui);
            go(ui, Icon::Web, &tr!("export-html"), &tr!("export-html-tip"), state.has_output, false, Action::Export(ExportFormat::Html));
            go(ui, Icon::Pdf, &tr!("export-pdf"), &tr!("export-pdf-tip"), state.has_output, false, Action::Export(ExportFormat::Pdf));
            let tip = &tr!("export-latex-tip");
            go(ui, Icon::Latex, &tr!("export-latex"), tip, state.has_output, false, Action::Export(ExportFormat::Latex));
            go(ui, Icon::Eraser, &tr!("clear-output"), &tr!("clear-output-tip"), state.has_output, false, Action::ClearOutput);
        }
        Tab::Data => {
            go(ui, Icon::InsertCase, &tr!("insert-case"), &tr!("insert-case-tip"), true, false, Action::InsertCase);
            go(ui, Icon::InsertVariable, &tr!("insert-variable"), &tr!("insert-variable-tip"), true, false, Action::InsertVariable);
            divider(ui);
            go(ui, Icon::Labels, &tr!("value-labels"), &tr!("value-labels-tip"), true, state.value_labels, Action::ToggleLabels);
            go(ui, Icon::Dictionary, &tr!("dictionary"), &tr!("dictionary-tip"), true, false, suggest(Procedure::Dictionary));
        }
        Tab::Transform => {
            let tip = &tr!("suggest-tip");
            go(ui, Icon::Compute, &tr!("compute"), tip, true, false, suggest(Procedure::Compute));
            go(ui, Icon::Recode, &tr!("recode"), tip, true, false, suggest(Procedure::Recode));
            go(ui, Icon::Filter, &tr!("select-cases"), tip, true, false, suggest(Procedure::SelectCases));
        }
        Tab::Analyze => {
            let dialog = &tr!("dialog-tip");
            go(ui, Icon::Table, &tr!("descriptives"), dialog, true, false, Action::Dialog(Kind::Descriptives));
            go(ui, Icon::Frequencies, &tr!("frequencies"), dialog, true, false, Action::Dialog(Kind::Frequencies));
            go(ui, Icon::Boxplot, &tr!("explore"), dialog, true, false, Action::Dialog(Kind::Explore));
            divider(ui);
            go(ui, Icon::Crosstabs, &tr!("crosstabs"), dialog, true, false, Action::Dialog(Kind::Crosstabs));
            go(ui, Icon::TTest, &tr!("t-test"), dialog, true, false, Action::Dialog(Kind::TTest));
            go(ui, Icon::Anova, &tr!("oneway"), dialog, true, false, Action::Dialog(Kind::Oneway));
            divider(ui);
            go(ui, Icon::Correlations, &tr!("correlate"), dialog, true, false, Action::Dialog(Kind::Correlations));
            go(ui, Icon::Regression, &tr!("regression"), dialog, true, false, Action::Dialog(Kind::Regression));
            go(ui, Icon::Logistic, &tr!("logistic"), dialog, true, false, Action::Dialog(Kind::Logistic));
            divider(ui);
            go(ui, Icon::Ranks, &tr!("nonparametric"), dialog, true, false, Action::Dialog(Kind::Nonparametric));
        }
        Tab::Graphs => {
            let dialog = &tr!("chart-dialog-tip");
            go(ui, Icon::Histogram, &tr!("histogram"), dialog, true, false, Action::Dialog(Kind::Histogram));
            go(ui, Icon::Bars, &tr!("bar"), dialog, true, false, Action::Dialog(Kind::Bar));
            go(ui, Icon::Scatter, &tr!("scatter"), dialog, true, false, Action::Dialog(Kind::Scatter));
            go(ui, Icon::Boxplot, &tr!("boxplot"), dialog, true, false, Action::Dialog(Kind::Boxplot));
        }
        Tab::View => {
            go(ui, Icon::Grid, &tr!("data-view"), &tr!("data-view-tip"), true, state.view == View::Data, Action::View(View::Data));
            go(ui, Icon::List, &tr!("variable-view"), &tr!("variable-view-tip"), true, state.view == View::Variables, Action::View(View::Variables));
            divider(ui);
            go(ui, Icon::Labels, &tr!("value-labels"), &tr!("value-labels-tip"), true, state.value_labels, Action::ToggleLabels);
            divider(ui);
            if let Some(syntax) = output_style(ui, &state.style, compact) {
                action = Some(Action::Syntax(syntax));
            }
            divider(ui);
            if let Some(lang) = language(ui) {
                action = Some(Action::Language(lang));
            }
        }
    }
    action
}

/// The interface's language.
fn language(ui: &mut egui::Ui) -> Option<mksigma_i18n::Lang> {
    let t = Tokens::get(ui.ctx());
    let current = mksigma_i18n::current();
    let mut chosen = None;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        ui.label(egui::RichText::new(tr!("language-menu")).size(12.0).color(t.text_dim));
        egui::ComboBox::from_id_salt("language").selected_text(current.name()).width(130.0).show_ui(ui, |ui| {
            for lang in mksigma_i18n::Lang::ALL {
                if ui.selectable_label(lang == current, lang.name()).clicked() && lang != current {
                    chosen = Some(lang);
                }
            }
        });
    });
    chosen
}

/// The Output style's controls: font, size, palette, decimals. A change is the `OUTPUT STYLE`
/// command that makes it.
fn output_style(ui: &mut egui::Ui, style: &mksigma_engine::output::Style, compact: bool) -> Option<String> {
    use mksigma_engine::output::style::{Family, Palette};
    let t = Tokens::get(ui.ctx());
    let mut syntax = None;
    // A button that opens the controls, so the toolbar stays one row.
    let button = small_response(ui, Icon::Page, &tr!("output-style"), &tr!("output-style-tip"), true, false, compact);
    egui::Popup::menu(&button).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
        let title = |f: &str| {
            let mut chars = f.chars();
            chars.next().map_or(String::new(), |c| c.to_string() + &chars.as_str().to_lowercase())
        };
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 4.0;
            egui::Grid::new("output-style").num_columns(4).spacing([6.0, 4.0]).show(ui, |ui| {
                ui.label(tr!("font"));
                egui::ComboBox::from_id_salt("style-font").selected_text(title(style.family.name())).width(80.0).show_ui(ui, |ui| {
                    for f in Family::ALL {
                        if ui.selectable_label(style.family == f, title(f.name())).clicked() && style.family != f {
                            syntax = Some(format!("OUTPUT STYLE /FONT={}.", f.name()));
                        }
                    }
                });
                ui.label(tr!("size"));
                egui::ComboBox::from_id_salt("style-size").selected_text(format!("{} pt", style.size)).width(64.0).show_ui(ui, |ui| {
                    for size in [8u8, 9, 10, 11, 12, 14, 16, 18] {
                        if ui.selectable_label(style.size == size, format!("{size} pt")).clicked() && style.size != size {
                            syntax = Some(format!("OUTPUT STYLE /SIZE={size}."));
                        }
                    }
                });
                ui.end_row();
                ui.label(tr!("palette"));
                egui::ComboBox::from_id_salt("style-palette").selected_text(title(style.palette.name())).width(80.0).show_ui(ui, |ui| {
                    for p in Palette::ALL {
                        let label = ui
                            .horizontal(|ui| {
                                let (rect, _) = ui.allocate_exact_size(egui::vec2(12.0, 12.0), egui::Sense::hover());
                                let [r, g, b] = p.colors().map_or([t.accent.r(), t.accent.g(), t.accent.b()], |(bar, _)| bar);
                                ui.painter().rect_filled(rect, 0.0, egui::Color32::from_rgb(r, g, b));
                                ui.selectable_label(style.palette == p, title(p.name()))
                            })
                            .inner;
                        if label.clicked() && style.palette != p {
                            syntax = Some(format!("OUTPUT STYLE /PALETTE={}.", p.name()));
                        }
                    }
                });
                ui.label(tr!("decimals"));
                let shown = style.decimals.map_or(tr!("decimals-auto"), |d| d.to_string());
                egui::ComboBox::from_id_salt("style-decimals").selected_text(shown).width(64.0).show_ui(ui, |ui| {
                    for d in std::iter::once(None).chain((0..=6).map(Some)) {
                        let name = d.map_or(tr!("decimals-auto"), |d: u8| d.to_string());
                        if ui.selectable_label(style.decimals == d, &name).clicked() && style.decimals != d {
                            syntax = Some(format!("OUTPUT STYLE /DECIMALS={}.", d.map_or("AUTO".to_string(), |d| d.to_string())));
                        }
                    }
                });
                ui.end_row();
            });
        });
    });
    syntax
}

/// Every button the ribbon and the File menu have, by name, for the search box.
pub(crate) fn commands() -> Vec<(String, Action)> {
    use Procedure as P;
    vec![
        (tr!("open-ellipsis"), Action::Open),
        (tr!("save"), Action::Save),
        (tr!("save-as-ellipsis"), Action::SaveAs),
        (tr!("save-dashboard-ellipsis"), Action::SaveDashboard),
        (tr!("undo"), Action::Undo),
        (tr!("redo"), Action::Redo),
        (tr!("run-syntax"), Action::Run),
        (tr!("export-html-menu"), Action::Export(ExportFormat::Html)),
        (tr!("export-pdf-menu"), Action::Export(ExportFormat::Pdf)),
        (tr!("export-latex-menu"), Action::Export(ExportFormat::Latex)),
        (tr!("clear-output"), Action::ClearOutput),
        (tr!("insert-case"), Action::InsertCase),
        (tr!("insert-variable"), Action::InsertVariable),
        (tr!("value-labels"), Action::ToggleLabels),
        (tr!("data-view"), Action::View(View::Data)),
        (tr!("variable-view"), Action::View(View::Variables)),
        (tr!("display-dictionary"), Action::Suggest(P::Dictionary)),
        (tr!("compute-variable"), Action::Suggest(P::Compute)),
        (tr!("recode"), Action::Suggest(P::Recode)),
        (tr!("select-cases"), Action::Suggest(P::SelectCases)),
        (tr!("descriptives"), Action::Dialog(Kind::Descriptives)),
        (tr!("frequencies"), Action::Dialog(Kind::Frequencies)),
        (tr!("explore"), Action::Dialog(Kind::Explore)),
        (tr!("crosstabs"), Action::Dialog(Kind::Crosstabs)),
        (tr!("t-test-independent"), Action::Dialog(Kind::TTest)),
        (tr!("oneway"), Action::Dialog(Kind::Oneway)),
        (tr!("correlations-bivariate"), Action::Dialog(Kind::Correlations)),
        (tr!("regression-linear"), Action::Dialog(Kind::Regression)),
        (tr!("logistic-binary"), Action::Dialog(Kind::Logistic)),
        (tr!("nonparametric-tests"), Action::Dialog(Kind::Nonparametric)),
        (tr!("histogram"), Action::Dialog(Kind::Histogram)),
        (tr!("bar-chart"), Action::Dialog(Kind::Bar)),
        (tr!("scatterplot"), Action::Dialog(Kind::Scatter)),
        (tr!("boxplot"), Action::Dialog(Kind::Boxplot)),
        (tr!("autosave"), Action::ToggleAutoSave),
        (tr!("theme-light-full"), Action::Theme(egui::ThemePreference::Light)),
        (tr!("theme-dark-full"), Action::Theme(egui::ThemePreference::Dark)),
    ]
}

/// The end of a group of buttons: a short, light line to the next.
fn divider(ui: &mut egui::Ui) {
    let t = Tokens::get(ui.ctx());
    let (rect, _) = ui.allocate_exact_size(egui::vec2(13.0, 28.0), egui::Sense::hover());
    ui.painter().vline(rect.center().x, rect.y_range().shrink(6.0), egui::Stroke::new(1.0, t.border));
}

/// A toolbar button: a 16-point icon with its name beside it, or the icon alone when
/// `compact` (the name then in the tooltip). `on` marks a toggle that is on.
fn small(ui: &mut egui::Ui, icon: Icon, label: &str, tip: &str, enabled: bool, on: bool, compact: bool) -> bool {
    small_response(ui, icon, label, tip, enabled, on, compact).clicked() && enabled
}

/// [`small`]'s button, for a caller that needs its response (to hang a menu on it).
fn small_response(ui: &mut egui::Ui, icon: Icon, label: &str, tip: &str, enabled: bool, on: bool, compact: bool) -> egui::Response {
    let t = Tokens::get(ui.ctx());
    let galley = (!compact).then(|| ui.painter().layout_no_wrap(label.to_string(), egui::FontId::proportional(12.5), t.text));
    let width = galley.as_ref().map_or(24.0, |g| g.size().x + 30.0);
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(width, BUTTON_HEIGHT), if enabled { egui::Sense::click() } else { egui::Sense::hover() });
    response.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, enabled, on, label));
    let painter = ui.painter();
    if on || (enabled && response.is_pointer_button_down_on()) {
        painter.rect_filled(rect, RADIUS, t.pressed);
    } else if enabled && response.hovered() {
        painter.rect_filled(rect, RADIUS, t.hover);
    }
    if on {
        painter.rect_stroke(rect, RADIUS, egui::Stroke::new(1.0, t.border), egui::StrokeKind::Inside);
    }
    let mut icon_painter = painter.clone();
    if !enabled {
        icon_painter.set_opacity(0.35);
    }
    let icon_rect = egui::Rect::from_center_size(egui::pos2(rect.left() + 12.0, rect.center().y), egui::vec2(16.0, 16.0));
    icons::paint(&icon_painter, icon_rect, icon, &t);
    if let Some(galley) = galley {
        let at = egui::pos2(rect.left() + 24.0, rect.center().y - galley.size().y / 2.0);
        painter.galley(at, galley, if enabled { t.text } else { t.text_dim });
    }
    let tip = if compact { format!("{label}\n{tip}") } else { tip.to_string() };
    response.on_hover_text(tip)
}
