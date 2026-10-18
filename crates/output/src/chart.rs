//! Charts. A chart is plain data; `layout` turns it into shapes in a box of a given size (axes,
//! ticks, bars, points, curves, labels) and every renderer draws those shapes: egui on screen,
//! SVG for export. Both show the same chart because neither decides the geometry.

/// What a chart shows. `categories` holds the category of each bin, bar or box, in order, when
/// they can be selected to filter other items; it is empty when they can't.
#[derive(Debug, Clone, PartialEq)]
pub enum Chart {
    /// Bins `[low, high)` with their (weighted) counts; with `normal`, the normal curve of the
    /// given mean and standard deviation scaled to the counts.
    Histogram { title: String, x_label: String, bins: Vec<(f64, f64, f64)>, normal: Option<(f64, f64)>, n: f64, categories: Vec<Category> },
    /// One bar per category.
    Bar { title: String, x_label: String, y_label: String, bars: Vec<(String, f64)>, categories: Vec<Category> },
    /// Points (x, y).
    Scatter { title: String, x_label: String, y_label: String, points: Vec<(f64, f64)> },
    /// One box per group.
    Boxplot { title: String, x_label: String, y_label: String, boxes: Vec<BoxStats>, categories: Vec<Category> },
}

/// A box of a boxplot: the hinges and median, the whisker ends (the most extreme values within
/// 1.5 box lengths), outliers (1.5 to 3 box lengths out) and extremes (beyond 3).
#[derive(Debug, Clone, PartialEq)]
pub struct BoxStats {
    pub label: String,
    pub low: f64,
    pub q1: f64,
    pub median: f64,
    pub q3: f64,
    pub high: f64,
    pub outliers: Vec<f64>,
    pub extremes: Vec<f64>,
}

impl Chart {
    pub fn title(&self) -> &str {
        match self {
            Chart::Histogram { title, .. } | Chart::Bar { title, .. } | Chart::Scatter { title, .. } | Chart::Boxplot { title, .. } => title,
        }
    }

    /// The category of bin, bar or box `part`, if it can be selected.
    pub fn category(&self, part: usize) -> Option<&Category> {
        match self {
            Chart::Histogram { categories, .. } | Chart::Bar { categories, .. } | Chart::Boxplot { categories, .. } => categories.get(part),
            Chart::Scatter { .. } => None,
        }
    }

    pub fn set_title(&mut self, new: &str) {
        match self {
            Chart::Histogram { title, .. } | Chart::Bar { title, .. } | Chart::Scatter { title, .. } | Chart::Boxplot { title, .. } => {
                *title = new.to_string();
            }
        }
    }
}

/// What a line is for; renderers pick its colour and width.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stroke {
    Axis,
    Grid,
    Curve,
    /// A boxplot's median: thick.
    Median,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Anchor {
    Start,
    Middle,
    End,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextSize {
    Small,
    Normal,
    Title,
}

/// A drawing primitive in chart coordinates: pixels from the top left.
#[derive(Debug, Clone, PartialEq)]
pub enum Shape {
    Line {
        from: (f64, f64),
        to: (f64, f64),
        stroke: Stroke,
    },
    /// A filled bar; `part` is the bin, bar or box it draws, for selection.
    Rect {
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        part: Option<usize>,
    },
    Circle {
        x: f64,
        y: f64,
        radius: f64,
    },
    /// A curve through these points.
    Path {
        points: Vec<(f64, f64)>,
    },
    Text {
        x: f64,
        y: f64,
        text: String,
        anchor: Anchor,
        size: TextSize,
        vertical: bool,
    },
}

/// Round tick positions covering `[low, high]`: steps of 1, 2, 2.5 or 5 times a power of ten.
pub fn ticks(low: f64, high: f64, target: usize) -> Vec<f64> {
    if !(low.is_finite() && high.is_finite()) || target == 0 {
        return Vec::new();
    }
    let (low, high) = if high > low { (low, high) } else { (low - 0.5, low + 0.5) };
    let raw = (high - low) / target as f64;
    let power = 10f64.powf(raw.log10().floor());
    let step = [1.0, 2.0, 2.5, 5.0, 10.0].iter().map(|m| m * power).find(|s| *s >= raw).unwrap_or(10.0 * power);
    let first = (low / step).floor() * step;
    let mut out = Vec::new();
    let mut i = 0.0;
    while out.len() < 100 {
        let t = first + i * step;
        // Snap away floating-point noise (-0.30000000000000004 is -0.3).
        let t = (t * 1e12).round() / 1e12;
        out.push(t);
        if t >= high - step * 1e-9 {
            break;
        }
        i += 1.0;
    }
    out
}

/// A tick label with as many decimals as the step needs (0.25 → 2, 0.5 → 1, 20 → 0).
fn label(value: f64, step: f64) -> String {
    let decimals = (0..10).find(|d| {
        let scaled = step * 10f64.powi(*d);
        (scaled - scaled.round()).abs() < 1e-9
    });
    let decimals = decimals.unwrap_or(10) as usize;
    let text = format!("{value:.decimals$}");
    if text == "-0" { "0".into() } else { text }
}

struct Frame {
    left: f64,
    right: f64,
    top: f64,
    bottom: f64,
}

impl Frame {
    fn new(width: f64, height: f64) -> Self {
        Self { left: 64.0, right: width - 16.0, top: 36.0, bottom: height - 46.0 }
    }
    fn x(&self, v: f64, (lo, hi): (f64, f64)) -> f64 {
        self.left + (v - lo) / (hi - lo) * (self.right - self.left)
    }
    fn y(&self, v: f64, (lo, hi): (f64, f64)) -> f64 {
        self.bottom - (v - lo) / (hi - lo) * (self.bottom - self.top)
    }
}

fn axis_titles(out: &mut Vec<Shape>, f: &Frame, width: f64, title: &str, x_label: &str, y_label: &str) {
    out.push(Shape::Text { x: width / 2.0, y: 20.0, text: title.into(), anchor: Anchor::Middle, size: TextSize::Title, vertical: false });
    out.push(Shape::Text {
        x: (f.left + f.right) / 2.0,
        y: f.bottom + 38.0,
        text: x_label.into(),
        anchor: Anchor::Middle,
        size: TextSize::Normal,
        vertical: false,
    });
    out.push(Shape::Text {
        x: 14.0,
        y: (f.top + f.bottom) / 2.0,
        text: y_label.into(),
        anchor: Anchor::Middle,
        size: TextSize::Normal,
        vertical: true,
    });
}

/// Y axis with grid lines and labels; returns the axis range.
fn y_axis(out: &mut Vec<Shape>, f: &Frame, low: f64, high: f64) -> (f64, f64) {
    let t = ticks(low, high, 5);
    let range = (t.first().copied().unwrap_or(low).min(low), t.last().copied().unwrap_or(high).max(high));
    let step = match t[..] {
        [a, b, ..] => b - a,
        _ => 1.0,
    };
    for v in &t {
        let y = f.y(*v, range);
        out.push(Shape::Line { from: (f.left, y), to: (f.right, y), stroke: Stroke::Grid });
        out.push(Shape::Text { x: f.left - 6.0, y: y + 4.0, text: label(*v, step), anchor: Anchor::End, size: TextSize::Small, vertical: false });
    }
    out.push(Shape::Line { from: (f.left, f.top), to: (f.left, f.bottom), stroke: Stroke::Axis });
    range
}

/// X axis for numbers; returns the axis range.
fn x_axis(out: &mut Vec<Shape>, f: &Frame, low: f64, high: f64) -> (f64, f64) {
    let t = ticks(low, high, 6);
    let range = (t.first().copied().unwrap_or(low).min(low), t.last().copied().unwrap_or(high).max(high));
    let step = match t[..] {
        [a, b, ..] => b - a,
        _ => 1.0,
    };
    for v in &t {
        let x = f.x(*v, range);
        out.push(Shape::Line { from: (x, f.bottom), to: (x, f.bottom + 4.0), stroke: Stroke::Axis });
        out.push(Shape::Text { x, y: f.bottom + 18.0, text: label(*v, step), anchor: Anchor::Middle, size: TextSize::Small, vertical: false });
    }
    out.push(Shape::Line { from: (f.left, f.bottom), to: (f.right, f.bottom), stroke: Stroke::Axis });
    range
}

fn normal_pdf(x: f64, mean: f64, sd: f64) -> f64 {
    (-0.5 * ((x - mean) / sd).powi(2)).exp() / (sd * (2.0 * std::f64::consts::PI).sqrt())
}

/// The shapes of `chart` in a `width` × `height` box.
pub fn layout(chart: &Chart, width: f64, height: f64) -> Vec<Shape> {
    let f = Frame::new(width, height);
    let mut out = Vec::new();
    match chart {
        Chart::Histogram { title, x_label, bins, normal, n, .. } => {
            axis_titles(&mut out, &f, width, title, x_label, "Frequency");
            let (lo, hi) = (bins.first().map_or(0.0, |b| b.0), bins.last().map_or(1.0, |b| b.1));
            let top = bins.iter().map(|b| b.2).fold(0.0, f64::max);
            let curve_top = normal.map_or(0.0, |(m, s)| normal_pdf(m, m, s) * n * bins.first().map_or(1.0, |b| b.1 - b.0));
            let yr = y_axis(&mut out, &f, 0.0, top.max(curve_top).max(1.0));
            let xr = x_axis(&mut out, &f, lo, hi);
            for (i, (a, b, count)) in bins.iter().enumerate() {
                let (x0, x1, y) = (f.x(*a, xr), f.x(*b, xr), f.y(*count, yr));
                out.push(Shape::Rect { x: x0, y, width: x1 - x0, height: f.bottom - y, part: Some(i) });
            }
            if let (Some((mean, sd)), Some((a, b, _))) = (normal, bins.first())
                && *sd > 0.0
            {
                let width_bin = b - a;
                let points = (0..=120)
                    .map(|i| {
                        let x = xr.0 + (xr.1 - xr.0) * f64::from(i) / 120.0;
                        (f.x(x, xr), f.y(normal_pdf(x, *mean, *sd) * n * width_bin, yr))
                    })
                    .collect();
                out.push(Shape::Path { points });
                out.push(Shape::Text {
                    x: f.right,
                    y: f.top - 4.0,
                    text: format!("Mean = {mean:.2}  Std. Dev. = {sd:.3}  N = {n:.0}"),
                    anchor: Anchor::End,
                    size: TextSize::Small,
                    vertical: false,
                });
            }
        }
        Chart::Bar { title, x_label, y_label, bars, .. } => {
            axis_titles(&mut out, &f, width, title, x_label, y_label);
            let top = bars.iter().map(|b| b.1).fold(0.0, f64::max);
            let bottom = bars.iter().map(|b| b.1).fold(0.0, f64::min);
            let yr = y_axis(&mut out, &f, bottom, top.max(bottom + 1.0));
            let slot = (f.right - f.left) / bars.len().max(1) as f64;
            let zero = f.y(0.0, yr);
            for (i, (name, value)) in bars.iter().enumerate() {
                let x = f.left + slot * i as f64 + slot * 0.15;
                let y = f.y(*value, yr);
                out.push(Shape::Rect { x, y: y.min(zero), width: slot * 0.7, height: (zero - y).abs(), part: Some(i) });
                out.push(Shape::Text {
                    x: x + slot * 0.35,
                    y: f.bottom + 18.0,
                    text: name.clone(),
                    anchor: Anchor::Middle,
                    size: TextSize::Small,
                    vertical: false,
                });
            }
            out.push(Shape::Line { from: (f.left, zero), to: (f.right, zero), stroke: Stroke::Axis });
        }
        Chart::Scatter { title, x_label, y_label, points } => {
            axis_titles(&mut out, &f, width, title, x_label, y_label);
            let (xmin, xmax) = points.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), p| (a.min(p.0), b.max(p.0)));
            let (ymin, ymax) = points.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), p| (a.min(p.1), b.max(p.1)));
            let yr = y_axis(&mut out, &f, ymin, ymax);
            let xr = x_axis(&mut out, &f, xmin, xmax);
            for (x, y) in points {
                out.push(Shape::Circle { x: f.x(*x, xr), y: f.y(*y, yr), radius: 3.0 });
            }
        }
        Chart::Boxplot { title, x_label, y_label, boxes, .. } => {
            axis_titles(&mut out, &f, width, title, x_label, y_label);
            let all = boxes.iter().flat_map(|b| [b.low, b.high].into_iter().chain(b.outliers.iter().copied()).chain(b.extremes.iter().copied()));
            let (lo, hi) = all.fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), v| (a.min(v), b.max(v)));
            let yr = y_axis(&mut out, &f, lo, hi);
            let slot = (f.right - f.left) / boxes.len().max(1) as f64;
            for (i, b) in boxes.iter().enumerate() {
                let center = f.left + slot * (i as f64 + 0.5);
                let half = (slot * 0.25).min(40.0);
                let (y1, y3) = (f.y(b.q1, yr), f.y(b.q3, yr));
                out.push(Shape::Rect { x: center - half, y: y3, width: half * 2.0, height: (y1 - y3).max(0.0), part: Some(i) });
                let ym = f.y(b.median, yr);
                out.push(Shape::Line { from: (center - half, ym), to: (center + half, ym), stroke: Stroke::Median });
                for (end, hinge) in [(b.low, b.q1), (b.high, b.q3)] {
                    let (ye, yh) = (f.y(end, yr), f.y(hinge, yr));
                    out.push(Shape::Line { from: (center, yh), to: (center, ye), stroke: Stroke::Axis });
                    out.push(Shape::Line { from: (center - half / 2.0, ye), to: (center + half / 2.0, ye), stroke: Stroke::Axis });
                }
                for v in &b.outliers {
                    out.push(Shape::Circle { x: center, y: f.y(*v, yr), radius: 3.0 });
                }
                for v in &b.extremes {
                    out.push(Shape::Text {
                        x: center,
                        y: f.y(*v, yr) + 4.0,
                        text: "*".into(),
                        anchor: Anchor::Middle,
                        size: TextSize::Normal,
                        vertical: false,
                    });
                }
                out.push(Shape::Text {
                    x: center,
                    y: f.bottom + 18.0,
                    text: b.label.clone(),
                    anchor: Anchor::Middle,
                    size: TextSize::Small,
                    vertical: false,
                });
            }
            out.push(Shape::Line { from: (f.left, f.bottom), to: (f.right, f.bottom), stroke: Stroke::Axis });
        }
    }
    out
}

/// The bin, bar or box whose column holds `x` in `shapes` (a `layout`): the whole height of the
/// plot counts, so a short bar is as easy to pick as a tall one.
pub fn part_at(shapes: &[Shape], x: f64) -> Option<usize> {
    shapes.iter().find_map(|shape| match shape {
        Shape::Rect { x: left, width, part, .. } if (*left..=left + width).contains(&x) => *part,
        _ => None,
    })
}

use crate::Category;
use crate::html::escape;

/// The chart as a standalone SVG element (colours via CSS classes, so a page can theme it).
pub fn svg(chart: &Chart, width: f64, height: f64) -> String {
    svg_marked(chart, width, height, false, None)
}

/// The chart as SVG whose bins, bars and boxes can be picked: each carries `data-part` (its
/// index) and the `pick` class; with `selected` among its categories, that one is `sel` and the
/// others `dim`.
pub fn svg_picking(chart: &Chart, width: f64, height: f64, selected: Option<&Category>) -> String {
    svg_marked(chart, width, height, true, selected)
}

fn svg_marked(chart: &Chart, width: f64, height: f64, picking: bool, selected: Option<&Category>) -> String {
    let shown = selected.filter(|s| (0..).map_while(|p| chart.category(p)).any(|c| c == *s));
    let mut s = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" class="chart" viewBox="0 0 {width} {height}" width="{width}" height="{height}" role="img"><title>{}</title>"#,
        escape(chart.title())
    );
    for shape in layout(chart, width, height) {
        match shape {
            Shape::Line { from, to, stroke } => {
                let class = match stroke {
                    Stroke::Axis => "axis",
                    Stroke::Grid => "grid",
                    Stroke::Curve => "curve",
                    Stroke::Median => "median",
                };
                s.push_str(&format!(r#"<line class="{class}" x1="{:.1}" y1="{:.1}" x2="{:.1}" y2="{:.1}"/>"#, from.0, from.1, to.0, to.1));
            }
            Shape::Rect { x, y, width, height, part } => {
                let category = part.and_then(|p| chart.category(p));
                let (class, data) = match (picking, part, category) {
                    (true, Some(p), Some(c)) => {
                        let state = match shown {
                            Some(sel) if sel == c => " sel",
                            Some(_) => " dim",
                            None => "",
                        };
                        (format!("bar pick{state}"), format!(r#" data-part="{p}""#))
                    }
                    _ => ("bar".to_string(), String::new()),
                };
                s.push_str(&format!(r#"<rect class="{class}"{data} x="{x:.1}" y="{y:.1}" width="{width:.1}" height="{height:.1}"/>"#))
            }
            Shape::Circle { x, y, radius } => s.push_str(&format!(r#"<circle class="point" cx="{x:.1}" cy="{y:.1}" r="{radius}"/>"#)),
            Shape::Path { points } => {
                let d: Vec<String> =
                    points.iter().enumerate().map(|(i, (x, y))| format!("{}{x:.1},{y:.1}", if i == 0 { "M" } else { "L" })).collect();
                s.push_str(&format!(r#"<path class="curve" d="{}"/>"#, d.join(" ")));
            }
            Shape::Text { x, y, text, anchor, size, vertical } => {
                let anchor = match anchor {
                    Anchor::Start => "start",
                    Anchor::Middle => "middle",
                    Anchor::End => "end",
                };
                let class = match size {
                    TextSize::Small => "tick",
                    TextSize::Normal => "label",
                    TextSize::Title => "title",
                };
                let rotate = if vertical { format!(r#" transform="rotate(-90 {x:.1} {y:.1})""#) } else { String::new() };
                s.push_str(&format!(r#"<text class="{class}" x="{x:.1}" y="{y:.1}" text-anchor="{anchor}"{rotate}>{}</text>"#, escape(&text)));
            }
        }
    }
    s.push_str("</svg>");
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_ticks() {
        assert_eq!(ticks(0.0, 10.0, 5), [0.0, 2.0, 4.0, 6.0, 8.0, 10.0]);
        assert_eq!(ticks(0.0, 1.0, 4), [0.0, 0.25, 0.5, 0.75, 1.0]);
        assert_eq!(ticks(13.0, 87.0, 5), [0.0, 20.0, 40.0, 60.0, 80.0, 100.0]);
        assert_eq!(ticks(-0.3, 0.3, 6), [-0.3, -0.2, -0.1, 0.0, 0.1, 0.2, 0.3]);
        assert_eq!(ticks(5.0, 5.0, 4), [4.5, 4.75, 5.0, 5.25, 5.5], "a single value gets a range around it");
        assert!(ticks(f64::NAN, 1.0, 4).is_empty());
        assert_eq!(label(0.25, 0.25), "0.25");
        assert_eq!(label(40.0, 20.0), "40");
        assert_eq!(label(0.2, 0.1), "0.2");
    }

    #[test]
    fn layouts_stay_inside_the_box() {
        let charts = [
            Chart::Histogram {
                title: "h".into(),
                x_label: "x".into(),
                bins: vec![(0.0, 10.0, 3.0), (10.0, 20.0, 7.0), (20.0, 30.0, 2.0)],
                normal: Some((14.0, 6.0)),
                n: 12.0,
                categories: Vec::new(),
            },
            Chart::Bar {
                title: "b".into(),
                x_label: "c".into(),
                y_label: "Count".into(),
                bars: vec![("A".into(), 4.0), ("B".into(), 9.0)],
                categories: Vec::new(),
            },
            Chart::Scatter { title: "s".into(), x_label: "x".into(), y_label: "y".into(), points: vec![(1.0, 2.0), (3.0, -1.0), (2.0, 5.5)] },
            Chart::Boxplot {
                title: "box".into(),
                x_label: "g".into(),
                y_label: "y".into(),
                boxes: vec![
                    BoxStats { label: "A".into(), low: 1.0, q1: 3.0, median: 5.0, q3: 8.0, high: 15.0, outliers: vec![], extremes: vec![40.0] },
                    BoxStats { label: "B".into(), low: 2.0, q1: 4.0, median: 4.5, q3: 6.0, high: 9.0, outliers: vec![12.0], extremes: vec![] },
                ],
                categories: Vec::new(),
            },
        ];
        for chart in &charts {
            let shapes = layout(chart, 480.0, 320.0);
            for shape in &shapes {
                let inside = |x: f64, y: f64| (0.0..=480.0).contains(&x) && (0.0..=320.0).contains(&y);
                let ok = match shape {
                    Shape::Line { from, to, .. } => inside(from.0, from.1) && inside(to.0, to.1),
                    Shape::Rect { x, y, width, height, .. } => inside(*x, *y) && inside(x + width, y + height) && *height >= 0.0,
                    Shape::Circle { x, y, .. } => inside(*x, *y),
                    Shape::Path { points } => points.iter().all(|(x, y)| inside(*x, *y)),
                    Shape::Text { x, y, .. } => inside(*x, *y),
                };
                assert!(ok, "{shape:?} leaves the box in {}", chart.title());
            }
            assert!(svg(chart, 480.0, 320.0).starts_with("<svg"));
        }
        let bars = layout(&charts[1], 480.0, 320.0);
        assert_eq!(bars.iter().filter(|s| matches!(s, Shape::Rect { .. })).count(), 2);
        // Two slots of 200 between x = 64 and 464; each bar takes the middle 70 %.
        assert_eq!(part_at(&bars, 164.0), Some(0));
        assert_eq!(part_at(&bars, 364.0), Some(1));
        assert_eq!(part_at(&bars, 70.0), None, "between bars");
    }
}
