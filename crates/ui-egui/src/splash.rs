//! The splash screen at start: Morrison Kühlsen's ink over the whole window, the σ, MK Sigma's
//! name and motto, the house's logo at the foot. It fades out by itself after a moment; a click
//! or a key closes it sooner.

use crate::backstage::{INK, ON_INK, ON_INK_SOFT};

/// Seconds it stays, then seconds it takes to fade.
const HOLD: f64 = 1.6;
const FADE: f64 = 0.4;

#[derive(Debug, Default)]
pub struct Splash {
    /// When it first showed, in egui's time; and whether it was dismissed then.
    started: Option<f64>,
    dismissed: Option<f64>,
}

impl Splash {
    /// Draws it over everything; `false` once it has faded out.
    pub(crate) fn show(&mut self, ctx: &egui::Context) -> bool {
        let now = ctx.input(|i| i.time);
        let started = *self.started.get_or_insert(now);
        let fading_from = self.dismissed.unwrap_or(started + HOLD);
        let alpha = (1.0 - (now - fading_from) / FADE).clamp(0.0, 1.0) as f32;
        if alpha <= 0.0 {
            return false;
        }
        let screen = ctx.content_rect();
        let fade = |c: egui::Color32| c.gamma_multiply(alpha);
        let response = egui::Area::new(egui::Id::new("splash"))
            .order(egui::Order::Foreground)
            .fixed_pos(screen.min)
            .show(ctx, |ui| {
                let (rect, response) = ui.allocate_exact_size(screen.size(), egui::Sense::click());
                let painter = ui.painter();
                painter.rect_filled(rect, 0.0, fade(INK));
                let center = rect.center() - egui::vec2(0.0, 40.0);
                if let Some(texture) = crate::logo::texture(ctx) {
                    let mark = egui::Rect::from_center_size(center - egui::vec2(0.0, 62.0), egui::vec2(88.0, 88.0));
                    painter.image(
                        texture.id(),
                        mark,
                        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                        fade(egui::Color32::WHITE),
                    );
                }
                painter.text(center + egui::vec2(0.0, 14.0), egui::Align2::CENTER_CENTER, "MK Sigma", egui::FontId::proportional(40.0), fade(ON_INK));
                painter.text(
                    center + egui::vec2(0.0, 50.0),
                    egui::Align2::CENTER_CENTER,
                    "Ex nihilo, ad æternum",
                    egui::FontId::new(15.0, egui::FontFamily::Proportional),
                    fade(ON_INK_SOFT),
                );
                painter.text(
                    center + egui::vec2(0.0, 74.0),
                    egui::Align2::CENTER_CENTER,
                    format!("Version {}", env!("CARGO_PKG_VERSION")),
                    egui::FontId::proportional(12.0),
                    fade(ON_INK_SOFT),
                );
                // The house's logo and line at the foot.
                let foot = rect.bottom() - 36.0;
                if let Some(texture) = crate::logo::morrison_kuhlsen(ctx) {
                    let width = 150.0;
                    let size = egui::vec2(width, width * texture.size_vec2().y / texture.size_vec2().x.max(1.0));
                    let logo = egui::Rect::from_center_size(egui::pos2(rect.center().x, foot - 22.0 - size.y / 2.0), size);
                    painter.image(texture.id(), logo, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), fade(ON_INK));
                }
                painter.text(
                    egui::pos2(rect.center().x, foot),
                    egui::Align2::CENTER_CENTER,
                    "A Morrison Kühlsen project",
                    egui::FontId::proportional(11.5),
                    fade(ON_INK_SOFT),
                );
                response
            })
            .inner;
        let key = ctx.input(|i| i.events.iter().any(|e| matches!(e, egui::Event::Key { pressed: true, .. })));
        if self.dismissed.is_none() && (response.clicked() || key) {
            // Fade from where it is, not from the start.
            self.dismissed = Some(now.min(fading_from));
        }
        ctx.request_repaint();
        true
    }
}
