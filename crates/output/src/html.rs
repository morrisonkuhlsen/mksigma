//! The Output as one self-contained HTML page: tables, notes, warnings and charts (inline SVG),
//! styled for screen (light and dark) and print, with nothing loaded from elsewhere.

use crate::Category;
use crate::chart::{svg, svg_picking};
use crate::style::{Family, Style};
use crate::{Cell, Item, Output, Table};
use mksigma_i18n::tr;

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
svg rect.pick, td.pick { cursor: pointer; }
svg rect.pick:hover { opacity: 0.8; }
svg rect.dim { opacity: 0.3; }
svg rect.sel { stroke: var(--fg); stroke-width: 2; }
td.pick { text-decoration: underline dotted var(--muted); }
td.sel { font-weight: 600; background: var(--stripe); }
@media print { body { max-width: none; } .wrap { overflow: visible; } }
body.dashboard { max-width: 1280px; }
p.filters { color: var(--muted); margin: -12px 0 18px; }
.tiles { display: grid; grid-template-columns: repeat(auto-fit, minmax(min(100%, 420px), 1fr)); gap: 16px; }
.tile { border: 1px solid var(--line); border-radius: 10px; padding: 12px 14px; break-inside: avoid; min-width: 0; }
.tile.wide { grid-column: 1 / -1; }
"#;

pub fn escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn table(t: &Table, style: &Style) -> String {
    table_marked(t, style, false, None)
}

/// A table whose rows that stand for a category can be picked at their first text cell, which
/// carries `data-row` and the `pick` class (`sel` when it is `selected`).
fn table_marked(t: &Table, style: &Style, picking: bool, selected: Option<&Category>) -> String {
    let mut s = format!("<div class=\"wrap\"><table><caption>{}</caption><thead><tr><th></th>", escape(&t.title));
    for c in &t.columns {
        s.push_str(&format!("<th>{}</th>", escape(c)));
    }
    s.push_str("</tr></thead><tbody>");
    for (r, (label, cells)) in t.rows.iter().enumerate() {
        s.push_str(&format!("<tr><th>{}</th>", escape(label)));
        let category = t.category(r).filter(|_| picking);
        let key = cells.iter().position(|c| matches!(c, Cell::Text(_)));
        for (c, cell) in cells.iter().enumerate() {
            let class = match (category, key == Some(c)) {
                (Some(cat), true) => format!(" class=\"pick{}\" data-row=\"{r}\"", if selected == Some(cat) { " sel" } else { "" }),
                _ if matches!(cell, Cell::Text(_) | Cell::Empty) => String::new(),
                _ => " class=\"num\"".to_string(),
            };
            s.push_str(&format!("<td{class}>{}</td>", escape(&style.cell(cell))));
        }
        s.push_str("</tr>");
    }
    s.push_str("</tbody></table></div>");
    s
}

fn item(item: &Item, style: &Style) -> String {
    match item {
        Item::Table(t) => table(t, style),
        Item::Text(s) => format!("<p class=\"note\">{}</p>", escape(s)),
        Item::Warning(s) => format!("<p class=\"warning\">{}</p>", escape(&tr!("warning", text = s.as_str()))),
        Item::Chart(c) => svg(c, 640.0, 400.0),
    }
}

/// CSS for what the style changes; nothing for the default style.
fn style_css(style: &Style) -> String {
    let mut css = String::new();
    let family = match style.family {
        Family::Default => None,
        Family::Sans => Some("\"Helvetica Neue\", Arial, sans-serif"),
        Family::Serif => Some("Georgia, \"Times New Roman\", serif"),
        Family::Mono => Some("ui-monospace, Menlo, Consolas, monospace"),
    };
    if let Some(f) = family {
        css.push_str(&format!("body {{ font-family: {f}; }}\n"));
    }
    if style.size != Style::SIZE {
        css.push_str(&format!("body {{ font-size: {:.1}px; }}\n", f64::from(style.size) * 1.4));
    }
    if let Some(([r, g, b], [cr, cg, cb])) = style.palette.colors() {
        // In both colour schemes: a palette is chosen for the figures, not the page.
        let vars = format!("--bar: rgb({r},{g},{b}); --curve: rgb({cr},{cg},{cb});");
        css.push_str(&format!(":root {{ {vars} }}\n@media (prefers-color-scheme: dark) {{ :root {{ {vars} }} }}\n"));
    }
    css
}

fn page(title: &str, class: &str, body: &str, style: &Style) -> String {
    format!(
        "<!doctype html>\n<html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
         <title>{t}</title><style>{STYLE}{css}</style></head><body class=\"{class}\"><h1>{t}</h1>\n{body}</body></html>\n",
        t = escape(title),
        css = style_css(style)
    )
}

/// The page's CSS: the base sheet and what the style changes.
pub fn css(style: &Style) -> String {
    format!("{STYLE}{}", style_css(style))
}

/// A dashboard tile for `Output::tiles_html`: the engine's id of the item, whether it takes a
/// whole row, and the category to show selected in it (the item a cross-filter came from).
pub struct Tile<'a> {
    pub id: usize,
    pub wide: bool,
    pub selected: Option<&'a Category>,
}

impl Output {
    /// The items as dashboard cards that can be picked (`data-item` on each card, `data-part`
    /// on bars and `data-row` on table values), one per tile, in order.
    pub fn tiles_html(&self, tiles: &[Tile]) -> String {
        let mut out = String::from("<div class=\"tiles\">\n");
        for (i, tile) in self.items.iter().zip(tiles) {
            let class = if tile.wide { "tile wide" } else { "tile" };
            let body = match i {
                Item::Table(t) => table_marked(t, &self.style, true, tile.selected),
                Item::Chart(c) => svg_picking(c, 640.0, 400.0, tile.selected),
                other => item(other, &self.style),
            };
            out.push_str(&format!("<div class=\"{class}\" data-item=\"{}\">{body}</div>\n", tile.id));
        }
        out.push_str("</div>\n");
        out
    }

    /// The whole Output as a standalone HTML page with `title` as its heading.
    pub fn to_html(&self, title: &str) -> String {
        let body: String = self.items.iter().map(|i| format!("<section>{}</section>\n", item(i, &self.style))).collect();
        page(title, "output", &body, &self.style)
    }

    /// The items as a dashboard: a card each, in a grid as many columns wide as the page
    /// allows (a `wide` item, by index, takes a whole row), under `title` and a line naming the
    /// filters, if any.
    pub fn to_dashboard_html(&self, title: &str, filters: Option<&str>, wide: &[bool]) -> String {
        let mut body = filters.map(|f| format!("<p class=\"filters\">{}</p>\n", escape(&tr!("fact-filtered", filters = f)))).unwrap_or_default();
        body.push_str("<div class=\"tiles\">\n");
        for (k, i) in self.items.iter().enumerate() {
            let class = if wide.get(k).copied().unwrap_or(false) { "tile wide" } else { "tile" };
            body.push_str(&format!("<div class=\"{class}\">{}</div>\n", item(i, &self.style)));
        }
        body.push_str("</div>\n");
        page(title, "dashboard", &body, &self.style)
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

    #[test]
    fn a_dashboard_is_a_grid_of_cards_naming_its_filters() {
        let mut out = Output::default();
        out.push(Item::Chart(Chart::Bar {
            title: "Bars".into(),
            x_label: "x".into(),
            y_label: "Count".into(),
            bars: vec![("A".into(), 1.0)],
            categories: Vec::new(),
        }));
        out.push(Item::Text("note".into()));
        let html = out.to_dashboard_html("Painel", Some("grupo: Jovem & co"), &[true]);
        assert!(html.contains("<body class=\"dashboard\"><h1>Painel</h1>"));
        assert!(html.contains("<p class=\"filters\">Filtered by grupo: Jovem &amp; co</p>"), "escaped");
        assert_eq!(html.matches("<div class=\"tile wide\">").count(), 1, "the chart takes a row");
        assert_eq!(html.matches("<div class=\"tile\">").count(), 1);
        assert!(!out.to_dashboard_html("P", None, &[]).contains("filters\">"));
    }
}
