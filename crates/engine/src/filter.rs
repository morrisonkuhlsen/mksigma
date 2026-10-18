//! Cross-filtering, as in Power BI: selecting a category in one item (a bar, a histogram bin, a
//! box, a row of a frequency table) filters every other item to the cases in it. The analyses
//! that produced those items run again on the filtered cases; the command whose item was
//! selected keeps its full view, so the selection can be changed from there. Nothing here
//! changes the data: the filter is a view, not an applied step.

use mksigma_core::{Dataset, Value};
use mksigma_output::{Cases, Category, Item};
use mksigma_syntax::Command;

use crate::EngineError;

/// An analysis command and the items it produced on all the cases.
#[derive(Debug, Clone)]
pub(crate) struct Run {
    pub command: Command,
    pub items: Vec<Item>,
}

/// The active cross-filter.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CrossFilter {
    pub category: Category,
    /// The run whose item was selected.
    pub source: usize,
}

/// The cases of `ds` in `category`, weights and dictionary unchanged.
pub(crate) fn subset(ds: &Dataset, category: &Category) -> Result<Dataset, EngineError> {
    let (index, var) = ds.variable(&category.variable).ok_or_else(|| EngineError::UnknownVariable(category.variable.clone()))?;
    let column = ds.column(index).unwrap_or_default();
    let outside: Vec<usize> = column
        .iter()
        .enumerate()
        .filter(|(_, v)| {
            var.is_missing(v)
                || match (v, &category.cases) {
                    (Value::Num(x), cases) => !cases.contains_num(*x),
                    (Value::Str(s), Cases::Str(wanted)) => s != wanted,
                    _ => true,
                }
        })
        .map(|(i, _)| i)
        .collect();
    let mut out = ds.clone();
    out.remove_cases(&outside)?;
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
