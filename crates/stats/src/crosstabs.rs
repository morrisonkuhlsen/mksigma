//! `CROSSTABS`: a Case Processing Summary, one crosstabulation per row × column variable pair,
//! and with `/STATISTICS=CHISQ` the Chi-Square Tests. Cases missing on either variable are left
//! out of that table. Weighted cell counts are rounded to whole numbers, as SPSS does by default.

use mksigma_core::{Dataset, Value, VarType, Variable};
use mksigma_i18n::tr;
use mksigma_output::{Cell, Item, Table};

use crate::groups::valid_value;
use crate::{StatsError, dist, n_cell, num, value_text};

/// What each crosstabulation cell shows (`/CELLS`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cells {
    pub count: bool,
    pub expected: bool,
    pub row: bool,
    pub column: bool,
    pub total: bool,
}

impl Default for Cells {
    fn default() -> Self {
        Self { count: true, expected: false, row: false, column: false, total: false }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Options {
    pub rows: Vec<String>,
    pub columns: Vec<String>,
    pub cells: Cells,
    pub chisq: bool,
}

/// Counts of one row × column table over the cases valid on both.
pub(crate) struct Counts {
    pub(crate) rows: Vec<Value>,
    pub(crate) columns: Vec<Value>,
    /// `cells[r][c]`, rounded when weighted.
    cells: Vec<Vec<f64>>,
    /// Weighted cases with a missing value on either variable.
    missing: f64,
}

impl Counts {
    pub(crate) fn row_total(&self, r: usize) -> f64 {
        self.cells.get(r).map_or(0.0, |row| row.iter().sum())
    }
    pub(crate) fn column_total(&self, c: usize) -> f64 {
        self.cells.iter().filter_map(|row| row.get(c)).sum()
    }
    pub(crate) fn total(&self) -> f64 {
        self.cells.iter().flatten().sum()
    }
    pub(crate) fn expected(&self, r: usize, c: usize) -> f64 {
        let n = self.total();
        if n > 0.0 { self.row_total(r) * self.column_total(c) / n } else { 0.0 }
    }
    pub(crate) fn get(&self, r: usize, c: usize) -> f64 {
        self.cells.get(r).and_then(|row| row.get(c)).copied().unwrap_or(0.0)
    }
}

fn compare(a: &Value, b: &Value) -> std::cmp::Ordering {
    match (a, b) {
        (Value::Num(x), Value::Num(y)) => x.total_cmp(y),
        (Value::Str(x), Value::Str(y)) => x.cmp(y),
        _ => std::cmp::Ordering::Equal,
    }
}

pub(crate) fn count(ds: &Dataset, row: usize, column: usize) -> Counts {
    let mut pairs: Vec<(Value, Value, f64)> = Vec::new();
    let mut missing = 0.0;
    for case in 0..ds.n_cases() {
        let w = ds.case_weight(case);
        if w <= 0.0 {
            continue;
        }
        match (valid_value(ds, row, case), valid_value(ds, column, case)) {
            (Some(r), Some(c)) => pairs.push((r.clone(), c.clone(), w)),
            _ => missing += w,
        }
    }
    let mut rows: Vec<Value> = Vec::new();
    let mut columns: Vec<Value> = Vec::new();
    for (r, c, _) in &pairs {
        if !rows.contains(r) {
            rows.push(r.clone());
        }
        if !columns.contains(c) {
            columns.push(c.clone());
        }
    }
    rows.sort_by(compare);
    columns.sort_by(compare);
    let mut cells = vec![vec![0.0; columns.len()]; rows.len()];
    for (r, c, w) in &pairs {
        let (Some(ri), Some(ci)) = (rows.iter().position(|v| v == r), columns.iter().position(|v| v == c)) else { continue };
        if let Some(cell) = cells.get_mut(ri).and_then(|row| row.get_mut(ci)) {
            *cell += w;
        }
    }
    for cell in cells.iter_mut().flatten() {
        *cell = cell.round();
    }
    Counts { rows, columns, cells, missing }
}

fn pct(part: f64, whole: f64) -> Cell {
    num((whole > 0.0).then(|| part / whole * 100.0), 1)
}

/// One line of statistics in a crosstabulation block.
#[derive(Debug, Clone, Copy)]
enum Line {
    Count,
    Expected,
    RowPercent,
    ColumnPercent,
    TotalPercent,
}

impl Line {
    fn requested(cells: Cells) -> Vec<Line> {
        [
            (cells.count, Line::Count),
            (cells.expected, Line::Expected),
            (cells.row, Line::RowPercent),
            (cells.column, Line::ColumnPercent),
            (cells.total, Line::TotalPercent),
        ]
        .into_iter()
        .filter_map(|(on, line)| on.then_some(line))
        .collect()
    }

    fn name(self) -> String {
        match self {
            Line::Count => tr!("o-count"),
            Line::Expected => tr!("o-expected-count"),
            Line::RowPercent => tr!("o-within-row"),
            Line::ColumnPercent => tr!("o-within-column"),
            Line::TotalPercent => tr!("o-of-total"),
        }
    }
}

/// What one cell of the table knows: its count, expected count, and the totals of its row,
/// its column and the table.
#[derive(Debug, Clone, Copy)]
struct Margins {
    count: f64,
    expected: f64,
    row: f64,
    column: f64,
    total: f64,
}

impl Margins {
    fn cell(self, line: Line) -> Cell {
        match line {
            Line::Count => n_cell(self.count),
            Line::Expected => num(Some(self.expected), 1),
            Line::RowPercent => pct(self.count, self.row),
            Line::ColumnPercent => pct(self.count, self.column),
            Line::TotalPercent => pct(self.count, self.total),
        }
    }
}

fn table(row_var: &Variable, column_var: &Variable, counts: &Counts, cells: Cells) -> Table {
    let n = counts.total();
    let mut columns = vec![String::new()];
    columns.extend(counts.columns.iter().map(|v| value_text(column_var, v)));
    columns.push(tr!("o-total").to_string());
    let lines = Line::requested(cells);
    let column_totals: Vec<f64> = (0..counts.columns.len()).map(|c| counts.column_total(c)).collect();

    // One block of lines per row category, then the Total block; each column, then Total.
    let mut blocks: Vec<(String, Vec<Margins>)> = counts
        .rows
        .iter()
        .enumerate()
        .map(|(r, value)| {
            let row = counts.row_total(r);
            let mut margins: Vec<Margins> = column_totals
                .iter()
                .enumerate()
                .map(|(c, &column)| Margins { count: counts.get(r, c), expected: counts.expected(r, c), row, column, total: n })
                .collect();
            margins.push(Margins { count: row, expected: row, row, column: n, total: n });
            (value_text(row_var, value), margins)
        })
        .collect();
    let mut totals: Vec<Margins> =
        column_totals.iter().map(|&column| Margins { count: column, expected: column, row: n, column, total: n }).collect();
    totals.push(Margins { count: n, expected: n, row: n, column: n, total: n });
    blocks.push((tr!("o-total").to_string(), totals));

    let mut rows = Vec::with_capacity(blocks.len() * lines.len());
    for (label, margins) in blocks {
        for (i, line) in lines.iter().enumerate() {
            let mut out = vec![Cell::Text(line.name().to_string())];
            out.extend(margins.iter().map(|m| m.cell(*line)));
            rows.push((if i == 0 { label.clone() } else { String::new() }, out));
        }
    }
    Table {
        title: tr!("o-crosstabulation", rows = row_var.display_name(), columns = column_var.display_name()),
        columns,
        rows,
        categories: Vec::new(),
    }
}

/// Pearson correlation of the row and column scores, weighted by the cell counts.
fn score_correlation(counts: &Counts) -> Option<f64> {
    let scores = |values: &[Value]| values.iter().map(Value::as_num).collect::<Option<Vec<f64>>>();
    let (rs, cs) = (scores(&counts.rows)?, scores(&counts.columns)?);
    let n = counts.total();
    let (mut sr, mut sc) = (0.0, 0.0);
    for (r, x) in rs.iter().enumerate() {
        for (c, y) in cs.iter().enumerate() {
            let w = counts.get(r, c);
            sr += w * x;
            sc += w * y;
        }
    }
    let (mr, mc) = (sr / n, sc / n);
    let (mut sxy, mut sxx, mut syy) = (0.0, 0.0, 0.0);
    for (r, x) in rs.iter().enumerate() {
        for (c, y) in cs.iter().enumerate() {
            let w = counts.get(r, c);
            sxy += w * (x - mr) * (y - mc);
            sxx += w * (x - mr).powi(2);
            syy += w * (y - mc).powi(2);
        }
    }
    let r = sxy / (sxx * syy).sqrt();
    r.is_finite().then_some(r)
}

fn chi_square(row_var: &Variable, column_var: &Variable, counts: &Counts) -> Vec<Item> {
    let (nr, nc) = (counts.rows.len(), counts.columns.len());
    let n = counts.total();
    if nr < 2 || nc < 2 {
        let constant = if nr < 2 { row_var } else { column_var };
        return vec![Item::Text(tr!("o-constant-no-statistics", name = constant.name.as_str()))];
    }
    let df = ((nr - 1) * (nc - 1)) as f64;
    let (mut pearson, mut lr, mut small, mut min_expected) = (0.0, 0.0, 0, f64::INFINITY);
    for r in 0..nr {
        for c in 0..nc {
            let (o, e) = (counts.get(r, c), counts.expected(r, c));
            pearson += (o - e).powi(2) / e;
            if o > 0.0 {
                lr += 2.0 * o * (o / e).ln();
            }
            if e < 5.0 {
                small += 1;
            }
            min_expected = min_expected.min(e);
        }
    }
    let sig = |x: f64, df: f64| num(dist::chi_square_sig(x, df), 3);
    let row = |name: String, value: f64, df: f64, extra: [Cell; 2]| {
        let [two, one] = extra;
        (name, vec![num(Some(value), 3), Cell::Int(df as i64), sig(value, df), two, one])
    };
    let mut rows = vec![row(tr!("o-pearson-chi-square"), pearson, df, [Cell::Empty, Cell::Empty])];
    let two_by_two = nr == 2 && nc == 2;
    if two_by_two {
        let (a, b, c, d) = (counts.get(0, 0), counts.get(0, 1), counts.get(1, 0), counts.get(1, 1));
        let (r1, r2, c1, c2) = (a + b, c + d, a + c, b + d);
        let yates = n * ((a * d - b * c).abs() - n / 2.0).max(0.0).powi(2) / (r1 * r2 * c1 * c2);
        rows.push(row(tr!("o-continuity-correction"), yates, 1.0, [Cell::Empty, Cell::Empty]));
        rows.push(row(tr!("o-likelihood-ratio"), lr, df, [Cell::Empty, Cell::Empty]));
        let (two, one) = dist::fisher_exact(a, b, c, d);
        rows.push((tr!("o-fisher-s-exact-test").to_string(), vec![Cell::Empty, Cell::Empty, Cell::Empty, num(Some(two), 3), num(Some(one), 3)]));
    } else {
        rows.push(row(tr!("o-likelihood-ratio"), lr, df, [Cell::Empty, Cell::Empty]));
    }
    if row_var.var_type == VarType::Numeric
        && column_var.var_type == VarType::Numeric
        && let Some(r) = score_correlation(counts)
    {
        rows.push(row(tr!("o-linear-by-linear-association"), (n - 1.0) * r * r, 1.0, [Cell::Empty, Cell::Empty]));
    }
    rows.push((tr!("o-n-of-valid-cases").to_string(), vec![n_cell(n), Cell::Empty, Cell::Empty, Cell::Empty, Cell::Empty]));
    let columns = [tr!("o-value"), tr!("o-df"), tr!("o-asymptotic-significance-2-sided"), tr!("o-exact-sig-2-sided"), tr!("o-exact-sig-1-sided")]
        .map(String::from)
        .to_vec();
    let total_cells = nr * nc;
    let note = tr!(
        "o-expected-note",
        cells = small,
        percent = format!("{:.1}", small as f64 / total_cells as f64 * 100.0),
        minimum = format!("{min_expected:.2}")
    );
    let mut items =
        vec![Item::Table(Table { title: tr!("o-chi-square-tests").to_string(), columns, rows, categories: Vec::new() }), Item::Text(note)];
    if two_by_two {
        items.push(Item::Text(tr!("o-b-continuity-correction-is-computed-only-for-a-2x2-table").to_string()));
    }
    items
}

pub fn crosstabs(ds: &Dataset, options: &Options) -> Result<Vec<Item>, StatsError> {
    let lookup = |name: &String| ds.variable(name).ok_or_else(|| StatsError::UnknownVariable(name.clone()));
    let rows = options.rows.iter().map(lookup).collect::<Result<Vec<_>, _>>()?;
    let columns = options.columns.iter().map(lookup).collect::<Result<Vec<_>, _>>()?;
    let mut summary = Vec::new();
    let mut tables = Vec::new();
    for (ri, row_var) in &rows {
        for (ci, column_var) in &columns {
            let counts = count(ds, *ri, *ci);
            let (valid, missing) = (counts.total(), counts.missing.round());
            let total = valid + missing;
            summary.push((
                format!("{} * {}", row_var.display_name(), column_var.display_name()),
                vec![n_cell(valid), pct(valid, total), n_cell(missing), pct(missing, total), n_cell(total), pct(total, total)],
            ));
            tables.push(Item::Table(table(row_var, column_var, &counts, options.cells)));
            if options.chisq {
                tables.extend(chi_square(row_var, column_var, &counts));
            }
        }
    }
    let columns = [tr!("o-valid-n"), tr!("o-valid-percent"), tr!("o-missing-n"), tr!("o-missing-percent"), tr!("o-total-n"), tr!("o-total-percent")]
        .map(String::from)
        .to_vec();
    let mut items = vec![Item::Table(Table { title: tr!("o-case-processing-summary").to_string(), columns, rows: summary, categories: Vec::new() })];
    items.extend(tables);
    Ok(items)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Treatment (1 = drug, 2 = placebo) by outcome (1 = improved, 2 = not): 12/3 and 5/10.
    fn trial() -> Dataset {
        let mut ds = Dataset::new();
        let mut t = Variable::numeric("treatment");
        t.value_labels = vec![(Value::Num(1.0), "Drug".into()), (Value::Num(2.0), "Placebo".into())];
        ds.add_variable(t).unwrap();
        ds.add_variable(Variable::numeric("improved")).unwrap();
        for (t, o, n) in [(1.0, 1.0, 12), (1.0, 2.0, 3), (2.0, 1.0, 5), (2.0, 2.0, 10)] {
            for _ in 0..n {
                ds.push_case(vec![Value::Num(t), Value::Num(o)]).unwrap();
            }
        }
        ds.push_case(vec![Value::Num(1.0), Value::SysMis]).unwrap();
        ds
    }

    fn opts(cells: Cells) -> Options {
        Options { rows: vec!["treatment".into()], columns: vec!["improved".into()], cells, chisq: true }
    }

    fn table_of(item: &Item) -> &Table {
        match item {
            Item::Table(t) => t,
            other => panic!("not a table: {other:?}"),
        }
    }

    fn value(c: &Cell) -> f64 {
        match c {
            Cell::Num { value, .. } => *value,
            Cell::Int(n) => *n as f64,
            other => panic!("not a number: {other:?}"),
        }
    }

    #[test]
    fn crosstabulation_counts_and_percentages() {
        let cells = Cells { count: true, expected: true, row: true, column: false, total: false };
        let items = crosstabs(&trial(), &opts(cells)).unwrap();
        let summary = table_of(&items[0]);
        assert_eq!(summary.rows[0].1[0], Cell::Int(30));
        assert_eq!(summary.rows[0].1[2], Cell::Int(1), "the case missing an outcome");
        let t = table_of(&items[1]);
        assert_eq!(t.title, "treatment * improved Crosstabulation");
        assert_eq!(t.columns, ["", "1.00", "2.00", "Total"]);
        assert_eq!(t.rows[0].0, "Drug");
        assert_eq!(t.rows[0].1[..4], [Cell::Text("Count".into()), Cell::Int(12), Cell::Int(3), Cell::Int(15)]);
        // Expected for Drug × improved: 15 × 17 / 30 = 8.5.
        assert_eq!(value(&t.rows[1].1[1]), 8.5);
        assert_eq!(value(&t.rows[2].1[1]), 80.0, "12 of 15 within the row");
        assert_eq!(t.rows[6].0, "Total");
        assert_eq!(t.rows[6].1[3], Cell::Int(30));
    }

    #[test]
    fn chi_square_tests_for_a_2x2_table() {
        let items = crosstabs(&trial(), &opts(Cells::default())).unwrap();
        let t = table_of(&items[2]);
        let get = |name: &str, col: usize| t.rows.iter().find(|(n, _)| n == name).map(|(_, c)| value(&c[col])).unwrap();
        // Hand computation: expected 8.5 6.5 / 8.5 6.5; Σ(O−E)²/E = 3.5² × (2/8.5 + 2/6.5).
        let pearson = 3.5f64.powi(2) * (2.0 / 8.5 + 2.0 / 6.5);
        assert!((get("Pearson Chi-Square", 0) - pearson).abs() < 1e-12);
        assert_eq!(get("Pearson Chi-Square", 1), 1.0);
        // Yates: 30 (|12·10 − 3·5| − 15)² / (15·15·17·13).
        let yates = 30.0 * (105.0f64 - 15.0).powi(2) / (15.0 * 15.0 * 17.0 * 13.0);
        assert!((get("Continuity Correction", 0) - yates).abs() < 1e-12);
        assert!(t.rows.iter().any(|(n, _)| n == "Fisher's Exact Test"));
        assert_eq!(get("N of Valid Cases", 0), 30.0);
        assert!(matches!(&items[3], Item::Text(s) if s.starts_with("a. 0 cells (0.0%)")));
    }

    #[test]
    fn a_constant_gets_no_statistics() {
        let mut ds = trial();
        ds.remove_cases(&(15..31).collect::<Vec<_>>()).unwrap();
        let items = crosstabs(&ds, &opts(Cells::default())).unwrap();
        assert!(matches!(&items[2], Item::Text(s) if s.contains("treatment is a constant")));
    }
}
