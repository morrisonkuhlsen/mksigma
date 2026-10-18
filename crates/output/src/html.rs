//! The Output as one self-contained HTML page: tables, notes, warnings and charts (inline SVG),
//! styled for screen (light and dark) and print, with nothing loaded from elsewhere.

use crate::chart::svg;
use crate::{Cell, Item, Output, Table};

const STYLE: &str = r#"
:root { --fg: #1d2330; --muted: #5b6475; --line: #d5d9e0; --stripe: #f4f6f9; --bg: #ffffff;
        --bar: #3a6ea5; --curve: #c2410c; --warn: #9a6700; }
@media (prefers-color-scheme: dark) {
  :root { --fg: #e6e9ef; --muted: #a3abba; --line: #3a4150; --stripe: #232834; --bg: #191d26;
          --bar: #6d9fd6; --curve: #f97316; --warn: #e3b341; }
}
body { font: 14px/1.45 system-ui, -apple-system, "Segoe UI", sans-serif; color: var(--fg);
       background: var(--bg); margin: 0 auto; padding: 24px 16px; max-width: 980px; }
h1 { font-size: 20px; margin: 0 0 20px; }
section { margin: 0 0 28px; break-inside: avoid; }
table { border-collapse: collapse; margin: 4px 0; font-variant-numeric: tabular-nums; }
caption { text-align: left; font-weight: 600; padding: 0 0 6px; }
th, td { padding: 3px 10px; border-bottom: 1px solid var(--line); text-align: left; vertical-align: top; }
thead th { border-bottom: 2px solid var(--fg); font-weight: 600; }
tbody tr:nth-child(even) { background: var(--stripe); }
td.num { text-align: right; }
.wrap { overflow-x: auto; }
p.note { color: var(--muted); font-size: 12.5px; margin: 4px 0; }
p.warning { color: var(--warn); }
svg.chart { max-width: 100%; height: auto; }
svg .axis { stroke: var(--fg); stroke-width: 1; }
svg .grid { stroke: var(--line); stroke-width: 1; }
svg .bar { fill: var(--bar); stroke: var(--bg); stroke-width: 1; }
svg .point { fill: var(--bar); fill-opacity: 0.75; }
svg path.curve { fill: none; stroke: var(--curve); stroke-width: 2; }
svg .median { stroke: var(--fg); stroke-width: 2.5; }
svg text { fill: var(--fg); font-family: inherit; }
svg .tick { font-size: 11px; fill: var(--muted); }
svg .label { font-size: 12px; }
svg .title { font-size: 14px; font-weight: 600; }
@media print { body { max-width: none; } .wrap { overflow: visible; } }
"#;

pub(crate) fn escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn table(t: &Table) -> String {
    let mut s = format!("<div class=\"wrap\"><table><caption>{}</caption><thead><tr><th></th>", escape(&t.title));
    for c in &t.columns {
        s.push_str(&format!("<th>{}</th>", escape(c)));
    }
    s.push_str("</tr></thead><tbody>");
    for (label, cells) in &t.rows {
        s.push_str(&format!("<tr><th>{}</th>", escape(label)));
        for cell in cells {
            let class = if matches!(cell, Cell::Text(_) | Cell::Empty) { "" } else { " class=\"num\"" };
            s.push_str(&format!("<td{class}>{}</td>", escape(&cell.render())));
        }
        s.push_str("</tr>");
    }
    s.push_str("</tbody></table></div>");
    s
}

impl Output {
    /// The whole Output as a standalone HTML page with `title` as its heading.
    pub fn to_html(&self, title: &str) -> String {
        let mut body = String::new();
        for item in &self.items {
            body.push_str("<section>");
            body.push_str(&match item {
                Item::Table(t) => table(t),
                Item::Text(s) => format!("<p class=\"note\">{}</p>", escape(s)),
                Item::Warning(s) => format!("<p class=\"warning\">Warning: {}</p>", escape(s)),
                Item::Chart(c) => svg(c, 640.0, 400.0),
            });
            body.push_str("</section>\n");
        }
        format!(
            "<!doctype html>\n<html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
             <title>{t}</title><style>{STYLE}</style></head><body><h1>{t}</h1>\n{body}</body></html>\n",
            t = escape(title)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Chart;

    #[test]
    fn a_page_with_a_table_a_note_and_a_chart() {
        let mut out = Output::default();
        out.push(Item::Table(Table {
            title: "Stats <1>".into(),
            columns: vec!["N".into(), "Label".into()],
            rows: vec![("age".into(), vec![Cell::Int(3), Cell::Text("a & b".into())])],
            categories: Vec::new(),
        }));
        out.push(Item::Text("a. note".into()));
        out.push(Item::Chart(Chart::Bar {
            title: "Bars".into(),
            x_label: "x".into(),
            y_label: "Count".into(),
            bars: vec![("A".into(), 1.0)],
            categories: Vec::new(),
        }));
        let html = out.to_html("Output");
        assert!(html.starts_with("<!doctype html>"));
        assert!(html.contains("<caption>Stats &lt;1&gt;</caption>"), "escaped");
        assert!(html.contains("<td class=\"num\">3</td><td>a &amp; b</td>"), "numbers right, text left");
        assert!(html.contains("<p class=\"note\">a. note</p>"));
        assert!(html.contains("<svg") && html.contains("class=\"bar\""));
        assert!(!html.contains("<link") && !html.contains("<script"), "nothing loaded from elsewhere");
    }

    #[test]
    fn transposing_a_table() {
        let t = Table {
            title: "t".into(),
            columns: vec!["a".into(), "b".into()],
            rows: vec![("r1".into(), vec![Cell::Int(1), Cell::Int(2)]), ("r2".into(), vec![Cell::Int(3)])],
            categories: Vec::new(),
        };
        let p = t.transposed();
        assert_eq!(p.columns, ["r1", "r2"]);
        assert_eq!(p.rows, [("a".to_string(), vec![Cell::Int(1), Cell::Int(3)]), ("b".to_string(), vec![Cell::Int(2), Cell::Empty])]);
        assert_eq!(p.transposed().rows[1].1, [Cell::Int(3), Cell::Empty], "twice gives the cells back (short rows padded)");
    }
}
