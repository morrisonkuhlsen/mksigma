//! Shared by `T-TEST` and `ONEWAY`: valid values with their case weights, statistics per group,
//! and Levene's test of equal variances; and the grouping of cases by the categories of a
//! variable, shared by the procedures that group.

use std::collections::HashMap;

use mksigma_core::{Dataset, Value, VarType, Variable};

use crate::{Moments, StatsError, dist};

/// A numeric variable's index and dictionary entry; strings are refused.
pub(crate) fn numeric<'a>(ds: &'a Dataset, name: &str) -> Result<(usize, &'a Variable), StatsError> {
    let (i, var) = ds.variable(name).ok_or_else(|| StatsError::UnknownVariable(name.to_string()))?;
    if var.var_type != VarType::Numeric {
        return Err(StatsError::NotNumeric(var.name.clone()));
    }
    Ok((i, var))
}

/// The most categories a grouping variable may have (a factor, a row or column of a
/// crosstabulation, a box or bar per category). Past it the analysis stops with an error: an ID
/// grouped by mistake would otherwise build a table of millions of cells.
pub(crate) const MAX_LEVELS: usize = 1_000;

/// A value as a hash key: numbers by their bits (−0 as 0), text as it is.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum Key {
    Num(u64),
    Str(String),
    Missing,
}

impl Key {
    pub(crate) fn of(value: &Value) -> Key {
        match value {
            Value::Num(x) => Key::Num(if *x == 0.0 { 0.0f64 } else { *x }.to_bits()),
            Value::Str(s) => Key::Str(s.clone()),
            Value::SysMis => Key::Missing,
        }
    }
}

/// Values in ascending order: numbers by value, text alphabetically.
pub(crate) fn compare(a: &Value, b: &Value) -> std::cmp::Ordering {
    match (a, b) {
        (Value::Num(x), Value::Num(y)) => x.total_cmp(y),
        (Value::Str(x), Value::Str(y)) => x.cmp(y),
        _ => std::cmp::Ordering::Equal,
    }
}

/// What is gathered for each category of a variable, each found by hash: at most `MAX_LEVELS`.
pub(crate) struct Categories<'a, T> {
    variable: &'a Variable,
    index: HashMap<Key, usize>,
    groups: Vec<(Value, T)>,
}

impl<'a, T: Default> Categories<'a, T> {
    pub(crate) fn new(variable: &'a Variable) -> Self {
        Self { variable, index: HashMap::new(), groups: Vec::new() }
    }

    /// The category of `value`, new or already seen; an error past `MAX_LEVELS`.
    pub(crate) fn of(&mut self, value: &Value) -> Result<&mut T, StatsError> {
        let key = Key::of(value);
        let k = match self.index.get(&key) {
            Some(k) => *k,
            None => {
                if self.groups.len() >= MAX_LEVELS {
                    return Err(StatsError::TooManyLevels(self.variable.display_name().to_string()));
                }
                self.index.insert(key, self.groups.len());
                self.groups.push((value.clone(), T::default()));
                self.groups.len() - 1
            }
        };
        self.groups.get_mut(k).map(|(_, t)| t).ok_or_else(|| StatsError::TooManyLevels(self.variable.display_name().to_string()))
    }

    /// The categories in ascending order of value.
    pub(crate) fn sorted(self) -> Vec<(Value, T)> {
        let mut groups = self.groups;
        groups.sort_by(|a, b| compare(&a.0, &b.0));
        groups
    }
}

/// A case's value of `var` if it is valid (not system- or user-missing).
pub(crate) fn valid(ds: &Dataset, var: usize, case: usize) -> Option<f64> {
    let value = ds.column(var)?.get(case)?;
    let missing = ds.variables().get(var).is_none_or(|v| v.is_missing(value));
    if missing { None } else { value.as_num() }
}

/// A case's raw value of `var`, if it isn't missing (for grouping variables of either type).
pub(crate) fn valid_value(ds: &Dataset, var: usize, case: usize) -> Option<&Value> {
    let value = ds.column(var)?.get(case)?;
    let missing = ds.variables().get(var).is_none_or(|v| v.is_missing(value));
    (!missing).then_some(value)
}

/// One group's weighted moments.
pub(crate) fn moments(values: &[(f64, f64)]) -> Moments {
    let mut m = Moments::default();
    for (x, w) in values {
        m.add(*x, *w);
    }
    m
}

/// Standard error of the mean.
pub(crate) fn std_error(m: &Moments) -> Option<f64> {
    m.std_dev().map(|sd| sd / m.w.sqrt())
}

/// Levene's test based on the mean (as `T-TEST` and `ONEWAY` report it): a one-way ANOVA on
/// each value's absolute deviation from its group mean. Returns F, df1, df2, significance.
pub(crate) fn levene(groups: &[Vec<(f64, f64)>]) -> Option<(f64, f64, f64, f64)> {
    let deviations: Vec<Vec<(f64, f64)>> = groups
        .iter()
        .map(|g| {
            let mean = moments(g).mean;
            g.iter().map(|(x, w)| ((x - mean).abs(), *w)).collect()
        })
        .collect();
    let (f, df1, df2) = anova(&deviations)?;
    Some((f.f, df1, df2, dist::f_sig(f.f, df1, df2)?))
}

/// Sums of squares of a one-way analysis of variance.
pub(crate) struct AnovaParts {
    pub(crate) between: f64,
    pub(crate) within: f64,
    pub(crate) f: f64,
}

/// One-way ANOVA over weighted groups: the sums of squares, F and the degrees of freedom
/// (between, within). `None` with fewer than two groups or no error degrees of freedom.
pub(crate) fn anova(groups: &[Vec<(f64, f64)>]) -> Option<(AnovaParts, f64, f64)> {
    let stats: Vec<Moments> = groups.iter().map(|g| moments(g)).filter(|m| m.w > 0.0).collect();
    let n: f64 = stats.iter().map(|m| m.w).sum();
    let k = stats.len() as f64;
    if k < 2.0 || n <= k {
        return None;
    }
    let grand = stats.iter().map(|m| m.mean * m.w).sum::<f64>() / n;
    let between: f64 = stats.iter().map(|m| m.w * (m.mean - grand).powi(2)).sum();
    let within: f64 = stats.iter().map(|m| m.m2).sum();
    let (df1, df2) = (k - 1.0, n - k);
    let f = (between / df1) / (within / df2);
    Some((AnovaParts { between, within, f }, df1, df2))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BarValue, CrosstabsOptions, ExamineOptions, OnewayOptions, PostHoc, StatsError};
    use mksigma_core::Value;
    use mksigma_output::Item;

    /// `levels` distinct ids, two cases each, with a measure `y`.
    fn ids(levels: usize) -> Dataset {
        let mut ds = Dataset::new();
        ds.add_variable(Variable::numeric("id")).unwrap();
        ds.add_variable(Variable::numeric("y")).unwrap();
        for i in 0..levels {
            let i = i as f64;
            // Two cases a level, so every group has a variance.
            ds.push_case(vec![Value::Num(i), Value::Num(i % 7.0)]).unwrap();
            ds.push_case(vec![Value::Num(i), Value::Num(i % 5.0 + 1.0)]).unwrap();
        }
        ds
    }

    #[test]
    fn grouping_by_an_id_stops_at_the_cap() {
        // An ID with 100,000 values grouped by mistake used to build a 100,000² table (#31).
        let ds = ids(MAX_LEVELS + 1);
        let too_many = Err(StatsError::TooManyLevels("id".into()));
        let crosstabs = CrosstabsOptions { rows: vec!["id".into()], columns: vec!["id".into()], cells: Default::default(), chisq: true };
        assert_eq!(crate::crosstabs(&ds, &crosstabs).map(|_| ()), too_many);
        assert_eq!(crate::explain::crosstabs(&ds, &["id".into()], &["id".into()], false).map(|_| ()), too_many);
        let oneway = OnewayOptions {
            dependents: vec!["y".into()],
            factor: "id".into(),
            descriptives: false,
            homogeneity: false,
            posthoc: Vec::new(),
            alpha: 0.05,
        };
        assert_eq!(crate::oneway(&ds, &oneway).map(|_| ()), too_many);
        assert_eq!(crate::explain::oneway(&ds, &oneway, false).map(|_| ()), too_many);
        let examine = ExamineOptions { variables: vec!["y".into()], by: Some("id".into()), descriptives: true, boxplot: true };
        assert_eq!(crate::examine(&ds, &examine).map(|_| ()), too_many);
        assert_eq!(crate::bar(&ds, &BarValue::Count, "id").map(|_| ()), too_many);
        assert!(too_many.is_err_and(|e| e.to_string().contains("1000")));
        // At the cap, grouping works.
        assert!(crate::crosstabs(&ids(MAX_LEVELS), &crosstabs).is_ok());
    }

    #[test]
    fn post_hoc_tests_past_their_cap_are_left_out_with_a_note() {
        // Tukey with 60 groups would take minutes; the analysis of variance still runs.
        let ds = ids(60);
        let options = OnewayOptions {
            dependents: vec!["y".into()],
            factor: "id".into(),
            descriptives: false,
            homogeneity: false,
            posthoc: vec![PostHoc::Tukey],
            alpha: 0.05,
        };
        let items = crate::oneway(&ds, &options).unwrap();
        assert!(items.iter().any(|i| matches!(i, Item::Warning(s) if s.contains("at most 50 groups") && s.contains("60"))), "{items:?}");
        let explained = crate::explain::oneway(&ds, &options, false).unwrap();
        assert!(format!("{:?}", explained.sections).contains("at most 50 groups"));
    }
}
