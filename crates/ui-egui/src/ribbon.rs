//! The top of the window: a title row (logo, quick-access Save, Undo and Redo, the document's
//! name, the theme switch), a row of tabs and the ribbon under them. Buttons only report what
//! was asked for; the app acts through the session.

use crate::View;
use crate::icons::{self, Icon};
use crate::output::ExportFormat;
use crate::procedure::Kind;
use crate::suggest::Procedure;
use crate::theme::{self, Tokens};
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

pub(crate) const TITLE_HEIGHT: f32 = 36.0;
const RIBBON_HEIGHT: f32 = 78.0;

/// An undo label short enough for a tooltip.
fn short(label: &str) -> String {
    let mut s: String = label.chars().take(40).collect();
    if label.chars().count() > 40 {
        s.push('…');
    }
    s
}

/// A small square icon button with an accessible name; clicks count only when enabled.
fn icon_button(ui: &mut egui::Ui, icon: Icon, name: &str, enabled: bool) -> egui::Response {
    let t = Tokens::get(ui.ctx());
    let (rect, response) = ui.allocate_exact_size(egui::vec2(28.0, 26.0), if enabled { egui::Sense::click() } else { egui::Sense::hover() });
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, name));
    if enabled && response.hovered() {
        ui.painter().rect_filled(rect, 0.0, t.hover);
    }
    let icon_rect = egui::Rect::from_center_size(rect.center(), egui::vec2(16.0, 16.0));
    // Disabled: the same drawing, faded.
    let mut painter = ui.painter().clone();
    if !enabled {
        painter.set_opacity(0.35);
    }
    icons::paint(&painter, icon_rect, icon, &t);
    response.on_hover_text(name)
}

/// An on/off switch: a square track with a square knob.
fn switch(ui: &mut egui::Ui, on: bool, name: &str) -> egui::Response {
    let t = Tokens::get(ui.ctx());
    let (r, response) = ui.allocate_exact_size(egui::vec2(30.0, 16.0), egui::Sense::click());
    response.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Checkbox, true, on, name));
    let painter = ui.painter();
    painter.rect_filled(r, 0.0, if on { t.accent } else { egui::Color32::TRANSPARENT });
    painter.rect_stroke(r, 0.0, egui::Stroke::new(1.0, if on { t.accent } else { t.text_dim }), egui::StrokeKind::Inside);
    let knob = if on { egui::pos2(r.right() - 8.0, r.center().y) } else { egui::pos2(r.left() + 8.0, r.center().y) };
    painter.rect_filled(egui::Rect::from_center_size(knob, egui::vec2(10.0, 10.0)), 0.0, if on { egui::Color32::WHITE } else { t.text_dim });
    response
}

/// A filled accent button with a white icon shape and text, for the one main action of a panel.
pub(crate) fn accent_button(ui: &mut egui::Ui, icon: Icon, text: &str) -> egui::Response {
    let t = Tokens::get(ui.ctx());
    let galley = ui.painter().layout_no_wrap(text.to_string(), egui::FontId::proportional(13.0), egui::Color32::WHITE);
    let size = egui::vec2(galley.size().x + 36.0, 24.0);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, text));
    let fill = if response.hovered() { t.accent.gamma_multiply(0.85) } else { t.accent };
    ui.painter().rect_filled(rect, 0.0, fill);
    let mut white = t;
    white.accent = egui::Color32::WHITE;
    icons::paint(ui.painter(), egui::Rect::from_center_size(egui::pos2(rect.left() + 15.0, rect.center().y), egui::vec2(12.0, 12.0)), icon, &white);
    ui.painter().galley(egui::pos2(rect.left() + 26.0, rect.center().y - galley.size().y / 2.0), galley, egui::Color32::WHITE);
    response
}

pub(crate) fn title_bar(ui: &mut egui::Ui, state: &State) -> Option<Action> {
    let t = Tokens::get(ui.ctx());
    let mut action = None;
    let (rect, background) = ui.allocate_exact_size(egui::vec2(ui.available_width(), TITLE_HEIGHT), egui::Sense::click_and_drag());
    // The row is the window's title bar: drag it to move the window, double-click to maximize.
    if background.drag_started() {
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
    }
    if background.double_clicked() {
        let maximized = ui.ctx().input(|i| i.viewport().maximized.unwrap_or(false));
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
    }
    ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, state.title, egui::FontId::proportional(13.5), t.text);
    // Bold by drawing twice, half a point apart: the built-in fonts have no bold face.
    ui.painter().text(rect.center() + egui::vec2(0.5, 0.0), egui::Align2::CENTER_CENTER, state.title, egui::FontId::proportional(13.5), t.text);
    let mut row =
        ui.new_child(egui::UiBuilder::new().max_rect(rect.shrink2(egui::vec2(10.0, 0.0))).layout(egui::Layout::left_to_right(egui::Align::Center)));
    row.spacing_mut().item_spacing.x = 2.0;
    match crate::logo::texture(row.ctx()) {
        Some(texture) => {
            let size = texture.size_vec2() * (22.0 / texture.size_vec2().y.max(1.0));
            // In the accent, like every icon: the pale original disappears on a light window.
            row.add(egui::Image::new((texture.id(), size)).tint(t.accent)).on_hover_text("MK Sigma");
        }
        None => {
            icons::show(&mut row, Icon::Sigma, 18.0).on_hover_text("MK Sigma");
        }
    }
    row.add_space(10.0);
    row.label(egui::RichText::new(tr!("autosave")).size(12.5).color(t.text_dim));
    row.add_space(4.0);
    let tip = if state.autosave { &tr!("autosave-on-tip") } else { &tr!("autosave-off-tip") };
    if switch(&mut row, state.autosave, &tr!("autosave")).on_hover_text(tip).clicked() {
        action = Some(Action::ToggleAutoSave);
    }
    row.add_space(8.0);
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
    row.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        if icon_button(ui, Icon::Search, &tr!("search-tip"), true).clicked() {
            action = Some(Action::Search);
        }
        let export = icon_button(ui, Icon::Share, &tr!("export-output-tip"), state.has_output);
        egui::Popup::menu(&export).show(|ui| {
            if ui.button(tr!("export-html-menu")).clicked() {
                action = Some(Action::Export(ExportFormat::Html));
            }
            if ui.button(tr!("export-pdf-menu")).clicked() {
                action = Some(Action::Export(ExportFormat::Pdf));
            }
        });
        let theme = icon_button(ui, Icon::Contrast, &tr!("theme-tip"), true);
        if theme.clicked() {
            action = Some(Action::Theme(if t.dark { egui::ThemePreference::Light } else { egui::ThemePreference::Dark }));
        }
        theme.context_menu(|ui| {
            for (preference, name) in [
                (egui::ThemePreference::Light, &tr!("theme-light")),
                (egui::ThemePreference::Dark, &tr!("theme-dark")),
                (egui::ThemePreference::System, &tr!("theme-system")),
            ] {
                if ui.button(name).clicked() {
                    action = Some(Action::Theme(preference));
                }
            }
        });
    });
    action
}

/// The tab row, with File as a menu at its start, and the ribbon of the current tab unless
/// `collapsed`. The chevron at the row's end, a double-click on a tab or Ctrl+F1 hides or shows
/// the ribbon; a tab clicked while it is hidden shows it again.
pub(crate) fn ribbon(ui: &mut egui::Ui, tab: &mut Tab, collapsed: &mut bool, state: &State) -> Option<Action> {
    let t = Tokens::get(ui.ctx());
    let mut action = None;
    ui.horizontal(|ui| {
        ui.add_space(4.0);
        ui.spacing_mut().item_spacing.x = 16.0;
        // Rows centre what they hold on the height known so far: make it the tabs' from the start.
        ui.set_min_height(28.0);
        // File reads as a tab, not a framed button.
        let widgets = &mut ui.visuals_mut().widgets;
        widgets.inactive.bg_stroke = egui::Stroke::NONE;
        widgets.inactive.weak_bg_fill = egui::Color32::TRANSPARENT;
        let file = egui::RichText::new(tr!("file")).size(14.0).color(t.accent);
        // File opens the backstage over the window: open, save, export, recent files.
        if ui.add(egui::Button::new(file).frame(false).min_size(egui::vec2(0.0, 28.0))).on_hover_text(tr!("file-tip")).clicked() {
            action = Some(Action::File);
        }
        ui.add_space(6.0);
        for which in TABS {
            let name = which.name();
            let name = name.as_str();
            let on = *tab == which;
            let text = egui::RichText::new(name).size(14.0).color(if on { t.text } else { t.text_dim });
            let response = ui.add(egui::Button::new(if on { text.strong() } else { text }).frame(false).min_size(egui::vec2(0.0, 28.0)));
            if on {
                let r = response.rect;
                let bar = egui::Rect::from_min_max(egui::pos2(r.left() + 4.0, r.bottom() - 3.0), egui::pos2(r.right() - 4.0, r.bottom()));
                ui.painter().rect_filled(bar, 0.0, t.accent);
            }
            if response.double_clicked() {
                *collapsed = !*collapsed;
            } else if response.clicked() {
                *tab = which;
                *collapsed = false;
            }
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_space(4.0);
            let (icon, name) = if *collapsed { (Icon::ChevronDown, &tr!("ribbon-show")) } else { (Icon::ChevronUp, &tr!("ribbon-hide")) };
            if icon_button(ui, icon, name, true).clicked() {
                *collapsed = !*collapsed;
            }
        });
    });
    if *collapsed {
        return action;
    }
    ui.add_space(2.0);
    theme::card(&t).inner_margin(egui::Margin::symmetric(8, 4)).show(ui, |ui| {
        ui.set_min_size(egui::vec2(ui.available_width(), RIBBON_HEIGHT));
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            if let Some(a) = buttons(ui, *tab, state) {
                action = Some(a);
            }
        });
    });
    action
}

/// One tab's buttons, in groups divided by thin lines.
fn buttons(ui: &mut egui::Ui, tab: Tab, state: &State) -> Option<Action> {
    let mut action = None;
    let mut go = |ui: &mut egui::Ui, icon: Icon, label: &str, tip: &str, enabled: bool, on: bool, then: Action| {
        if large(ui, icon, label, tip, enabled, on) {
            action = Some(then.clone());
        }
    };
    let suggest = |p| Action::Suggest(p);
    match tab {
        Tab::Home => {
            go(ui, Icon::Open, &tr!("open"), &tr!("open-tip"), true, false, Action::Open);
            go(ui, Icon::Save, &tr!("save"), &tr!("save-tip"), true, false, Action::Save);
            divider(ui);
            go(ui, Icon::Undo, &tr!("undo"), state.undo.unwrap_or(&tr!("nothing-to-undo")), state.undo.is_some(), false, Action::Undo);
            go(ui, Icon::Redo, &tr!("redo"), state.redo.unwrap_or(&tr!("nothing-to-redo")), state.redo.is_some(), false, Action::Redo);
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
            if let Some(syntax) = output_style(ui, &state.style) {
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
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 4.0;
        ui.label(egui::RichText::new(tr!("language-menu")).size(11.0).color(t.text_dim));
        egui::ComboBox::from_id_salt("language").selected_text(current.name()).width(150.0).show_ui(ui, |ui| {
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
fn output_style(ui: &mut egui::Ui, style: &mksigma_engine::output::Style) -> Option<String> {
    use mksigma_engine::output::style::{Family, Palette};
    let t = Tokens::get(ui.ctx());
    let mut syntax = None;
    let title = |f: &str| {
        let mut chars = f.chars();
        chars.next().map_or(String::new(), |c| c.to_string() + &chars.as_str().to_lowercase())
    };
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 4.0;
        ui.label(egui::RichText::new(tr!("output-style")).size(11.0).color(t.text_dim)).on_hover_text(tr!("output-style-tip"));
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

fn divider(ui: &mut egui::Ui) {
    let t = Tokens::get(ui.ctx());
    let (rect, _) = ui.allocate_exact_size(egui::vec2(13.0, RIBBON_HEIGHT - 8.0), egui::Sense::hover());
    ui.painter().vline(rect.center().x, rect.y_range().shrink(6.0), egui::Stroke::new(1.0, t.border));
}

/// A large ribbon button: a 32-point icon over its name. `on` marks a toggle that is on.
fn large(ui: &mut egui::Ui, icon: Icon, label: &str, tip: &str, enabled: bool, on: bool) -> bool {
    let t = Tokens::get(ui.ctx());
    let font = egui::FontId::proportional(12.0);
    // Names of two words go on two lines, as ribbons set them.
    let text = match label.split_once(' ') {
        Some((a, b)) if label.len() > 9 => format!("{a}\n{b}"),
        _ => label.to_string(),
    };
    let mut job = egui::text::LayoutJob::simple(text, font, if enabled { t.text } else { t.text_dim }, f32::INFINITY);
    job.halign = egui::Align::Center;
    let galley = ui.painter().layout_job(job);
    let width = (galley.size().x + 14.0).max(52.0);
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(width, RIBBON_HEIGHT - 4.0), if enabled { egui::Sense::click() } else { egui::Sense::hover() });
    response.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, enabled, on, label));
    let painter = ui.painter();
    if on {
        painter.rect_filled(rect, 0.0, t.accent_soft);
    } else if enabled && response.hovered() {
        painter.rect_filled(rect, 0.0, if response.is_pointer_button_down_on() { t.pressed } else { t.hover });
    }
    let icon_rect = egui::Rect::from_center_size(egui::pos2(rect.center().x, rect.top() + 22.0), egui::vec2(30.0, 30.0));
    let mut icon_painter = painter.clone();
    if !enabled {
        icon_painter.set_opacity(0.35);
    }
    icons::paint(&icon_painter, icon_rect, icon, &t);
    // Centred lines are laid out around x = 0.
    painter.galley(egui::pos2(rect.center().x, rect.top() + 42.0), galley, t.text);
    response.on_hover_text(tip).clicked() && enabled
}
