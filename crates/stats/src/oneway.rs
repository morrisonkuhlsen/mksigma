//! `ONEWAY`: one-way analysis of variance of each dependent variable across the groups of a
//! factor, with optional Descriptives, Test of Homogeneity of Variances (Levene, based on the
//! mean) and post hoc Multiple Comparisons (Tukey HSD, Tukey–Kramer for unequal groups;
//! Bonferroni). Missing values are excluded analysis by analysis.

use mksigma_core::{Dataset, Value};
use mksigma_i18n::tr;
use mksigma_output::{Cell, Item, Table};

use crate::groups::{anova, levene, moments, numeric, std_error, valid};
use crate::{Moments, StatsError, dist, n_cell, num, range, value_text};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostHoc {
    Tukey,
    Bonferroni,
}

#[derive(Debug, Clone, Default)]
pub struct Options {
    pub dependents: Vec<String>,
    pub factor: String,
    pub descriptives: bool,
    pub homogeneity: bool,
    pub posthoc: Vec<PostHoc>,
    /// Significance level of the post hoc tests (0.05 by default).
    pub alpha: f64,
}

/// Every ordered pair of groups (I, J), I ≠ J: the mean difference flagged at `alpha`, its
/// standard error, significance and confidence interval.
fn comparisons(test: PostHoc, labels: &[String], stats: &[Moments], mse: f64, df: f64, alpha: f64) -> Vec<(String, Vec<Cell>)> {
    let k = stats.len() as f64;
    let m = k * (k - 1.0) / 2.0;
    let name = match test {
        PostHoc::Tukey => tr!("o-tukey-hsd"),
        PostHoc::Bonferroni => tr!("o-bonferroni"),
    };
    // The critical value for the intervals, the same for every pair.
    let critical = match test {
        PostHoc::Tukey => range::quantile(1.0 - alpha, k, df).map(|q| q / std::f64::consts::SQRT_2),
        PostHoc::Bonferroni => dist::t_quantile(1.0 - alpha / (2.0 * m), df),
    };
    let mut rows = Vec::new();
    for (i, (li, mi)) in labels.iter().zip(stats).enumerate() {
        for (j, (lj, mj)) in labels.iter().zip(stats).enumerate() {
            if i == j {
                continue;
            }
            let diff = mi.mean - mj.mean;
            let se = (mse * (1.0 / mi.w + 1.0 / mj.w)).sqrt();
            let sig = match test {
                PostHoc::Tukey => range::cdf((diff / se).abs() * std::f64::consts::SQRT_2, k, df).map(|p| 1.0 - p),
                PostHoc::Bonferroni => dist::t_sig(diff / se, df).map(|p| (p * m).min(1.0)),
            };
            let flagged = sig.is_some_and(|p| p < alpha);
            let first = j == 0 || (i == 0 && j == 1);
            rows.push((
                if i == 0 && first { name.to_string() } else { String::new() },
                vec![
                    Cell::Text(if first { li.clone() } else { String::new() }),
                    Cell::Text(lj.clone()),
                    if flagged { Cell::Text(format!("{}*", mksigma_i18n::decimal(&format!("{diff:.3}")))) } else { num(Some(diff), 3) },
                    num(Some(se), 3),
                    num(sig, 3),
                    num(critical.map(|c| diff - c * se), 3),
                    num(critical.map(|c| diff + c * se), 3),
                ],
            ));
        }
    }
    rows
}

pub fn oneway(ds: &Dataset, options: &Options) -> Result<Vec<Item>, StatsError> {
    let (fi, fvar) = numeric(ds, &options.factor)?;
    let (mut anova_rows, mut descriptive_rows, mut levene_rows, mut posthoc_rows) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    let alpha = if options.alpha > 0.0 && options.alpha < 1.0 { options.alpha } else { 0.05 };
    for name in &options.dependents {
        let (i, var) = numeric(ds, name)?;
        // Groups by factor value, in ascending order.
        let mut levels: Vec<f64> = Vec::new();
        let mut groups: Vec<Vec<(f64, f64)>> = Vec::new();
        for case in 0..ds.n_cases() {
            let w = ds.case_weight(case);
            let (Some(level), Some(x)) = (valid(ds, fi, case), valid(ds, i, case)) else { continue };
            if w <= 0.0 {
                continue;
            }
            match levels.iter().position(|l| *l == level) {
                Some(k) => groups.get_mut(k).into_iter().for_each(|g| g.push((x, w))),
                None => {
                    levels.push(level);
                    groups.push(vec![(x, w)]);
                }
            }
        }
        let mut order: Vec<usize> = (0..levels.len()).collect();
        order.sort_by(|a, b| levels.get(*a).zip(levels.get(*b)).map_or(std::cmp::Ordering::Equal, |(x, y)| x.total_cmp(y)));
        let groups: Vec<Vec<(f64, f64)>> = order.iter().filter_map(|k| groups.get(*k).cloned()).collect();
        let levels: Vec<f64> = order.iter().filter_map(|k| levels.get(*k).copied()).collect();
        let label = var.display_name().to_string();

        if let Some((parts, _, df2)) = anova(&groups).filter(|_| !options.posthoc.is_empty()) {
            let labels: Vec<String> = levels.iter().map(|l| value_text(fvar, &Value::Num(*l))).collect();
            let stats: Vec<Moments> = groups.iter().map(|g| moments(g)).collect();
            let mse = parts.within / df2;
            for (t, test) in options.posthoc.iter().enumerate() {
                let mut rows = comparisons(*test, &labels, &stats, mse, df2, alpha);
                if t == 0
                    && let Some(first) = rows.first_mut()
                {
                    first.0 = format!("{label}: {}", first.0);
                }
                posthoc_rows.extend(rows);
            }
        }
        match anova(&groups) {
            Some((parts, df1, df2)) => {
                // No variance within the groups: F can't be computed, and has no significance.
                let f = Some(parts.f).filter(|f| f.is_finite());
                let sig = f.and_then(|f| dist::f_sig(f, df1, df2));
                anova_rows.push((
                    label.clone(),
                    vec![
                        Cell::Text(tr!("o-between-groups")),
                        num(Some(parts.between), 3),
                        n_cell(df1),
                        num(Some(parts.between / df1), 3),
                        num(f, 3),
                        num(sig, 3),
                    ],
                ));
                anova_rows.push((
                    String::new(),
                    vec![
                        Cell::Text(tr!("o-within-groups")),
                        num(Some(parts.within), 3),
                        n_cell(df2),
                        num(Some(parts.within / df2), 3),
                        Cell::Empty,
                        Cell::Empty,
                    ],
                ));
                anova_rows.push((
                    String::new(),
                    vec![
                        Cell::Text(tr!("o-total")),
                        num(Some(parts.between + parts.within), 3),
                        n_cell(df1 + df2),
                        Cell::Empty,
                        Cell::Empty,
                        Cell::Empty,
                    ],
                ));
            }
            None => anova_rows.push((label.clone(), vec![Cell::Text(tr!("o-anova-needs-groups"))])),
        }

        if options.descriptives {
            let all: Vec<(f64, f64)> = groups.iter().flatten().copied().collect();
            let lines = levels
                .iter()
                .map(|l| value_text(fvar, &Value::Num(*l)))
                .zip(groups.iter().map(Vec::as_slice))
                .chain(std::iter::once((tr!("o-total").to_string(), all.as_slice())));
            for (k, (level, values)) in lines.enumerate() {
                let m = moments(values);
                let se = std_error(&m);
                let half = se.zip(dist::t_quantile(0.975, m.w - 1.0)).map(|(se, t)| t * se);
                let (min, max) = values.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), (x, _)| (lo.min(*x), hi.max(*x)));
                descriptive_rows.push((
                    if k == 0 { label.clone() } else { String::new() },
                    vec![
                        Cell::Text(level),
                        n_cell(m.w),
                        num(Some(m.mean), 3),
                        num(m.std_dev(), 3),
                        num(se, 3),
                        num(half.map(|h| m.mean - h), 3),
                        num(half.map(|h| m.mean + h), 3),
                        num(Some(min), 2),
                        num(Some(max), 2),
                    ],
                ));
            }
        }
        if options.homogeneity {
            let (f, df1, df2, sig) = levene(&groups).map_or((None, None, None, None), |(f, a, b, s)| (Some(f), Some(a), Some(b), Some(s)));
            levene_rows.push((label, vec![num(f, 3), df1.map_or(Cell::Empty, n_cell), df2.map_or(Cell::Empty, n_cell), num(sig, 3)]));
        }
    }
    let mut items = Vec::new();
    if options.descriptives {
        let columns = [
            String::new(),
            tr!("o-n"),
            tr!("o-mean"),
            tr!("o-std-deviation"),
            tr!("o-std-error"),
            tr!("o-95-ci-lower"),
            tr!("o-95-ci-upper"),
            tr!("o-minimum"),
            tr!("o-maximum"),
        ]
        .map(String::from)
        .to_vec();
        items.push(Item::Table(Table { title: tr!("o-descriptives"), columns, rows: descriptive_rows, categories: Vec::new() }));
    }
    if options.homogeneity {
        let columns = [tr!("o-levene-statistic"), tr!("o-df1"), tr!("o-df2"), tr!("o-sig")].map(String::from).to_vec();
        items.push(Item::Table(Table { title: tr!("o-test-of-homogeneity-of-variances"), columns, rows: levene_rows, categories: Vec::new() }));
    }
    let columns = [String::new(), tr!("o-sum-of-squares"), tr!("o-df"), tr!("o-mean-square"), tr!("o-f"), tr!("o-sig")].map(String::from).to_vec();
    items.push(Item::Table(Table { title: tr!("o-anova"), columns, rows: anova_rows, categories: Vec::new() }));
    if !options.posthoc.is_empty() {
        let mut columns = [tr!("o-i"), tr!("o-j"), tr!("o-mean-difference-i-j"), tr!("o-std-error"), tr!("o-sig")].map(String::from).to_vec();
        columns.extend([
            tr!("o-ci-lower", level = format!("{:.0}", (1.0 - alpha) * 100.0)),
            tr!("o-ci-upper", level = format!("{:.0}", (1.0 - alpha) * 100.0)),
        ]);
        items.push(Item::Table(Table {
            title: tr!("o-multiple-comparisons", factor = fvar.display_name()),
            columns,
            rows: posthoc_rows,
            categories: Vec::new(),
        }));
        items.push(Item::Text(tr!("o-mean-difference-significant", alpha = format!("{alpha}"))));
    }
    Ok(items)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mksigma_core::Variable;

    #[test]
    fn no_variance_within_groups_gives_no_f_and_no_sig() {
        // Every group constant (#28): F would be infinite, and Sig shown beside a blank F.
        let mut ds = Dataset::new();
        ds.add_variable(Variable::numeric("y")).unwrap();
        ds.add_variable(Variable::numeric("g")).unwrap();
        for (g, y) in [(1.0, 2.0), (1.0, 2.0), (2.0, 5.0), (2.0, 5.0)] {
            ds.push_case(vec![Value::Num(y), Value::Num(g)]).unwrap();
        }
        let items = oneway(&ds, &Options { dependents: vec!["y".into()], factor: "g".into(), ..Options::default() }).unwrap();
        let anova = items.iter().find_map(|i| if let Item::Table(t) = i { Some(t) } else { None }).unwrap();
        assert_eq!(anova.rows[0].1[1], Cell::Num { value: 9.0, decimals: 3 }, "between groups still shown");
        assert_eq!((&anova.rows[0].1[4], &anova.rows[0].1[5]), (&Cell::Empty, &Cell::Empty));
    }

    #[test]
    fn anova_by_hand() {
        let mut ds = Dataset::new();
        ds.add_variable(Variable::numeric("y")).unwrap();
        ds.add_variable(Variable::numeric("g")).unwrap();
        // Groups 1: 2 3 4 (mean 3), 2: 6 7 8 (mean 7), 3: 4 5 6 (mean 5); grand mean 5.
        for (g, ys) in [(1.0, [2.0, 3.0, 4.0]), (2.0, [6.0, 7.0, 8.0]), (3.0, [4.0, 5.0, 6.0])] {
            for y in ys {
                ds.push_case(vec![Value::Num(y), Value::Num(g)]).unwrap();
            }
        }
        let options = Options { dependents: vec!["y".into()], factor: "g".into(), descriptives: true, homogeneity: true, ..Options::default() };
        let items = oneway(&ds, &options).unwrap();
        let Item::Table(anova) = &items[2] else { panic!("ANOVA table last") };
        // SSB = 3·(4 + 4 + 0) = 24, SSW = 2 + 2 + 2 = 6, F = (24/2)/(6/6) = 12.
        assert_eq!(anova.rows[0].1[1], Cell::Num { value: 24.0, decimals: 3 });
        assert_eq!(anova.rows[1].1[1], Cell::Num { value: 6.0, decimals: 3 });
        assert_eq!(anova.rows[0].1[4], Cell::Num { value: 12.0, decimals: 3 });
        let Cell::Num { value: sig, .. } = anova.rows[0].1[5] else { panic!("sig") };
        assert!((sig - 0.008).abs() < 0.001, "F(2, 6) = 12 has p ≈ .008");
        let Item::Table(levene) = &items[1] else { panic!() };
        assert_eq!(levene.rows[0].1[0], Cell::Num { value: 0.0, decimals: 3 }, "equal spreads");
        let Item::Table(descriptives) = &items[0] else { panic!() };
        assert_eq!(descriptives.rows[3].1[0], Cell::Text("Total".into()));
        assert_eq!(descriptives.rows[3].1[1], Cell::Int(9));
    }

    #[test]
    fn tukey_and_bonferroni_by_hand() {
        let mut ds = Dataset::new();
        ds.add_variable(Variable::numeric("y")).unwrap();
        ds.add_variable(Variable::numeric("g")).unwrap();
        for (g, ys) in [(1.0, [2.0, 3.0, 4.0]), (2.0, [6.0, 7.0, 8.0]), (3.0, [4.0, 5.0, 6.0])] {
            for y in ys {
                ds.push_case(vec![Value::Num(y), Value::Num(g)]).unwrap();
            }
        }
        let options = Options {
            dependents: vec!["y".into()],
            factor: "g".into(),
            posthoc: vec![PostHoc::Tukey, PostHoc::Bonferroni],
            alpha: 0.05,
            ..Options::default()
        };
        let items = oneway(&ds, &options).unwrap();
        let Item::Table(t) = &items[1] else { panic!("multiple comparisons after ANOVA") };
        // MSE = 1, n = 3 each: SE = √(2/3). 1 vs 2: difference −4.
        let se = (2.0f64 / 3.0).sqrt();
        assert_eq!(t.rows[0].1[2], Cell::Text("-4.000*".into()));
        assert_eq!(t.rows[0].1[3], Cell::Num { value: se, decimals: 3 });
        // Tukey: p = 1 − P(Q ≤ 4/√(1/3) ; k = 3, df = 6).
        let Cell::Num { value: tukey, .. } = t.rows[0].1[4] else { panic!() };
        let expected = 1.0 - range::cdf(4.0 / (1.0f64 / 3.0).sqrt(), 3.0, 6.0).unwrap();
        assert!((tukey - expected).abs() < 1e-12);
        // Bonferroni: three comparisons times the t test's p.
        let Cell::Num { value: bonferroni, .. } = t.rows[6].1[4] else { panic!() };
        let expected = 3.0 * dist::t_sig(-4.0 / se, 6.0).unwrap();
        assert!((bonferroni - expected).abs() < 1e-12);
        assert_eq!(t.rows.len(), 12, "six ordered pairs per test");
    }
}
