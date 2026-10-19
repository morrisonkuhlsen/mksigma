//! The splash screen: a small window of its own, without decorations, before the main one, as
//! desktop suites have. The house's spiral staircase as artwork under Morrison Kühlsen's ink,
//! MK Sigma's name, motto and version, a bar that fills while it shows with the program's
//! components named in turn, and the house's logo. It closes by itself after a moment; a click
//! or Esc closes it sooner.

use crate::backstage::{BRAND, INK, ON_INK, ON_INK_SOFT};

/// The window's size, in points.
pub const SIZE: [f32; 2] = [680.0, 400.0];
/// Seconds it stays.
const HOLD: f64 = 2.4;

/// The program's components, named in turn as the bar fills.
const COMPONENTS: [&str; 12] = [
    "the syntax parser",
    "the SPSS data reader and writer",
    "the delimited text reader",
    "descriptive statistics and frequencies",
    "crosstabs and chi-square tests",
    "t tests, ANOVA and post hoc tests",
    "correlations and regression",
    "nonparametric tests",
    "charts",
    "the Output Viewer and its exports",
    "slicers, cross-filtering and dashboards",
    "the calculations by hand (Σ)",
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
    painter.rect_filled(rect, 0.0, INK);
    // The artwork over the whole window, then the ink over it: dense on the left where the
    // words are, gone by the middle; and a little at the foot, under the house's logo.
    if let Some(texture) = crate::logo::spiral(ctx) {
        painter.image(texture.id(), rect, uv, egui::Color32::WHITE);
    }
    let ink = |alpha: u8| egui::Color32::from_rgba_unmultiplied(INK.r(), INK.g(), INK.b(), alpha);
    let split = rect.left() + rect.width() * 0.62;
    gradient(painter, egui::Rect::from_min_max(rect.min, egui::pos2(split, rect.bottom())), [ink(242), ink(0)], true);
    gradient(painter, egui::Rect::from_min_max(egui::pos2(rect.left(), rect.bottom() - 120.0), rect.max), [ink(0), ink(170)], false);
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
    let shown = ((progress * COMPONENTS.len() as f32) as usize).min(COMPONENTS.len() - 1);
    let status = COMPONENTS.get(shown).map_or_else(String::new, |c| format!("Loading {c}…"));
    painter.text(bar.left_top() - egui::vec2(0.0, 10.0), egui::Align2::LEFT_BOTTOM, status, egui::FontId::proportional(12.0), ON_INK_SOFT);
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

/// `rect` filled from `colors[0]` to `colors[1]`, left to right when `across`, else top to bottom.
fn gradient(painter: &egui::Painter, rect: egui::Rect, colors: [egui::Color32; 2], across: bool) {
    let [a, b] = colors;
    let (tl, tr, bl, br) = if across { (a, b, a, b) } else { (a, a, b, b) };
    let mut mesh = egui::Mesh::default();
    let uv = egui::epaint::WHITE_UV;
    mesh.vertices.extend([
        egui::epaint::Vertex { pos: rect.left_top(), uv, color: tl },
        egui::epaint::Vertex { pos: rect.right_top(), uv, color: tr },
        egui::epaint::Vertex { pos: rect.left_bottom(), uv, color: bl },
        egui::epaint::Vertex { pos: rect.right_bottom(), uv, color: br },
    ]);
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(1, 3, 2);
    painter.add(mesh);
}
