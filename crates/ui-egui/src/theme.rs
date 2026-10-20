//! Design tokens: the colours every part of the window is painted with, light and dark. The
//! layout follows modern desktop office suites (a title row, a ribbon of tabs, a grid with
//! headers, tabs and a status bar at the bottom); the palette is MK Sigma's own, accented with
//! Morrison Kühlsen's blue.

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
    /// Icon colours beside the accent: data, analysis, charts, warnings.
    pub blue: Color32,
    pub green: Color32,
    pub orange: Color32,
    pub red: Color32,
    pub danger: Color32,
    pub warn: Color32,
}

impl Tokens {
    pub fn light() -> Tokens {
        Tokens {
            dark: false,
            window: Color32::from_rgb(0xF2, 0xF2, 0xF5),
            ribbon: Color32::WHITE,
            border: Color32::from_rgb(0xE0, 0xE0, 0xE6),
            text: Color32::from_rgb(0x22, 0x23, 0x2A),
            text_dim: Color32::from_rgb(0x62, 0x64, 0x70),
            // Morrison Kühlsen's blue.
            accent: Color32::from_rgb(0x25, 0x40, 0xD8),
            accent_soft: Color32::from_rgb(0xE3, 0xE8, 0xFC),
            hover: Color32::from_rgb(0xEA, 0xEA, 0xEF),
            pressed: Color32::from_rgb(0xDC, 0xDC, 0xE4),
            grid_bg: Color32::WHITE,
            gridline: Color32::from_rgb(0xE3, 0xE3, 0xE8),
            header_bg: Color32::from_rgb(0xF7, 0xF7, 0xFA),
            header_text: Color32::from_rgb(0x44, 0x46, 0x52),
            header_line: Color32::from_rgb(0xD4, 0xD4, 0xDC),
            cell_text: Color32::from_rgb(0x14, 0x14, 0x18),
            input_bg: Color32::WHITE,
            paper: Color32::WHITE,
            blue: Color32::from_rgb(0x1F, 0x6F, 0xC5),
            green: Color32::from_rgb(0x1E, 0x8A, 0x5A),
            orange: Color32::from_rgb(0xE0, 0x7A, 0x1F),
            red: Color32::from_rgb(0xC9, 0x3C, 0x3C),
            danger: Color32::from_rgb(0xC4, 0x2B, 0x1C),
            warn: Color32::from_rgb(0xA3, 0x62, 0x00),
        }
    }

    pub fn dark() -> Tokens {
        Tokens {
            dark: true,
            window: Color32::from_rgb(0x1F, 0x20, 0x25),
            ribbon: Color32::from_rgb(0x2A, 0x2B, 0x31),
            border: Color32::from_rgb(0x3B, 0x3C, 0x44),
            text: Color32::from_rgb(0xEC, 0xEC, 0xF1),
            text_dim: Color32::from_rgb(0xA8, 0xAA, 0xB6),
            accent: Color32::from_rgb(0x8E, 0x9C, 0xFF),
            accent_soft: Color32::from_rgb(0x2E, 0x34, 0x5E),
            hover: Color32::from_rgb(0x37, 0x38, 0x40),
            pressed: Color32::from_rgb(0x43, 0x44, 0x4E),
            grid_bg: Color32::from_rgb(0x24, 0x25, 0x2B),
            gridline: Color32::from_rgb(0x36, 0x37, 0x3F),
            header_bg: Color32::from_rgb(0x2C, 0x2D, 0x34),
            header_text: Color32::from_rgb(0xC6, 0xC7, 0xD0),
            header_line: Color32::from_rgb(0x44, 0x45, 0x4F),
            cell_text: Color32::from_rgb(0xEC, 0xEC, 0xF1),
            input_bg: Color32::from_rgb(0x1A, 0x1B, 0x20),
            paper: Color32::from_rgb(0x26, 0x27, 0x2D),
            blue: Color32::from_rgb(0x6C, 0xAE, 0xF5),
            green: Color32::from_rgb(0x5C, 0xC5, 0x8E),
            orange: Color32::from_rgb(0xF2, 0xA0, 0x55),
            red: Color32::from_rgb(0xF0, 0x7A, 0x7A),
            danger: Color32::from_rgb(0xF1, 0x70, 0x5F),
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
        v.popup_shadow = egui::epaint::Shadow { offset: [0, 4], blur: 16, spread: 0, color: Color32::from_black_alpha(if t.dark { 90 } else { 40 }) };
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
        s.spacing.button_padding = egui::vec2(6.0, 3.0);
        s.spacing.interact_size.y = 22.0;
        s.text_styles.insert(egui::TextStyle::Body, egui::FontId::proportional(13.0));
        s.text_styles.insert(egui::TextStyle::Button, egui::FontId::proportional(13.0));
        s.text_styles.insert(egui::TextStyle::Small, egui::FontId::proportional(11.0));
        s.text_styles.insert(egui::TextStyle::Heading, egui::FontId::proportional(18.0));
        s.text_styles.insert(egui::TextStyle::Monospace, egui::FontId::monospace(12.5));
    });
}

/// A white (or dark) panel with a hairline border: the ribbon, the syntax editor, the Output.
pub fn card(t: &Tokens) -> egui::Frame {
    egui::Frame::new().fill(t.ribbon).stroke(Stroke::new(1.0, t.border)).inner_margin(egui::Margin::same(8))
}
