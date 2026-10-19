//! Cross-filtering, as in Power BI: selecting a category in one item (a bar, a histogram bin, a
//! box, a row of a frequency table) filters every other item to the cases in it. The analyses
//! that produced those items run again on the filtered cases; the command whose item was
//! selected keeps its full view, so the selection can be changed from there. Nothing here
//! changes the data: the filter is a view, not an applied step.
//!
//! Slicers (`SLICE`) filter every item, the selected one included: the cases they keep are what
//! the analyses, and then the cross-filter, start from.

use mksigma_core::{Dataset, Value, Variable};
use mksigma_i18n::tr;
use mksigma_output::{Cases, Category, Item};
use mksigma_syntax::Command;

use crate::EngineError;

/// An analysis command and the items it produced on all the cases.
#[derive(Debug, Clone)]
pub(crate) struct Run {
    pub command: Command,
    /// The command as written, for a saved dashboard.
    pub source: String,
    pub items: Vec<Item>,
    /// `Session::version` when it ran: its items are on the data of then.
    pub version: u64,
}

/// The active cross-filter.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CrossFilter {
    pub category: Category,
    /// The run whose item was selected.
    pub source: usize,
}

/// Which values of its variable a slicer keeps.
#[derive(Debug, Clone, PartialEq)]
pub enum Selection {
    Values(Vec<Value>),
    /// `None` ends are open.
    Range {
        low: Option<f64>,
        high: Option<f64>,
    },
}

/// A slicer: the cases whose `variable` has a value in `selection`.
#[derive(Debug, Clone, PartialEq)]
pub struct Slicer {
    pub variable: String,
    pub selection: Selection,
}

impl Slicer {
    fn keeps(&self, var: &Variable, v: &Value) -> bool {
        if var.is_missing(v) {
            return false;
        }
        match (&self.selection, v) {
            (Selection::Values(values), v) => values.contains(v),
            (Selection::Range { low, high }, Value::Num(x)) => low.is_none_or(|l| *x >= l) && high.is_none_or(|h| *x <= h),
            _ => false,
        }
    }

    /// How the slicer reads in the Output and exports: `grupo: Jovem, Adulto`, `renda 2000 to
    /// 5000`. Plain text that every export can print.
    pub fn describe(&self, ds: &Dataset) -> String {
        let var = ds.variable(&self.variable).map(|(_, v)| v);
        let name = var.map_or(self.variable.as_str(), |v| v.name.as_str());
        let show = |v: &Value| {
            var.map_or_else(
                || mksigma_core::Format::new(mksigma_core::FormatKind::F, 8, 2).render(v),
                |var| var.value_label(v).map_or_else(|| mksigma_i18n::decimal(&var.format.render(v)), str::to_string),
            )
        };
        match &self.selection {
            Selection::Values(values) => tr!("slice-values", name = name, values = values.iter().map(show).collect::<Vec<_>>().join(", ")),
            Selection::Range { low: Some(l), high: Some(h) } => {
                tr!("slice-range", name = name, low = show(&Value::Num(*l)), high = show(&Value::Num(*h)))
            }
            Selection::Range { low: Some(l), high: None } => tr!("slice-from", name = name, low = show(&Value::Num(*l))),
            Selection::Range { low: None, high: Some(h) } => tr!("slice-up-to", name = name, high = show(&Value::Num(*h))),
            Selection::Range { low: None, high: None } => tr!("slice-any", name = name),
        }
    }

    /// The `SLICE` command that sets it.
    pub fn syntax(&self) -> String {
        let number = |x: f64| x.to_string();
        let inner = match &self.selection {
            Selection::Values(values) => values
                .iter()
                .map(|v| match v {
                    Value::Num(x) => number(*x),
                    Value::Str(s) => mksigma_syntax::quote(s),
                    Value::SysMis => ".".to_string(),
                })
                .collect::<Vec<_>>()
                .join(" "),
            Selection::Range { low, high } => format!("{} THRU {}", low.map_or("LO".to_string(), number), high.map_or("HI".to_string(), number)),
        };
        format!("SLICE {} ({inner}).", self.variable)
    }
}

/// The cases of `ds` whose `variable` value `keep` accepts, weights and dictionary unchanged.
fn retain(ds: &Dataset, variable: &str, keep: impl Fn(&Variable, &Value) -> bool) -> Result<Dataset, EngineError> {
    let (index, var) = ds.variable(variable).ok_or_else(|| EngineError::UnknownVariable(variable.to_string()))?;
    let column = ds.column(index).unwrap_or_default();
    let outside: Vec<usize> = column.iter().enumerate().filter(|(_, v)| !keep(var, v)).map(|(i, _)| i).collect();
    let mut out = ds.clone();
    out.remove_cases(&outside)?;
    Ok(out)
}

/// The cases of `ds` in `category`.
pub(crate) fn subset(ds: &Dataset, category: &Category) -> Result<Dataset, EngineError> {
    retain(ds, &category.variable, |var, v| {
        !var.is_missing(v)
            && match (v, &category.cases) {
                (Value::Num(x), cases) => cases.contains_num(*x),
                (Value::Str(s), Cases::Str(wanted)) => s == wanted,
                _ => false,
            }
    })
}

/// The cases every slicer keeps.
pub(crate) fn slice(ds: &Dataset, slicers: &[Slicer]) -> Result<Dataset, EngineError> {
    let mut out = ds.clone();
    for slicer in slicers {
        out = retain(&out, &slicer.variable, |var, v| slicer.keeps(var, v))?;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mksigma_core::{MissingValues, Variable};

    fn data() -> Dataset {
        let mut ds = Dataset::new();
        let mut x = Variable::numeric("x");
        x.missing = MissingValues::Discrete(vec![Value::Num(9.0)]);
        ds.add_variable(x).unwrap();
        ds.add_variable(Variable::string("s", 4)).unwrap();
        for (x, s) in [(1.0, "a"), (2.0, "b"), (9.0, "a"), (3.0, "a")] {
            ds.push_case(vec![Value::Num(x), Value::Str(s.into())]).unwrap();
        }
        ds.push_case(vec![Value::SysMis, Value::Str("b".into())]).unwrap();
        ds
    }

    fn cat(variable: &str, cases: Cases) -> Category {
        Category { variable: variable.into(), cases, label: String::new() }
    }

    fn xs(ds: &Dataset) -> Vec<Value> {
        ds.column(0).unwrap().to_vec()
    }

    #[test]
    fn keeps_the_cases_in_the_category() {
        let ds = data();
        assert_eq!(xs(&subset(&ds, &cat("x", Cases::Num(2.0))).unwrap()), [Value::Num(2.0)]);
        assert_eq!(xs(&subset(&ds, &cat("s", Cases::Str("a".into()))).unwrap()), [Value::Num(1.0), Value::Num(9.0), Value::Num(3.0)]);
        let half_open = cat("x", Cases::Range { low: 1.0, high: 3.0, closed: false });
        assert_eq!(xs(&subset(&ds, &half_open).unwrap()), [Value::Num(1.0), Value::Num(2.0)]);
        let closed = cat("x", Cases::Range { low: 1.0, high: 9.0, closed: true });
        assert_eq!(xs(&subset(&ds, &closed).unwrap()), [Value::Num(1.0), Value::Num(2.0), Value::Num(3.0)], "9 is user-missing");
        assert!(subset(&ds, &cat("nope", Cases::Num(1.0))).is_err());
    }
}
