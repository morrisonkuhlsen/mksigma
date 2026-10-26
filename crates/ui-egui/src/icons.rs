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
    /// Tiles of a dashboard: one with bars, two beside it.
    Dashboard,
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
    Pin,
    ChevronUp,
    ChevronDown,
    ChevronLeft,
    ChevronRight,
    House,
    Page,
    /// Measurement levels, beside a variable's name.
    Scale,
    Ordinal,
    Nominal,
    Info,
    /// An open book: Learn.
    Book,
    Close,
    /// The LaTeX name typeset as LaTeX sets it (raised A, lowered E): the name, not the
    /// LaTeX Project's logo artwork, whose terms don't allow it as a program's icon.
    Latex,
}

/// The colours an icon is drawn with: a hue per meaning, a neutral ink for axes and chrome, and
/// the colour of knockouts (the white of the Σ on its tile).
#[derive(Clone, Copy)]
struct Hues {
    ink: Color32,
    dim: Color32,
    accent: Color32,
    blue: Color32,
    teal: Color32,
    green: Color32,
    orange: Color32,
    red: Color32,
    violet: Color32,
    knock: Color32,
    /// How strongly a hue tints a shape's fill.
    tint: f32,
}

impl Hues {
    fn color(t: &Tokens) -> Hues {
        Hues {
            ink: t.text,
            dim: t.text_dim,
            accent: t.accent,
            blue: t.blue,
            teal: t.teal,
            green: t.green,
            orange: t.orange,
            red: t.red,
            violet: t.violet,
            knock: crate::theme::on_accent(t),
            tint: if t.dark { 0.28 } else { 0.16 },
        }
    }

    fn mono(color: Color32, behind: Color32) -> Hues {
        Hues {
            ink: color,
            dim: color,
            accent: color,
            blue: color,
            teal: color,
            green: color,
            orange: color,
            red: color,
            violet: color,
            knock: behind,
            tint: 0.25,
        }
    }
}

/// Draws `icon` in `rect` (square) in colour: objects in duotone (a soft tint of their hue under
/// a crisp line of it), the window's controls (arrows, close, search) in the text colour.
pub fn paint(painter: &Painter, rect: Rect, icon: Icon, t: &Tokens) {
    draw(painter, rect, icon, &Hues::color(t));
}

/// Draws `icon` in one colour, for icons on a filled background (`behind`), such as the accent
/// button or the File backstage's column.
pub fn paint_mono(painter: &Painter, rect: Rect, icon: Icon, color: Color32, behind: Color32) {
    draw(painter, rect, icon, &Hues::mono(color, behind));
}

fn draw(painter: &Painter, rect: Rect, icon: Icon, h: &Hues) {
    let s = rect.width().min(rect.height()) / 16.0;
    let o = rect.center() - vec2(8.0 * s, 8.0 * s);
    let p = |x: f32, y: f32| -> Pos2 { o + vec2(x * s, y * s) };
    let r = |x0: f32, y0: f32, x1: f32, y1: f32| Rect::from_min_max(p(x0, y0), p(x1, y1));
    let w = (1.25 * s).max(1.0);
    let tint = |c: Color32| c.gamma_multiply(h.tint);
    let line = |a: (f32, f32), b: (f32, f32), color: Color32, k: f32| {
        painter.line_segment([p(a.0, a.1), p(b.0, b.1)], Stroke::new(w * k, color));
    };
    let poly = |points: &[(f32, f32)], color: Color32, k: f32| {
        painter.add(PathShape::line(points.iter().map(|(x, y)| p(*x, *y)).collect(), Stroke::new(w * k, color)));
    };
    let fill = |points: &[(f32, f32)], color: Color32| {
        painter.add(PathShape::convex_polygon(points.iter().map(|(x, y)| p(*x, *y)).collect(), color, Stroke::NONE));
    };
    // A convex outline over a tint of its colour: the duotone shape most icons are made of.
    let duo = |points: &[(f32, f32)], color: Color32| {
        let pts: Vec<Pos2> = points.iter().map(|(x, y)| p(*x, *y)).collect();
        painter.add(PathShape::convex_polygon(pts.clone(), tint(color), Stroke::NONE));
        painter.add(PathShape::closed_line(pts, Stroke::new(w, color)));
    };
    let card = |rect: Rect, color: Color32, filled: bool| {
        if filled {
            painter.rect_filled(rect, 1.4 * s, tint(color));
        }
        painter.rect_stroke(rect, 1.4 * s, Stroke::new(w, color), egui::StrokeKind::Middle);
    };
    let block = |rect: Rect, color: Color32| {
        painter.rect_filled(rect, 0.6 * s, color);
    };
    let dot = |x: f32, y: f32, radius: f32, color: Color32| {
        painter.circle_filled(p(x, y), radius * s, color);
    };
    let axes = || poly(&[(2.0, 1.5), (2.0, 14.0), (14.5, 14.0)], h.dim, 0.9);
    let page = |color: Color32| duo(&[(3.5, 1.5), (9.5, 1.5), (12.5, 4.5), (12.5, 14.5), (3.5, 14.5)], color);
    match icon {
        Icon::Sigma => {
            painter.rect_filled(r(1.0, 1.0, 15.0, 15.0), 3.5 * s, h.accent);
            poly(&[(11.0, 4.5), (5.0, 4.5), (8.5, 8.0), (5.0, 11.5), (11.0, 11.5)], h.knock, 1.4);
        }
        Icon::Open => {
            duo(&[(1.5, 13.0), (1.5, 3.5), (5.8, 3.5), (7.3, 5.0), (13.0, 5.0), (13.0, 13.0)], h.orange);
            fill(&[(3.6, 7.5), (15.0, 7.5), (13.0, 13.0), (1.5, 13.0)], h.orange);
        }
        Icon::Save => {
            card(r(2.0, 2.0, 14.0, 14.0), h.accent, true);
            block(r(5.0, 2.0, 11.0, 6.0), h.accent);
            painter.rect_stroke(r(4.5, 9.0, 11.5, 14.0), 0.8 * s, Stroke::new(w, h.accent), egui::StrokeKind::Middle);
        }
        Icon::Undo | Icon::Redo => {
            let flip = |x: f32| if icon == Icon::Redo { 16.0 - x } else { x };
            let arc: Vec<(f32, f32)> = (0..=12)
                .map(|i| {
                    let a = std::f32::consts::PI * (1.0 - i as f32 / 12.0) * 0.95;
                    (flip(8.5 - 4.5 * a.cos()), 10.0 - 4.5 * a.sin())
                })
                .collect();
            poly(&arc, h.ink, 1.1);
            fill(&[(flip(1.5), 9.0), (flip(6.5), 9.0), (flip(4.0), 13.0)], h.ink);
        }
        Icon::Run => {
            fill(&[(4.0, 2.5), (13.5, 8.0), (4.0, 13.5)], h.green);
        }
        Icon::Web => {
            painter.circle_filled(p(8.0, 8.0), 6.2 * s, tint(h.teal));
            painter.circle_stroke(p(8.0, 8.0), 6.2 * s, Stroke::new(w, h.teal));
            line((1.8, 8.0), (14.2, 8.0), h.teal, 0.8);
            painter.add(egui::Shape::ellipse_stroke(p(8.0, 8.0), vec2(2.6 * s, 6.2 * s), Stroke::new(w * 0.8, h.teal)));
        }
        Icon::Pdf => {
            page(h.red);
            painter.rect_filled(r(1.5, 8.0, 11.0, 12.5), 1.0 * s, h.red);
            line((3.5, 9.6), (8.5, 9.6), h.knock, 0.8);
            line((3.5, 11.0), (7.0, 11.0), h.knock, 0.8);
        }
        Icon::Eraser => {
            fill(&[(2.0, 10.0), (8.0, 4.0), (11.0, 7.0), (5.0, 13.0)], h.red);
            duo(&[(8.0, 4.0), (10.0, 2.0), (14.0, 6.0), (11.0, 7.0)], h.red);
            line((5.0, 13.6), (14.0, 13.6), h.dim, 0.9);
        }
        Icon::Dashboard => {
            card(r(1.5, 1.5, 7.5, 14.5), h.accent, true);
            for (x, top) in [(3.0, 9.0), (5.0, 5.5)] {
                block(r(x, top, x + 1.5, 13.0), h.accent);
            }
            card(r(9.0, 1.5, 14.5, 7.0), h.orange, true);
            card(r(9.0, 9.0, 14.5, 14.5), h.teal, true);
        }
        Icon::Table | Icon::Grid => {
            let hue = h.accent;
            card(r(1.5, 2.5, 14.5, 13.5), hue, false);
            if icon == Icon::Table {
                painter.rect_filled(r(1.5, 2.5, 14.5, 5.5), egui::CornerRadius { nw: (1.4 * s) as u8, ne: (1.4 * s) as u8, sw: 0, se: 0 }, hue);
            } else {
                painter.rect_filled(r(1.5, 2.5, 14.5, 5.5), 0.0, tint(hue));
            }
            line((1.5, 9.5), (14.5, 9.5), hue.gamma_multiply(0.55), 0.7);
            for x in [6.0, 10.5] {
                line((x, 5.5), (x, 13.5), hue.gamma_multiply(0.55), 0.7);
            }
        }
        Icon::Frequencies | Icon::Histogram | Icon::Bars => {
            axes();
            let (heights, hue): (&[f32], Color32) = match icon {
                Icon::Frequencies => (&[5.0, 9.0, 6.5], h.accent),
                Icon::Histogram => (&[3.0, 7.0, 10.5, 6.0, 2.5], h.teal),
                _ => (&[6.0, 10.5, 4.0], h.blue),
            };
            let n = heights.len() as f32;
            let slot = 11.5 / n;
            let gap = if icon == Icon::Histogram { 0.35 } else { slot * 0.32 };
            for (i, height) in heights.iter().enumerate() {
                let x = 2.9 + slot * i as f32 + gap / 2.0;
                block(r(x, 13.2 - height, x + slot - gap, 13.2), hue);
            }
        }
        Icon::Crosstabs => {
            card(r(1.5, 1.5, 14.5, 14.5), h.dim, false);
            painter.rect_filled(r(1.5, 1.5, 5.5, 14.5), 0.0, tint(h.accent));
            painter.rect_filled(r(5.5, 1.5, 14.5, 5.5), 0.0, tint(h.accent));
            for v in [5.5, 10.0] {
                line((v, 1.5), (v, 14.5), h.dim, 0.6);
                line((1.5, v), (14.5, v), h.dim, 0.6);
            }
            dot(7.75, 7.75, 1.3, h.accent);
            dot(12.25, 12.25, 1.3, h.orange);
        }
        Icon::TTest | Icon::Anova => {
            let curve = |center: f32, color: Color32, height: f32| {
                let pts: Vec<(f32, f32)> = (0..=18)
                    .map(|i| {
                        let x = center - 4.5 + 9.0 * i as f32 / 18.0;
                        let z = (x - center) / 1.6;
                        (x, 13.3 - height * (-0.5 * z * z).exp())
                    })
                    .collect();
                // The bell's body as thin vertical strokes of its tint, then its outline.
                for pair in pts.windows(2) {
                    if let [a, b] = pair {
                        fill(&[(a.0, a.1), (b.0, b.1), (b.0, 13.3), (a.0, 13.3)], tint(color));
                    }
                }
                poly(&pts, color, 1.1);
            };
            line((1.0, 13.6), (15.0, 13.6), h.dim, 0.9);
            if icon == Icon::TTest {
                curve(5.5, h.blue, 9.5);
                curve(10.5, h.orange, 9.5);
            } else {
                curve(4.5, h.blue, 8.0);
                curve(8.0, h.green, 10.5);
                curve(11.5, h.orange, 7.0);
            }
        }
        Icon::Correlations | Icon::Scatter | Icon::Regression => {
            axes();
            let hue = if icon == Icon::Scatter { h.teal } else { h.violet };
            if icon == Icon::Regression {
                line((3.5, 12.5), (14.0, 3.0), h.orange, 1.1);
            }
            for (x, y) in [(4.5, 11.5), (6.0, 8.8), (7.6, 10.0), (9.0, 6.8), (10.6, 7.6), (12.5, 4.5)] {
                dot(x, y, 1.15, hue);
            }
        }
        Icon::Logistic => {
            axes();
            poly(&[(3.0, 3.0), (14.5, 3.0)], h.violet.gamma_multiply(0.35), 0.7);
            let pts: Vec<(f32, f32)> = (0..=24)
                .map(|i| {
                    let x = 3.0 + 11.5 * i as f32 / 24.0;
                    (x, 12.5 - 9.5 / (1.0 + (-(x - 8.5) * 1.1).exp()))
                })
                .collect();
            poly(&pts, h.violet, 1.3);
        }
        Icon::Ranks => {
            for (i, height) in [3.0f32, 6.0, 9.0, 12.0].iter().enumerate() {
                let x = 2.0 + 3.3 * i as f32;
                block(r(x, 14.0 - height, x + 2.4, 14.0), h.teal.gamma_multiply(0.4 + 0.2 * i as f32));
            }
        }
        Icon::Boxplot => {
            for (x, low, q1, q3, high, hue) in [(5.0, 13.5, 10.0, 5.5, 2.5, h.teal), (11.0, 14.0, 11.5, 7.5, 4.5, h.orange)] {
                line((x, low), (x, q1), h.dim, 0.8);
                line((x, q3), (x, high), h.dim, 0.8);
                line((x - 1.2, low), (x + 1.2, low), h.dim, 0.8);
                line((x - 1.2, high), (x + 1.2, high), h.dim, 0.8);
                let body = r(x - 2.3, q3, x + 2.3, q1);
                painter.rect_filled(body, 0.6 * s, tint(hue));
                painter.rect_stroke(body, 0.6 * s, Stroke::new(w, hue), egui::StrokeKind::Middle);
                line((x - 2.3, (q1 + q3) / 2.0), (x + 2.3, (q1 + q3) / 2.0), hue, 1.2);
            }
        }
        Icon::Compute => {
            card(r(2.5, 1.5, 13.5, 14.5), h.green, true);
            block(r(4.2, 3.2, 11.8, 6.2), h.green);
            for y in [8.8, 11.8] {
                for x in [5.2, 8.0, 10.8] {
                    dot(x, y, 0.95, h.green);
                }
            }
        }
        Icon::Recode => {
            line((2.0, 5.0), (12.0, 5.0), h.accent, 1.1);
            fill(&[(11.0, 2.5), (14.5, 5.0), (11.0, 7.5)], h.accent);
            line((4.0, 11.0), (14.0, 11.0), h.orange, 1.1);
            fill(&[(5.0, 8.5), (1.5, 11.0), (5.0, 13.5)], h.orange);
        }
        Icon::Filter => {
            fill(&[(1.5, 2.5), (14.5, 2.5), (9.5, 8.5), (6.5, 8.5)], tint(h.violet));
            fill(&[(6.5, 8.5), (9.5, 8.5), (9.5, 12.5), (6.5, 14.5)], tint(h.violet));
            painter.add(PathShape::closed_line(
                [(1.5, 2.5), (14.5, 2.5), (9.5, 8.5), (9.5, 12.5), (6.5, 14.5), (6.5, 8.5)].iter().map(|(x, y)| p(*x, *y)).collect(),
                Stroke::new(w, h.violet),
            ));
        }
        Icon::InsertCase | Icon::InsertVariable => {
            card(r(1.5, 1.5, 11.5, 11.5), h.dim, false);
            let band = if icon == Icon::InsertCase { r(1.5, 5.0, 11.5, 8.0) } else { r(5.0, 1.5, 8.0, 11.5) };
            painter.rect_filled(band, 0.0, tint(h.accent));
            painter.rect_stroke(band, 0.0, Stroke::new(w * 0.8, h.accent), egui::StrokeKind::Middle);
            dot(12.0, 12.0, 3.6, h.green);
            line((10.2, 12.0), (13.8, 12.0), h.knock, 1.0);
            line((12.0, 10.2), (12.0, 13.8), h.knock, 1.0);
        }
        Icon::Labels => {
            duo(&[(2.0, 8.5), (8.5, 2.0), (14.0, 2.0), (14.0, 7.5), (7.5, 14.0)], h.orange);
            dot(11.0, 5.0, 1.2, h.orange);
        }
        Icon::List => {
            for (i, y) in [3.5, 8.0, 12.5].iter().enumerate() {
                block(r(1.5, y - 1.2, 3.9, y + 1.2), if i == 0 { h.accent } else { tint(h.accent).gamma_multiply(2.5) });
                line((6.0, *y), (14.5, *y), h.dim, 0.9);
            }
        }
        Icon::Search => {
            painter.circle_stroke(p(6.5, 6.5), 4.5 * s, Stroke::new(w * 1.1, h.ink));
            line((9.8, 9.8), (14.0, 14.0), h.ink, 1.5);
        }
        Icon::Contrast => {
            painter.circle_stroke(p(8.0, 8.0), 6.0 * s, Stroke::new(w, h.ink));
            let half: Vec<(f32, f32)> = (0..=16)
                .map(|i| {
                    let a = -std::f32::consts::FRAC_PI_2 + std::f32::consts::PI * i as f32 / 16.0;
                    (8.0 + 6.0 * a.cos(), 8.0 + 6.0 * a.sin())
                })
                .collect();
            fill(&half, h.ink);
        }
        Icon::Share => {
            poly(&[(5.0, 6.0), (2.5, 6.0), (2.5, 14.0), (13.5, 14.0), (13.5, 6.0), (11.0, 6.0)], h.ink, 1.0);
            line((8.0, 1.5), (8.0, 10.0), h.ink, 1.0);
            poly(&[(5.0, 4.5), (8.0, 1.5), (11.0, 4.5)], h.ink, 1.0);
        }
        Icon::Latex => {
            let big = egui::FontId::proportional(7.0 * s);
            let small = egui::FontId::proportional(5.2 * s);
            // As \LaTeX sets it: A raised and tucked under the L's arm, E lowered.
            for (letter, font, x, dy) in
                [("L", &big, -0.6, 0.0), ("A", &small, 2.5, -1.8), ("T", &big, 5.2, 0.0), ("E", &big, 8.5, 1.6), ("X", &big, 11.8, 0.0)]
            {
                painter.text(p(x, 8.0 + dy), egui::Align2::LEFT_CENTER, letter, font.clone(), h.teal);
            }
        }
        Icon::House => {
            poly(&[(2.0, 8.0), (8.0, 2.5), (14.0, 8.0)], h.ink, 1.1);
            poly(&[(3.8, 6.8), (3.8, 14.0), (12.2, 14.0), (12.2, 6.8)], h.ink, 1.0);
            poly(&[(6.8, 14.0), (6.8, 10.0), (9.2, 10.0), (9.2, 14.0)], h.ink, 1.0);
        }
        Icon::Page => page(h.accent),
        Icon::Scale => {
            // A ruler.
            painter.rect_filled(r(1.5, 5.0, 14.5, 11.0), 1.2 * s, tint(h.orange));
            painter.rect_stroke(r(1.5, 5.0, 14.5, 11.0), 1.2 * s, Stroke::new(w, h.orange), egui::StrokeKind::Middle);
            for (k, x) in [4.0, 6.5, 9.0, 11.5].iter().enumerate() {
                let long = if k % 2 == 0 { 8.5 } else { 7.3 };
                line((*x, 5.0), (*x, long), h.orange, 0.8);
            }
        }
        Icon::Ordinal => {
            for (k, height) in [4.0f32, 7.0, 10.0].iter().enumerate() {
                let x = 2.5 + 4.0 * k as f32;
                block(r(x, 13.5 - height, x + 3.0, 13.5), h.green.gamma_multiply(0.5 + 0.25 * k as f32));
            }
        }
        Icon::Nominal => {
            for (x, y, hue) in [(4.5, 5.0, h.blue), (11.5, 5.0, h.orange), (8.0, 11.5, h.violet)] {
                dot(x, y, 2.5, hue);
            }
        }
        Icon::Info => {
            painter.circle_filled(p(8.0, 8.0), 6.3 * s, tint(h.blue));
            painter.circle_stroke(p(8.0, 8.0), 6.3 * s, Stroke::new(w, h.blue));
            line((8.0, 7.0), (8.0, 11.5), h.blue, 1.2);
            dot(8.0, 4.8, 0.9, h.blue);
        }
        Icon::Book => {
            // Two pages open from the spine.
            fill(&[(1.8, 3.8), (8.0, 4.8), (8.0, 13.2), (1.8, 12.2)], tint(h.violet));
            fill(&[(8.0, 4.8), (14.2, 3.8), (14.2, 12.2), (8.0, 13.2)], tint(h.violet));
            poly(
                &[
                    (8.0, 4.8),
                    (5.0, 3.6),
                    (1.8, 3.8),
                    (1.8, 12.2),
                    (5.0, 12.0),
                    (8.0, 13.2),
                    (11.0, 12.0),
                    (14.2, 12.2),
                    (14.2, 3.8),
                    (11.0, 3.6),
                    (8.0, 4.8),
                ],
                h.violet,
                1.0,
            );
            line((8.0, 4.8), (8.0, 13.2), h.violet, 1.0);
        }
        Icon::Close => {
            line((3.5, 3.5), (12.5, 12.5), h.ink, 1.1);
            line((12.5, 3.5), (3.5, 12.5), h.ink, 1.1);
        }
        Icon::ChevronUp => poly(&[(3.5, 10.5), (8.0, 6.0), (12.5, 10.5)], h.ink, 1.2),
        Icon::ChevronDown => poly(&[(3.5, 6.0), (8.0, 10.5), (12.5, 6.0)], h.ink, 1.2),
        Icon::ChevronLeft => poly(&[(10.5, 3.5), (6.0, 8.0), (10.5, 12.5)], h.ink, 1.2),
        Icon::ChevronRight => poly(&[(6.0, 3.5), (10.5, 8.0), (6.0, 12.5)], h.ink, 1.2),
        Icon::Pin => {
            // A pushpin, tilted: head, collar and needle.
            fill(&[(9.0, 1.5), (14.5, 7.0), (12.0, 8.0), (8.0, 4.0)], h.red);
            fill(&[(5.0, 6.0), (10.0, 11.0), (11.0, 9.5), (6.5, 5.0)], h.red);
            line((7.5, 8.5), (2.0, 14.0), h.dim, 1.1);
        }
        Icon::Minus => line((3.0, 8.0), (13.0, 8.0), h.ink, 1.2),
        Icon::Plus => {
            line((3.0, 8.0), (13.0, 8.0), h.ink, 1.2);
            line((8.0, 3.0), (8.0, 13.0), h.ink, 1.2);
        }
        Icon::Dictionary => {
            painter.rect_filled(r(3.0, 1.5, 13.0, 13.0), 1.4 * s, h.accent);
            poly(&[(3.0, 12.0), (3.0, 14.5), (13.0, 14.5)], h.accent, 1.0);
            line((5.5, 5.0), (10.5, 5.0), h.knock, 0.9);
            line((5.5, 7.5), (9.0, 7.5), h.knock, 0.9);
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
pub(crate) const ALL: [Icon; 51] = [
    Icon::Book,
    Icon::Dashboard,
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
    Icon::Pin,
    Icon::Latex,
    Icon::ChevronUp,
    Icon::ChevronDown,
    Icon::ChevronLeft,
    Icon::ChevronRight,
    Icon::House,
    Icon::Page,
    Icon::Scale,
    Icon::Ordinal,
    Icon::Nominal,
    Icon::Info,
    Icon::Close,
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
            for mono in [false, true] {
                let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
                    let painter = ui.ctx().layer_painter(egui::LayerId::debug());
                    if mono {
                        paint_mono(&painter, rect, icon, Color32::WHITE, Color32::BLUE);
                    } else {
                        paint(&painter, rect, icon, &Tokens::light());
                    }
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

    /// The colours an icon is painted with.
    fn colors(icon: Icon, t: &Tokens) -> Vec<Color32> {
        let ctx = egui::Context::default();
        let rect = Rect::from_min_size(pos2(0.0, 0.0), vec2(32.0, 32.0));
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| paint(&ui.ctx().layer_painter(egui::LayerId::debug()), rect, icon, t));
        out.textures_delta.clear();
        let mut found = Vec::new();
        for clipped in out.shapes {
            match clipped.shape {
                egui::Shape::Path(path) => {
                    found.push(path.fill);
                    if let egui::epaint::ColorMode::Solid(color) = path.stroke.color {
                        found.push(color);
                    }
                }
                egui::Shape::Rect(rect) => found.extend([rect.fill, rect.stroke.color]),
                egui::Shape::Circle(circle) => found.extend([circle.fill, circle.stroke.color]),
                egui::Shape::LineSegment { stroke, .. } => found.push(stroke.color),
                _ => {}
            }
        }
        found
    }

    #[test]
    fn icons_are_coloured_by_meaning() {
        let t = Tokens::light();
        let ttest = colors(Icon::TTest, &t);
        assert!(ttest.contains(&t.blue) && ttest.contains(&t.orange), "the two groups' curves differ in colour");
        assert!(colors(Icon::Run, &t).contains(&t.green));
        assert!(colors(Icon::Pdf, &t).contains(&t.red));
        assert!(colors(Icon::Correlations, &t).contains(&t.violet));
        // The window's controls stay in the text colour.
        assert!(colors(Icon::Close, &t).iter().all(|c| *c == t.text || *c == Color32::TRANSPARENT));
    }
}
