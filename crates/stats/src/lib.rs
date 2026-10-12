//! MK Sigma statistical procedures. Each procedure reads a dataset and returns output items.
//! Numerical behaviour comes from public algorithm documentation and is checked against
//! independently computed reference values (see `docs/architecture.md`, "Numerical parity").
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use mksigma_core::{Dataset, Value};
use mksigma_output::{Cell, Table};
use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
pub enum StatsError {
    #[error("variable {0} does not exist")]
    UnknownVariable(String),
    #[error("variable {0} is not numeric")]
    NotNumeric(String),
}

/// Running count, mean and sum of squared deviations (Welford), plus min and max.
#[derive(Debug, Clone, Copy, Default)]
struct Moments {
    n: usize,
    mean: f64,
    m2: f64,
    min: f64,
    max: f64,
}

impl Moments {
    fn add(&mut self, x: f64) {
        if self.n == 0 {
            self.min = x;
            self.max = x;
        }
        self.n += 1;
        let delta = x - self.mean;
        self.mean += delta / self.n as f64;
        self.m2 += delta * (x - self.mean);
        self.min = self.min.min(x);
        self.max = self.max.max(x);
    }

    /// Sample standard deviation (n − 1 denominator); undefined below two cases.
    fn std_dev(&self) -> Option<f64> {
        (self.n > 1).then(|| (self.m2 / (self.n - 1) as f64).sqrt())
    }
}

/// `DESCRIPTIVES`: N, minimum, maximum, mean and standard deviation of each variable, with
/// missing values excluded variable by variable, and a "Valid N (listwise)" row.
pub fn descriptives(ds: &Dataset, names: &[String]) -> Result<Table, StatsError> {
    let mut vars = Vec::with_capacity(names.len());
    for name in names {
        let (index, var) = ds.variable(name).ok_or_else(|| StatsError::UnknownVariable(name.clone()))?;
        if var.var_type != mksigma_core::VarType::Numeric {
            return Err(StatsError::NotNumeric(var.name.clone()));
        }
        vars.push((var, ds.column(index).unwrap_or_default()));
    }

    let mut rows = Vec::with_capacity(vars.len() + 1);
    for (var, column) in &vars {
        let mut m = Moments::default();
        for v in column.iter().filter(|v| !var.is_missing(v)) {
            if let Some(x) = v.as_num() {
                m.add(x);
            }
        }
        let num = |value: f64| Cell::Num { value, decimals: 2 };
        let cells = if m.n == 0 {
            vec![Cell::Int(0), Cell::Empty, Cell::Empty, Cell::Empty, Cell::Empty]
        } else {
            vec![Cell::Int(i64::try_from(m.n).unwrap_or(i64::MAX)), num(m.min), num(m.max), num(m.mean), m.std_dev().map_or(Cell::Empty, num)]
        };
        rows.push((var.display_name().to_string(), cells));
    }

    let listwise = (0..ds.n_cases())
        .filter(|&case| vars.iter().all(|(var, column)| column.get(case).is_some_and(|v| matches!(v, Value::Num(_)) && !var.is_missing(v))))
        .count();
    rows.push((
        "Valid N (listwise)".to_string(),
        vec![Cell::Int(i64::try_from(listwise).unwrap_or(i64::MAX)), Cell::Empty, Cell::Empty, Cell::Empty, Cell::Empty],
    ));

    Ok(Table {
        title: "Descriptive Statistics".to_string(),
        columns: ["N", "Minimum", "Maximum", "Mean", "Std. Deviation"].map(String::from).to_vec(),
        rows,
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

    #[test]
    fn unknown_variable() {
        assert_eq!(descriptives(&dataset(), &["z".into()]), Err(StatsError::UnknownVariable("z".into())));
    }
}
