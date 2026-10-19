//! The splash screen: a small window of its own, without decorations, before the main one, as
//! desktop suites have. Morrison Kühlsen's ink with a brand stripe, the σ large and faint as
//! artwork, MK Sigma's name, motto and version, a bar that fills while it shows, and the
//! house's logo. It closes by itself after a moment; a click or Esc closes it sooner.

use crate::backstage::{BRAND, INK, ON_INK, ON_INK_SOFT};

/// The window's size, in points.
pub const SIZE: [f32; 2] = [680.0, 400.0];
/// Seconds it stays.
const HOLD: f64 = 2.0;

#[derive(Debug, Default)]
pub struct SplashScreen {
    /// When it first showed, in egui's time.
    started: Option<f64>,
    closed: bool,
}

impl SplashScreen {
    /// Whether it asked its window to close.
    pub fn closed(&self) -> bool {
        self.closed
    }

    /// Draws it over the whole window; asks the window to close when its time is up.
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let now = ctx.input(|i| i.time);
        let started = *self.started.get_or_insert(now);
        let progress = ((now - started) / HOLD).clamp(0.0, 1.0) as f32;
        // The whole window: the root Ui comes inset by a margin.
        let rect = ctx.content_rect();
        let response = ui.allocate_rect(ui.max_rect(), egui::Sense::click());
        paint(&ui.painter().with_clip_rect(rect), &ctx, rect, progress);
        let done = progress >= 1.0 || response.clicked() || ctx.input(|i| i.key_pressed(egui::Key::Escape));
        if done && !self.closed {
            self.closed = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        ctx.request_repaint();
    }
}

fn paint(painter: &egui::Painter, ctx: &egui::Context, rect: egui::Rect, progress: f32) {
    let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
    painter.rect_filled(rect, 0.0, INK);
    // The artwork: the σ, large and faint, running off the right edge.
    if let Some(texture) = crate::logo::texture(ctx) {
        let side = rect.height() * 1.25;
        let art = egui::Rect::from_center_size(egui::pos2(rect.right() - side * 0.22, rect.center().y + side * 0.06), egui::vec2(side, side));
        painter.with_clip_rect(rect).image(texture.id(), art, uv, egui::Color32::from_white_alpha(20));
    }
    // The brand's stripe along the top.
    painter.rect_filled(egui::Rect::from_min_size(rect.min, egui::vec2(rect.width(), 5.0)), 0.0, BRAND);

    let left = rect.left() + 44.0;
    // The mark and the name.
    if let Some(texture) = crate::logo::texture(ctx) {
        painter.image(texture.id(), egui::Rect::from_min_size(egui::pos2(left, rect.top() + 58.0), egui::vec2(56.0, 56.0)), uv, egui::Color32::WHITE);
    }
    painter.text(egui::pos2(left + 72.0, rect.top() + 86.0), egui::Align2::LEFT_CENTER, "MK Sigma", egui::FontId::proportional(42.0), ON_INK);
    painter.text(
        egui::pos2(left, rect.top() + 146.0),
        egui::Align2::LEFT_CENTER,
        "Ex nihilo, ad æternum",
        egui::FontId::proportional(16.0),
        ON_INK_SOFT,
    );
    painter.text(
        egui::pos2(left, rect.top() + 178.0),
        egui::Align2::LEFT_CENTER,
        "Statistics, open and auditable: every figure with its calculation.",
        egui::FontId::proportional(12.5),
        ON_INK_SOFT,
    );
    painter.text(
        egui::pos2(left, rect.top() + 200.0),
        egui::Align2::LEFT_CENTER,
        format!("Version {}", env!("CARGO_PKG_VERSION")),
        egui::FontId::proportional(12.5),
        ON_INK_SOFT,
    );

    // The status line over the progress bar, at the foot.
    let bar = egui::Rect::from_min_size(egui::pos2(left, rect.bottom() - 40.0), egui::vec2(260.0, 3.0));
    painter.text(bar.left_top() - egui::vec2(0.0, 10.0), egui::Align2::LEFT_BOTTOM, "Starting…", egui::FontId::proportional(12.0), ON_INK_SOFT);
    painter.rect_filled(bar, 1.5, egui::Color32::from_white_alpha(28));
    painter.rect_filled(egui::Rect::from_min_size(bar.min, egui::vec2(bar.width() * progress, bar.height())), 1.5, BRAND);

    // The house: its logo and line, bottom right.
    let right = rect.right() - 40.0;
    if let Some(texture) = crate::logo::morrison_kuhlsen(ctx) {
        let width = 132.0;
        let size = egui::vec2(width, width * texture.size_vec2().y / texture.size_vec2().x.max(1.0));
        let logo = egui::Rect::from_min_size(egui::pos2(right - width, rect.bottom() - 60.0 - size.y), size);
        painter.image(texture.id(), logo, uv, ON_INK);
    }
    painter.text(
        egui::pos2(right, rect.bottom() - 37.0),
        egui::Align2::RIGHT_BOTTOM,
        "A Morrison Kühlsen project",
        egui::FontId::proportional(11.5),
        ON_INK_SOFT,
    );
}
