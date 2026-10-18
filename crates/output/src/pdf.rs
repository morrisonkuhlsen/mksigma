//! The Output as a PDF: A4 pages with the tables (split across pages, header repeated), notes,
//! warnings and charts (drawn from the same layout as on screen and in HTML), and page numbers.
//!
//! Text uses the standard Helvetica fonts every PDF reader has, so nothing is embedded; text is
//! encoded as Windows-1252 (WinAnsiEncoding), which covers Portuguese and the other Western
//! European languages. Widths come from Helvetica's published metrics for ASCII; other letters
//! take the width of the letter they're built on.

use crate::chart::{self, Anchor, Shape, Stroke, TextSize};
use crate::style::{Family, Style};
use crate::{Cell, Item, Output, Table};

const PAGE: (f64, f64) = (595.28, 841.89);
const MARGIN: f64 = 50.0;
const BOTTOM: f64 = 56.0;

/// Helvetica advance widths (thousandths of an em) for ASCII 32 to 126.
const HELVETICA: [u16; 95] = [
    278, 278, 355, 556, 556, 889, 667, 191, 333, 333, 389, 584, 278, 333, 278, 278, // space to /
    556, 556, 556, 556, 556, 556, 556, 556, 556, 556, // 0 to 9
    278, 278, 584, 584, 584, 556, 1015, // : to @
    667, 667, 722, 722, 667, 611, 778, 722, 278, 500, 667, 556, 833, 722, 778, 667, 778, 722, 667, 611, 722, 667, 944, 667, 667, 611, // A to Z
    278, 278, 278, 469, 556, 333, // [ to `
    556, 556, 500, 556, 556, 278, 556, 556, 222, 222, 500, 222, 833, 556, 556, 556, 556, 333, 500, 278, 556, 500, 722, 500, 500, 500, // a to z
    334, 260, 334, 584, // { to ~
];

/// The ASCII letter an accented letter is built on (for its width).
fn base(c: char) -> char {
    match c {
        'à'..='å' => 'a',
        'À'..='Å' => 'A',
        'ç' => 'c',
        'Ç' => 'C',
        'è'..='ë' => 'e',
        'È'..='Ë' => 'E',
        'ì'..='ï' => 'i',
        'Ì'..='Ï' => 'I',
        'ñ' => 'n',
        'Ñ' => 'N',
        'ò'..='ö' | 'ø' => 'o',
        'Ò'..='Ö' | 'Ø' => 'O',
        'ù'..='ü' => 'u',
        'Ù'..='Ü' => 'U',
        'ý' | 'ÿ' => 'y',
        'Ý' => 'Y',
        '–' => 'n',
        '—' => 'M',
        c => c,
    }
}

/// Width of `text` in points at `size` in `family`'s standard font. Helvetica's metrics are
/// exact (bold about 6% wider); Courier is 600 units a letter; Times runs about 9% narrower
/// than Helvetica, close enough to set columns.
fn width(text: &str, size: f64, bold: bool, family: Family) -> f64 {
    if family == Family::Mono {
        return text.chars().count() as f64 * 0.6 * size;
    }
    let narrower = if family == Family::Serif { 0.91 } else { 1.0 };
    let units: u32 = text
        .chars()
        .map(|c| {
            let c = base(c);
            u32::from(c).checked_sub(32).and_then(|i| HELVETICA.get(i as usize)).map_or(556, |w| u32::from(*w))
        })
        .sum();
    f64::from(units) / 1000.0 * size * if bold { 1.06 } else { 1.0 } * narrower
}

/// The standard fonts of a family, regular and bold: built into every PDF reader.
fn fonts(family: Family) -> (&'static str, &'static str) {
    match family {
        Family::Serif => ("Times-Roman", "Times-Bold"),
        Family::Mono => ("Courier", "Courier-Bold"),
        Family::Default | Family::Sans => ("Helvetica", "Helvetica-Bold"),
    }
}

/// Windows-1252 bytes of `c`, `?` when it has none.
fn win_ansi(c: char) -> u8 {
    match c {
        ' '..='~' => c as u8,
        '\u{a0}'..='\u{ff}' => c as u32 as u8,
        '€' => 0x80,
        '‚' => 0x82,
        '„' => 0x84,
        '…' => 0x85,
        '‘' => 0x91,
        '’' => 0x92,
        '“' => 0x93,
        '”' => 0x94,
        '•' => 0x95,
        '–' => 0x96,
        '—' => 0x97,
        _ => b'?',
    }
}

/// A PDF literal string: `(…)` with `\`, `(` and `)` escaped.
fn literal(text: &str) -> Vec<u8> {
    let mut out = vec![b'('];
    for c in text.chars() {
        let b = win_ansi(c);
        if matches!(b, b'\\' | b'(' | b')') {
            out.push(b'\\');
        }
        out.push(b);
    }
    out.push(b')');
    out
}

/// A text string for the document information (outside page content): UTF-16BE with a byte-order
/// mark, as a hex string, so any character survives.
fn info_string(text: &str) -> String {
    let mut hex = String::from("<FEFF");
    for unit in text.encode_utf16() {
        hex.push_str(&format!("{unit:04X}"));
    }
    hex.push('>');
    hex
}

#[derive(Clone, Copy)]
struct Rgb(f64, f64, f64);

/// How a piece of text is set: size in points, weight and colour.
#[derive(Clone, Copy)]
struct Font {
    size: f64,
    bold: bool,
    color: Rgb,
}

const INK: Rgb = Rgb(0.11, 0.14, 0.19);
const MUTED: Rgb = Rgb(0.36, 0.39, 0.46);
const LINE: Rgb = Rgb(0.82, 0.84, 0.88);
const STRIPE: Rgb = Rgb(0.955, 0.965, 0.976);
const BAR: Rgb = Rgb(0.23, 0.43, 0.65);
const CURVE: Rgb = Rgb(0.76, 0.25, 0.05);
const WARN: Rgb = Rgb(0.6, 0.4, 0.0);

/// Pages under construction: one content stream each, and where the next item goes.
struct Pages {
    style: Style,
    /// Bars and curves: the palette's, or the PDF's own blue and orange.
    bar: Rgb,
    curve: Rgb,
    done: Vec<Vec<u8>>,
    current: Vec<u8>,
    /// Top of the free space, in PDF coordinates (from the bottom of the page).
    y: f64,
}

impl Pages {
    fn new(style: Style) -> Self {
        let rgb = |[r, g, b]: [u8; 3]| Rgb(f64::from(r) / 255.0, f64::from(g) / 255.0, f64::from(b) / 255.0);
        let (bar, curve) = style.palette.colors().map_or((BAR, CURVE), |(bar, curve)| (rgb(bar), rgb(curve)));
        Pages { style, bar, curve, done: Vec::new(), current: Vec::new(), y: PAGE.1 - MARGIN }
    }

    /// `points` at the style's size (sizes here are for 10-point body text).
    fn pt(&self, points: f64) -> f64 {
        points * self.style.scale()
    }

    fn op(&mut self, s: &str) {
        self.current.extend_from_slice(s.as_bytes());
        self.current.push(b'\n');
    }

    fn new_page(&mut self) {
        self.done.push(std::mem::take(&mut self.current));
        self.y = PAGE.1 - MARGIN;
    }

    /// Starts a new page unless `height` fits below the current position.
    fn need(&mut self, height: f64) {
        if self.y - height < BOTTOM && self.y < PAGE.1 - MARGIN - 1.0 {
            self.new_page();
        }
    }

    fn fill(&mut self, c: Rgb) {
        self.op(&format!("{:.3} {:.3} {:.3} rg", c.0, c.1, c.2));
    }

    fn stroke(&mut self, c: Rgb, w: f64) {
        self.op(&format!("{:.3} {:.3} {:.3} RG {w:.2} w", c.0, c.1, c.2));
    }

    fn line(&mut self, (x1, y1): (f64, f64), (x2, y2): (f64, f64), c: Rgb, w: f64) {
        self.stroke(c, w);
        self.op(&format!("{x1:.2} {y1:.2} m {x2:.2} {y2:.2} l S"));
    }

    fn rect(&mut self, x: f64, y: f64, w: f64, h: f64, c: Rgb) {
        self.fill(c);
        self.op(&format!("{x:.2} {y:.2} {w:.2} {h:.2} re f"));
    }

    /// Text with its baseline at `y`; `x` is its left, centre or right per `anchor`.
    fn text(&mut self, x: f64, y: f64, text: &str, font: Font, anchor: Anchor, vertical: bool) {
        let Font { size, bold, color: c } = font;
        let w = width(text, size, bold, self.style.family);
        let shift = match anchor {
            Anchor::Start => 0.0,
            Anchor::Middle => w / 2.0,
            Anchor::End => w,
        };
        self.fill(c);
        let font = if bold { "F2" } else { "F1" };
        let matrix = if vertical { format!("0 1 -1 0 {x:.2} {:.2}", y - shift) } else { format!("1 0 0 1 {:.2} {y:.2}", x - shift) };
        self.current.extend_from_slice(format!("BT /{font} {size:.2} Tf {matrix} Tm ").as_bytes());
        self.current.extend_from_slice(&literal(text));
        self.current.extend_from_slice(b" Tj ET\n");
    }

    /// Words wrapped to `max` points wide, one line after another.
    fn paragraph(&mut self, text: &str, size: f64, c: Rgb, max: f64) {
        let mut line = String::new();
        let mut lines = Vec::new();
        for word in text.split_whitespace() {
            let candidate = if line.is_empty() { word.to_string() } else { format!("{line} {word}") };
            if width(&candidate, size, false, self.style.family) > max && !line.is_empty() {
                lines.push(std::mem::replace(&mut line, word.to_string()));
            } else {
                line = candidate;
            }
        }
        lines.push(line);
        for l in lines {
            self.need(size * 1.5);
            self.y -= size * 1.3;
            self.text(MARGIN, self.y, &l, Font { size, bold: false, color: c }, Anchor::Start, false);
        }
        self.y -= size * 0.6;
    }

    fn table(&mut self, t: &Table) {
        let content = PAGE.0 - 2.0 * MARGIN;
        let header: Vec<String> = std::iter::once(String::new()).chain(t.columns.iter().cloned()).collect();
        let body: Vec<Vec<String>> =
            t.rows.iter().map(|(label, cells)| std::iter::once(label.clone()).chain(cells.iter().map(|c| self.style.cell(c))).collect()).collect();
        let family = self.style.family;
        let numeric: Vec<bool> = std::iter::once(false)
            .chain(
                (0..t.columns.len())
                    .map(|c| t.rows.iter().filter_map(|(_, cells)| cells.get(c)).any(|c| matches!(c, Cell::Num { .. } | Cell::Int(_)))),
            )
            .collect();
        // A column is as wide as its widest cell and its header's longest word: headers wrap.
        // The size is the largest from 8.5 down to 6 points at which the table fits the page.
        let widths_at = |size: f64| -> Vec<f64> {
            (0..header.len())
                .map(|c| {
                    let longest_word = header.get(c).map_or(0.0, |h| h.split_whitespace().map(|w| width(w, size, true, family)).fold(0.0, f64::max));
                    let cells = body.iter().filter_map(|r| r.get(c)).map(|s| width(s, size, c == 0, family));
                    cells.fold(longest_word, f64::max) + 10.0
                })
                .collect()
        };
        let mut size = self.pt(8.5);
        let mut widths = widths_at(size);
        let smallest = self.pt(6.0).min(size);
        while widths.iter().sum::<f64>() > content && size > smallest {
            size -= 0.5;
            widths = widths_at(size);
        }
        let row_h = size * 1.75;
        // Header text broken into lines that fit each column.
        let header_lines: Vec<Vec<String>> = header
            .iter()
            .zip(&widths)
            .map(|(h, w)| {
                let mut lines: Vec<String> = Vec::new();
                for word in h.split_whitespace() {
                    match lines.last_mut() {
                        Some(line) if width(&format!("{line} {word}"), size, true, family) <= w - 10.0 => {
                            line.push(' ');
                            line.push_str(word);
                        }
                        _ => lines.push(word.to_string()),
                    }
                }
                lines
            })
            .collect();
        let header_rows = header_lines.iter().map(Vec::len).max().unwrap_or(1).max(1);
        let line_h = size * 1.2;
        let header_h = row_h + (header_rows - 1) as f64 * line_h;
        let draw_header = |pages: &mut Pages| {
            let mut x = MARGIN;
            for (c, w) in widths.iter().enumerate() {
                let lines = header_lines.get(c).map_or(&[][..], Vec::as_slice);
                // Bottom-aligned, so the last header lines share a baseline.
                let first = pages.y - header_h + size * 0.55 + (lines.len().max(1) - 1) as f64 * line_h;
                let right = numeric.get(c).copied().unwrap_or(false);
                for (k, line) in lines.iter().enumerate() {
                    let y = first - k as f64 * line_h;
                    if right {
                        pages.text(x + w - 5.0, y, line, Font { size, bold: true, color: INK }, Anchor::End, false);
                    } else {
                        pages.text(x + 5.0, y, line, Font { size, bold: true, color: INK }, Anchor::Start, false);
                    }
                }
                x += w;
            }
            pages.y -= header_h;
        };
        let draw_row = |pages: &mut Pages, cells: &[String], bold: bool, stripe: bool| {
            let total: f64 = widths.iter().sum();
            if stripe {
                pages.rect(MARGIN, pages.y - row_h, total, row_h, STRIPE);
            }
            let mut x = MARGIN;
            let baseline = pages.y - row_h + size * 0.55;
            for (c, w) in widths.iter().enumerate() {
                let text = cells.get(c).map_or("", String::as_str);
                let right = numeric.get(c).copied().unwrap_or(false) && !bold;
                if right {
                    pages.text(x + w - 5.0, baseline, text, Font { size, bold: false, color: INK }, Anchor::End, false);
                } else {
                    pages.text(x + 5.0, baseline, text, Font { size, bold: bold || c == 0, color: INK }, Anchor::Start, false);
                }
                x += w;
            }
            pages.y -= row_h;
        };
        let total: f64 = widths.iter().sum();
        let caption = |pages: &mut Pages, more: bool| {
            pages.y -= 12.0;
            let title = if more { format!("{} (continued)", t.title) } else { t.title.clone() };
            let size = pages.pt(10.0);
            pages.text(MARGIN, pages.y, &title, Font { size, bold: true, color: INK }, Anchor::Start, false);
            pages.y -= 4.0;
        };
        let head = |pages: &mut Pages| {
            draw_header(pages);
            pages.line((MARGIN, pages.y), (MARGIN + total, pages.y), INK, 1.0);
        };
        self.need(16.0 + header_h + row_h * 2.0);
        caption(self, false);
        head(self);
        for (i, row) in body.iter().enumerate() {
            if self.y - row_h < BOTTOM {
                self.new_page();
                caption(self, true);
                head(self);
            }
            draw_row(self, row, false, i % 2 == 1);
            self.line((MARGIN, self.y), (MARGIN + total, self.y), LINE, 0.5);
        }
        self.y -= 14.0;
    }

    fn chart(&mut self, c: &crate::Chart) {
        let (w, h) = (PAGE.0 - 2.0 * MARGIN, (PAGE.0 - 2.0 * MARGIN) * 0.62);
        self.need(h + 10.0);
        let top = self.y;
        let at = |x: f64, y: f64| (MARGIN + x, top - y);
        for shape in chart::layout(c, w, h) {
            match shape {
                Shape::Line { from, to, stroke } => {
                    let (color, width) = match stroke {
                        Stroke::Axis => (INK, 0.8),
                        Stroke::Grid => (LINE, 0.5),
                        Stroke::Curve => (self.curve, 1.5),
                        Stroke::Median => (INK, 2.0),
                    };
                    self.line(at(from.0, from.1), at(to.0, to.1), color, width);
                }
                Shape::Rect { x, y, width, height, .. } => {
                    let (px, py) = at(x, y + height);
                    self.rect(px, py, width, height, self.bar);
                }
                Shape::Circle { x, y, radius } => {
                    // Four Bézier arcs; k is the usual 0.5523 control-point factor.
                    let (cx, cy) = at(x, y);
                    let (r, k) = (radius, radius * 0.5523);
                    self.fill(self.bar);
                    self.op(&format!(
                        "{:.2} {cy:.2} m {:.2} {:.2} {:.2} {:.2} {cx:.2} {:.2} c {:.2} {:.2} {:.2} {:.2} {:.2} {cy:.2} c {:.2} {:.2} {:.2} {:.2} {cx:.2} {:.2} c {:.2} {:.2} {:.2} {:.2} {:.2} {cy:.2} c f",
                        cx + r,
                        cx + r,
                        cy + k,
                        cx + k,
                        cy + r,
                        cy + r,
                        cx - k,
                        cy + r,
                        cx - r,
                        cy + k,
                        cx - r,
                        cx - r,
                        cy - k,
                        cx - k,
                        cy - r,
                        cy - r,
                        cx + k,
                        cy - r,
                        cx + r,
                        cy - k,
                        cx + r,
                    ));
                }
                Shape::Path { points } => {
                    self.stroke(self.curve, 1.5);
                    let ops: Vec<String> = points
                        .iter()
                        .enumerate()
                        .map(|(i, (x, y))| {
                            let (px, py) = at(*x, *y);
                            format!("{px:.2} {py:.2} {}", if i == 0 { "m" } else { "l" })
                        })
                        .collect();
                    self.op(&format!("{} S", ops.join(" ")));
                }
                Shape::Text { x, y, text, anchor, size, vertical } => {
                    let (px, py) = at(x, y);
                    let (pt, bold, color) = match size {
                        TextSize::Small => (8.0, false, MUTED),
                        TextSize::Normal => (9.0, false, INK),
                        TextSize::Title => (11.0, true, INK),
                    };
                    self.text(px, py, &text, Font { size: pt, bold, color }, anchor, vertical);
                }
            }
        }
        self.y = top - h - 16.0;
    }
}

/// Assembles objects into a PDF file with a cross-reference table.
fn assemble(pages: Vec<Vec<u8>>, title: &str, family: Family) -> Vec<u8> {
    let (regular, bold) = fonts(family);
    let mut objects: Vec<Vec<u8>> = Vec::new();
    let n = pages.len();
    // 1 catalog, 2 page tree, 3 and 4 fonts, 5 info, then a page and its contents per page.
    let page_ids: Vec<usize> = (0..n).map(|i| 6 + 2 * i).collect();
    objects.push(b"<< /Type /Catalog /Pages 2 0 R >>".to_vec());
    let kids: Vec<String> = page_ids.iter().map(|id| format!("{id} 0 R")).collect();
    objects.push(format!("<< /Type /Pages /Kids [{}] /Count {n} >>", kids.join(" ")).into_bytes());
    objects.push(format!("<< /Type /Font /Subtype /Type1 /BaseFont /{regular} /Encoding /WinAnsiEncoding >>").into_bytes());
    objects.push(format!("<< /Type /Font /Subtype /Type1 /BaseFont /{bold} /Encoding /WinAnsiEncoding >>").into_bytes());
    objects.push(format!("<< /Producer (MK Sigma) /Title {} >>", info_string(title)).into_bytes());
    for (i, content) in pages.into_iter().enumerate() {
        let contents_id = page_ids.get(i).map_or(0, |id| id + 1);
        objects.push(
            format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {} {}] /Resources << /Font << /F1 3 0 R /F2 4 0 R >> >> /Contents {contents_id} 0 R >>",
                PAGE.0, PAGE.1
            )
            .into_bytes(),
        );
        let mut stream = format!("<< /Length {} >>\nstream\n", content.len()).into_bytes();
        stream.extend(content);
        stream.extend_from_slice(b"\nendstream");
        objects.push(stream);
    }
    let mut out = b"%PDF-1.4\n%\xe2\xe3\xcf\xd3\n".to_vec();
    let mut offsets = Vec::with_capacity(objects.len());
    for (i, body) in objects.iter().enumerate() {
        offsets.push(out.len());
        out.extend(format!("{} 0 obj\n", i + 1).into_bytes());
        out.extend_from_slice(body);
        out.extend_from_slice(b"\nendobj\n");
    }
    let xref = out.len();
    out.extend(format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).into_bytes());
    for o in offsets {
        out.extend(format!("{o:010} 00000 n \n").into_bytes());
    }
    out.extend(format!("trailer\n<< /Size {} /Root 1 0 R /Info 5 0 R >>\nstartxref\n{xref}\n%%EOF\n", objects.len() + 1).into_bytes());
    out
}

impl Output {
    /// The whole Output as a PDF document with `title` as its heading.
    pub fn to_pdf(&self, title: &str) -> Vec<u8> {
        let mut pages = Pages::new(self.style);
        let content = PAGE.0 - 2.0 * MARGIN;
        pages.y -= 14.0;
        let size = pages.pt(14.0);
        pages.text(MARGIN, pages.y, title, Font { size, bold: true, color: INK }, Anchor::Start, false);
        pages.y -= 18.0;
        for item in &self.items {
            match item {
                Item::Table(t) => pages.table(t),
                Item::Text(s) => {
                    let size = pages.pt(8.0);
                    pages.paragraph(s, size, MUTED, content);
                }
                Item::Warning(s) => {
                    let size = pages.pt(8.5);
                    pages.paragraph(&format!("Warning: {s}"), size, WARN, content);
                }
                Item::Chart(c) => pages.chart(c),
            }
        }
        pages.new_page();
        // Page numbers, now that the count is known.
        let total = pages.done.len();
        let mut done = std::mem::take(&mut pages.done);
        for (i, content) in done.iter_mut().enumerate() {
            let mut footer = Pages::new(self.style);
            footer.text(
                PAGE.0 - MARGIN,
                30.0,
                &format!("Page {} of {total}", i + 1),
                Font { size: 8.0, bold: false, color: MUTED },
                Anchor::End,
                false,
            );
            content.extend(footer.current);
        }
        assemble(done, title, self.style.family)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Chart;

    fn sample(rows: usize) -> Output {
        let mut out = Output::default();
        out.push(Item::Table(Table {
            title: "Satisfação (n)".into(),
            columns: vec!["N".into(), "Média".into()],
            rows: (0..rows).map(|i| (format!("Grupo {i}"), vec![Cell::Int(i as i64), Cell::Num { value: 1.5, decimals: 2 }])).collect(),
            categories: Vec::new(),
        }));
        out.push(Item::Text("a. Nota (com parênteses) e \\ barra.".into()));
        out.push(Item::Chart(Chart::Bar {
            title: "Barras".into(),
            x_label: "x".into(),
            y_label: "N".into(),
            bars: vec![("A".into(), 2.0)],
            categories: Vec::new(),
        }));
        out
    }

    /// Every object the cross-reference table lists starts where it says. Works on bytes: the
    /// file isn't UTF-8, and offsets are byte offsets.
    fn check_xref(pdf: &[u8]) -> usize {
        let marker = b"startxref\n";
        let at = pdf.windows(marker.len()).rposition(|w| w == marker).unwrap() + marker.len();
        let digits: String = pdf[at..].iter().take_while(|b| b.is_ascii_digit()).map(|b| *b as char).collect();
        let start: usize = digits.parse().unwrap();
        // The table and trailer are plain ASCII.
        let xref = std::str::from_utf8(&pdf[start..]).unwrap();
        let mut lines = xref.lines().skip(1);
        let count: usize = lines.next().unwrap().split(' ').nth(1).unwrap().parse().unwrap();
        for (id, line) in lines.skip(1).take(count - 1).enumerate() {
            let offset: usize = line[..10].parse().unwrap();
            assert!(pdf[offset..].starts_with(format!("{} 0 obj", id + 1).as_bytes()), "object {} at {offset}", id + 1);
        }
        count - 1
    }

    #[test]
    fn a_valid_document_with_latin_text() {
        let pdf = sample(3).to_pdf("Relatório");
        assert!(pdf.starts_with(b"%PDF-1.4"));
        assert!(pdf.ends_with(b"%%EOF\n"));
        assert_eq!(check_xref(&pdf), 7, "catalog, pages, 2 fonts, info, 1 page + contents");
        let has = |needle: &[u8]| pdf.windows(needle.len()).any(|w| w == needle);
        assert!(has(b"(Satisfa\xe7\xe3o \\(n\\))"), "Windows-1252 bytes, parentheses escaped");
        assert!(has(b"(Page 1 of 1)"));
        assert!(has(b"/Title <FEFF00520065006C0061007400F300720069006F>"), "the title in UTF-16");
    }

    #[test]
    fn long_tables_continue_on_new_pages() {
        let pdf = sample(120).to_pdf("Long");
        let pages = String::from_utf8_lossy(&pdf).matches("/Type /Page /Parent").count();
        assert!(pages >= 3, "{pages} pages");
        assert_eq!(check_xref(&pdf), 5 + 2 * pages);
        let has = |needle: &[u8]| pdf.windows(needle.len()).any(|w| w == needle);
        assert!(has(b"\\(continued\\)"), "the title and header repeat on the next page (parentheses escaped)");
    }

    #[test]
    fn widths_follow_helvetica() {
        assert!((width("0123456789", 10.0, false, Family::Sans) - 55.6).abs() < 1e-9, "every digit is 556 units");
        assert_eq!(width("ção", 10.0, false, Family::Default), width("cao", 10.0, false, Family::Default));
        assert!((width("abc", 10.0, true, Family::Mono) - 18.0).abs() < 1e-9, "Courier: 600 units a letter");
        assert_eq!(win_ansi('ç'), 0xe7);
        assert_eq!(win_ansi('–'), 0x96);
        assert_eq!(win_ansi('中'), b'?');
    }
}
