//! Charts from the data: histograms (with an optional normal curve), bar charts of counts or
//! means by category, and scatterplots. Missing values are left out; weights count as
//! frequencies (except in scatterplots, where each case is one point).

use mksigma_core::{Dataset, Value, Variable};
use mksigma_i18n::tr;
use mksigma_output::chart::ticks;
use mksigma_output::{Cases, Category, Chart, Item};

use crate::groups::{Categories, moments, numeric, valid, valid_value};
use crate::{StatsError, category, value_text};

/// A histogram of `name`. The bin count follows Sturges' rule (1 + log₂ n, 5 to 30 bins), on
/// round bin edges; the last bin includes its upper edge.
pub fn histogram(ds: &Dataset, name: &str, normal: bool) -> Result<Item, StatsError> {
    let (i, var) = numeric(ds, name)?;
    let values: Vec<(f64, f64)> = (0..ds.n_cases()).filter_map(|c| Some((valid(ds, i, c)?, ds.case_weight(c)))).filter(|(_, w)| *w > 0.0).collect();
    if values.is_empty() {
        return Ok(Item::Warning(tr!("o-nothing-to-chart", name = var.name.as_str())));
    }
    let (lo, hi) = values.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), (x, _)| (a.min(*x), b.max(*x)));
    let m = moments(&values);
    let target = ((m.w.log2() + 1.0).ceil() as usize).clamp(5, 30);
    // Round steps only grow (1, 2, 2.5, 5, 10...), so ask for half again as many edges: the bins
    // land near Sturges' count instead of well under it.
    let edges = ticks(lo, hi, target * 3 / 2);
    let mut bins: Vec<(f64, f64, f64)> = edges
        .windows(2)
        .filter_map(|w| match w {
            [a, b] => Some((*a, *b, 0.0)),
            _ => None,
        })
        .collect();
    let last = bins.len().saturating_sub(1);
    for (x, w) in &values {
        let k = bins.iter().position(|(a, b, _)| *a <= *x && *x < *b).unwrap_or(last);
        if let Some(bin) = bins.get_mut(k) {
            bin.2 += w;
        }
    }
    let normal = if normal { m.std_dev().map(|sd| (m.mean, sd)) } else { None };
    let categories = bins
        .iter()
        .enumerate()
        .map(|(k, (low, high, _))| Category {
            variable: var.name.clone(),
            cases: Cases::Range { low: *low, high: *high, closed: k == last },
            label: tr!("o-bin", name = var.display_name(), low = format!("{low}"), high = format!("{high}")),
        })
        .collect();
    Ok(Item::Chart(Chart::Histogram {
        title: var.display_name().to_string(),
        x_label: var.display_name().to_string(),
        bins,
        normal,
        n: m.w,
        categories,
    }))
}

/// What the bars of a bar chart measure.
#[derive(Debug, Clone, PartialEq)]
pub enum BarValue {
    /// The (weighted) number of cases in each category.
    Count,
    /// The mean of a variable in each category.
    Mean(String),
}

/// The cases of each category of `by`, in ascending order of value: the measured variable's
/// value (1 for a count) and the weight. With the measured variable, when there is one.
pub(crate) type BarGroups<'a> = (&'a Variable, Option<&'a Variable>, Vec<(Value, Vec<(f64, f64)>)>);

pub(crate) fn bar_groups<'a>(ds: &'a Dataset, value: &BarValue, by: &str) -> Result<BarGroups<'a>, StatsError> {
    let (bi, bvar) = ds.variable(by).ok_or_else(|| StatsError::UnknownVariable(by.to_string()))?;
    let measured = match value {
        BarValue::Count => None,
        BarValue::Mean(name) => Some(numeric(ds, name)?),
    };
    let mut groups = Categories::<Vec<(f64, f64)>>::new(bvar);
    for case in 0..ds.n_cases() {
        let w = ds.case_weight(case);
        let Some(category) = valid_value(ds, bi, case) else { continue };
        let x = match measured {
            Some((mi, _)) => match valid(ds, mi, case) {
                Some(x) => x,
                None => continue,
            },
            None => 1.0,
        };
        if w <= 0.0 {
            continue;
        }
        groups.of(category)?.push((x, w));
    }
    let groups = groups.sorted();
    Ok((bvar, measured.map(|(_, v)| v), groups))
}

/// A bar per category of `by` (labels shown), in ascending order of value.
pub fn bar(ds: &Dataset, value: &BarValue, by: &str) -> Result<Item, StatsError> {
    let (bvar, measured, groups) = bar_groups(ds, value, by)?;
    let bars = groups
        .iter()
        .map(|(v, list)| {
            let m = moments(list);
            (value_text(bvar, v), if measured.is_some() { m.mean } else { m.w })
        })
        .collect();
    // Every category is a valid value, so each bar gets one; all or none keeps them in step.
    let categories = groups.iter().map(|(v, _)| category(bvar, v)).collect::<Option<Vec<_>>>().unwrap_or_default();
    let (y_label, title) = match measured {
        Some(var) => (tr!("o-mean-of", name = var.display_name()), tr!("o-mean-of-by", name = var.display_name(), by = bvar.display_name())),
        None => (tr!("o-count").to_string(), bvar.display_name().to_string()),
    };
    Ok(Item::Chart(Chart::Bar { title, x_label: bvar.display_name().to_string(), y_label, bars, categories }))
}

/// A point per case valid on both variables.
pub fn scatter(ds: &Dataset, x: &str, y: &str) -> Result<Item, StatsError> {
    let ((xi, xvar), (yi, yvar)) = (numeric(ds, x)?, numeric(ds, y)?);
    let points: Vec<(f64, f64)> =
        (0..ds.n_cases()).filter(|&c| ds.case_weight(c) > 0.0).filter_map(|c| Some((valid(ds, xi, c)?, valid(ds, yi, c)?))).collect();
    if points.is_empty() {
        return Ok(Item::Warning(tr!("o-no-pairs", x = xvar.name.as_str(), y = yvar.name.as_str())));
    }
    Ok(Item::Chart(Chart::Scatter {
        title: tr!("o-by", y = yvar.display_name(), x = xvar.display_name()),
        x_label: xvar.display_name().to_string(),
        y_label: yvar.display_name().to_string(),
        points,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use mksigma_core::{MissingValues, Variable};

    fn data() -> Dataset {
        let mut ds = Dataset::new();
        let mut x = Variable::numeric("x");
        x.missing = MissingValues::Discrete(vec![Value::Num(99.0)]);
        ds.add_variable(x).unwrap();
        let mut g = Variable::numeric("g");
        g.value_labels = vec![(Value::Num(1.0), "One".into()), (Value::Num(2.0), "Two".into())];
        ds.add_variable(g).unwrap();
        for (x, g) in [(1.0, 2.0), (2.0, 1.0), (2.5, 1.0), (4.0, 2.0), (9.0, 2.0), (99.0, 1.0)] {
            ds.push_case(vec![Value::Num(x), Value::Num(g)]).unwrap();
        }
        ds
    }

    #[test]
    fn histogram_bins_every_valid_value_once() {
        let Item::Chart(Chart::Histogram { bins, normal, n, .. }) = histogram(&data(), "x", true).unwrap() else { panic!() };
        assert_eq!(n, 5.0, "the user-missing 99 is left out");
        assert_eq!(bins.iter().map(|b| b.2).sum::<f64>(), 5.0);
        assert!(bins.first().unwrap().0 <= 1.0 && bins.last().unwrap().1 >= 9.0);
        assert!(bins.windows(2).all(|w| w[0].1 == w[1].0), "bins touch");
        assert!(normal.is_some());
    }

    #[test]
    fn bars_of_counts_and_means() {
        let Item::Chart(Chart::Bar { bars, .. }) = bar(&data(), &BarValue::Count, "g").unwrap() else { panic!() };
        assert_eq!(bars, [("One".to_string(), 3.0), ("Two".to_string(), 3.0)]);
        let Item::Chart(Chart::Bar { bars, y_label, .. }) = bar(&data(), &BarValue::Mean("x".into()), "g").unwrap() else { panic!() };
        // One: 2, 2.5 (99 is missing); Two: 1, 4, 9.
        assert_eq!((bars[0].0.as_str(), bars[1].0.as_str()), ("One", "Two"));
        assert!((bars[0].1 - 2.25).abs() < 1e-12 && (bars[1].1 - 14.0 / 3.0).abs() < 1e-12);
        assert_eq!(y_label, "Mean x");
    }

    #[test]
    fn scatter_points() {
        let Item::Chart(Chart::Scatter { points, .. }) = scatter(&data(), "g", "x").unwrap() else { panic!() };
        assert_eq!(points.len(), 5);
    }
}
