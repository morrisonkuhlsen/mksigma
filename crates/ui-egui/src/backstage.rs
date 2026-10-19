//! The File backstage: a page over the whole window body. On the left, a dark column in
//! Morrison Kühlsen's ink with its logo, MK Sigma's name and the places, grouped (Start: Home,
//! New, Open; This data: Info, Save, Save As, Export, Close). On the right, the place's page,
//! with a Back button in its corner; Esc goes back too. Buttons only report what was asked for.

use mksigma_i18n::tr;
use std::path::{Path, PathBuf};

use crate::icons::{self, Icon};
use crate::output::ExportFormat;
use crate::theme::Tokens;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Page {
    #[default]
    Home,
    New,
    Open,
    Info,
    Export,
}

/// What was asked for in the backstage this frame.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Action {
    Back,
    Page(Page),
    /// A new empty dataset.
    Blank,
    /// The built-in sample dataset.
    Sample,
    /// A recent file.
    Open(PathBuf),
    Browse,
    Save,
    SaveAs,
    /// Close the data (asks first when there are unsaved changes).
    Close,
    /// Close it although it has unsaved changes.
    Discard,
    Export(ExportFormat),
    ExportDashboard,
    SaveDashboard,
}

/// What the pages show.
pub(crate) struct View<'a> {
    pub page: Page,
    pub recent: &'a [PathBuf],
    pub file_name: Option<&'a str>,
    pub file_path: Option<&'a Path>,
    pub modified: bool,
    pub cases: usize,
    pub variables: usize,
    pub weight: Option<&'a str>,
    pub steps: usize,
    pub has_output: bool,
    pub has_dashboard: bool,
    /// Replacing the data was asked for with unsaved changes: Info asks what to do.
    pub confirm_close: bool,
}

const COLUMN: f32 = 248.0;
/// Morrison Kühlsen's ink and the colours on it, the same in both themes.
pub(crate) const INK: egui::Color32 = egui::Color32::from_rgb(0x14, 0x16, 0x1C);
pub(crate) const ON_INK: egui::Color32 = egui::Color32::from_rgb(0xFF, 0xFF, 0xFF);
pub(crate) const ON_INK_SOFT: egui::Color32 = egui::Color32::from_rgba_premultiplied(184, 184, 184, 184);
pub(crate) const BRAND: egui::Color32 = egui::Color32::from_rgb(0x25, 0x40, 0xD8);

/// The theme's tokens drawing in white, for icons on the ink column.
fn on_ink(t: &Tokens) -> Tokens {
    Tokens { accent: ON_INK, accent_soft: egui::Color32::from_white_alpha(50), header_line: egui::Color32::from_white_alpha(50), ..*t }
}

/// A place in the column: a pill, blue when it is the page shown.
fn place(ui: &mut egui::Ui, icon: Icon, label: &str, on: bool) -> bool {
    let t = Tokens::get(ui.ctx());
    let (slot, response) = ui.allocate_exact_size(egui::vec2(COLUMN, 38.0), egui::Sense::click());
    response.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, true, on, label));
    let pill = slot.shrink2(egui::vec2(14.0, 3.0));
    if on {
        ui.painter().rect_filled(pill, 10.0, BRAND);
    } else if response.hovered() {
        ui.painter().rect_filled(pill, 10.0, egui::Color32::from_white_alpha(18));
    }
    let color = if on { ON_INK } else { ON_INK_SOFT };
    icons::paint(
        ui.painter(),
        egui::Rect::from_center_size(egui::pos2(pill.left() + 22.0, pill.center().y), egui::vec2(16.0, 16.0)),
        icon,
        &on_ink(&t),
    );
    ui.painter().text(egui::pos2(pill.left() + 44.0, pill.center().y), egui::Align2::LEFT_CENTER, label, egui::FontId::proportional(14.5), color);
    response.clicked()
}

/// A caption over a group of places.
fn caption(ui: &mut egui::Ui, text: &str) {
    let (slot, _) = ui.allocate_exact_size(egui::vec2(COLUMN, 26.0), egui::Sense::hover());
    ui.painter().text(
        egui::pos2(slot.left() + 30.0, slot.bottom() - 6.0),
        egui::Align2::LEFT_BOTTOM,
        text,
        egui::FontId::proportional(11.0),
        ON_INK_SOFT,
    );
}

pub(crate) fn show(ui: &mut egui::Ui, view: &View) -> Option<Action> {
    let t = Tokens::get(ui.ctx());
    let mut action = None;
    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        action = Some(Action::Back);
    }
    let area = ui.available_rect_before_wrap();
    let column = egui::Rect::from_min_size(area.min, egui::vec2(COLUMN, area.height()));
    ui.painter().rect_filled(column, egui::CornerRadius { nw: 0, ne: 12, sw: 0, se: 12 }, INK);
    ui.painter().rect_filled(egui::Rect::from_min_max(egui::pos2(column.right(), area.top()), area.max), 0.0, t.window);

    let mut col = ui.new_child(egui::UiBuilder::new().max_rect(column).layout(egui::Layout::top_down(egui::Align::Min)));
    col.spacing_mut().item_spacing.y = 0.0;
    col.add_space(26.0);
    // The house's mark, then the program's name and motto.
    col.horizontal(|ui| {
        ui.add_space(28.0);
        if let Some(texture) = crate::logo::morrison_kuhlsen(ui.ctx()) {
            let width = COLUMN - 70.0;
            let size = egui::vec2(width, width * texture.size_vec2().y / texture.size_vec2().x.max(1.0));
            ui.add(egui::Image::new((texture.id(), size)).tint(ON_INK)).on_hover_text("Morrison Kühlsen");
        }
    });
    col.add_space(18.0);
    col.horizontal(|ui| {
        ui.add_space(30.0);
        ui.label(egui::RichText::new("MK Sigma").size(20.0).color(ON_INK));
    });
    col.add_space(2.0);
    col.horizontal(|ui| {
        ui.add_space(30.0);
        ui.label(egui::RichText::new("Ex nihilo, ad æternum").italics().size(12.0).color(ON_INK_SOFT));
    });
    col.add_space(26.0);
    let mut go = |ui: &mut egui::Ui, icon: Icon, label: &str, on: bool, then: Action| {
        if place(ui, icon, label, on) {
            action = Some(then);
        }
    };
    caption(&mut col, &tr!("bs-start"));
    go(&mut col, Icon::House, &tr!("bs-home"), view.page == Page::Home, Action::Page(Page::Home));
    go(&mut col, Icon::Page, &tr!("bs-new"), view.page == Page::New, Action::Page(Page::New));
    go(&mut col, Icon::Open, &tr!("open"), view.page == Page::Open, Action::Page(Page::Open));
    col.add_space(10.0);
    caption(&mut col, &tr!("bs-this-data"));
    go(&mut col, Icon::Info, &tr!("bs-info"), view.page == Page::Info, Action::Page(Page::Info));
    go(&mut col, Icon::Save, &tr!("save"), false, Action::Save);
    go(&mut col, Icon::Save, &tr!("bs-save-as"), false, Action::SaveAs);
    go(&mut col, Icon::Share, &tr!("bs-export"), view.page == Page::Export, Action::Page(Page::Export));
    go(&mut col, Icon::Close, &tr!("bs-close"), false, Action::Close);
    // The house's line at the column's foot.
    let foot = egui::pos2(column.left() + 30.0, column.bottom() - 22.0);
    col.painter().text(foot, egui::Align2::LEFT_BOTTOM, tr!("mk-project"), egui::FontId::proportional(11.5), ON_INK_SOFT);
    col.painter().text(foot + egui::vec2(0.0, 15.0), egui::Align2::LEFT_BOTTOM, "morrisonkuhlsen.com", egui::FontId::proportional(11.5), ON_INK);

    let content = egui::Rect::from_min_max(egui::pos2(column.right() + 48.0, area.top() + 30.0), egui::pos2(area.right() - 28.0, area.bottom()));
    let mut page = ui.new_child(egui::UiBuilder::new().max_rect(content).layout(egui::Layout::top_down(egui::Align::Min)));
    page.set_clip_rect(content.expand2(egui::vec2(8.0, 0.0)));
    // Back to the data, in the page's corner.
    let corner = egui::Rect::from_min_size(egui::pos2(content.right() - 200.0, content.top() - 6.0), egui::vec2(200.0, 32.0));
    let mut back = ui.new_child(egui::UiBuilder::new().max_rect(corner).layout(egui::Layout::right_to_left(egui::Align::Center)));
    let button = back.add(egui::Button::new(egui::RichText::new(tr!("bs-back")).size(13.5)).min_size(egui::vec2(0.0, 30.0)).corner_radius(15));
    if button.on_hover_text("Esc").clicked() {
        action = Some(Action::Back);
    }
    let page_action = egui::ScrollArea::vertical().id_salt("backstage").auto_shrink([false, false]).show(&mut page, |ui| pages(ui, view)).inner;
    ui.allocate_rect(area, egui::Sense::hover());
    page_action.or(action)
}

fn heading(ui: &mut egui::Ui, title: &str, line: &str) {
    let t = Tokens::get(ui.ctx());
    ui.label(egui::RichText::new(title).size(30.0).color(t.text));
    ui.label(egui::RichText::new(line).size(14.0).color(t.text_dim));
    ui.add_space(22.0);
}

fn section(ui: &mut egui::Ui, text: &str) {
    let t = Tokens::get(ui.ctx());
    ui.label(egui::RichText::new(text).size(12.0).color(t.text_dim));
    ui.add_space(8.0);
}

fn pages(ui: &mut egui::Ui, view: &View) -> Option<Action> {
    let t = Tokens::get(ui.ctx());
    let mut action = None;
    match view.page {
        Page::Home | Page::New => {
            if view.page == Page::Home {
                heading(ui, &tr!("bs-welcome"), &tr!("bs-welcome-sub"));
            } else {
                heading(ui, &tr!("bs-new"), &tr!("bs-new-sub"));
            }
            section(ui, &tr!("bs-start"));
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(16.0, 16.0);
                if start_card(ui, Icon::Page, &tr!("bs-blank"), &tr!("bs-blank-sub")) {
                    action = Some(Action::Blank);
                }
                if start_card(ui, Icon::Table, &tr!("bs-sample"), &tr!("bs-sample-sub")) {
                    action = Some(Action::Sample);
                }
                if start_card(ui, Icon::Open, &tr!("bs-open-file"), &tr!("bs-open-file-sub")) {
                    action = Some(Action::Browse);
                }
            });
            if view.page == Page::Home {
                ui.add_space(30.0);
                section(ui, &tr!("bs-recent"));
                if let Some(a) = recent(ui, view.recent) {
                    action = Some(a);
                }
            }
        }
        Page::Open => {
            heading(ui, &tr!("open"), &tr!("bs-open-sub"));
            if ui
                .add(
                    egui::Button::new(egui::RichText::new(tr!("bs-browse")).size(14.0).color(ON_INK))
                        .fill(BRAND)
                        .min_size(egui::vec2(130.0, 34.0))
                        .corner_radius(17),
                )
                .on_hover_text("Ctrl+O")
                .clicked()
            {
                action = Some(Action::Browse);
            }
            ui.add_space(28.0);
            section(ui, &tr!("bs-recent"));
            if let Some(a) = recent(ui, view.recent) {
                action = Some(a);
            }
        }
        Page::Info => {
            heading(ui, view.file_name.unwrap_or(&tr!("untitled")), &view.file_path.map_or_else(|| tr!("bs-not-saved"), |p| p.display().to_string()));
            if view.confirm_close {
                egui::Frame::new()
                    .fill(t.accent_soft)
                    .stroke(egui::Stroke::new(1.0, BRAND))
                    .corner_radius(12)
                    .inner_margin(egui::Margin::same(14))
                    .show(ui, |ui| {
                        ui.label(egui::RichText::new(tr!("bs-unsaved")).strong());
                        ui.add_space(6.0);
                        ui.horizontal(|ui| {
                            if ui.add(egui::Button::new(egui::RichText::new(tr!("save")).color(ON_INK)).fill(BRAND).corner_radius(14)).clicked() {
                                action = Some(Action::Save);
                            }
                            if ui.add(egui::Button::new(tr!("bs-close-without")).corner_radius(14)).clicked() {
                                action = Some(Action::Discard);
                            }
                        });
                    });
                ui.add_space(18.0);
            }
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(14.0, 14.0);
                fact(ui, &tr!("bs-cases"), &view.cases.to_string());
                fact(ui, &tr!("bs-variables"), &view.variables.to_string());
                fact(ui, &tr!("bs-weighted-by"), view.weight.unwrap_or(&tr!("bs-nothing")));
                fact(ui, &tr!("bs-applied-steps"), &view.steps.to_string());
                fact(ui, &tr!("bs-saved"), &if view.modified { tr!("bs-not-yet") } else { tr!("bs-yes") });
            });
        }
        Page::Export => {
            heading(ui, &tr!("bs-export"), &tr!("bs-export-sub"));
            let output = view.has_output;
            let board = view.has_dashboard;
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(16.0, 16.0);
                let mut card = |ui: &mut egui::Ui, icon: Icon, title: &str, text: &str, enabled: bool, then: Action| {
                    if ui.add_enabled_ui(enabled, |ui| start_card(ui, icon, title, text)).inner {
                        action = Some(then);
                    }
                };
                card(ui, Icon::Web, &tr!("bs-web-page"), &tr!("bs-web-page-sub"), output, Action::Export(ExportFormat::Html));
                card(ui, Icon::Pdf, "PDF", &tr!("bs-pdf-sub"), output, Action::Export(ExportFormat::Pdf));
                card(ui, Icon::Latex, "LaTeX", &tr!("bs-latex-sub"), output, Action::Export(ExportFormat::Latex));
                card(ui, Icon::Grid, &tr!("bs-interactive"), &tr!("bs-interactive-sub"), board, Action::ExportDashboard);
                card(ui, Icon::List, &tr!("bs-dashboard-syntax"), &tr!("bs-dashboard-syntax-sub"), board, Action::SaveDashboard);
            });
        }
    }
    action
}

/// A large card to start something: an icon in a round badge, a title and a line about it.
fn start_card(ui: &mut egui::Ui, icon: Icon, title: &str, text: &str) -> bool {
    let t = Tokens::get(ui.ctx());
    let size = egui::vec2(236.0, 132.0);
    let (rect, response) = ui.allocate_exact_size(size, if ui.is_enabled() { egui::Sense::click() } else { egui::Sense::hover() });
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), title));
    let hot = response.hovered() && ui.is_enabled();
    let p = ui.painter();
    p.rect_filled(rect, 14.0, t.ribbon);
    p.rect_stroke(rect, 14.0, egui::Stroke::new(if hot { 2.0 } else { 1.0 }, if hot { BRAND } else { t.border }), egui::StrokeKind::Inside);
    let badge = egui::pos2(rect.left() + 34.0, rect.top() + 34.0);
    p.circle_filled(badge, 18.0, if hot { BRAND } else { t.accent_soft });
    let ink = if hot { on_ink(&t) } else { t };
    let mut painter = p.clone();
    if !ui.is_enabled() {
        painter.set_opacity(0.4);
    }
    icons::paint(&painter, egui::Rect::from_center_size(badge, egui::vec2(18.0, 18.0)), icon, &ink);
    let text_color = if ui.is_enabled() { t.text } else { t.text_dim };
    painter.text(egui::pos2(rect.left() + 18.0, rect.top() + 66.0), egui::Align2::LEFT_TOP, title, egui::FontId::proportional(15.5), text_color);
    let galley = painter.layout(text.to_string(), egui::FontId::proportional(12.5), t.text_dim, size.x - 36.0);
    painter.galley(egui::pos2(rect.left() + 18.0, rect.top() + 88.0), galley, t.text_dim);
    response.clicked() && ui.is_enabled()
}

/// One fact about the data on Info: a small card with a figure.
fn fact(ui: &mut egui::Ui, name: &str, value: &str) {
    let t = Tokens::get(ui.ctx());
    egui::Frame::new().fill(t.ribbon).stroke(egui::Stroke::new(1.0, t.border)).corner_radius(12).inner_margin(egui::Margin::symmetric(16, 12)).show(
        ui,
        |ui| {
            ui.set_min_width(130.0);
            ui.label(egui::RichText::new(name).size(12.0).color(t.text_dim));
            ui.label(egui::RichText::new(value).size(20.0).color(t.text));
        },
    );
}

/// How long ago, in words: "just now", "5 min ago", "3 h ago", "2 days ago".
fn ago(seconds: u64) -> String {
    match seconds {
        0..60 => tr!("ago-now"),
        60..3600 => tr!("ago-min", n = (seconds / 60) as usize),
        3600..86_400 => tr!("ago-h", n = (seconds / 3600) as usize),
        86_400..172_800 => tr!("ago-yesterday"),
        _ => tr!("ago-days", n = (seconds / 86_400) as usize),
    }
}

/// The recent files, newest first: a row each with the kind, name, folder and when the file
/// last changed. Files gone since show dimmed.
fn recent(ui: &mut egui::Ui, files: &[PathBuf]) -> Option<Action> {
    let t = Tokens::get(ui.ctx());
    if files.is_empty() {
        ui.label(egui::RichText::new(tr!("bs-recent-empty")).color(t.text_dim));
        return None;
    }
    let mut action = None;
    let width = ui.available_width().min(760.0);
    for path in files {
        let name = path.file_name().map_or_else(|| path.display().to_string(), |n| n.to_string_lossy().into_owned());
        let folder = path.parent().map(|p| p.display().to_string()).unwrap_or_default();
        let changed = std::fs::metadata(path).and_then(|m| m.modified()).ok().and_then(|m| m.elapsed().ok()).map(|d| ago(d.as_secs()));
        let exists = changed.is_some();
        let (rect, response) = ui.allocate_exact_size(egui::vec2(width, 44.0), if exists { egui::Sense::click() } else { egui::Sense::hover() });
        response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, exists, &name));
        if response.hovered() && exists {
            ui.painter().rect_filled(rect, 10.0, t.hover);
        }
        let kind = match path.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase).as_deref() {
            Some("sps") => (Icon::Grid, &tr!("bs-kind-dashboard")),
            Some("csv" | "tsv" | "txt" | "dat") => (Icon::List, &tr!("bs-kind-text")),
            _ => (Icon::Table, &tr!("bs-kind-spss")),
        };
        let mut painter = ui.painter().clone();
        if !exists {
            painter.set_opacity(0.45);
        }
        icons::paint(&painter, egui::Rect::from_center_size(egui::pos2(rect.left() + 22.0, rect.center().y), egui::vec2(18.0, 18.0)), kind.0, &t);
        painter.text(
            egui::pos2(rect.left() + 46.0, rect.center().y - 8.0),
            egui::Align2::LEFT_CENTER,
            &name,
            egui::FontId::proportional(14.0),
            t.text,
        );
        painter.text(
            egui::pos2(rect.left() + 46.0, rect.center().y + 9.0),
            egui::Align2::LEFT_CENTER,
            &folder,
            egui::FontId::proportional(11.5),
            t.text_dim,
        );
        let right = format!("{}  ·  {}", kind.1, changed.as_deref().unwrap_or(&tr!("bs-not-found")));
        painter.text(
            egui::pos2(rect.right() - 14.0, rect.center().y),
            egui::Align2::RIGHT_CENTER,
            right,
            egui::FontId::proportional(12.0),
            t.text_dim,
        );
        if response.clicked() && exists {
            action = Some(Action::Open(path.clone()));
        }
    }
    action
}

#[cfg(test)]
mod tests {
    #[test]
    fn ago_in_words() {
        assert_eq!(super::ago(5), "just now");
        assert_eq!(super::ago(125), "2 min ago");
        assert_eq!(super::ago(7200), "2 h ago");
        assert_eq!(super::ago(90_000), "yesterday");
        assert_eq!(super::ago(3 * 86_400), "3 days ago");
    }
}
