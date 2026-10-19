//! Mathematics on screen: lays out an `output::math::Math` tree (fractions, sums with their
//! limits, roots, scripts, bars, parentheses that grow) and paints it. egui has no LaTeX, and
//! this only has to set what MK Sigma itself writes. A long equation breaks before an `=` or a
//! `+` of its outer row, the lines after the first indented to its first `=`.
//!
//! Coordinates are relative to a box's left end on its baseline, y down: ascent is above it.

use std::sync::Arc;

use egui::{Color32, Galley, Stroke, Vec2, pos2, vec2};
use mksigma_engine::output::math::Math;

/// Smallest size scripts and limits shrink to.
const MIN_SIZE: f32 = 8.0;

enum Prim {
    /// A galley at its top-left corner.
    Text(Vec2, Arc<Galley>),
    /// A polyline of this width.
    Path(Vec<Vec2>, f32),
}

/// A laid-out box.
struct Laid {
    width: f32,
    ascent: f32,
    descent: f32,
    prims: Vec<Prim>,
}

impl Laid {
    fn empty() -> Self {
        Laid { width: 0.0, ascent: 0.0, descent: 0.0, prims: Vec::new() }
    }

    /// Adds `child`'s prims with its origin at `at`.
    fn place(&mut self, child: Laid, at: Vec2) {
        for prim in child.prims {
            self.prims.push(match prim {
                Prim::Text(p, g) => Prim::Text(p + at, g),
                Prim::Path(points, w) => Prim::Path(points.into_iter().map(|p| p + at).collect(), w),
            });
        }
    }

    fn height(&self) -> f32 {
        self.ascent + self.descent
    }
}

/// Operators get room on both sides.
fn is_operator(s: &str) -> bool {
    matches!(s, "=" | "+" | "−" | "-" | "·" | "×" | "≈" | "≤" | "≥")
}

/// Where a long equation may break: before these.
fn breaks_before(m: &Math) -> bool {
    matches!(m, Math::Text(s) if matches!(s.as_str(), "=" | "+" | "≈"))
}

struct Typesetter<'a> {
    painter: &'a egui::Painter,
    color: Color32,
}

impl Typesetter<'_> {
    fn text(&self, s: &str, size: f32, italic: bool) -> Laid {
        // An arrow the interface's fonts lack: drawn.
        if s == "→" {
            let (axis, stroke) = (size * 0.3, (size / 14.0).max(1.0));
            let (pad, length, head) = (size * 0.3, size * 0.9, size * 0.22);
            let tip = vec2(pad + length, -axis);
            return Laid {
                width: length + 2.0 * pad,
                ascent: axis + head,
                descent: 0.0,
                prims: vec![
                    Prim::Path(vec![vec2(pad, -axis), tip], stroke),
                    Prim::Path(vec![tip + vec2(-head, -head * 0.8), tip, tip + vec2(-head, head * 0.8)], stroke),
                ],
            };
        }
        // Glyphs the interface's fonts may not have, as ones they do.
        let s = match s {
            "⋯" | "…" => "···",
            "-" => "−",
            s => s,
        };
        let mut job = egui::text::LayoutJob::default();
        job.append(s, 0.0, egui::TextFormat { font_id: egui::FontId::proportional(size), color: self.color, italics: italic, ..Default::default() });
        let galley = self.painter.layout_job(job);
        let height = galley.size().y;
        let baseline = galley.rows.first().and_then(|r| r.row.glyphs.first().map(|g| r.pos.y + g.pos.y)).unwrap_or(height * 0.8);
        let pad = if is_operator(s) { size * 0.25 } else { 0.0 };
        // Italics lean past their advance: a little room after them.
        let lean = if italic { size * 0.06 } else { 0.0 };
        let width = galley.size().x + 2.0 * pad + lean;
        Laid { width, ascent: baseline, descent: height - baseline, prims: vec![Prim::Text(vec2(pad, -baseline), galley)] }
    }

    fn row(&self, parts: &[Math], size: f32) -> Laid {
        let mut out = Laid::empty();
        for part in parts {
            let laid = self.lay(part, size);
            let x = out.width;
            out.width += laid.width;
            out.ascent = out.ascent.max(laid.ascent);
            out.descent = out.descent.max(laid.descent);
            out.place(laid, vec2(x, 0.0));
        }
        out
    }

    fn lay(&self, m: &Math, size: f32) -> Laid {
        let axis = size * 0.3;
        let stroke = (size / 14.0).max(1.0);
        let small = (size * 0.72).max(MIN_SIZE);
        match m {
            Math::Var(s) => self.text(s, size, true),
            Math::Text(s) => self.text(s, size, false),
            Math::Row(parts) => self.row(parts, size),
            Math::Frac(a, b) => {
                let inner = (size * 0.9).max(MIN_SIZE);
                let (a, b) = (self.lay(a, inner), self.lay(b, inner));
                let gap = size * 0.18;
                let pad = size * 0.15;
                let width = a.width.max(b.width) + 2.0 * pad;
                let mut out = Laid {
                    width: width + size * 0.1,
                    ascent: axis + gap + a.height(),
                    descent: gap + b.height() - axis,
                    prims: vec![Prim::Path(vec![vec2(0.0, -axis), vec2(width, -axis)], stroke)],
                };
                let a_at = vec2((width - a.width) / 2.0, -axis - gap - a.descent);
                let b_at = vec2((width - b.width) / 2.0, -axis + gap + b.ascent);
                out.place(a, a_at);
                out.place(b, b_at);
                out
            }
            Math::Sqrt(a) => {
                let a = self.lay(a, size);
                let gap = size * 0.15;
                let hook = size * 0.6;
                let top = -(a.ascent + gap);
                let bottom = a.descent;
                let mid = bottom - (bottom - top) * 0.4;
                let end = hook + a.width + size * 0.1;
                let mut out = Laid {
                    width: end + size * 0.1,
                    ascent: a.ascent + gap + stroke,
                    descent: a.descent,
                    prims: vec![Prim::Path(
                        vec![
                            vec2(0.0, mid + size * 0.05),
                            vec2(hook * 0.3, mid - size * 0.05),
                            vec2(hook * 0.6, bottom),
                            vec2(hook, top),
                            vec2(end, top),
                        ],
                        stroke,
                    )],
                };
                out.place(a, vec2(hook + size * 0.05, 0.0));
                out
            }
            Math::Sub(a, b) => {
                let (a, b) = (self.lay(a, size), self.lay(b, small));
                let drop = size * 0.25;
                let mut out = Laid {
                    width: a.width + b.width,
                    ascent: a.ascent.max(b.ascent - drop),
                    descent: a.descent.max(drop + b.descent),
                    prims: Vec::new(),
                };
                let x = a.width;
                out.place(a, Vec2::ZERO);
                out.place(b, vec2(x, drop));
                out
            }
            Math::Sup(a, b) => {
                let (a, b) = (self.lay(a, size), self.lay(b, small));
                let rise = (a.ascent - b.ascent * 0.5).max(size * 0.38);
                let mut out = Laid {
                    width: a.width + b.width,
                    ascent: a.ascent.max(rise + b.ascent),
                    descent: a.descent.max(b.descent - rise),
                    prims: Vec::new(),
                };
                let x = a.width;
                out.place(a, Vec2::ZERO);
                out.place(b, vec2(x, -rise));
                out
            }
            Math::Bar(a) => {
                let a = self.lay(a, size);
                let y = -(a.ascent + size * 0.05);
                let inset = size * 0.08;
                let mut out = Laid {
                    width: a.width,
                    ascent: a.ascent + size * 0.05 + stroke,
                    descent: a.descent,
                    // Over an italic letter, shifted with its lean.
                    prims: vec![Prim::Path(vec![vec2(inset + size * 0.08, y), vec2(a.width - inset + size * 0.04, y)], stroke)],
                };
                out.place(a, Vec2::ZERO);
                out
            }
            Math::Sum { from, to, body } => {
                let height = size * 1.25;
                let width = size * 0.95;
                let (top, bottom) = (-axis - height / 2.0, -axis + height / 2.0);
                let from = from.as_deref().map(|f| self.lay(f, small));
                let to = to.as_deref().map(|t| self.lay(t, small));
                let column = [Some(width), from.as_ref().map(|f| f.width), to.as_ref().map(|t| t.width)].into_iter().flatten().fold(0.0, f32::max);
                let x0 = (column - width) / 2.0;
                let sigma = vec![
                    vec2(x0 + width, top + size * 0.12),
                    vec2(x0 + width, top),
                    vec2(x0, top),
                    vec2(x0 + width * 0.55, -axis),
                    vec2(x0, bottom),
                    vec2(x0 + width, bottom),
                    vec2(x0 + width, bottom - size * 0.12),
                ];
                let gap = size * 0.12;
                let mut out = Laid { width: column, ascent: -top, descent: bottom, prims: vec![Prim::Path(sigma, stroke * 1.2)] };
                if let Some(to) = to {
                    out.ascent = -top + gap + to.height();
                    let at = vec2((column - to.width) / 2.0, top - gap - to.descent);
                    out.place(to, at);
                }
                if let Some(from) = from {
                    out.descent = bottom + gap + from.height();
                    let at = vec2((column - from.width) / 2.0, bottom + gap + from.ascent);
                    out.place(from, at);
                }
                let body = self.lay(body, size);
                let x = column + size * 0.2;
                out.width = x + body.width;
                out.ascent = out.ascent.max(body.ascent);
                out.descent = out.descent.max(body.descent);
                out.place(body, vec2(x, 0.0));
                out
            }
            Math::Paren(a) => {
                let a = self.lay(a, size);
                let open = self.text("(", size, false);
                if a.height() <= open.height() * 1.45 {
                    let close = self.text(")", size, false);
                    let mut out = Laid {
                        width: open.width + a.width + close.width,
                        ascent: a.ascent.max(open.ascent),
                        descent: a.descent.max(open.descent),
                        prims: Vec::new(),
                    };
                    let (x1, x2) = (open.width, open.width + a.width);
                    out.place(open, Vec2::ZERO);
                    out.place(a, vec2(x1, 0.0));
                    out.place(close, vec2(x2, 0.0));
                    return out;
                }
                // Drawn to the height of what they hold.
                let (top, bottom) = (-a.ascent - size * 0.1, a.descent + size * 0.1);
                let w = size * 0.35;
                let curve = |x_out: f32, x_in: f32| -> Vec<Vec2> {
                    (0..=12)
                        .map(|k| {
                            let t = k as f32 / 12.0;
                            let bulge = (std::f32::consts::PI * t).sin();
                            vec2(x_in + (x_out - x_in) * bulge, top + (bottom - top) * t)
                        })
                        .collect()
                };
                let width = 2.0 * w + a.width;
                let mut out = Laid {
                    width,
                    ascent: -top,
                    descent: bottom,
                    prims: vec![Prim::Path(curve(size * 0.08, w * 0.85), stroke), Prim::Path(curve(width - size * 0.08, width - w * 0.85), stroke)],
                };
                out.place(a, vec2(w, 0.0));
                out
            }
        }
    }
}

/// Pieces of `m`'s outer row (and the rows in it), where a long line may break.
fn flatten<'m>(m: &'m Math, out: &mut Vec<&'m Math>) {
    match m {
        Math::Row(parts) => parts.iter().for_each(|p| flatten(p, out)),
        m => out.push(m),
    }
}

/// Shows `m` at `size`, in as many lines as it needs to fit the width; `label` is what screen
/// readers (and tests) get for it, its LaTeX.
pub(crate) fn show(ui: &mut egui::Ui, m: &Math, size: f32) -> egui::Response {
    let color = ui.visuals().text_color();
    let painter = ui.painter().clone();
    let setter = Typesetter { painter: &painter, color };
    let mut pieces = Vec::new();
    flatten(m, &mut pieces);
    // A list may break after its commas too.
    let after_comma = |k: usize| k.checked_sub(1).and_then(|j| pieces.get(j)).is_some_and(|p| matches!(p, Math::Text(s) if s.starts_with(',')));
    let laid: Vec<(bool, Laid)> = pieces.iter().enumerate().map(|(k, p)| (breaks_before(p) || after_comma(k), setter.lay(p, size))).collect();
    let max = ui.available_width().max(size * 10.0);
    // Lines after the first start under the first `=`, or a little in when that is far.
    let first_eq = pieces.iter().position(|p| matches!(p, Math::Text(s) if s == "="));
    let indent = first_eq.map_or(size * 2.0, |k| laid.iter().take(k).map(|(_, l)| l.width).sum::<f32>()).min(max * 0.3);
    // Each break point with what follows it up to the next, which must fit together.
    let chunk: Vec<f32> = (0..laid.len())
        .map(|k| {
            let rest = laid.iter().skip(k + 1).take_while(|(b, _)| !*b).map(|(_, l)| l.width).sum::<f32>();
            laid.get(k).map_or(0.0, |(_, l)| l.width) + rest
        })
        .collect();
    let mut lines: Vec<Vec<Laid>> = vec![Vec::new()];
    let mut x = 0.0;
    for ((can_break, piece), chunk) in laid.into_iter().zip(chunk) {
        let start = lines.last().is_some_and(|l| l.is_empty());
        if can_break && !start && x + chunk > max {
            lines.push(Vec::new());
            x = indent;
        }
        x += piece.width;
        if let Some(line) = lines.last_mut() {
            line.push(piece);
        }
    }
    let gap = size * 0.35;
    let sizes: Vec<(f32, f32, f32)> = lines
        .iter()
        .enumerate()
        .map(|(k, line)| {
            let lead = if k == 0 { 0.0 } else { indent };
            let width = lead + line.iter().map(|l| l.width).sum::<f32>();
            (width, line.iter().map(|l| l.ascent).fold(size * 0.8, f32::max), line.iter().map(|l| l.descent).fold(size * 0.25, f32::max))
        })
        .collect();
    let width = sizes.iter().map(|s| s.0).fold(0.0, f32::max);
    let height = sizes.iter().map(|s| s.1 + s.2).sum::<f32>() + gap * sizes.len().saturating_sub(1) as f32;
    let (rect, response) = ui.allocate_exact_size(vec2(width, height), egui::Sense::hover());
    let mut y = rect.top();
    for (k, (line, (_, ascent, descent))) in lines.into_iter().zip(sizes).enumerate() {
        let baseline = y + ascent;
        let mut x = rect.left() + if k == 0 { 0.0 } else { indent };
        for piece in line {
            let origin = pos2(x, baseline);
            for prim in &piece.prims {
                match prim {
                    Prim::Text(at, galley) => {
                        painter.galley(origin + *at, galley.clone(), color);
                    }
                    Prim::Path(points, w) => {
                        painter.add(egui::Shape::line(points.iter().map(|p| origin + *p).collect(), Stroke::new(*w, color)));
                    }
                }
            }
            x += piece.width;
        }
        y = baseline + descent + gap;
    }
    let label = m.to_latex();
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, &label));
    response
}
