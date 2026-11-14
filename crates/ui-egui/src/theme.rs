//! Design tokens: the colours every part of the window is painted with, light and dark. The
//! look is its sibling MK Matrix's: in light, white chrome and neutral greys with terracotta
//! (#A0522D) the only colour; in dark, Morrison Kühlsen's ink in cool layers with a lighter
//! terracotta (#E08A5F). Panes are set apart by hairlines, the data grid runs edge to edge. The
//! icons keep one colour per meaning.

use egui::{Color32, Stroke, Visuals};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tokens {
    pub dark: bool,
    /// Behind everything: the title row, the tab bar, the status bar.
    pub window: Color32,
    /// The ribbon's panel and the cards (syntax, steps, Output).
    pub ribbon: Color32,
    pub border: Color32,
    pub text: Color32,
    pub text_dim: Color32,
    pub accent: Color32,
    pub accent_soft: Color32,
    pub hover: Color32,
    pub pressed: Color32,
    pub grid_bg: Color32,
    pub gridline: Color32,
    pub header_bg: Color32,
    pub header_text: Color32,
    pub header_line: Color32,
    /// Cells' text; dimmed for missing values.
    pub cell_text: Color32,
    pub input_bg: Color32,
    /// The Output's page.
    pub paper: Color32,
    /// The desk the Output's page lies on: a tone below the window, so the page stands out.
    pub desk: Color32,
    /// Icon colours beside the accent, one meaning each: charts and pages (blue, teal), running
    /// and adding (green), files and labels (orange), removing and PDF (red), relations between
    /// variables and Learn (violet).
    pub blue: Color32,
    pub teal: Color32,
    pub green: Color32,
    pub orange: Color32,
    pub red: Color32,
    pub violet: Color32,
    pub danger: Color32,
    pub warn: Color32,
}

impl Tokens {
    pub fn light() -> Tokens {
        Tokens {
            dark: false,
            window: Color32::from_rgb(0xFF, 0xFF, 0xFF),
            ribbon: Color32::from_rgb(0xFF, 0xFF, 0xFF),
            border: Color32::from_rgb(0xE0, 0xE0, 0xE0),
            text: Color32::from_rgb(0x24, 0x24, 0x24),
            text_dim: Color32::from_rgb(0x61, 0x61, 0x61),
            accent: Color32::from_rgb(0xA0, 0x52, 0x2D),
            accent_soft: Color32::from_rgb(0xF5, 0xE3, 0xD8),
            hover: Color32::from_rgb(0xEB, 0xEB, 0xEB),
            pressed: Color32::from_rgb(0xDD, 0xDD, 0xDD),
            grid_bg: Color32::from_rgb(0xFF, 0xFF, 0xFF),
            gridline: Color32::from_rgb(0xE9, 0xE7, 0xE4),
            header_bg: Color32::from_rgb(0xFF, 0xFF, 0xFF),
            header_text: Color32::from_rgb(0x6F, 0x6B, 0x66),
            header_line: Color32::from_rgb(0xE0, 0xDE, 0xDB),
            cell_text: Color32::from_rgb(0x1F, 0x1F, 0x1F),
            input_bg: Color32::from_rgb(0xFF, 0xFF, 0xFF),
            paper: Color32::from_rgb(0xFF, 0xFF, 0xFF),
            desk: Color32::from_rgb(0xEF, 0xEF, 0xEF),
            blue: Color32::from_rgb(0x2F, 0x6F, 0xCF),
            teal: Color32::from_rgb(0x13, 0x80, 0x8A),
            green: Color32::from_rgb(0x1E, 0x8A, 0x4C),
            orange: Color32::from_rgb(0xC8, 0x60, 0x1A),
            red: Color32::from_rgb(0xC8, 0x37, 0x2D),
            violet: Color32::from_rgb(0x83, 0x50, 0xC8),
            danger: Color32::from_rgb(0xC4, 0x2B, 0x1C),
            warn: Color32::from_rgb(0xA3, 0x62, 0x00),
        }
    }

    pub fn dark() -> Tokens {
        Tokens {
            dark: true,
            window: Color32::from_rgb(0x1B, 0x1E, 0x25),
            ribbon: Color32::from_rgb(0x1F, 0x23, 0x2B),
            border: Color32::from_rgb(0x34, 0x38, 0x41),
            text: Color32::from_rgb(0xE6, 0xE8, 0xEC),
            text_dim: Color32::from_rgb(0x9A, 0xA1, 0xAD),
            accent: Color32::from_rgb(0xE0, 0x8A, 0x5F),
            accent_soft: Color32::from_rgb(0x4A, 0x2A, 0x1E),
            hover: Color32::from_rgb(0x2A, 0x2E, 0x37),
            pressed: Color32::from_rgb(0x34, 0x38, 0x41),
            grid_bg: Color32::from_rgb(0x2B, 0x2F, 0x38),
            gridline: Color32::from_rgb(0x34, 0x38, 0x41),
            header_bg: Color32::from_rgb(0x2B, 0x2F, 0x38),
            header_text: Color32::from_rgb(0x9A, 0xA1, 0xAD),
            header_line: Color32::from_rgb(0x3A, 0x3F, 0x4A),
            cell_text: Color32::from_rgb(0xE6, 0xE8, 0xEC),
            input_bg: Color32::from_rgb(0x10, 0x12, 0x17),
            paper: Color32::from_rgb(0x2A, 0x2E, 0x37),
            desk: Color32::from_rgb(0x15, 0x17, 0x1C),
            blue: Color32::from_rgb(0x6E, 0xA8, 0xF7),
            teal: Color32::from_rgb(0x4C, 0xC7, 0xCF),
            green: Color32::from_rgb(0x5C, 0xC5, 0x87),
            orange: Color32::from_rgb(0xF0, 0x96, 0x5A),
            red: Color32::from_rgb(0xF0, 0x7B, 0x6E),
            violet: Color32::from_rgb(0xB3, 0x94, 0xEE),
            danger: Color32::from_rgb(0xFF, 0x95, 0x85),
            warn: Color32::from_rgb(0xF0, 0xC0, 0x60),
        }
    }

    /// The palette of the theme in use.
    pub fn get(ctx: &egui::Context) -> Tokens {
        if ctx.theme() == egui::Theme::Dark { Tokens::dark() } else { Tokens::light() }
    }
}

/// Installs both palettes in egui's light and dark style slots and follows the system's choice
/// (light when the system doesn't say).
pub fn apply(ctx: &egui::Context) {
    for theme in [egui::Theme::Light, egui::Theme::Dark] {
        let t = if theme == egui::Theme::Dark { Tokens::dark() } else { Tokens::light() };
        let mut v = if t.dark { Visuals::dark() } else { Visuals::light() };
        v.panel_fill = t.window;
        v.window_fill = t.ribbon;
        v.window_stroke = Stroke::new(1.0, t.border);
        v.extreme_bg_color = t.input_bg;
        v.faint_bg_color = t.header_bg;
        v.selection.bg_fill = t.accent_soft;
        v.selection.stroke = Stroke::new(1.0, t.accent);
        v.hyperlink_color = t.accent;
        v.warn_fg_color = t.warn;
        v.error_fg_color = t.danger;
        v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, t.text);
        v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, t.border);
        v.widgets.inactive.fg_stroke = Stroke::new(1.0, t.text);
        v.widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
        v.widgets.inactive.bg_fill = t.hover;
        v.widgets.inactive.bg_stroke = Stroke::new(1.0, t.border);
        v.widgets.hovered.weak_bg_fill = t.hover;
        v.widgets.hovered.bg_fill = t.hover;
        v.widgets.hovered.bg_stroke = Stroke::new(1.0, t.header_line);
        v.widgets.active.weak_bg_fill = t.pressed;
        v.widgets.active.bg_fill = t.pressed;
        for w in [&mut v.widgets.noninteractive, &mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active, &mut v.widgets.open] {
            w.corner_radius = egui::CornerRadius::ZERO;
        }
        v.window_corner_radius = egui::CornerRadius::ZERO;
        v.menu_corner_radius = egui::CornerRadius::ZERO;
        // A dialog's title bar is the open state's fill: the faint header tone, not a grey band.
        v.widgets.open.weak_bg_fill = t.header_bg;
        v.widgets.open.bg_fill = t.header_bg;
        v.widgets.open.bg_stroke = Stroke::new(1.0, t.border);
        // A soft, wide shadow lifts dialogs and menus without a heavy edge.
        v.popup_shadow =
            egui::epaint::Shadow { offset: [0, 8], blur: 28, spread: 0, color: Color32::from_black_alpha(if t.dark { 110 } else { 28 }) };
        v.window_shadow = v.popup_shadow;
        v.striped = true;
        ctx.set_visuals_of(theme, v);
    }
    ctx.options_mut(|o| {
        o.fallback_theme = egui::Theme::Light;
        o.theme_preference = egui::ThemePreference::System;
    });
    ctx.all_styles_mut(|s| {
        s.spacing.item_spacing = egui::vec2(6.0, 4.0);
        s.spacing.button_padding = egui::vec2(10.0, 4.0);
        s.spacing.interact_size.y = 24.0;
        s.spacing.window_margin = egui::Margin::same(14);
        s.spacing.menu_margin = egui::Margin::same(6);
        s.text_styles.insert(egui::TextStyle::Body, egui::FontId::proportional(13.0));
        s.text_styles.insert(egui::TextStyle::Button, egui::FontId::proportional(13.0));
        s.text_styles.insert(egui::TextStyle::Small, egui::FontId::proportional(11.0));
        s.text_styles.insert(egui::TextStyle::Heading, egui::FontId::proportional(18.0));
        s.text_styles.insert(egui::TextStyle::Monospace, egui::FontId::monospace(12.5));
    });
}

/// Corner radius of a pane.
pub const PANE_RADIUS: u8 = 6;

/// A pane: the syntax editor, the Output. No border: the space around it and its tone, a step
/// lighter (or darker) than the window's, set it apart.
pub fn card(t: &Tokens) -> egui::Frame {
    egui::Frame::new().fill(t.ribbon).corner_radius(PANE_RADIUS).inner_margin(egui::Margin::symmetric(12, 8))
}

/// Paints the shadow and hairline of a sheet lying at `rect` on the desk, as paper is lifted:
/// a wide, soft shadow beneath and a close, crisp one at its edge. Call it before the sheet's fill
/// is painted, or into a shape slot reserved before it.
pub fn sheet_shadow(t: &Tokens, rect: egui::Rect) -> Vec<egui::Shape> {
    let (wide, close) = if t.dark { (110, 140) } else { (28, 40) };
    let shadow = |offset: [i8; 2], blur: u8, alpha: u8| {
        egui::Shadow { offset, blur, spread: 0, color: Color32::from_black_alpha(alpha) }.as_shape(rect, egui::CornerRadius::ZERO)
    };
    vec![
        shadow([0, 10], 36, wide).into(),
        shadow([0, 1], 3, close).into(),
        egui::Shape::rect_stroke(rect, 0.0, Stroke::new(1.0, Color32::from_black_alpha(if t.dark { 90 } else { 18 })), egui::StrokeKind::Outside),
    ]
}

/// Text and icons on the accent colour: white on the light theme's deep blue, near-black on the
/// dark theme's pale one, so both read clearly.
pub fn on_accent(t: &Tokens) -> Color32 {
    if t.dark { t.window } else { Color32::WHITE }
}

/// Height of a pane's header row.
pub const HEADER_HEIGHT: f32 = 28.0;

/// A pane's header, the same in every pane: an icon and the title on the left, `actions` laid
/// out from the right, and a hairline under the row.
pub fn pane_header<R>(ui: &mut egui::Ui, icon: crate::icons::Icon, title: &str, tip: &str, actions: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let t = Tokens::get(ui.ctx());
    let inner = ui
        .horizontal(|ui| {
            ui.set_min_height(HEADER_HEIGHT);
            ui.spacing_mut().item_spacing.x = 6.0;
            crate::icons::show(ui, icon, 14.0);
            let label = ui.label(egui::RichText::new(title).size(13.5).color(t.text).strong());
            if !tip.is_empty() {
                label.on_hover_text(tip);
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                actions(ui)
            })
            .inner
        })
        .inner;
    ui.painter().hline(ui.max_rect().x_range(), ui.cursor().top(), Stroke::new(1.0, t.border));
    ui.add_space(8.0);
    inner
}

/// A quiet action in a header or beside an Output item: text in the dim colour, a tint only
/// under the pointer. Disabled, it fades.
pub fn ghost(ui: &mut egui::Ui, text: &str, tip: &str, enabled: bool) -> bool {
    let t = Tokens::get(ui.ctx());
    let galley = ui.painter().layout_no_wrap(text.to_string(), egui::FontId::proportional(12.5), t.text_dim);
    let size = egui::vec2(galley.size().x + 14.0, 22.0);
    let sense = if enabled { egui::Sense::click() } else { egui::Sense::hover() };
    let (rect, response) = ui.allocate_exact_size(size, sense);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, text));
    let color = if !enabled {
        t.text_dim.gamma_multiply(0.45)
    } else if response.hovered() {
        ui.painter().rect_filled(rect, 0.0, t.hover);
        t.text
    } else {
        t.text_dim
    };
    ui.painter().galley_with_override_text_color(rect.center() - galley.size() / 2.0, galley, color);
    let response = if tip.is_empty() { response } else { response.on_hover_text(tip) };
    response.clicked()
}
