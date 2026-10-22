//! `FREQUENCIES`: a Statistics table (N valid and missing, plus any requested statistics) and,
//! per variable, a frequency table with valid values, user-missing values, system-missing and
//! totals. Counts are sums of case weights, shown rounded; percentages use the exact sums.

use mksigma_i18n::tr;
use std::collections::HashMap;

use mksigma_core::{Dataset, Value, VarType, Variable};
use mksigma_output::{Cell, Item, Table};

use crate::{Moments, StatsError, category, n_cell, value_text};

/// Row order of a frequency table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Order {
    #[default]
    AscendingValue,
    DescendingValue,
    AscendingFrequency,
    DescendingFrequency,
}

/// Statistics shown in the Statistics table, in the order given.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Statistic {
    Mean,
    SeMean,
    Mode,
    StdDev,
    Variance,
    Range,
    Minimum,
    Maximum,
    Sum,
}

impl Statistic {
    fn label(self) -> String {
        match self {
            Statistic::Mean => tr!("o-mean"),
            Statistic::SeMean => tr!("o-std-error-of-mean"),
            Statistic::Mode => tr!("o-mode"),
            Statistic::StdDev => tr!("o-std-deviation"),
            Statistic::Variance => tr!("o-variance"),
            Statistic::Range => tr!("o-range"),
            Statistic::Minimum => tr!("o-minimum"),
            Statistic::Maximum => tr!("o-maximum"),
            Statistic::Sum => tr!("o-sum"),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Options {
    pub variables: Vec<String>,
    /// Whether to show the frequency tables (`/FORMAT=NOTABLE` turns them off).
    pub table: bool,
    pub order: Order,
    pub statistics: Vec<Statistic>,
}

/// Distinct values compare by bits (numbers, with −0 as 0) or by text (strings).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum Key {
    Num(u64),
    Str(String),
}

/// One variable's weighted counts.
pub(crate) struct Counts {
    /// Valid values and their summed weights, in table order.
    pub(crate) valid: Vec<(Value, f64)>,
    /// User-missing values and their summed weights, ascending.
    user_missing: Vec<(Value, f64)>,
    system_missing: f64,
}

impl Counts {
    pub(crate) fn valid_total(&self) -> f64 {
        self.valid.iter().map(|(_, w)| w).sum()
    }

    pub(crate) fn missing_total(&self) -> f64 {
        self.user_missing.iter().map(|(_, w)| w).sum::<f64>() + self.system_missing
    }
}

fn compare_values(a: &Value, b: &Value) -> std::cmp::Ordering {
    match (a, b) {
        (Value::Num(x), Value::Num(y)) => x.total_cmp(y),
        (Value::Str(x), Value::Str(y)) => x.cmp(y),
        _ => std::cmp::Ordering::Equal,
    }
}

pub(crate) fn count(var: &Variable, column: &[Value], weights: &[f64], order: Order) -> Counts {
    let mut map: HashMap<Key, (Value, f64)> = HashMap::new();
    let mut system_missing = 0.0;
    for (value, &w) in column.iter().zip(weights).filter(|(_, w)| **w > 0.0) {
        let key = match value {
            // A non-finite number is system-missing, not a "NaN" category (#21).
            Value::SysMis => {
                system_missing += w;
                continue;
            }
            Value::Num(x) if !x.is_finite() => {
                system_missing += w;
                continue;
            }
            Value::Num(x) => Key::Num(if *x == 0.0 { 0.0f64 } else { *x }.to_bits()),
            Value::Str(s) => Key::Str(s.clone()),
        };
        map.entry(key).or_insert_with(|| (value.clone(), 0.0)).1 += w;
    }
    let (mut user_missing, mut valid): (Vec<_>, Vec<_>) = map.into_values().partition(|(v, _)| var.is_missing(v));
    user_missing.sort_by(|a, b| compare_values(&a.0, &b.0));
    valid.sort_by(|a, b| compare_values(&a.0, &b.0));
    match order {
        Order::AscendingValue => {}
        Order::DescendingValue => valid.reverse(),
        // Stable sorts keep ties in ascending value order.
        Order::AscendingFrequency => valid.sort_by(|a, b| a.1.total_cmp(&b.1)),
        Order::DescendingFrequency => valid.sort_by(|a, b| b.1.total_cmp(&a.1)),
    }
    Counts { valid, user_missing, system_missing }
}

fn percent(part: f64, whole: f64) -> Cell {
    if whole > 0.0 { Cell::Num { value: part / whole * 100.0, decimals: 1 } } else { Cell::Empty }
}

fn frequency_table(var: &Variable, counts: &Counts) -> Table {
    let valid = counts.valid_total();
    let total = valid + counts.missing_total();
    let mut rows = Vec::new();
    // Only the rows of valid values stand for a category; they come first.
    let categories = counts.valid.iter().map(|(value, _)| category(var, value)).collect();
    let mut cumulative = 0.0;
    for (i, (value, w)) in counts.valid.iter().enumerate() {
        cumulative += w;
        let stub = if i == 0 { tr!("o-valid") } else { String::new() };
        rows.push((
            stub.to_string(),
            vec![Cell::Text(value_text(var, value)), n_cell(*w), percent(*w, total), percent(*w, valid), percent(cumulative, valid)],
        ));
    }
    if !counts.valid.is_empty() {
        rows.push((String::new(), vec![Cell::Text(tr!("o-total")), n_cell(valid), percent(valid, total), percent(valid, valid), Cell::Empty]));
    }
    let mut missing_rows: Vec<(String, f64)> = counts.user_missing.iter().map(|(v, w)| (value_text(var, v), *w)).collect();
    if counts.system_missing > 0.0 {
        missing_rows.push((tr!("o-system").to_string(), counts.system_missing));
    }
    if !missing_rows.is_empty() {
        let missing = counts.missing_total();
        for (i, (text, w)) in missing_rows.into_iter().enumerate() {
            let stub = if i == 0 { tr!("o-missing") } else { String::new() };
            rows.push((stub.to_string(), vec![Cell::Text(text), n_cell(w), percent(w, total), Cell::Empty, Cell::Empty]));
        }
        rows.push((String::new(), vec![Cell::Text(tr!("o-total")), n_cell(missing), percent(missing, total), Cell::Empty, Cell::Empty]));
        rows.push((tr!("o-total").to_string(), vec![Cell::Empty, n_cell(total), percent(total, total), Cell::Empty, Cell::Empty]));
    }
    Table {
        title: var.display_name().to_string(),
        columns: [String::new(), tr!("o-frequency"), tr!("o-percent"), tr!("o-valid-percent"), tr!("o-cumulative-percent")]
            .map(String::from)
            .to_vec(),
        rows,
        categories,
    }
}

/// The requested statistics for one variable, and whether its mode was one of several.
fn statistics(var: &Variable, counts: &Counts, wanted: &[Statistic]) -> (Vec<Cell>, bool) {
    if var.var_type != VarType::Numeric {
        return (vec![Cell::Empty; wanted.len()], false);
    }
    let mut m = Moments::default();
    for (v, w) in &counts.valid {
        if let Some(x) = v.as_num() {
            m.add(x, *w);
        }
    }
    let top = counts.valid.iter().map(|(_, w)| *w).fold(0.0, f64::max);
    let modes: Vec<f64> = counts.valid.iter().filter(|(_, w)| *w == top).filter_map(|(v, _)| v.as_num()).collect();
    let mode = modes.iter().copied().reduce(f64::min);
    let num = |value: f64| crate::num(Some(value), 2);
    let have = m.w > 0.0;
    let sd = m.std_dev();
    let cells =
        wanted
            .iter()
            .map(|s| match s {
                Statistic::Mean if have => num(m.mean),
                Statistic::SeMean => sd.map_or(Cell::Empty, |sd| num(sd / m.w.sqrt())),
                Statistic::Mode => mode.map_or(Cell::Empty, |x| {
                    if modes.len() > 1 { Cell::Text(format!("{}a", mksigma_i18n::decimal(&format!("{x:.2}")))) } else { num(x) }
                }),
                Statistic::StdDev => sd.map_or(Cell::Empty, num),
                Statistic::Variance => sd.map_or(Cell::Empty, |sd| num(sd * sd)),
                Statistic::Range if have => num(m.max - m.min),
                Statistic::Minimum if have => num(m.min),
                Statistic::Maximum if have => num(m.max),
                Statistic::Sum if have => num(m.mean * m.w),
                _ => Cell::Empty,
            })
            .collect();
    (cells, modes.len() > 1)
}

pub fn frequencies(ds: &Dataset, options: &Options) -> Result<Vec<Item>, StatsError> {
    let weights: Vec<f64> = (0..ds.n_cases()).map(|case| ds.case_weight(case)).collect();
    let mut vars = Vec::with_capacity(options.variables.len());
    for name in &options.variables {
        let (index, var) = ds.variable(name).ok_or_else(|| StatsError::UnknownVariable(name.clone()))?;
        let counts = count(var, ds.column(index).unwrap_or_default(), &weights, options.order);
        vars.push((var, counts));
    }

    // The Statistics table has one column per variable.
    let per_var: Vec<(Vec<Cell>, bool)> = vars.iter().map(|(var, counts)| statistics(var, counts, &options.statistics)).collect();
    let mut rows = vec![
        (tr!("o-n-valid").to_string(), vars.iter().map(|(_, c)| n_cell(c.valid_total())).collect()),
        (tr!("o-n-missing").to_string(), vars.iter().map(|(_, c)| n_cell(c.missing_total())).collect()),
    ];
    for (i, stat) in options.statistics.iter().enumerate() {
        rows.push((stat.label().to_string(), per_var.iter().map(|(cells, _)| cells.get(i).cloned().unwrap_or(Cell::Empty)).collect()));
    }
    let mut items = vec![Item::Table(Table {
        title: tr!("o-statistics").to_string(),
        columns: vars.iter().map(|(var, _)| var.display_name().to_string()).collect(),
        rows,
        categories: Vec::new(),
    })];
    if per_var.iter().any(|(_, multiple)| *multiple) {
        items.push(Item::Text(tr!("o-a-multiple-modes-exist-the-smallest-value-is-shown").to_string()));
    }

    if options.table {
        for (var, counts) in &vars {
            if counts.valid.is_empty() && counts.missing_total() == 0.0 {
                items.push(Item::Warning(tr!("o-no-cases", name = var.name.as_str())));
            } else {
                items.push(Item::Table(frequency_table(var, counts)));
            }
        }
    }
    Ok(items)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mksigma_core::MissingValues;

    /// Satisfaction 1–5 with labels on 1 and 5, 9 = no answer (user-missing), one system-missing.
    fn survey() -> Dataset {
        let mut ds = Dataset::new();
        let mut q = Variable::numeric("q").with_label("Satisfaction");
        q.format = mksigma_core::Format::new(mksigma_core::FormatKind::F, 1, 0);
        q.missing = MissingValues::Discrete(vec![Value::Num(9.0)]);
        q.value_labels = vec![(Value::Num(1.0), "Very low".into()), (Value::Num(5.0), "Very high".into()), (Value::Num(9.0), "No answer".into())];
        ds.add_variable(q).unwrap();
        ds.add_variable(Variable::string("city", 8)).unwrap();
        let cases = [
            (Some(3.0), "Rio"),
            (Some(1.0), "Rio"),
            (Some(3.0), "SP"),
            (Some(5.0), "SP"),
            (Some(9.0), "SP"),
            (None, "BH"),
            (Some(3.0), "Rio"),
            (Some(5.0), "SP"),
        ];
        for (q, city) in cases {
            ds.push_case(vec![q.map_or(Value::SysMis, Value::Num), Value::Str(city.into())]).unwrap();
        }
        ds
    }

    fn options(vars: &[&str]) -> Options {
        Options { variables: vars.iter().map(|v| v.to_string()).collect(), table: true, ..Options::default() }
    }

    fn table(item: &Item) -> &Table {
        match item {
            Item::Table(t) => t,
            other => panic!("not a table: {other:?}"),
        }
    }

    #[test]
    fn frequency_table_with_labels_and_missing() {
        let items = frequencies(&survey(), &options(&["q"])).unwrap();
        let stats = table(&items[0]);
        assert_eq!(stats.rows[0], ("N Valid".to_string(), vec![Cell::Int(6)]));
        assert_eq!(stats.rows[1], ("N Missing".to_string(), vec![Cell::Int(2)]));
        let t = table(&items[1]);
        assert_eq!(t.title, "Satisfaction");
        // Compared as shown: percentages to one decimal.
        let row = |i: usize| -> (&str, Vec<String>) { (t.rows[i].0.as_str(), t.rows[i].1.iter().map(Cell::render).collect()) };
        // 8 cases: 6 valid (1, 3, 3, 3, 5, 5), 9 user-missing, one system-missing.
        assert_eq!(row(0), ("Valid", vec!["Very low".into(), "1".into(), "12.5".into(), "16.7".into(), "16.7".into()]));
        assert_eq!(row(1), ("", vec!["3".into(), "3".into(), "37.5".into(), "50.0".into(), "66.7".into()]));
        assert_eq!(row(2), ("", vec!["Very high".into(), "2".into(), "25.0".into(), "33.3".into(), "100.0".into()]));
        assert_eq!(row(3), ("", vec!["Total".into(), "6".into(), "75.0".into(), "100.0".into(), String::new()]));
        assert_eq!(row(4), ("Missing", vec!["No answer".into(), "1".into(), "12.5".into(), String::new(), String::new()]));
        assert_eq!(row(5), ("", vec!["System".into(), "1".into(), "12.5".into(), String::new(), String::new()]));
        assert_eq!(row(6), ("", vec!["Total".into(), "2".into(), "25.0".into(), String::new(), String::new()]));
        assert_eq!(row(7), ("Total", vec![String::new(), "8".into(), "100.0".into(), String::new(), String::new()]));
        assert_eq!(t.rows.len(), 8);
    }

    #[test]
    fn strings_and_frequency_order() {
        let mut o = options(&["city"]);
        o.order = Order::DescendingFrequency;
        let items = frequencies(&survey(), &o).unwrap();
        let t = table(&items[1]);
        let labels: Vec<&Cell> = t.rows.iter().map(|(_, cells)| &cells[0]).collect();
        // SP 4, Rio 3, BH 1; no missing section for a string with no missing values.
        assert_eq!(labels, [&Cell::Text("SP".into()), &Cell::Text("Rio".into()), &Cell::Text("BH".into()), &Cell::Text("Total".into())]);
    }

    #[test]
    fn statistics_and_multiple_modes() {
        let mut o = options(&["q", "city"]);
        o.table = false;
        o.statistics = vec![Statistic::Mean, Statistic::Mode, Statistic::StdDev, Statistic::Sum];
        let items = frequencies(&survey(), &o).unwrap();
        assert_eq!(items.len(), 1, "NOTABLE shows only the Statistics table");
        let t = table(&items[0]);
        assert_eq!(t.columns, ["Satisfaction", "city"]);
        // Valid q: 1, 3, 3, 3, 5, 5 → mean 10/3, mode 3, Σ(x − x̄)² = 102/9, sample variance that / 5.
        let Cell::Num { value: mean, .. } = t.rows[2].1[0] else { panic!("mean") };
        assert!((mean - 20.0 / 6.0).abs() < 1e-12);
        assert_eq!(t.rows[3].1[0], Cell::Num { value: 3.0, decimals: 2 });
        let Cell::Num { value: sd, .. } = t.rows[4].1[0] else { panic!("sd") };
        assert!((sd - (102.0f64 / 9.0 / 5.0).sqrt()).abs() < 1e-12);
        assert_eq!(t.rows[5].1[0], Cell::Num { value: 20.0, decimals: 2 });
        // Statistics don't apply to strings.
        assert_eq!(t.rows[2].1[1], Cell::Empty);

        let mut ds = Dataset::new();
        ds.add_variable(Variable::numeric("x")).unwrap();
        for x in [2.0, 1.0, 2.0, 1.0, 7.0] {
            ds.push_case(vec![Value::Num(x)]).unwrap();
        }
        let mut o = options(&["x"]);
        o.statistics = vec![Statistic::Mode];
        let items = frequencies(&ds, &o).unwrap();
        assert_eq!(table(&items[0]).rows[2].1[0], Cell::Text("1.00a".into()));
        assert_eq!(items[1], Item::Text("a. Multiple modes exist. The smallest value is shown".into()));
    }

    #[test]
    fn weighted_counts() {
        let mut ds = survey();
        // Weight by q itself: the 9s are user-missing, so those cases weigh 0 and drop out entirely.
        ds.weight = Some(0);
        let items = frequencies(&ds, &options(&["city"])).unwrap();
        let t = table(&items[1]);
        // Rio: 3 + 1 + 3 = 7; SP: 3 + 5 + 5 = 13 (the 9 drops out); BH: system-missing weight → out.
        let rows: Vec<(Cell, Cell)> = t.rows.iter().map(|(_, c)| (c[0].clone(), c[1].clone())).collect();
        assert_eq!(
            rows,
            [(Cell::Text("Rio".into()), Cell::Int(7)), (Cell::Text("SP".into()), Cell::Int(13)), (Cell::Text("Total".into()), Cell::Int(20))]
        );
    }
}
