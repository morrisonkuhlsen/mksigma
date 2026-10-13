//! `T-TEST`: one sample against a test value, two independent groups (with Levene's test and
//! both the pooled and the Welch version), and paired samples. Missing values are excluded
//! analysis by analysis; case weights count as frequencies.

use mksigma_core::{Dataset, Value};
use mksigma_output::{Cell, Item, Table};

use crate::groups::{levene, moments, numeric, std_error, valid, valid_value};
use crate::{Moments, StatsError, dist, n_cell, num, value_text};

#[derive(Debug, Clone, PartialEq)]
pub enum TTest {
    OneSample { variables: Vec<String>, test_value: f64 },
    Independent { variables: Vec<String>, group: String, first: Value, second: Value },
    Paired { pairs: Vec<(String, String)> },
}

fn ci_label(ci: f64, end: &str) -> String {
    format!("{:.0}% CI {end}", ci * 100.0)
}

/// `mean ± t × se` for a two-sided interval of confidence `ci`.
fn interval(mean: f64, se: Option<f64>, df: f64, ci: f64) -> (Cell, Cell) {
    let half = se.zip(dist::t_quantile(0.5 + ci / 2.0, df)).map(|(se, t)| t * se);
    (num(half.map(|h| mean - h), 3), num(half.map(|h| mean + h), 3))
}

fn one_sample(ds: &Dataset, variables: &[String], test_value: f64, ci: f64) -> Result<Vec<Item>, StatsError> {
    let (mut stats, mut tests) = (Vec::new(), Vec::new());
    for name in variables {
        let (i, var) = numeric(ds, name)?;
        let values: Vec<(f64, f64)> =
            (0..ds.n_cases()).filter_map(|c| valid(ds, i, c).map(|x| (x, ds.case_weight(c)))).filter(|(_, w)| *w > 0.0).collect();
        let m = moments(&values);
        let se = std_error(&m);
        stats.push((var.display_name().to_string(), vec![n_cell(m.w), num((m.w > 0.0).then_some(m.mean), 3), num(m.std_dev(), 3), num(se, 3)]));
        let df = m.w - 1.0;
        let diff = m.mean - test_value;
        let t = se.map(|se| diff / se);
        let (lower, upper) = interval(diff, se, df, ci);
        let sig = t.and_then(|t| dist::t_sig(t, df));
        tests.push((
            var.display_name().to_string(),
            vec![num(t, 3), n_cell(df.max(0.0)), num(sig, 3), num((m.w > 0.0).then_some(diff), 3), lower, upper],
        ));
    }
    let stat_columns = ["N", "Mean", "Std. Deviation", "Std. Error Mean"].map(String::from).to_vec();
    let mut test_columns = ["t", "df", "Sig. (2-tailed)", "Mean Difference"].map(String::from).to_vec();
    test_columns.extend([ci_label(ci, "Lower"), ci_label(ci, "Upper")]);
    Ok(vec![
        Item::Table(Table { title: "One-Sample Statistics".into(), columns: stat_columns, rows: stats }),
        Item::Table(Table { title: format!("One-Sample Test (Test Value = {test_value})"), columns: test_columns, rows: tests }),
    ])
}

fn independent(ds: &Dataset, variables: &[String], group: &str, first: &Value, second: &Value, ci: f64) -> Result<Vec<Item>, StatsError> {
    let (gi, gvar) = ds.variable(group).ok_or_else(|| StatsError::UnknownVariable(group.to_string()))?;
    let (mut stats, mut tests) = (Vec::new(), Vec::new());
    for name in variables {
        let (i, var) = numeric(ds, name)?;
        let mut groups: [Vec<(f64, f64)>; 2] = [Vec::new(), Vec::new()];
        for case in 0..ds.n_cases() {
            let w = ds.case_weight(case);
            let (Some(g), Some(x)) = (valid_value(ds, gi, case), valid(ds, i, case)) else { continue };
            if w <= 0.0 {
                continue;
            }
            if g == first {
                groups[0].push((x, w));
            } else if g == second {
                groups[1].push((x, w));
            }
        }
        let [m1, m2]: [Moments; 2] = [moments(&groups[0]), moments(&groups[1])];
        for (k, (m, value)) in [(&m1, first), (&m2, second)].into_iter().enumerate() {
            let label = if k == 0 { var.display_name().to_string() } else { String::new() };
            stats.push((
                label,
                vec![
                    Cell::Text(value_text(gvar, value)),
                    n_cell(m.w),
                    num((m.w > 0.0).then_some(m.mean), 3),
                    num(m.std_dev(), 3),
                    num(std_error(m), 3),
                ],
            ));
        }

        let diff = m1.mean - m2.mean;
        let (v1, v2) = (m1.std_dev().map(|s| s * s), m2.std_dev().map(|s| s * s));
        let lev = levene(&groups);
        // Pooled variance.
        let df_pooled = m1.w + m2.w - 2.0;
        let se_pooled = v1.zip(v2).map(|(v1, v2)| (((m1.w - 1.0) * v1 + (m2.w - 1.0) * v2) / df_pooled * (1.0 / m1.w + 1.0 / m2.w)).sqrt());
        // Welch–Satterthwaite.
        let se_welch = v1.zip(v2).map(|(v1, v2)| (v1 / m1.w + v2 / m2.w).sqrt());
        let df_welch = v1.zip(v2).map(|(v1, v2)| {
            let (a, b) = (v1 / m1.w, v2 / m2.w);
            (a + b).powi(2) / (a * a / (m1.w - 1.0) + b * b / (m2.w - 1.0))
        });
        let row = |se: Option<f64>, df: Option<f64>, df_cell: Cell, levene: [Cell; 2]| {
            let t = se.map(|se| diff / se);
            let sig = t.zip(df).and_then(|(t, df)| dist::t_sig(t, df));
            let (lower, upper) = match df {
                Some(df) => interval(diff, se, df, ci),
                None => (Cell::Empty, Cell::Empty),
            };
            let [f, f_sig] = levene;
            vec![Cell::Text(String::new()), f, f_sig, num(t, 3), df_cell, num(sig, 3), num(Some(diff), 3), num(se, 3), lower, upper]
        };
        let levene_cells = [num(lev.map(|l| l.0), 3), num(lev.map(|l| l.3), 3)];
        let mut equal = row(se_pooled, Some(df_pooled), n_cell(df_pooled), levene_cells);
        let mut unequal = row(se_welch, df_welch, num(df_welch, 3), [Cell::Empty, Cell::Empty]);
        equal[0] = Cell::Text("Equal variances assumed".into());
        unequal[0] = Cell::Text("Equal variances not assumed".into());
        tests.push((var.display_name().to_string(), equal));
        tests.push((String::new(), unequal));
    }
    let stat_columns = [gvar.display_name(), "N", "Mean", "Std. Deviation", "Std. Error Mean"].map(String::from).to_vec();
    let mut test_columns =
        ["", "Levene F", "Levene Sig.", "t", "df", "Sig. (2-tailed)", "Mean Difference", "Std. Error Difference"].map(String::from).to_vec();
    test_columns.extend([ci_label(ci, "Lower"), ci_label(ci, "Upper")]);
    Ok(vec![
        Item::Table(Table { title: "Group Statistics".into(), columns: stat_columns, rows: stats }),
        Item::Table(Table { title: "Independent Samples Test".into(), columns: test_columns, rows: tests }),
    ])
}

fn paired(ds: &Dataset, pairs: &[(String, String)], ci: f64) -> Result<Vec<Item>, StatsError> {
    let (mut stats, mut correlations, mut tests) = (Vec::new(), Vec::new(), Vec::new());
    for (p, (a, b)) in pairs.iter().enumerate() {
        let ((ai, avar), (bi, bvar)) = (numeric(ds, a)?, numeric(ds, b)?);
        // Cases valid on both variables.
        let rows: Vec<(f64, f64, f64)> =
            (0..ds.n_cases()).filter_map(|c| Some((valid(ds, ai, c)?, valid(ds, bi, c)?, ds.case_weight(c)))).filter(|(_, _, w)| *w > 0.0).collect();
        let ma = moments(&rows.iter().map(|(x, _, w)| (*x, *w)).collect::<Vec<_>>());
        let mb = moments(&rows.iter().map(|(_, y, w)| (*y, *w)).collect::<Vec<_>>());
        let md = moments(&rows.iter().map(|(x, y, w)| (x - y, *w)).collect::<Vec<_>>());
        let pair = format!("Pair {}", p + 1);
        for (k, (var, m)) in [(avar, &ma), (bvar, &mb)].into_iter().enumerate() {
            let label = if k == 0 { pair.clone() } else { String::new() };
            stats.push((
                label,
                vec![
                    Cell::Text(var.display_name().to_string()),
                    num((m.w > 0.0).then_some(m.mean), 3),
                    n_cell(m.w),
                    num(m.std_dev(), 3),
                    num(std_error(m), 3),
                ],
            ));
        }
        let n = ma.w;
        let sxy: f64 = rows.iter().map(|(x, y, w)| w * (x - ma.mean) * (y - mb.mean)).sum();
        let r = Some(sxy / (ma.m2 * mb.m2).sqrt()).filter(|r| r.is_finite());
        // Significance of r: t = r √((n − 2) / (1 − r²)) with n − 2 degrees of freedom.
        let r_sig = r.and_then(|r| dist::t_sig(r * ((n - 2.0) / (1.0 - r * r)).sqrt(), n - 2.0));
        let label = format!("{} & {}", avar.display_name(), bvar.display_name());
        correlations.push((pair.clone(), vec![Cell::Text(label), n_cell(n), num(r, 3), num(r_sig, 3)]));

        let se = std_error(&md);
        let df = md.w - 1.0;
        let t = se.map(|se| md.mean / se);
        let (lower, upper) = interval(md.mean, se, df, ci);
        let label = format!("{} - {}", avar.display_name(), bvar.display_name());
        tests.push((
            pair,
            vec![
                Cell::Text(label),
                num((md.w > 0.0).then_some(md.mean), 3),
                num(md.std_dev(), 3),
                num(se, 3),
                lower,
                upper,
                num(t, 3),
                n_cell(df.max(0.0)),
                num(t.and_then(|t| dist::t_sig(t, df)), 3),
            ],
        ));
    }
    let stat_columns = ["", "Mean", "N", "Std. Deviation", "Std. Error Mean"].map(String::from).to_vec();
    let corr_columns = ["", "N", "Correlation", "Sig."].map(String::from).to_vec();
    let mut test_columns = ["", "Mean", "Std. Deviation", "Std. Error Mean"].map(String::from).to_vec();
    test_columns.extend([ci_label(ci, "Lower"), ci_label(ci, "Upper")]);
    test_columns.extend(["t", "df", "Sig. (2-tailed)"].map(String::from));
    Ok(vec![
        Item::Table(Table { title: "Paired Samples Statistics".into(), columns: stat_columns, rows: stats }),
        Item::Table(Table { title: "Paired Samples Correlations".into(), columns: corr_columns, rows: correlations }),
        Item::Table(Table { title: "Paired Samples Test".into(), columns: test_columns, rows: tests }),
    ])
}

/// Runs a t test; `ci` is the confidence level of the intervals (0.95 by default in syntax).
pub fn t_test(ds: &Dataset, test: &TTest, ci: f64) -> Result<Vec<Item>, StatsError> {
    match test {
        TTest::OneSample { variables, test_value } => one_sample(ds, variables, *test_value, ci),
        TTest::Independent { variables, group, first, second } => independent(ds, variables, group, first, second, ci),
        TTest::Paired { pairs } => paired(ds, pairs, ci),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mksigma_core::Variable;

    fn table(item: &Item) -> &Table {
        match item {
            Item::Table(t) => t,
            other => panic!("not a table: {other:?}"),
        }
    }

    fn value(c: &Cell) -> f64 {
        match c {
            Cell::Num { value, .. } => *value,
            Cell::Int(n) => *n as f64,
            other => panic!("not a number: {other:?}"),
        }
    }

    fn data() -> Dataset {
        let mut ds = Dataset::new();
        ds.add_variable(Variable::numeric("g")).unwrap();
        ds.add_variable(Variable::numeric("before")).unwrap();
        ds.add_variable(Variable::numeric("after")).unwrap();
        let rows = [(1.0, 5.0, 7.0), (1.0, 6.0, 8.0), (1.0, 7.0, 7.0), (1.0, 8.0, 11.0), (2.0, 4.0, 5.0), (2.0, 4.0, 4.0), (2.0, 6.0, 9.0)];
        for (g, b, a) in rows {
            ds.push_case(vec![Value::Num(g), Value::Num(b), Value::Num(a)]).unwrap();
        }
        ds
    }

    #[test]
    fn one_sample_by_hand() {
        // before = 5 6 7 8 4 4 6: mean 40/7, against 5.
        let items = t_test(&data(), &TTest::OneSample { variables: vec!["before".into()], test_value: 5.0 }, 0.95).unwrap();
        let t = table(&items[1]);
        let xs = [5.0, 6.0, 7.0, 8.0, 4.0, 4.0, 6.0];
        let mean = xs.iter().sum::<f64>() / 7.0;
        let sd = (xs.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / 6.0).sqrt();
        let expected_t = (mean - 5.0) / (sd / 7f64.sqrt());
        assert!((value(&t.rows[0].1[0]) - expected_t).abs() < 1e-12);
        assert_eq!(t.rows[0].1[1], Cell::Int(6));
        let sig = value(&t.rows[0].1[2]);
        assert!(sig > 0.0 && sig < 1.0);
    }

    #[test]
    fn independent_groups_pooled_and_welch() {
        let test = TTest::Independent { variables: vec!["before".into()], group: "g".into(), first: Value::Num(1.0), second: Value::Num(2.0) };
        let items = t_test(&data(), &test, 0.95).unwrap();
        let groups = table(&items[0]);
        assert_eq!(groups.rows[0].1[1], Cell::Int(4));
        assert_eq!(groups.rows[1].1[1], Cell::Int(3));
        let t = table(&items[1]);
        // Group 1: 5 6 7 8 (mean 6.5, var 5/3); group 2: 4 4 6 (mean 14/3, var 4/3).
        let (v1, v2, d): (f64, f64, f64) = (5.0 / 3.0, 4.0 / 3.0, 6.5 - 14.0 / 3.0);
        let pooled = ((3.0 * v1 + 2.0 * v2) / 5.0 * (0.25 + 1.0 / 3.0)).sqrt();
        assert!((value(&t.rows[0].1[3]) - d / pooled).abs() < 1e-12);
        assert_eq!(t.rows[0].1[4], Cell::Int(5));
        let welch_se = (v1 / 4.0 + v2 / 3.0).sqrt();
        assert!((value(&t.rows[1].1[3]) - d / welch_se).abs() < 1e-12);
        let welch_df = (v1 / 4.0 + v2 / 3.0).powi(2) / ((v1 / 4.0).powi(2) / 3.0 + (v2 / 3.0).powi(2) / 2.0);
        assert!((value(&t.rows[1].1[4]) - welch_df).abs() < 1e-12);
    }

    #[test]
    fn paired_differences() {
        let items = t_test(&data(), &TTest::Paired { pairs: vec![("after".into(), "before".into())] }, 0.95).unwrap();
        let t = table(&items[2]);
        // Differences 2 2 0 3 1 0 3: mean 11/7.
        assert!((value(&t.rows[0].1[1]) - 11.0 / 7.0).abs() < 1e-12);
        assert_eq!(t.rows[0].1[7], Cell::Int(6));
        let r = value(&table(&items[1]).rows[0].1[2]);
        assert!(r > 0.5 && r < 1.0);
    }
}
