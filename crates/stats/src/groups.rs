//! Shared by `T-TEST` and `ONEWAY`: valid values with their case weights, statistics per group,
//! and Levene's test of equal variances.

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
