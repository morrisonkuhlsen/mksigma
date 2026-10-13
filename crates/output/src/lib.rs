//! MK Sigma output model: what procedures produce and the Output Viewer shows. Tables are plain
//! data here; rendering (egui, text, later HTML/PDF/.spv-like export) lives in the frontends.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

pub mod chart;
mod html;
mod pdf;

pub use chart::Chart;

#[derive(Debug, Clone, PartialEq)]
pub enum Cell {
    Num { value: f64, decimals: u8 },
    Int(i64),
    Text(String),
    Empty,
}

impl Cell {
    pub fn render(&self) -> String {
        match self {
            Cell::Num { value, decimals } => format!("{value:.*}", usize::from(*decimals)),
            Cell::Int(n) => n.to_string(),
            Cell::Text(s) => s.clone(),
            Cell::Empty => String::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Table {
    pub title: String,
    pub columns: Vec<String>,
    /// Each row is a label (the stub column) and one cell per column.
    pub rows: Vec<(String, Vec<Cell>)>,
}

impl Table {
    /// The pivot of rows and columns: row labels become column headers and the other way round.
    pub fn transposed(&self) -> Table {
        let columns = self.rows.iter().map(|(label, _)| label.clone()).collect();
        let rows = self
            .columns
            .iter()
            .enumerate()
            .map(|(c, name)| (name.clone(), self.rows.iter().map(|(_, cells)| cells.get(c).cloned().unwrap_or(Cell::Empty)).collect()))
            .collect();
        Table { title: self.title.clone(), columns, rows }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Item {
    Table(Table),
    Text(String),
    Warning(String),
    Chart(Chart),
}

/// Everything a session has produced, in order.
#[derive(Debug, Clone, Default)]
pub struct Output {
    pub items: Vec<Item>,
}

impl Output {
    pub fn push(&mut self, item: Item) {
        self.items.push(item);
    }

    /// Plain-text rendering, used by the CLI and in tests.
    pub fn to_text(&self) -> String {
        let mut out = String::new();
        for item in &self.items {
            match item {
                Item::Table(t) => out.push_str(&table_to_text(t)),
                Item::Text(s) => out.push_str(s),
                Item::Warning(s) => {
                    out.push_str("Warning: ");
                    out.push_str(s);
                }
                Item::Chart(c) => {
                    out.push_str("[Chart: ");
                    out.push_str(c.title());
                    out.push(']');
                }
            }
            out.push_str("\n\n");
        }
        out
    }
}

fn table_to_text(t: &Table) -> String {
    let body: Vec<Vec<String>> =
        t.rows.iter().map(|(label, cells)| std::iter::once(label.clone()).chain(cells.iter().map(Cell::render)).collect()).collect();
    let header: Vec<String> = std::iter::once(String::new()).chain(t.columns.iter().cloned()).collect();
    let mut widths: Vec<usize> = header.iter().map(|h| h.chars().count()).collect();
    for row in &body {
        for (w, cell) in widths.iter_mut().zip(row) {
            *w = (*w).max(cell.chars().count());
        }
    }
    // Text columns align left, number columns right; the stub column holds labels.
    let right: Vec<bool> = std::iter::once(false)
        .chain(
            (0..t.columns.len())
                .map(|col| t.rows.iter().filter_map(|(_, cells)| cells.get(col)).any(|c| matches!(c, Cell::Num { .. } | Cell::Int(_)))),
        )
        .collect();
    let line = |row: &[String]| -> String {
        let cells: Vec<String> =
            row.iter().zip(&widths).zip(&right).map(|((c, w), right)| if *right { format!("{c:>w$}") } else { format!("{c:<w$}") }).collect();
        cells.join("  ").trim_end().to_string()
    };
    let mut out = format!("{}\n{}\n", t.title, line(&header));
    for row in &body {
        out.push_str(&line(row));
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_a_table_as_aligned_text() {
        let mut out = Output::default();
        out.push(Item::Table(Table {
            title: "Descriptive Statistics".into(),
            columns: vec!["N".into(), "Mean".into()],
            rows: vec![("age".into(), vec![Cell::Int(3), Cell::Num { value: 30.0, decimals: 2 }])],
        }));
        assert_eq!(out.to_text(), "Descriptive Statistics\n     N   Mean\nage  3  30.00\n\n\n");
    }
}
