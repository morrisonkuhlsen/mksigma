//! MK Sigma statistical procedures. Each procedure reads a dataset and returns output items.
//! Numerical behaviour comes from public algorithm documentation and is checked against
//! independently computed reference values (see `docs/architecture.md`, "Numerical parity").
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use mksigma_core::{Dataset, Value, Variable};
use mksigma_i18n::tr;
use mksigma_output::{Cases, Category, Cell, Table};
use thiserror::Error;

mod charts;
mod correlations;
mod crosstabs;
mod dist;
mod examine;
pub mod explain;
mod frequencies;
mod groups;
mod logistic;
pub mod noncentral;
mod npar;
mod oneway;
mod range;
mod regression;
mod ttest;

pub use charts::{BarValue, bar, histogram, scatter};
pub use correlations::{Options as CorrelationsOptions, correlations};
pub use crosstabs::{Cells as CrosstabsCells, Options as CrosstabsOptions, crosstabs};
pub use examine::{Options as ExamineOptions, examine};
pub use logistic::{Options as LogisticOptions, logistic};
pub use npar::{NparTest, npar_tests};
pub use oneway::{Options as OnewayOptions, PostHoc, oneway};
pub use regression::{Options as RegressionOptions, regression};
pub use ttest::{TTest, t_test};

pub use frequencies::{Options as FrequenciesOptions, Order as FrequenciesOrder, Statistic as FrequenciesStatistic, frequencies};

#[derive(Debug, Error, PartialEq)]
pub enum StatsError {
    #[error("{}", mksigma_i18n::tr!("err-unknown-variable", name = .0.as_str()))]
    UnknownVariable(String),
    #[error("{}", mksigma_i18n::tr!("err-not-numeric", name = .0.as_str()))]
    NotNumeric(String),
    #[error("{}", mksigma_i18n::tr!("err-too-many-levels", name = .0.as_str(), max = crate::groups::MAX_LEVELS))]
    TooManyLevels(String),
}

/// Running sum of weights, weighted mean and weighted sum of squared deviations (West's weighted
/// form of Welford's method), plus min and max.
#[derive(Debug, Clone, Copy, Default)]
struct Moments {
    w: f64,
    mean: f64,
    m2: f64,
    min: f64,
    max: f64,
}

impl Moments {
    fn add(&mut self, x: f64, w: f64) {
        if w <= 0.0 {
            return;
        }
        if self.w == 0.0 {
            self.min = x;
            self.max = x;
        }
        self.w += w;
        let delta = x - self.mean;
        self.mean += delta * w / self.w;
        self.m2 += w * delta * (x - self.mean);
        self.min = self.min.min(x);
        self.max = self.max.max(x);
    }

    /// Sample standard deviation with weights as frequencies (W − 1 denominator, as SPSS does);
    /// undefined unless the weights sum to more than 1.
    fn std_dev(&self) -> Option<f64> {
        (self.w > 1.0).then(|| (self.m2 / (self.w - 1.0)).sqrt())
    }
}

/// A value's label, else the value in the variable's format.
pub(crate) fn value_text(var: &Variable, v: &Value) -> String {
    let shown = || match v {
        Value::Num(_) => mksigma_i18n::decimal(&var.format.render(v)),
        Value::Str(_) => var.format.render(v),
        _ => var.format.render(v),
    };
    var.value_label(v).map_or_else(shown, str::to_string)
}

/// The cases where `var` has the value `v`, for cross-filtering; none for system-missing.
pub(crate) fn category(var: &Variable, v: &Value) -> Option<Category> {
    let cases = match v {
        Value::Num(x) => Cases::Num(*x),
        Value::Str(s) => Cases::Str(s.clone()),
        Value::SysMis => return None,
    };
    Some(Category { variable: var.name.clone(), cases, label: format!("{} = {}", var.display_name(), value_text(var, v)) })
}

/// A statistic shown with `decimals` places, or empty when it can't be computed.
pub(crate) fn num(value: Option<f64>, decimals: u8) -> Cell {
    value.filter(|v| v.is_finite()).map_or(Cell::Empty, |value| Cell::Num { value, decimals })
}

/// A significance, or empty when it can't be computed.
pub(crate) fn p_value(value: Option<f64>) -> Cell {
    value.filter(|v| v.is_finite()).map_or(Cell::Empty, Cell::Sig)
}

/// N is the sum of weights, shown rounded to a whole number as SPSS shows it.
fn n_cell(w: f64) -> Cell {
    Cell::Int(w.round() as i64)
}

/// `DESCRIPTIVES`: N, minimum, maximum, mean and standard deviation of each variable, with
/// missing values excluded variable by variable, and a tr!("o-valid-n-listwise") row. Under
/// `WEIGHT BY` each case counts as its weight; cases with no valid positive weight are left out.
pub fn descriptives(ds: &Dataset, names: &[String]) -> Result<Table, StatsError> {
    let mut vars = Vec::with_capacity(names.len());
    for name in names {
        let (index, var) = ds.variable(name).ok_or_else(|| StatsError::UnknownVariable(name.clone()))?;
        if var.var_type != mksigma_core::VarType::Numeric {
            return Err(StatsError::NotNumeric(var.name.clone()));
        }
        vars.push((var, ds.column(index).unwrap_or_default()));
    }

    let weights: Vec<f64> = (0..ds.n_cases()).map(|case| ds.case_weight(case)).collect();

    let mut rows = Vec::with_capacity(vars.len() + 1);
    for (var, column) in &vars {
        let mut m = Moments::default();
        for (v, w) in column.iter().zip(&weights).filter(|(v, _)| !var.is_missing(v)) {
            if let Some(x) = v.as_num() {
                m.add(x, *w);
            }
        }
        let num = |value: f64| num(Some(value), 2);
        let cells = if m.w == 0.0 {
            vec![Cell::Int(0), Cell::Empty, Cell::Empty, Cell::Empty, Cell::Empty]
        } else {
            vec![n_cell(m.w), num(m.min), num(m.max), num(m.mean), m.std_dev().map_or(Cell::Empty, num)]
        };
        rows.push((var.display_name().to_string(), cells));
    }

    let listwise: f64 = weights
        .iter()
        .enumerate()
        .filter(|&(case, _)| vars.iter().all(|(var, column)| column.get(case).is_some_and(|v| matches!(v, Value::Num(_)) && !var.is_missing(v))))
        .map(|(_, w)| w)
        .sum();
    rows.push((tr!("o-valid-n-listwise").to_string(), vec![n_cell(listwise), Cell::Empty, Cell::Empty, Cell::Empty, Cell::Empty]));

    Ok(Table {
        title: tr!("o-descriptive-statistics").to_string(),
        columns: [tr!("o-n"), tr!("o-minimum"), tr!("o-maximum"), tr!("o-mean"), tr!("o-std-deviation")].map(String::from).to_vec(),
        rows,
        categories: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use mksigma_core::{MissingValues, Variable};

    fn dataset() -> Dataset {
        let mut ds = Dataset::new();
        ds.add_variable(Variable::numeric("x")).unwrap();
        let mut y = Variable::numeric("y");
        y.missing = MissingValues::Discrete(vec![Value::Num(99.0)]);
        ds.add_variable(y).unwrap();
        let cases = [
            (2.0, Some(1.0)),
            (4.0, Some(99.0)),
            (4.0, Some(3.0)),
            (4.0, Some(5.0)),
            (5.0, None),
            (5.0, Some(7.0)),
            (7.0, Some(9.0)),
            (9.0, Some(11.0)),
        ];
        for (x, y) in cases {
            ds.push_case(vec![Value::Num(x), y.map_or(Value::SysMis, Value::Num)]).unwrap();
        }
        ds
    }

    #[test]
    fn descriptives_matches_hand_computed_values() {
        let t = descriptives(&dataset(), &["x".into(), "Y".into()]).unwrap();
        // x = 2,4,4,4,5,5,7,9: mean 5, sample variance 32/7.
        let (_, x) = &t.rows[0];
        assert_eq!(x[0], Cell::Int(8));
        assert_eq!(x[3], Cell::Num { value: 5.0, decimals: 2 });
        let Cell::Num { value: sd, .. } = x[4] else { panic!("no std dev") };
        assert!((sd - (32.0f64 / 7.0).sqrt()).abs() < 1e-12);
        // y without the user-missing 99 and the system-missing case: 1,3,5,7,9,11.
        let (_, y) = &t.rows[1];
        assert_eq!(y[0], Cell::Int(6));
        assert_eq!(y[1], Cell::Num { value: 1.0, decimals: 2 });
        assert_eq!(y[3], Cell::Num { value: 6.0, decimals: 2 });
        assert_eq!(t.rows[2], ("Valid N (listwise)".to_string(), vec![Cell::Int(6), Cell::Empty, Cell::Empty, Cell::Empty, Cell::Empty]));
    }

    fn num(cell: &Cell) -> f64 {
        match cell {
            Cell::Num { value, .. } | Cell::Sig(value) => *value,
            other => panic!("not a number: {other:?}"),
        }
    }

    /// Integer weights behave exactly like repeating each case that many times.
    #[test]
    fn integer_weights_equal_repeated_cases() {
        let (xs, ws) = ([1.0, 2.0, 3.0, 10.0], [1.0, 2.0, 3.0, 1.0]);
        let mut weighted = Dataset::new();
        weighted.add_variable(Variable::numeric("x")).unwrap();
        weighted.add_variable(Variable::numeric("w")).unwrap();
        let mut repeated = Dataset::new();
        repeated.add_variable(Variable::numeric("x")).unwrap();
        for (x, w) in xs.iter().zip(ws) {
            weighted.push_case(vec![Value::Num(*x), Value::Num(w)]).unwrap();
            for _ in 0..w as usize {
                repeated.push_case(vec![Value::Num(*x)]).unwrap();
            }
        }
        weighted.weight = Some(1);
        let a = descriptives(&weighted, &["x".into()]).unwrap();
        let b = descriptives(&repeated, &["x".into()]).unwrap();
        // 1,2,2,3,3,3,10: mean 24/7.
        assert_eq!(a.rows[0].1[0], Cell::Int(7));
        assert!((num(&a.rows[0].1[3]) - 24.0 / 7.0).abs() < 1e-12);
        for col in 1..5 {
            assert!((num(&a.rows[0].1[col]) - num(&b.rows[0].1[col])).abs() < 1e-12, "column {col}");
        }
        assert_eq!(a.rows[1], b.rows[1]);
    }

    #[test]
    fn fractional_and_invalid_weights() {
        let mut ds = Dataset::new();
        ds.add_variable(Variable::numeric("x")).unwrap();
        ds.add_variable(Variable::numeric("w")).unwrap();
        // The last three cases have no valid weight and are left out.
        for (x, w) in [(2.0, Value::Num(0.5)), (4.0, Value::Num(1.5)), (100.0, Value::Num(0.0)), (100.0, Value::Num(-2.0)), (100.0, Value::SysMis)] {
            ds.push_case(vec![Value::Num(x), w]).unwrap();
        }
        ds.weight = Some(1);
        let t = descriptives(&ds, &["x".into()]).unwrap();
        let row = &t.rows[0].1;
        // W = 2, mean = (0.5·2 + 1.5·4) / 2 = 3.5, Σw(x − mean)² = 0.5·2.25 + 1.5·0.25 = 1.5.
        assert_eq!(row[0], Cell::Int(2));
        assert_eq!((num(&row[1]), num(&row[2])), (2.0, 4.0));
        assert!((num(&row[3]) - 3.5).abs() < 1e-12);
        assert!((num(&row[4]) - 1.5f64.sqrt()).abs() < 1e-12);
        assert_eq!(t.rows[1].1[0], Cell::Int(2));
    }

    #[test]
    fn unknown_variable() {
        assert_eq!(descriptives(&dataset(), &["z".into()]), Err(StatsError::UnknownVariable("z".into())));
    }
}
