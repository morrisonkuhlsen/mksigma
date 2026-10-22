//! The splash screen: a small window of its own, without decorations, before the main one, as
//! desktop suites have. In two halves: on Morrison Kühlsen's ink, MK Sigma's name, motto and
//! version, a bar that fills while it shows with the program's components named in turn, and
//! the house's logo; beside them, the house's spiral staircase as artwork. It closes by itself after a moment; a click
//! or Esc closes it sooner.

use crate::backstage::{BRAND, INK, ON_INK, ON_INK_SOFT};
use mksigma_i18n::tr;

/// The window's size, in points, the same as MK aeGIS's: each half keeps the artwork's 680×800
/// proportions.
pub const SIZE: [f32; 2] = [816.0, 480.0];
/// The height the layout's measures are given for; they scale with the window's.
const LAYOUT_HEIGHT: f32 = 400.0;
/// Seconds it stays.
const HOLD: f64 = 5.0;

/// The program's components, named in turn as the bar fills (their messages).
const COMPONENTS: [&str; 12] = [
    "splash-c1",
    "splash-c2",
    "splash-c3",
    "splash-c4",
    "splash-c5",
    "splash-c6",
    "splash-c7",
    "splash-c8",
    "splash-c9",
    "splash-c10",
    "splash-c11",
    "splash-c12",
];

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
    // The layout is drawn for 400 points of height and scales with the window.
    let k = rect.height() / LAYOUT_HEIGHT;
    // Two halves: the words on Morrison Kühlsen's ink, the artwork beside them.
    let half = rect.left() + rect.width() / 2.0;
    let words = egui::Rect::from_min_max(rect.min, egui::pos2(half, rect.bottom()));
    let art = egui::Rect::from_min_max(egui::pos2(half, rect.top()), rect.max);
    painter.rect_filled(rect, 0.0, INK);
    if let Some(texture) = crate::logo::spiral(ctx) {
        painter.image(texture.id(), art, uv, egui::Color32::WHITE);
    }
    // The brand's stripe along the top of the words.
    painter.rect_filled(egui::Rect::from_min_size(words.min, egui::vec2(words.width(), 5.0 * k)), 0.0, BRAND);

    let left = words.left() + 36.0 * k;
    let width = words.width() - 72.0 * k;
    // The mark and the name.
    if let Some(texture) = crate::logo::texture(ctx) {
        painter.image(
            texture.id(),
            egui::Rect::from_min_size(egui::pos2(left, words.top() + 52.0 * k), egui::vec2(46.0, 46.0) * k),
            uv,
            egui::Color32::WHITE,
        );
    }
    painter.text(
        egui::pos2(left + 58.0 * k, words.top() + 75.0 * k),
        egui::Align2::LEFT_CENTER,
        "MK Sigma",
        egui::FontId::proportional(34.0 * k),
        ON_INK,
    );
    painter.text(
        egui::pos2(left, words.top() + 128.0 * k),
        egui::Align2::LEFT_CENTER,
        "Ex nihilo, ad æternum",
        egui::FontId::proportional(15.0 * k),
        ON_INK_SOFT,
    );
    let tagline = painter.layout(tr!("splash-tagline"), egui::FontId::proportional(12.5 * k), ON_INK_SOFT, width);
    let below = words.top() + 152.0 * k + tagline.size().y;
    painter.galley(egui::pos2(left, words.top() + 152.0 * k), tagline, ON_INK_SOFT);
    painter.text(
        egui::pos2(left, below + 10.0 * k),
        egui::Align2::LEFT_TOP,
        tr!("splash-version", version = env!("CARGO_PKG_VERSION")),
        egui::FontId::proportional(12.5 * k),
        ON_INK_SOFT,
    );

    // The component loading, over the progress bar.
    let bar = egui::Rect::from_min_size(egui::pos2(left, words.bottom() - 96.0 * k), egui::vec2(width, 3.0 * k));
    let shown = ((progress * COMPONENTS.len() as f32) as usize).min(COMPONENTS.len() - 1);
    let status = COMPONENTS.get(shown).map_or_else(String::new, |c| tr!("splash-loading", component = mksigma_i18n::message(c, &[])));
    // A long component name shrinks to the bar's width rather than run into the artwork.
    let size = 12.0 * k;
    let mut galley = painter.layout_no_wrap(status.clone(), egui::FontId::proportional(size), ON_INK_SOFT);
    if galley.size().x > width {
        galley = painter.layout_no_wrap(status, egui::FontId::proportional(size * width / galley.size().x), ON_INK_SOFT);
    }
    painter.galley(bar.left_top() - egui::vec2(0.0, 10.0 * k + galley.size().y), galley, ON_INK_SOFT);
    painter.rect_filled(bar, 0.0, egui::Color32::from_white_alpha(28));
    painter.rect_filled(egui::Rect::from_min_size(bar.min, egui::vec2(bar.width() * progress, bar.height())), 0.0, BRAND);

    // The house: its logo and line at the foot.
    let foot = words.bottom() - 28.0 * k;
    if let Some(texture) = crate::logo::morrison_kuhlsen(ctx) {
        let logo_width = 104.0 * k;
        let size = egui::vec2(logo_width, logo_width * texture.size_vec2().y / texture.size_vec2().x.max(1.0));
        painter.image(texture.id(), egui::Rect::from_min_size(egui::pos2(left, foot - size.y), size), uv, ON_INK);
    }
    painter.text(
        egui::pos2(left + width, foot - 4.0 * k),
        egui::Align2::RIGHT_BOTTOM,
        tr!("mk-project"),
        egui::FontId::proportional(11.0 * k),
        ON_INK_SOFT,
    );
}
