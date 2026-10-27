//! `T-TEST`: one sample against a test value, two independent groups (with Levene's test and
//! both the pooled and the Welch version), and paired samples. Missing values are excluded
//! analysis by analysis; case weights count as frequencies.

use mksigma_core::{Dataset, Value};
use mksigma_i18n::tr;
use mksigma_output::{Cell, Item, Table};

use crate::groups::{levene, moments, numeric, std_error, valid, valid_value};
use crate::{Moments, StatsError, dist, n_cell, num, p_value, value_text};

#[derive(Debug, Clone, PartialEq)]
pub enum TTest {
    OneSample { variables: Vec<String>, test_value: f64 },
    Independent { variables: Vec<String>, group: String, first: Value, second: Value },
    Paired { pairs: Vec<(String, String)> },
}

fn ci_label(ci: f64, end: &str) -> String {
    tr!(end, level = format!("{:.0}", ci * 100.0))
}

/// `mean ± t × se` for a two-sided interval of confidence `ci`; none when the standard error
/// is 0, where t can't be computed.
fn interval(mean: f64, se: Option<f64>, df: f64, ci: f64) -> (Cell, Cell) {
    let half = se.filter(|se| *se > 0.0).zip(dist::t_quantile(0.5 + ci / 2.0, df)).map(|(se, t)| t * se);
    (num(half.map(|h| mean - h), 3), num(half.map(|h| mean + h), 3))
}

/// t = difference / standard error, when the standard error isn't 0.
fn t_value(diff: f64, se: Option<f64>) -> Option<f64> {
    se.map(|se| diff / se).filter(|t| t.is_finite())
}

/// The note for a test that has data but no t: every value the same.
fn zero_sd_note(m: &Moments, name: &str, notes: &mut Vec<Item>) {
    if m.w > 0.0 && m.std_dev().is_none_or(|sd| sd == 0.0) {
        notes.push(Item::Text(tr!("o-t-zero-sd", name = name)));
    }
}

fn one_sample(ds: &Dataset, variables: &[String], test_value: f64, ci: f64) -> Result<Vec<Item>, StatsError> {
    let (mut stats, mut tests, mut notes) = (Vec::new(), Vec::new(), Vec::new());
    for name in variables {
        let (i, var) = numeric(ds, name)?;
        let values: Vec<(f64, f64)> =
            (0..ds.n_cases()).filter_map(|c| valid(ds, i, c).map(|x| (x, ds.case_weight(c)))).filter(|(_, w)| *w > 0.0).collect();
        let m = moments(&values);
        let se = std_error(&m);
        stats.push((var.display_name().to_string(), vec![n_cell(m.w), num((m.w > 0.0).then_some(m.mean), 3), num(m.std_dev(), 3), num(se, 3)]));
        let df = m.w - 1.0;
        let diff = m.mean - test_value;
        let t = t_value(diff, se);
        zero_sd_note(&m, var.display_name(), &mut notes);
        let (lower, upper) = interval(diff, se, df, ci);
        let sig = t.and_then(|t| dist::t_sig(t, df));
        tests.push((
            var.display_name().to_string(),
            vec![num(t, 3), n_cell(df.max(0.0)), p_value(sig), num((m.w > 0.0).then_some(diff), 3), lower, upper],
        ));
    }
    let stat_columns = [tr!("o-n"), tr!("o-mean"), tr!("o-std-deviation"), tr!("o-std-error-mean")].map(String::from).to_vec();
    let mut test_columns = [tr!("o-t"), tr!("o-df"), tr!("o-sig-2-tailed"), tr!("o-mean-difference")].map(String::from).to_vec();
    test_columns.extend([ci_label(ci, "o-ci-lower"), ci_label(ci, "o-ci-upper")]);
    let mut items = vec![
        Item::Table(Table { title: tr!("o-one-sample-statistics"), columns: stat_columns, rows: stats, categories: Vec::new() }),
        Item::Table(Table {
            title: tr!("o-one-sample-test", value = format!("{test_value}")),
            columns: test_columns,
            rows: tests,
            categories: Vec::new(),
        }),
    ];
    items.extend(notes);
    Ok(items)
}

fn independent(ds: &Dataset, variables: &[String], group: &str, first: &Value, second: &Value, ci: f64) -> Result<Vec<Item>, StatsError> {
    let (gi, gvar) = ds.variable(group).ok_or_else(|| StatsError::UnknownVariable(group.to_string()))?;
    let (mut stats, mut tests, mut notes) = (Vec::new(), Vec::new(), Vec::new());
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

        // Nothing to compare when a group is empty: a note says so instead of a t.
        let both = m1.w > 0.0 && m2.w > 0.0;
        let diff = both.then_some(m1.mean - m2.mean);
        if !both {
            notes.push(Item::Text(tr!("o-t-empty-group", name = var.display_name())));
        } else if m1.m2 == 0.0 && m2.m2 == 0.0 {
            notes.push(Item::Text(tr!("o-t-zero-sd-both", name = var.display_name())));
        }
        let (v1, v2) = (m1.std_dev().map(|s| s * s), m2.std_dev().map(|s| s * s));
        let lev = levene(&groups);
        // Pooled variance, from the sums of squares: defined with a group of one case too.
        let df_pooled = m1.w + m2.w - 2.0;
        let se_pooled = (both && df_pooled > 0.0).then(|| ((m1.m2 + m2.m2) / df_pooled * (1.0 / m1.w + 1.0 / m2.w)).sqrt());
        // Welch–Satterthwaite.
        let se_welch = v1.zip(v2).map(|(v1, v2)| (v1 / m1.w + v2 / m2.w).sqrt());
        let df_welch = v1.zip(v2).map(|(v1, v2)| {
            let (a, b) = (v1 / m1.w, v2 / m2.w);
            (a + b).powi(2) / (a * a / (m1.w - 1.0) + b * b / (m2.w - 1.0))
        });
        let row = |se: Option<f64>, df: Option<f64>, df_cell: Cell, levene: [Cell; 2]| {
            let t = diff.and_then(|d| t_value(d, se));
            let sig = t.zip(df).and_then(|(t, df)| dist::t_sig(t, df));
            let (lower, upper) = match (diff, df) {
                (Some(diff), Some(df)) => interval(diff, se, df, ci),
                _ => (Cell::Empty, Cell::Empty),
            };
            let [f, f_sig] = levene;
            vec![Cell::Text(String::new()), f, f_sig, num(t, 3), df_cell, p_value(sig), num(diff, 3), num(se, 3), lower, upper]
        };
        let levene_cells = [num(lev.map(|l| l.0), 3), p_value(lev.map(|l| l.3))];
        let mut equal = row(se_pooled, Some(df_pooled), n_cell(df_pooled), levene_cells);
        let mut unequal = row(se_welch, df_welch, num(df_welch, 3), [Cell::Empty, Cell::Empty]);
        equal[0] = Cell::Text(tr!("o-equal-variances-assumed"));
        unequal[0] = Cell::Text(tr!("o-equal-variances-not-assumed"));
        tests.push((var.display_name().to_string(), equal));
        tests.push((String::new(), unequal));
    }
    let stat_columns =
        [gvar.display_name().to_string(), tr!("o-n"), tr!("o-mean"), tr!("o-std-deviation"), tr!("o-std-error-mean")].map(String::from).to_vec();
    let mut test_columns = [
        String::new(),
        tr!("o-levene-f"),
        tr!("o-levene-sig"),
        tr!("o-t"),
        tr!("o-df"),
        tr!("o-sig-2-tailed"),
        tr!("o-mean-difference"),
        tr!("o-std-error-difference"),
    ]
    .map(String::from)
    .to_vec();
    test_columns.extend([ci_label(ci, "o-ci-lower"), ci_label(ci, "o-ci-upper")]);
    Ok(vec![
        Item::Table(Table { title: tr!("o-group-statistics"), columns: stat_columns, rows: stats, categories: Vec::new() }),
        Item::Table(Table { title: tr!("o-independent-samples-test"), columns: test_columns, rows: tests, categories: Vec::new() }),
    ]
    .into_iter()
    .chain(notes)
    .collect())
}

fn paired(ds: &Dataset, pairs: &[(String, String)], ci: f64) -> Result<Vec<Item>, StatsError> {
    let (mut stats, mut correlations, mut tests, mut notes) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for (p, (a, b)) in pairs.iter().enumerate() {
        let ((ai, avar), (bi, bvar)) = (numeric(ds, a)?, numeric(ds, b)?);
        // Cases valid on both variables.
        let rows: Vec<(f64, f64, f64)> =
            (0..ds.n_cases()).filter_map(|c| Some((valid(ds, ai, c)?, valid(ds, bi, c)?, ds.case_weight(c)))).filter(|(_, _, w)| *w > 0.0).collect();
        let ma = moments(&rows.iter().map(|(x, _, w)| (*x, *w)).collect::<Vec<_>>());
        let mb = moments(&rows.iter().map(|(_, y, w)| (*y, *w)).collect::<Vec<_>>());
        let md = moments(&rows.iter().map(|(x, y, w)| (x - y, *w)).collect::<Vec<_>>());
        let pair = tr!("o-pair", n = p + 1);
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
        // Rounding can take an exactly linear pair a hair past 1.
        let r = Some((sxy / (ma.m2 * mb.m2).sqrt()).clamp(-1.0, 1.0)).filter(|r| r.is_finite());
        // Significance of r: t = r √((n − 2) / (1 − r²)) with n − 2 degrees of freedom.
        let r_sig = r.and_then(|r| dist::t_sig(r * ((n - 2.0) / (1.0 - r * r)).sqrt(), n - 2.0));
        let label = format!("{} & {}", avar.display_name(), bvar.display_name());
        correlations.push((pair.clone(), vec![Cell::Text(label), n_cell(n), num(r, 3), p_value(r_sig)]));

        let se = std_error(&md);
        let df = md.w - 1.0;
        let t = t_value(md.mean, se);
        let (lower, upper) = interval(md.mean, se, df, ci);
        let label = format!("{} - {}", avar.display_name(), bvar.display_name());
        zero_sd_note(&md, &label, &mut notes);
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
                p_value(t.and_then(|t| dist::t_sig(t, df))),
            ],
        ));
    }
    let stat_columns = [String::new(), tr!("o-mean"), tr!("o-n"), tr!("o-std-deviation"), tr!("o-std-error-mean")].map(String::from).to_vec();
    let corr_columns = [String::new(), tr!("o-n"), tr!("o-correlation"), tr!("o-sig")].map(String::from).to_vec();
    let mut test_columns = [String::new(), tr!("o-mean"), tr!("o-std-deviation"), tr!("o-std-error-mean")].map(String::from).to_vec();
    test_columns.extend([ci_label(ci, "o-ci-lower"), ci_label(ci, "o-ci-upper")]);
    test_columns.extend([tr!("o-t"), tr!("o-df"), tr!("o-sig-2-tailed")].map(String::from));
    Ok(vec![
        Item::Table(Table { title: tr!("o-paired-samples-statistics"), columns: stat_columns, rows: stats, categories: Vec::new() }),
        Item::Table(Table { title: tr!("o-paired-samples-correlations"), columns: corr_columns, rows: correlations, categories: Vec::new() }),
        Item::Table(Table { title: tr!("o-paired-samples-test"), columns: test_columns, rows: tests, categories: Vec::new() }),
    ]
    .into_iter()
    .chain(notes)
    .collect())
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
            Cell::Num { value, .. } | Cell::Sig(value) => *value,
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

    fn dataset(rows: &[(f64, f64, f64)]) -> Dataset {
        let mut ds = Dataset::new();
        for name in ["g", "x", "y"] {
            ds.add_variable(Variable::numeric(name)).unwrap();
        }
        for (g, x, y) in rows {
            ds.push_case(vec![Value::Num(*g), Value::Num(*x), Value::Num(*y)]).unwrap();
        }
        ds
    }

    fn independent_x(ds: &Dataset) -> Vec<Item> {
        let test = TTest::Independent { variables: vec!["x".into()], group: "g".into(), first: Value::Num(1.0), second: Value::Num(2.0) };
        t_test(ds, &test, 0.95).unwrap()
    }

    fn note(items: &[Item]) -> Option<&str> {
        items.iter().find_map(|i| match i {
            Item::Text(s) => Some(s.as_str()),
            _ => None,
        })
    }

    /// The edge cases of #28: each gives a note and blanks, never a made-up number.
    #[test]
    fn an_empty_group_has_no_difference() {
        let items = independent_x(&dataset(&[(1.0, 3.0, 0.0), (1.0, 5.0, 0.0), (1.0, 7.0, 0.0)]));
        let t = table(&items[1]);
        for row in &t.rows {
            assert_eq!(row.1[3], Cell::Empty, "t");
            assert_eq!(row.1[5], Cell::Empty, "Sig");
            assert_eq!(row.1[6], Cell::Empty, "the mean difference isn't −0 or the other mean");
        }
        assert_eq!(note(&items), Some("No t for x: one of the two groups has no valid cases."));
    }

    #[test]
    fn zero_variance_gives_no_t_and_no_sig() {
        let items = independent_x(&dataset(&[(1.0, 2.0, 0.0), (1.0, 2.0, 0.0), (2.0, 5.0, 0.0), (2.0, 5.0, 0.0)]));
        let t = table(&items[1]);
        for row in &t.rows {
            assert_eq!((&row.1[3], &row.1[5], &row.1[8], &row.1[9]), (&Cell::Empty, &Cell::Empty, &Cell::Empty, &Cell::Empty));
            assert_eq!(value(&row.1[6]), -3.0, "the difference is still shown");
        }
        assert!(note(&items).is_some_and(|n| n.contains("within each group every value is the same")));
        // One sample and paired: every value (or difference) the same.
        let ds = dataset(&[(1.0, 4.0, 1.0), (1.0, 4.0, 1.0), (1.0, 4.0, 1.0)]);
        let one = t_test(&ds, &TTest::OneSample { variables: vec!["x".into()], test_value: 0.0 }, 0.95).unwrap();
        assert_eq!(&table(&one[1]).rows[0].1[..3], [Cell::Empty, Cell::Int(2), Cell::Empty]);
        assert!(note(&one).is_some_and(|n| n.ends_with("the standard error is 0.")));
        let paired = t_test(&ds, &TTest::Paired { pairs: vec![("x".into(), "y".into())] }, 0.95).unwrap();
        assert_eq!(table(&paired[2]).rows[0].1[8], Cell::Empty);
        assert!(note(&paired).is_some());
    }

    #[test]
    fn a_group_of_one_still_has_the_pooled_row() {
        // Group 1: 2 4 6 (sum of squares 8); group 2: 10 alone. Pooled variance 8 / 2 = 4.
        let items = independent_x(&dataset(&[(1.0, 2.0, 0.0), (1.0, 4.0, 0.0), (1.0, 6.0, 0.0), (2.0, 10.0, 0.0)]));
        let pooled = &table(&items[1]).rows[0].1;
        let se = (4.0f64 * (1.0 / 3.0 + 1.0)).sqrt();
        assert!((value(&pooled[7]) - se).abs() < 1e-12);
        assert!((value(&pooled[3]) - (-6.0 / se)).abs() < 1e-12);
        assert_eq!(pooled[4], Cell::Int(2));
        assert_eq!(table(&items[1]).rows[1].1[3], Cell::Empty, "Welch needs both variances");
    }

    #[test]
    fn an_exactly_linear_pair_has_r_of_one_and_a_sig() {
        // y = 0.1 x + 0.3: rounding takes r past 1 for many such pairs.
        let rows: Vec<(f64, f64, f64)> = (0..7).map(|i| (1.0, f64::from(i) * 0.7 + 0.1, (f64::from(i) * 0.7 + 0.1) * 0.1 + 0.3)).collect();
        let items = t_test(&dataset(&rows), &TTest::Paired { pairs: vec![("x".into(), "y".into())] }, 0.95).unwrap();
        let row = &table(&items[1]).rows[0].1;
        assert!(value(&row[2]) <= 1.0);
        assert!((value(&row[2]) - 1.0).abs() < 1e-12);
        assert_eq!(value(&row[3]), 0.0, "Sig shows .000, not a blank");
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
