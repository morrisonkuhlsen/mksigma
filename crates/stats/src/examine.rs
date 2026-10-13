//! `EXAMINE`: the Descriptives table (mean and its confidence interval, 5% trimmed mean, median,
//! variance, standard deviation, minimum, maximum, range, interquartile range, skewness and
//! kurtosis with their standard errors) and boxplots, for each variable, optionally by group.
//! Weights count as frequencies; missing values are excluded variable by variable.
//!
//! Percentiles in the table use the weighted average at (W + 1)p (SPSS's HAVERAGE); boxplots use
//! Tukey's hinges, as SPSS's boxplots do.

use mksigma_core::{Dataset, Value};
use mksigma_output::chart::BoxStats;
use mksigma_output::{Cell, Chart, Item, Table};

use crate::groups::{moments, numeric, std_error, valid, valid_value};
use crate::{StatsError, dist, n_cell, num, value_text};

#[derive(Debug, Clone, Default)]
pub struct Options {
    pub variables: Vec<String>,
    /// Group by this variable's values (`BY`).
    pub by: Option<String>,
    pub descriptives: bool,
    pub boxplot: bool,
}

/// A group's key (the `BY` value, `None` without `BY`) and its values with their weights.
type Group = (Option<Value>, Vec<(f64, f64)>);

/// Values sorted ascending with their weights.
struct Sorted(Vec<(f64, f64)>);

impl Sorted {
    fn new(mut values: Vec<(f64, f64)>) -> Self {
        values.sort_by(|a, b| a.0.total_cmp(&b.0));
        Sorted(values)
    }

    fn total(&self) -> f64 {
        self.0.iter().map(|(_, w)| w).sum()
    }

    /// The value at 1-based rank `r` of the data with weights as frequencies.
    fn at_rank(&self, r: f64) -> f64 {
        let mut seen = 0.0;
        for (x, w) in &self.0 {
            seen += w;
            if seen >= r - 1e-9 {
                return *x;
            }
        }
        self.0.last().map_or(f64::NAN, |v| v.0)
    }

    /// The value at a depth that may end in .5 (the mean of the two neighbouring ranks).
    fn at_depth(&self, d: f64) -> f64 {
        let (lo, hi) = (d.floor(), d.ceil());
        (self.at_rank(lo) + self.at_rank(hi)) / 2.0
    }

    /// The `p` percentile by the weighted average at (W + 1)p.
    fn percentile(&self, p: f64) -> f64 {
        let w = self.total();
        let pos = (w + 1.0) * p;
        if pos <= 1.0 {
            return self.at_rank(1.0);
        }
        if pos >= w {
            return self.at_rank(w);
        }
        let k = pos.floor();
        let frac = pos - k;
        let (a, b) = (self.at_rank(k), self.at_rank(k + 1.0));
        a + frac * (b - a)
    }

    /// Tukey's hinges: the medians of the lower and upper halves (the median joins both halves
    /// when the count is odd).
    fn hinges(&self) -> (f64, f64) {
        let n = self.total().round();
        let depth = ((((n + 1.0) / 2.0).floor()) + 1.0) / 2.0;
        (self.at_depth(depth), self.at_depth(n + 1.0 - depth))
    }

    /// Mean after cutting `share` of the weight from each end (partial weights at the cuts).
    fn trimmed_mean(&self, share: f64) -> f64 {
        let cut = self.total() * share;
        let (mut below, mut sum, mut kept) = (0.0, 0.0, 0.0);
        let total = self.total();
        for (x, w) in &self.0 {
            let start = below;
            let end = below + w;
            below = end;
            let keep = (end.min(total - cut) - start.max(cut)).max(0.0);
            sum += keep * x;
            kept += keep;
        }
        sum / kept
    }

    fn boxplot(&self, label: String) -> BoxStats {
        let (q1, q3) = self.hinges();
        let median = self.at_depth((self.total().round() + 1.0) / 2.0);
        let iqr = q3 - q1;
        let inside = |x: f64| x >= q1 - 1.5 * iqr && x <= q3 + 1.5 * iqr;
        let low = self.0.iter().map(|v| v.0).find(|x| inside(*x)).unwrap_or(q1);
        let high = self.0.iter().rev().map(|v| v.0).find(|x| inside(*x)).unwrap_or(q3);
        let extreme = |x: f64| x < q1 - 3.0 * iqr || x > q3 + 3.0 * iqr;
        let outliers = self.0.iter().map(|v| v.0).filter(|x| !inside(*x) && !extreme(*x)).collect();
        let extremes = self.0.iter().map(|v| v.0).filter(|x| extreme(*x)).collect();
        BoxStats { label, low, q1, median, q3, high, outliers, extremes }
    }
}

fn descriptive_rows(label: String, values: &Sorted) -> Vec<(String, Vec<Cell>)> {
    let m = moments(&values.0);
    let w = m.w;
    let sd = m.std_dev();
    let se = std_error(&m);
    let half = se.zip(dist::t_quantile(0.975, w - 1.0)).map(|(se, t)| t * se);
    let (min, max) = (values.0.first().map(|v| v.0), values.0.last().map(|v| v.0));
    // Skewness and kurtosis: the bias-corrected G1 and G2.
    let central = |k: i32| values.0.iter().map(|(x, wt)| wt * (x - m.mean).powi(k)).sum::<f64>();
    let skew = sd.filter(|_| w > 2.0).map(|s| w / ((w - 1.0) * (w - 2.0)) * central(3) / s.powi(3));
    let se_skew = (w > 2.0).then(|| (6.0 * w * (w - 1.0) / ((w - 2.0) * (w + 1.0) * (w + 3.0))).sqrt());
    let kurt = sd
        .filter(|_| w > 3.0)
        .map(|s| w * (w + 1.0) / ((w - 1.0) * (w - 2.0) * (w - 3.0)) * central(4) / s.powi(4) - 3.0 * (w - 1.0).powi(2) / ((w - 2.0) * (w - 3.0)));
    let se_kurt = se_skew.filter(|_| w > 3.0).map(|s| (4.0 * (w * w - 1.0) * s * s / ((w - 3.0) * (w + 5.0))).sqrt());
    let lines: Vec<(&str, Option<f64>, Option<f64>)> = vec![
        ("Mean", Some(m.mean), se),
        ("95% CI for Mean, Lower Bound", half.map(|h| m.mean - h), None),
        ("95% CI for Mean, Upper Bound", half.map(|h| m.mean + h), None),
        ("5% Trimmed Mean", Some(values.trimmed_mean(0.05)), None),
        ("Median", Some(values.percentile(0.5)), None),
        ("Variance", sd.map(|s| s * s), None),
        ("Std. Deviation", sd, None),
        ("Minimum", min, None),
        ("Maximum", max, None),
        ("Range", min.zip(max).map(|(a, b)| b - a), None),
        ("Interquartile Range", Some(values.percentile(0.75) - values.percentile(0.25)), None),
        ("Skewness", skew, se_skew),
        ("Kurtosis", kurt, se_kurt),
    ];
    lines
        .into_iter()
        .enumerate()
        .map(|(i, (name, statistic, error))| {
            (if i == 0 { label.clone() } else { String::new() }, vec![Cell::Text(name.into()), num(statistic, 3), num(error, 3)])
        })
        .collect()
}

pub fn examine(ds: &Dataset, options: &Options) -> Result<Vec<Item>, StatsError> {
    let by = options.by.as_ref().map(|b| ds.variable(b).ok_or_else(|| StatsError::UnknownVariable(b.clone()))).transpose()?;
    let mut summary = Vec::new();
    let mut descriptives = Vec::new();
    let mut charts = Vec::new();
    for name in &options.variables {
        let (vi, var) = numeric(ds, name)?;
        // One group of values per category of `by`, or a single group.
        let mut groups: Vec<Group> = Vec::new();
        let mut missing = 0.0;
        for case in 0..ds.n_cases() {
            let w = ds.case_weight(case);
            if w <= 0.0 {
                continue;
            }
            let key = match by {
                Some((bi, _)) => match valid_value(ds, bi, case) {
                    Some(v) => Some(v.clone()),
                    None => continue,
                },
                None => None,
            };
            let Some(x) = valid(ds, vi, case) else {
                missing += w;
                continue;
            };
            match groups.iter_mut().find(|(k, _)| *k == key) {
                Some((_, list)) => list.push((x, w)),
                None => groups.push((key, vec![(x, w)])),
            }
        }
        groups.sort_by(|a, b| match (&a.0, &b.0) {
            (Some(Value::Num(x)), Some(Value::Num(y))) => x.total_cmp(y),
            (Some(Value::Str(x)), Some(Value::Str(y))) => x.cmp(y),
            _ => std::cmp::Ordering::Equal,
        });
        let valid_n: f64 = groups.iter().flat_map(|(_, g)| g.iter().map(|(_, w)| w)).sum();
        summary.push((var.display_name().to_string(), vec![n_cell(valid_n), n_cell(missing), n_cell(valid_n + missing)]));
        let mut boxes = Vec::new();
        for (key, values) in groups {
            let group_label = match (&key, by) {
                (Some(v), Some((_, bvar))) => value_text(bvar, v),
                _ => var.display_name().to_string(),
            };
            let sorted = Sorted::new(values);
            if options.descriptives {
                let label = if by.is_some() { format!("{}: {group_label}", var.display_name()) } else { group_label.clone() };
                descriptives.extend(descriptive_rows(label, &sorted));
            }
            boxes.push(sorted.boxplot(group_label));
        }
        if options.boxplot && !boxes.is_empty() {
            let x_label = by.map_or_else(String::new, |(_, b)| b.display_name().to_string());
            charts.push(Item::Chart(Chart::Boxplot {
                title: var.display_name().to_string(),
                x_label,
                y_label: var.display_name().to_string(),
                boxes,
            }));
        }
    }
    let mut items = vec![Item::Table(Table {
        title: "Case Processing Summary".into(),
        columns: ["Valid N", "Missing N", "Total N"].map(String::from).to_vec(),
        rows: summary,
    })];
    if options.descriptives {
        items.push(Item::Table(Table {
            title: "Descriptives".into(),
            columns: ["", "Statistic", "Std. Error"].map(String::from).to_vec(),
            rows: descriptives,
        }));
    }
    items.extend(charts);
    Ok(items)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sorted(xs: &[f64]) -> Sorted {
        Sorted::new(xs.iter().map(|x| (*x, 1.0)).collect())
    }

    #[test]
    fn percentiles_hinges_and_trimming() {
        let s = sorted(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]);
        // HAVERAGE: 25th at 9 × .25 = 2.25 → 2.25; 75th at 6.75 → 6.75.
        assert_eq!(s.percentile(0.25), 2.25);
        assert_eq!(s.percentile(0.75), 6.75);
        assert_eq!(s.percentile(0.5), 4.5);
        // Tukey: n = 8, median depth 4.5, hinge depth 2.5 → 2.5 and 6.5.
        assert_eq!(s.hinges(), (2.5, 6.5));
        let odd = sorted(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0]);
        assert_eq!(odd.hinges(), (3.0, 7.0), "n = 9: hinge depth 3");
        // 5% of 20 is one case from each end.
        let twenty = sorted(&(1..=20).map(f64::from).collect::<Vec<_>>());
        assert_eq!(twenty.trimmed_mean(0.05), (2..=19).sum::<i32>() as f64 / 18.0);
        // Weights are frequencies: a weight of 2 is two cases.
        let weighted = Sorted::new(vec![(1.0, 2.0), (5.0, 1.0)]);
        assert_eq!(weighted.percentile(0.5), 1.0);
    }

    #[test]
    fn boxplot_outliers_and_extremes() {
        let b = sorted(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 15.0, 40.0]).boxplot("x".into());
        // Hinges 3 and 8 (depth 3); IQR 5: fences 0.5..15.5 and -12..23.
        assert_eq!((b.q1, b.median, b.q3), (3.0, 5.5, 8.0));
        assert_eq!((b.low, b.high), (1.0, 15.0));
        assert!(b.outliers.is_empty());
        assert_eq!(b.extremes, [40.0]);
    }
}
