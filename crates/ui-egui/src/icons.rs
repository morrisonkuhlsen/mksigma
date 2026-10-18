//! MK Sigma's icons, drawn in code on a 16×16 design grid (original artwork, nothing bundled).
//! Small icons go in the title row and buttons; the ribbon draws the same shapes at 32×32.

use egui::epaint::PathShape;
use egui::{Color32, Painter, Pos2, Rect, Stroke, vec2};

use crate::theme::Tokens;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    Sigma,
    Open,
    Save,
    Undo,
    Redo,
    Run,
    Web,
    Pdf,
    Eraser,
    Table,
    Frequencies,
    Crosstabs,
    TTest,
    Anova,
    Correlations,
    Regression,
    Logistic,
    Ranks,
    Boxplot,
    Histogram,
    Bars,
    Scatter,
    Compute,
    Recode,
    Filter,
    InsertCase,
    InsertVariable,
    Labels,
    Grid,
    List,
    Dictionary,
    Search,
    Contrast,
    Share,
    Minus,
    Plus,
}

/// Draws `icon` in `rect` (square) in one colour, the theme's accent; tints of it fill shapes.
pub fn paint(painter: &Painter, rect: Rect, icon: Icon, t: &Tokens) {
    let ink = t.accent;
    let t = &Tokens { text: ink, text_dim: ink, blue: ink, green: ink, orange: ink, red: ink, header_line: t.accent_soft, ..*t };
    let s = rect.width().min(rect.height()) / 16.0;
    let o = rect.center() - vec2(8.0 * s, 8.0 * s);
    let p = |x: f32, y: f32| -> Pos2 { o + vec2(x * s, y * s) };
    let r = |x0: f32, y0: f32, x1: f32, y1: f32| Rect::from_min_max(p(x0, y0), p(x1, y1));
    let w = (1.2 * s).max(1.0);
    let ink = Stroke::new(w, t.text);
    let line = |a: (f32, f32), b: (f32, f32), stroke: Stroke| {
        painter.line_segment([p(a.0, a.1), p(b.0, b.1)], stroke);
    };
    let poly = |points: &[(f32, f32)], stroke: Stroke| {
        painter.add(PathShape::line(points.iter().map(|(x, y)| p(*x, *y)).collect(), stroke));
    };
    let fill = |points: &[(f32, f32)], color: Color32| {
        painter.add(PathShape::convex_polygon(points.iter().map(|(x, y)| p(*x, *y)).collect(), color, Stroke::NONE));
    };
    let frame = |rect: Rect, color: Color32| {
        painter.rect_stroke(rect, 1.0 * s, Stroke::new(w, color), egui::StrokeKind::Middle);
    };
    let dot = |x: f32, y: f32, color: Color32| {
        painter.circle_filled(p(x, y), 1.1 * s, color);
    };
    let page = |color: Color32| {
        poly(&[(3.5, 1.5), (9.5, 1.5), (12.5, 4.5), (12.5, 14.5), (3.5, 14.5), (3.5, 1.5)], Stroke::new(w, color));
        poly(&[(9.5, 1.5), (9.5, 4.5), (12.5, 4.5)], Stroke::new(w, color));
    };
    let axes = || {
        poly(&[(2.0, 2.0), (2.0, 14.0), (14.5, 14.0)], ink);
    };
    match icon {
        Icon::Sigma => {
            painter.rect_filled(r(1.0, 1.0, 15.0, 15.0), 3.0 * s, t.accent);
            poly(&[(11.0, 4.5), (5.0, 4.5), (8.5, 8.0), (5.0, 11.5), (11.0, 11.5)], Stroke::new(w * 1.4, Color32::WHITE));
        }
        Icon::Open => {
            fill(&[(1.5, 4.0), (6.0, 4.0), (7.5, 5.5), (13.0, 5.5), (13.0, 13.0), (1.5, 13.0)], t.orange.gamma_multiply(0.35));
            poly(&[(1.5, 13.0), (1.5, 3.5), (6.0, 3.5), (7.5, 5.0), (13.0, 5.0), (13.0, 7.5)], Stroke::new(w, t.orange));
            fill(&[(3.5, 7.5), (15.0, 7.5), (13.0, 13.0), (1.5, 13.0)], t.orange);
        }
        Icon::Save => {
            frame(r(2.0, 2.0, 14.0, 14.0), t.accent);
            painter.rect_filled(r(4.5, 2.0, 11.5, 6.0), 0.0, t.accent);
            painter.rect_stroke(r(4.5, 9.0, 11.5, 14.0), 0.0, Stroke::new(w, t.accent), egui::StrokeKind::Middle);
        }
        Icon::Undo | Icon::Redo => {
            let flip = |x: f32| if icon == Icon::Redo { 16.0 - x } else { x };
            let arc: Vec<(f32, f32)> = (0..=12)
                .map(|i| {
                    let a = std::f32::consts::PI * (1.0 - i as f32 / 12.0) * 0.95;
                    (flip(8.5 - 4.5 * a.cos()), 10.0 - 4.5 * a.sin())
                })
                .collect();
            poly(&arc, Stroke::new(w * 1.2, t.text));
            fill(&[(flip(1.5), 9.0), (flip(6.5), 9.0), (flip(4.0), 13.0)], t.text);
        }
        Icon::Run => fill(&[(4.0, 2.5), (13.5, 8.0), (4.0, 13.5)], t.green),
        Icon::Web => {
            painter.circle_stroke(p(8.0, 8.0), 6.0 * s, Stroke::new(w, t.blue));
            line((2.0, 8.0), (14.0, 8.0), Stroke::new(w, t.blue));
            painter.add(egui::Shape::ellipse_stroke(p(8.0, 8.0), vec2(2.6 * s, 6.0 * s), Stroke::new(w, t.blue)));
        }
        Icon::Pdf => {
            page(t.red);
            painter.rect_filled(r(1.5, 8.0, 11.0, 12.5), 1.0 * s, t.red);
            for (a, b) in [((3.5, 9.5), (8.5, 9.5)), ((3.5, 11.0), (7.0, 11.0))] {
                line(a, b, Stroke::new(w * 0.8, Color32::WHITE));
            }
        }
        Icon::Eraser => {
            fill(&[(2.0, 10.0), (8.0, 4.0), (11.0, 7.0), (5.0, 13.0)], t.red.gamma_multiply(0.75));
            poly(&[(8.0, 4.0), (10.0, 2.0), (14.0, 6.0), (11.0, 9.0)], Stroke::new(w, t.text_dim));
            line((5.0, 13.0), (14.0, 13.0), ink);
        }
        Icon::Table | Icon::Grid => {
            frame(r(1.5, 2.5, 14.5, 13.5), t.text_dim);
            painter.rect_filled(r(1.5, 2.5, 14.5, 5.5), 0.0, if icon == Icon::Table { t.accent } else { t.header_line });
            line((1.5, 9.5), (14.5, 9.5), Stroke::new(w * 0.8, t.text_dim));
            for x in [6.0, 10.5] {
                line((x, 2.5), (x, 13.5), Stroke::new(w * 0.8, t.text_dim));
            }
        }
        Icon::Frequencies | Icon::Histogram | Icon::Bars => {
            axes();
            let heights: &[f32] = match icon {
                Icon::Frequencies => &[5.0, 9.0, 6.0],
                Icon::Histogram => &[3.0, 7.0, 10.0, 6.0, 2.5],
                _ => &[6.0, 10.0, 4.0],
            };
            let n = heights.len() as f32;
            let slot = 11.5 / n;
            let gap = if icon == Icon::Histogram { 0.0 } else { slot * 0.3 };
            for (i, h) in heights.iter().enumerate() {
                let x = 2.8 + slot * i as f32 + gap / 2.0;
                painter.rect_filled(r(x, 13.5 - h, x + slot - gap, 13.5), 0.0, if icon == Icon::Frequencies { t.accent } else { t.blue });
            }
        }
        Icon::Crosstabs => {
            frame(r(1.5, 1.5, 14.5, 14.5), t.text_dim);
            painter.rect_filled(r(1.5, 1.5, 5.5, 14.5), 0.0, t.accent_soft);
            painter.rect_filled(r(1.5, 1.5, 14.5, 5.5), 0.0, t.accent_soft);
            for v in [5.5, 10.0] {
                line((v, 1.5), (v, 14.5), Stroke::new(w * 0.8, t.text_dim));
                line((1.5, v), (14.5, v), Stroke::new(w * 0.8, t.text_dim));
            }
            dot(7.75, 7.75, t.accent);
            dot(12.25, 12.25, t.accent);
        }
        Icon::TTest | Icon::Anova => {
            let curve = |center: f32, color: Color32, height: f32| {
                let pts: Vec<(f32, f32)> = (0..=16)
                    .map(|i| {
                        let x = center - 4.5 + 9.0 * i as f32 / 16.0;
                        let z = (x - center) / 1.6;
                        (x, 13.5 - height * (-0.5 * z * z).exp())
                    })
                    .collect();
                poly(&pts, Stroke::new(w * 1.1, color));
            };
            line((1.0, 13.5), (15.0, 13.5), ink);
            if icon == Icon::TTest {
                curve(5.5, t.accent, 9.0);
                curve(10.5, t.orange, 9.0);
            } else {
                curve(4.5, t.accent, 8.0);
                curve(8.0, t.green, 10.0);
                curve(11.5, t.orange, 7.0);
            }
        }
        Icon::Correlations | Icon::Scatter | Icon::Regression => {
            axes();
            for (x, y) in [(4.5, 11.5), (6.0, 9.0), (7.5, 10.0), (9.0, 7.0), (10.5, 7.5), (12.5, 4.5)] {
                dot(x, y, if icon == Icon::Correlations { t.accent } else { t.blue });
            }
            if icon == Icon::Regression {
                line((3.5, 12.5), (14.0, 3.5), Stroke::new(w * 1.2, t.orange));
            }
        }
        Icon::Logistic => {
            axes();
            let pts: Vec<(f32, f32)> = (0..=20)
                .map(|i| {
                    let x = 3.0 + 11.0 * i as f32 / 20.0;
                    (x, 12.5 - 9.5 / (1.0 + (-(x - 8.5) * 1.1).exp()))
                })
                .collect();
            poly(&pts, Stroke::new(w * 1.3, t.accent));
        }
        Icon::Ranks => {
            for (i, h) in [3.0f32, 6.0, 9.0, 12.0].iter().enumerate() {
                let x = 2.0 + 3.3 * i as f32;
                painter.rect_filled(r(x, 14.0 - h, x + 2.4, 14.0), 0.5 * s, t.accent.gamma_multiply(0.45 + 0.15 * i as f32));
            }
        }
        Icon::Boxplot => {
            for (x, low, q1, q3, high, color) in [(5.0, 13.0, 9.5, 5.5, 2.5, t.accent), (11.0, 14.0, 11.0, 7.5, 4.5, t.orange)] {
                line((x, low), (x, high), ink);
                painter.rect_filled(r(x - 2.2, q3, x + 2.2, q1), 0.0, color);
                line((x - 2.2, (q1 + q3) / 2.0), (x + 2.2, (q1 + q3) / 2.0), Stroke::new(w, Color32::WHITE));
            }
        }
        Icon::Compute => {
            frame(r(2.5, 1.5, 13.5, 14.5), t.text_dim);
            painter.rect_filled(r(4.0, 3.0, 12.0, 6.0), 0.5 * s, t.green);
            for y in [8.5, 11.5] {
                for x in [5.0, 8.0, 11.0] {
                    dot(x, y, t.text_dim);
                }
            }
        }
        Icon::Recode => {
            line((2.0, 5.0), (12.5, 5.0), Stroke::new(w * 1.2, t.accent));
            fill(&[(11.0, 2.5), (14.5, 5.0), (11.0, 7.5)], t.accent);
            line((3.5, 11.0), (14.0, 11.0), Stroke::new(w * 1.2, t.orange));
            fill(&[(5.0, 8.5), (1.5, 11.0), (5.0, 13.5)], t.orange);
        }
        Icon::Filter => {
            fill(&[(1.5, 2.5), (14.5, 2.5), (9.5, 8.5), (6.5, 8.5)], t.accent);
            fill(&[(6.5, 8.5), (9.5, 8.5), (9.5, 12.5), (6.5, 14.5)], t.accent);
        }
        Icon::InsertCase | Icon::InsertVariable => {
            frame(r(1.5, 1.5, 11.5, 11.5), t.text_dim);
            if icon == Icon::InsertCase {
                painter.rect_filled(r(1.5, 5.0, 11.5, 8.0), 0.0, t.accent_soft);
                line((1.5, 5.0), (11.5, 5.0), Stroke::new(w * 0.8, t.text_dim));
                line((1.5, 8.0), (11.5, 8.0), Stroke::new(w * 0.8, t.text_dim));
            } else {
                painter.rect_filled(r(5.0, 1.5, 8.0, 11.5), 0.0, t.accent_soft);
                line((5.0, 1.5), (5.0, 11.5), Stroke::new(w * 0.8, t.text_dim));
                line((8.0, 1.5), (8.0, 11.5), Stroke::new(w * 0.8, t.text_dim));
            }
            painter.circle_filled(p(12.0, 12.0), 3.6 * s, t.green);
            line((10.0, 12.0), (14.0, 12.0), Stroke::new(w * 1.1, Color32::WHITE));
            line((12.0, 10.0), (12.0, 14.0), Stroke::new(w * 1.1, Color32::WHITE));
        }
        Icon::Labels => {
            fill(&[(1.5, 6.5), (6.5, 1.5), (14.5, 1.5), (14.5, 11.5), (9.5, 14.5), (1.5, 14.5)], t.accent_soft);
            poly(&[(2.0, 8.5), (8.5, 2.0), (14.0, 2.0), (14.0, 7.5), (7.5, 14.0), (2.0, 8.5)], Stroke::new(w, t.accent));
            dot(11.0, 5.0, t.accent);
        }
        Icon::List => {
            for (i, y) in [3.5, 8.0, 12.5].iter().enumerate() {
                painter.rect_filled(r(1.5, y - 1.2, 4.0, y + 1.2), 0.5 * s, if i == 0 { t.accent } else { t.text_dim });
                line((6.0, *y), (14.5, *y), Stroke::new(w, t.text_dim));
            }
        }
        Icon::Search => {
            painter.circle_stroke(p(6.5, 6.5), 4.5 * s, Stroke::new(w * 1.2, ink.color));
            line((9.8, 9.8), (14.0, 14.0), Stroke::new(w * 1.6, ink.color));
        }
        Icon::Contrast => {
            painter.circle_stroke(p(8.0, 8.0), 6.0 * s, Stroke::new(w * 1.1, ink.color));
            let half: Vec<(f32, f32)> = (0..=16)
                .map(|i| {
                    let a = -std::f32::consts::FRAC_PI_2 + std::f32::consts::PI * i as f32 / 16.0;
                    (8.0 + 6.0 * a.cos(), 8.0 + 6.0 * a.sin())
                })
                .collect();
            fill(&half, ink.color);
        }
        Icon::Share => {
            poly(&[(5.0, 6.0), (2.5, 6.0), (2.5, 14.0), (13.5, 14.0), (13.5, 6.0), (11.0, 6.0)], ink);
            line((8.0, 1.5), (8.0, 10.0), ink);
            poly(&[(5.0, 4.5), (8.0, 1.5), (11.0, 4.5)], ink);
        }
        Icon::Minus => line((3.0, 8.0), (13.0, 8.0), Stroke::new(w * 1.2, ink.color)),
        Icon::Plus => {
            line((3.0, 8.0), (13.0, 8.0), Stroke::new(w * 1.2, ink.color));
            line((8.0, 3.0), (8.0, 13.0), Stroke::new(w * 1.2, ink.color));
        }
        Icon::Dictionary => {
            fill(&[(3.0, 2.0), (12.5, 2.0), (12.5, 13.0), (3.0, 13.0)], t.accent);
            poly(&[(3.0, 13.0), (3.0, 14.5), (13.0, 14.5)], Stroke::new(w, t.text_dim));
            line((5.5, 5.0), (10.5, 5.0), Stroke::new(w, Color32::WHITE));
            line((5.5, 7.5), (9.0, 7.5), Stroke::new(w, Color32::WHITE));
        }
    }
}

/// An icon-sized square allocated in the layout and drawn, for labels and menus.
pub fn show(ui: &mut egui::Ui, icon: Icon, size: f32) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(vec2(size, size), egui::Sense::hover());
    paint(ui.painter(), rect, icon, &Tokens::get(ui.ctx()));
    response
}

/// For tests: every icon stays inside its square.
#[cfg(test)]
pub(crate) const ALL: [Icon; 36] = [
    Icon::Sigma,
    Icon::Open,
    Icon::Save,
    Icon::Undo,
    Icon::Redo,
    Icon::Run,
    Icon::Web,
    Icon::Pdf,
    Icon::Eraser,
    Icon::Table,
    Icon::Frequencies,
    Icon::Crosstabs,
    Icon::TTest,
    Icon::Anova,
    Icon::Correlations,
    Icon::Regression,
    Icon::Logistic,
    Icon::Ranks,
    Icon::Boxplot,
    Icon::Histogram,
    Icon::Bars,
    Icon::Scatter,
    Icon::Compute,
    Icon::Recode,
    Icon::Filter,
    Icon::InsertCase,
    Icon::InsertVariable,
    Icon::Labels,
    Icon::Grid,
    Icon::List,
    Icon::Dictionary,
    Icon::Search,
    Icon::Contrast,
    Icon::Share,
    Icon::Minus,
    Icon::Plus,
];

#[cfg(test)]
mod tests {
    use super::*;
    use egui::pos2;

    #[test]
    fn icons_stay_in_their_square() {
        let ctx = egui::Context::default();
        let rect = Rect::from_min_size(pos2(100.0, 100.0), vec2(32.0, 32.0));
        for icon in ALL {
            let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
                paint(&ui.ctx().layer_painter(egui::LayerId::debug()), rect, icon, &Tokens::light());
            });
            out.textures_delta.clear();
            let drawn: Vec<_> = out.shapes.into_iter().filter(|c| c.shape.visual_bounding_rect().is_positive()).collect();
            assert!(!drawn.is_empty(), "{icon:?} draws something");
            for clipped in drawn {
                let bounds = clipped.shape.visual_bounding_rect();
                assert!(rect.expand(2.0).contains_rect(bounds), "{icon:?} leaves its square: {bounds:?}");
            }
        }
    }
}
