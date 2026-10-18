//! The Output as LaTeX: tables with `booktabs` (no vertical rules, as APA style has them) and
//! charts as `pgfplots` axes drawn from the data, so they take the document's fonts. A whole
//! Output is a document that compiles as it is (`pdflatex`); one item is a fragment to paste.

use crate::chart::BoxStats;
use crate::style::{Family, Style};
use crate::{Cell, Chart, Item, Output, Table};

/// The charts' colours as `xcolor` specifications, written inline so a pasted fragment needs no
/// colour definitions: bars, their outline, the curve. Grey and black by default, for print.
struct Ink {
    fill: String,
    curve: String,
}

impl Ink {
    fn of(style: &Style) -> Ink {
        let spec = |[r, g, b]: [u8; 3]| format!("{{rgb,255:red,{r};green,{g};blue,{b}}}");
        match style.palette.colors() {
            Some((bar, curve)) => Ink { fill: spec(bar), curve: spec(curve) },
            None => Ink { fill: "black!20".into(), curve: "black".into() },
        }
    }
}

/// Text with LaTeX's special characters escaped.
pub fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\textbackslash{}"),
            '&' | '%' | '$' | '#' | '_' | '{' | '}' => {
                out.push('\\');
                out.push(c);
            }
            '~' => out.push_str("\\textasciitilde{}"),
            '^' => out.push_str("\\textasciicircum{}"),
            '<' => out.push_str("\\textless{}"),
            '>' => out.push_str("\\textgreater{}"),
            '\n' => out.push(' '),
            c => out.push(c),
        }
    }
    out
}

/// A label from a title: lower case letters and digits joined by hyphens.
fn slug(title: &str) -> String {
    let mut out = String::new();
    for c in title.chars().flat_map(char::to_lowercase) {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    let out = out.trim_end_matches('-').to_string();
    if out.is_empty() { "item".to_string() } else { out }
}

/// Labels unique within one document: `tab:anova`, `tab:anova-2`...
#[derive(Default)]
struct Labels(Vec<String>);

impl Labels {
    fn next(&mut self, kind: &str, title: &str) -> String {
        let base = format!("{kind}:{}", slug(title));
        let mut label = base.clone();
        let mut n = 1;
        while self.0.contains(&label) {
            n += 1;
            label = format!("{base}-{n}");
        }
        self.0.push(label.clone());
        label
    }
}

fn number(x: f64) -> String {
    if x.is_finite() { format!("{x}") } else { "0".to_string() }
}

/// Notes under a float, as tables in APA style carry theirs.
fn notes(notes: &[&Item]) -> String {
    notes
        .iter()
        .map(|n| match n {
            Item::Warning(s) => format!("\n\\par\\smallskip{{\\raggedright\\footnotesize\\textit{{Warning: {}}}\\par}}", escape(s)),
            Item::Text(s) => format!("\n\\par\\smallskip{{\\raggedright\\footnotesize {}\\par}}", escape(s)),
            _ => String::new(),
        })
        .collect()
}

/// Roughly how many characters wide a table's rows are.
fn width(t: &Table, style: &Style) -> usize {
    let stub = t.rows.iter().map(|(l, _)| l.chars().count()).max().unwrap_or(0);
    let columns: usize = (0..t.columns.len())
        .map(|c| {
            let cells = t.rows.iter().filter_map(|(_, cells)| cells.get(c)).map(|cell| style.cell(cell).chars().count()).max().unwrap_or(0);
            t.columns.get(c).map_or(0, |h| h.chars().count()).max(cells) + 3
        })
        .sum();
    stub + columns
}

fn table(t: &Table, labels: &mut Labels, after: &[&Item], style: &Style) -> String {
    let columns = t.columns.len();
    // Numbers line up on the right, text on the left; the stub column holds labels.
    let align: String = std::iter::once('l')
        .chain((0..columns).map(|c| {
            let numeric = t.rows.iter().filter_map(|(_, cells)| cells.get(c)).any(|cell| matches!(cell, Cell::Num { .. } | Cell::Int(_)));
            if numeric { 'r' } else { 'l' }
        }))
        .collect();
    let mut s = format!("\\begin{{table}}[htbp]\n\\centering\n\\caption{{{}}}\n\\label{{{}}}\n", escape(&t.title), labels.next("tab", &t.title));
    // Wide tables shrink to the line rather than run off the page (about 85 characters fit).
    let wide = width(t, style) * usize::from(style.size) > 85 * usize::from(Style::SIZE);
    if wide {
        s.push_str("\\resizebox{\\linewidth}{!}{%\n");
    }
    s.push_str(&format!("\\begin{{tabular}}{{{align}}}\n\\toprule\n"));
    let header: Vec<String> = std::iter::once(String::new()).chain(t.columns.iter().map(|c| escape(c))).collect();
    s.push_str(&header.join(" & "));
    s.push_str(" \\\\\n\\midrule\n");
    for (label, cells) in &t.rows {
        let row: Vec<String> = std::iter::once(escape(label))
            .chain((0..columns).map(|c| cells.get(c).map(|cell| escape(&style.cell(cell))).unwrap_or_default()))
            .collect();
        s.push_str(&row.join(" & "));
        s.push_str(" \\\\\n");
    }
    s.push_str("\\bottomrule\n\\end{tabular}");
    if wide {
        s.push('}');
    }
    s.push_str(&notes(after));
    s.push_str("\n\\end{table}\n");
    s
}

/// The axis options every chart shares: size, labels, title-less (the caption has it).
fn axis(x_label: &str, y_label: &str, extra: &str) -> String {
    format!(
        "\\begin{{axis}}[width=0.85\\linewidth, height=6.5cm, xlabel={{{}}}, ylabel={{{}}}, tick label style={{font=\\small}}, /pgf/number format/1000 sep={{}}{extra}]\n",
        escape(x_label),
        escape(y_label)
    )
}

fn coordinates(points: impl Iterator<Item = (f64, f64)>) -> String {
    points.filter(|(x, y)| x.is_finite() && y.is_finite()).map(|(x, y)| format!("({}, {})", number(x), number(y))).collect::<Vec<_>>().join(" ")
}

fn boxplot(i: usize, b: &BoxStats, ink: &Ink) -> String {
    let outliers: Vec<String> = b.outliers.iter().chain(&b.extremes).map(|y| format!("(0, {})", number(*y))).collect();
    format!(
        "\\addplot+[boxplot prepared={{draw position={}, lower whisker={}, lower quartile={}, median={}, upper quartile={}, upper whisker={}}}, black, fill={}, mark=*, mark size=1.2pt] coordinates {{{}}};\n",
        i + 1,
        number(b.low),
        number(b.q1),
        number(b.median),
        number(b.q3),
        number(b.high),
        ink.fill,
        outliers.join(" ")
    )
}

fn chart(c: &Chart, labels: &mut Labels, after: &[&Item], style: &Style) -> String {
    let ink = Ink::of(style);
    let mut s = String::from("\\begin{figure}[htbp]\n\\centering\n\\begin{tikzpicture}\n");
    match c {
        Chart::Histogram { x_label, bins, normal, n, .. } => {
            s.push_str(&axis(x_label, "Frequency", ", ymin=0, area style"));
            // `ybar interval` draws each bin from its point to the next; the last edge closes it.
            let mut points: Vec<(f64, f64)> = bins.iter().map(|(a, _, count)| (*a, *count)).collect();
            if let Some((_, b, _)) = bins.last() {
                points.push((*b, 0.0));
            }
            s.push_str(&format!("\\addplot+[ybar interval, fill={}, draw=black] coordinates {{{}}};\n", ink.fill, coordinates(points.into_iter())));
            if let (Some((mean, sd)), Some((a, b, _)), Some((_, end, _))) = (normal, bins.first(), bins.last())
                && *sd > 0.0
            {
                let scale = n * (b - a);
                s.push_str(&format!(
                    "\\addplot[domain={}:{}, samples=120, smooth, thick, draw={}] {{{} * exp(-((x - {})^2) / (2 * {}^2)) / ({} * sqrt(2 * pi))}};\n",
                    number(*a),
                    number(*end),
                    ink.curve,
                    number(scale),
                    number(*mean),
                    number(*sd),
                    number(*sd)
                ));
            }
        }
        Chart::Bar { x_label, y_label, bars, .. } => {
            let ticks: Vec<String> = (1..=bars.len()).map(|i| i.to_string()).collect();
            let names: Vec<String> = bars.iter().map(|(name, _)| format!("{{{}}}", escape(name))).collect();
            let extra =
                format!(", ybar, ymin=0, bar width=0.6, enlarge x limits=0.15, xtick={{{}}}, xticklabels={{{}}}", ticks.join(","), names.join(","));
            s.push_str(&axis(x_label, y_label, &extra));
            s.push_str(&format!(
                "\\addplot[fill={}, draw=black] coordinates {{{}}};\n",
                ink.fill,
                coordinates(bars.iter().enumerate().map(|(i, (_, v))| ((i + 1) as f64, *v)))
            ));
        }
        Chart::Scatter { x_label, y_label, points, .. } => {
            s.push_str(&axis(x_label, y_label, ""));
            s.push_str(&format!(
                "\\addplot[only marks, mark=*, mark size=1.4pt, color={}] coordinates {{{}}};\n",
                if style.palette.colors().is_some() { &ink.fill } else { "black" },
                coordinates(points.iter().copied())
            ));
        }
        Chart::Boxplot { x_label, y_label, boxes, .. } => {
            let ticks: Vec<String> = (1..=boxes.len()).map(|i| i.to_string()).collect();
            let names: Vec<String> = boxes.iter().map(|b| format!("{{{}}}", escape(&b.label))).collect();
            let extra = format!(", boxplot/draw direction=y, xtick={{{}}}, xticklabels={{{}}}", ticks.join(","), names.join(","));
            s.push_str(&axis(x_label, y_label, &extra));
            for (i, b) in boxes.iter().enumerate() {
                s.push_str(&boxplot(i, b, &ink));
            }
        }
    }
    s.push_str("\\end{axis}\n\\end{tikzpicture}");
    s.push_str(&notes(after));
    s.push_str(&format!("\n\\caption{{{}}}\n\\label{{{}}}\n\\end{{figure}}\n", escape(c.title()), labels.next("fig", c.title())));
    s
}

fn item(i: &Item, labels: &mut Labels, after: &[&Item], style: &Style) -> String {
    match i {
        Item::Table(t) => table(t, labels, after, style),
        Item::Chart(c) => chart(c, labels, after, style),
        Item::Text(s) => format!("\\begin{{flushleft}}\\footnotesize {}\\end{{flushleft}}\n", escape(s)),
        Item::Warning(s) => format!("\\begin{{flushleft}}\\footnotesize\\textit{{Warning: {}}}\\end{{flushleft}}\n", escape(s)),
    }
}

/// The items, with the notes and warnings that follow a table or chart inside its float, so
/// they stay with it wherever LaTeX places it.
fn items(list: &[Item], style: &Style) -> String {
    let mut labels = Labels::default();
    let mut out = String::new();
    let mut k = 0;
    while let Some(first) = list.get(k) {
        let notes: Vec<&Item> = if matches!(first, Item::Table(_) | Item::Chart(_)) {
            list.iter().skip(k + 1).take_while(|i| matches!(i, Item::Text(_) | Item::Warning(_))).collect()
        } else {
            Vec::new()
        };
        out.push_str(&item(first, &mut labels, &notes, style));
        out.push('\n');
        k += 1 + notes.len();
    }
    out
}

/// The packages the output needs, for a document's preamble.
pub const PREAMBLE: &str =
    "\\usepackage{booktabs}\n\\usepackage{graphicx}\n\\usepackage{pgfplots}\n\\usepgfplotslibrary{statistics}\n\\pgfplotsset{compat=1.17}\n";

/// One item as a fragment to paste into a document that loads `PREAMBLE`'s packages.
pub fn item_to_latex(i: &Item, style: &Style) -> String {
    item(i, &mut Labels::default(), &[], style)
}

impl Output {
    /// The whole Output as a LaTeX document with `title` as its heading.
    pub fn to_latex(&self, title: &str) -> String {
        let body = items(&self.items, &self.style);
        // The article class has 10, 11 and 12 point text: the nearest one.
        let size = match self.style.size {
            0..=10 => "10pt",
            11 => "11pt",
            _ => "12pt",
        };
        let family = match self.style.family {
            Family::Default => "",
            Family::Sans => "\\renewcommand{\\familydefault}{\\sfdefault}\n",
            Family::Serif => "\\renewcommand{\\familydefault}{\\rmdefault}\n",
            Family::Mono => "\\renewcommand{\\familydefault}{\\ttdefault}\n",
        };
        format!(
            "% Written by MK Sigma. Compile with pdflatex, lualatex or xelatex.\n\\documentclass[{size}]{{article}}\n\\usepackage[T1]{{fontenc}}\n\\usepackage[margin=2.5cm]{{geometry}}\n\\usepackage{{xcolor}}\n{PREAMBLE}{family}\\begin{{document}}\n\\section*{{{}}}\n\n{body}\\end{{document}}\n",
            escape(title)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chart::BoxStats;

    #[test]
    fn escapes_and_labels() {
        assert_eq!(
            escape("50% of a_b & {c} #1 $2 ~x^2 <3>"),
            "50\\% of a\\_b \\& \\{c\\} \\#1 \\$2 \\textasciitilde{}x\\textasciicircum{}2 \\textless{}3\\textgreater{}"
        );
        let mut labels = Labels::default();
        assert_eq!(labels.next("tab", "Descriptive Statistics"), "tab:descriptive-statistics");
        assert_eq!(labels.next("tab", "Descriptive Statistics"), "tab:descriptive-statistics-2");
        assert_eq!(labels.next("fig", "Média — idade (anos)"), "fig:m-dia-idade-anos");
        assert_eq!(labels.next("fig", "§§"), "fig:item");
    }

    #[test]
    fn a_table_with_booktabs() {
        let t = Table {
            title: "Descriptive Statistics".into(),
            columns: vec!["N".into(), "Mean".into()],
            rows: vec![
                ("Monthly income".into(), vec![Cell::Int(8), Cell::Num { value: 3725.0, decimals: 2 }]),
                ("Valid N".into(), vec![Cell::Int(8)]),
            ],
            categories: Vec::new(),
        };
        let tex = item_to_latex(&Item::Table(t), &Style::default());
        assert!(
            tex.contains(
                "\\begin{tabular}{lrr}\n\\toprule\n & N & Mean \\\\\n\\midrule\nMonthly income & 8 & 3725.00 \\\\\nValid N & 8 &  \\\\\n\\bottomrule"
            ),
            "{tex}"
        );
        assert!(tex.contains("\\caption{Descriptive Statistics}\n\\label{tab:descriptive-statistics}"));
    }

    #[test]
    fn charts_as_pgfplots() {
        let bar = Chart::Bar {
            title: "Grupos".into(),
            x_label: "grupo".into(),
            y_label: "Count".into(),
            bars: vec![("Jovem".into(), 3.0), ("A&B".into(), 5.0)],
            categories: Vec::new(),
        };
        let tex = item_to_latex(&Item::Chart(bar), &Style::default());
        assert!(tex.contains("xtick={1,2}, xticklabels={{Jovem},{A\\&B}}"), "{tex}");
        assert!(tex.contains("coordinates {(1, 3) (2, 5)}"));
        let hist = Chart::Histogram {
            title: "h".into(),
            x_label: "x".into(),
            bins: vec![(0.0, 10.0, 3.0), (10.0, 20.0, 7.0)],
            normal: Some((12.0, 5.0)),
            n: 10.0,
            categories: Vec::new(),
        };
        let tex = item_to_latex(&Item::Chart(hist), &Style::default());
        assert!(tex.contains("coordinates {(0, 3) (10, 7) (20, 0)}"), "the last edge closes the last bin: {tex}");
        assert!(tex.contains("domain=0:20") && tex.contains("100 * exp"), "the curve is scaled by n times the bin width");
        let boxes = Chart::Boxplot {
            title: "b".into(),
            x_label: "g".into(),
            y_label: "y".into(),
            boxes: vec![BoxStats { label: "A".into(), low: 1.0, q1: 2.0, median: 3.0, q3: 4.0, high: 5.0, outliers: vec![9.0], extremes: vec![] }],
            categories: Vec::new(),
        };
        assert!(item_to_latex(&Item::Chart(boxes), &Style::default()).contains("lower whisker=1, lower quartile=2, median=3, upper quartile=4, upper whisker=5}, black, fill=black!20, mark=*, mark size=1.2pt] coordinates {(0, 9)}"));
    }

    #[test]
    fn notes_stay_inside_their_float() {
        let t = Table {
            title: "Chi-Square Tests".into(),
            columns: vec!["Value".into()],
            rows: vec![("Pearson".into(), vec![Cell::Int(8)])],
            categories: Vec::new(),
        };
        let mut out = Output::default();
        out.push(Item::Table(t));
        out.push(Item::Text("a. 4 cells have expected count less than 5.".into()));
        out.push(Item::Warning("w".into()));
        let tex = out.to_latex("x");
        let inside = tex.split("\\end{table}").next().unwrap();
        assert!(inside.contains("a. 4 cells") && inside.contains("Warning: w"), "{tex}");
        assert_eq!(tex.matches("flushleft").count(), 0, "no loose notes");
    }

    #[test]
    fn a_whole_document() {
        let mut out = Output::default();
        out.push(Item::Text("a. 50% note".into()));
        let tex = out.to_latex("Output & more");
        assert!(tex.starts_with("% Written by MK Sigma"));
        assert!(tex.contains("\\usepackage{booktabs}") && tex.contains("\\section*{Output \\& more}") && tex.ends_with("\\end{document}\n"));
        assert!(tex.contains("\\documentclass[10pt]{article}") && !tex.contains("familydefault"), "the default style changes nothing");
        out.style = Style { family: Family::Serif, size: 12, palette: crate::style::Palette::Blue, decimals: Some(1) };
        let tex = out.to_latex("x");
        assert!(tex.contains("\\documentclass[12pt]{article}") && tex.contains("\\renewcommand{\\familydefault}{\\rmdefault}"));
        assert!(tex.contains("a. 50\\% note"));
    }
}
