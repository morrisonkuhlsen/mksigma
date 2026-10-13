//! `ONEWAY`: one-way analysis of variance of each dependent variable across the groups of a
//! factor, with optional Descriptives and Test of Homogeneity of Variances (Levene, based on the
//! mean). Missing values are excluded analysis by analysis.

use mksigma_core::{Dataset, Value};
use mksigma_output::{Cell, Item, Table};

use crate::groups::{anova, levene, moments, numeric, std_error, valid};
use crate::{StatsError, dist, n_cell, num, value_text};

#[derive(Debug, Clone, Default)]
pub struct Options {
    pub dependents: Vec<String>,
    pub factor: String,
    pub descriptives: bool,
    pub homogeneity: bool,
}

pub fn oneway(ds: &Dataset, options: &Options) -> Result<Vec<Item>, StatsError> {
    let (fi, fvar) = numeric(ds, &options.factor)?;
    let (mut anova_rows, mut descriptive_rows, mut levene_rows) = (Vec::new(), Vec::new(), Vec::new());
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

        match anova(&groups) {
            Some((parts, df1, df2)) => {
                let sig = dist::f_sig(parts.f, df1, df2);
                anova_rows.push((
                    label.clone(),
                    vec![
                        Cell::Text("Between Groups".into()),
                        num(Some(parts.between), 3),
                        n_cell(df1),
                        num(Some(parts.between / df1), 3),
                        num(Some(parts.f), 3),
                        num(sig, 3),
                    ],
                ));
                anova_rows.push((
                    String::new(),
                    vec![
                        Cell::Text("Within Groups".into()),
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
                        Cell::Text("Total".into()),
                        num(Some(parts.between + parts.within), 3),
                        n_cell(df1 + df2),
                        Cell::Empty,
                        Cell::Empty,
                        Cell::Empty,
                    ],
                ));
            }
            None => anova_rows.push((label.clone(), vec![Cell::Text("needs at least two groups with more cases than groups".into())])),
        }

        if options.descriptives {
            let all: Vec<(f64, f64)> = groups.iter().flatten().copied().collect();
            let lines = levels
                .iter()
                .map(|l| value_text(fvar, &Value::Num(*l)))
                .zip(groups.iter().map(Vec::as_slice))
                .chain(std::iter::once(("Total".to_string(), all.as_slice())));
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
        let columns =
            ["", "N", "Mean", "Std. Deviation", "Std. Error", "95% CI Lower", "95% CI Upper", "Minimum", "Maximum"].map(String::from).to_vec();
        items.push(Item::Table(Table { title: "Descriptives".into(), columns, rows: descriptive_rows }));
    }
    if options.homogeneity {
        let columns = ["Levene Statistic", "df1", "df2", "Sig."].map(String::from).to_vec();
        items.push(Item::Table(Table { title: "Test of Homogeneity of Variances".into(), columns, rows: levene_rows }));
    }
    let columns = ["", "Sum of Squares", "df", "Mean Square", "F", "Sig."].map(String::from).to_vec();
    items.push(Item::Table(Table { title: "ANOVA".into(), columns, rows: anova_rows }));
    Ok(items)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mksigma_core::Variable;

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
        let options = Options { dependents: vec!["y".into()], factor: "g".into(), descriptives: true, homogeneity: true };
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
}
