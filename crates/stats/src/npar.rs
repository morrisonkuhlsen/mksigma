//! `NPAR TESTS`: Mann-Whitney (two independent groups), Wilcoxon signed ranks (paired) and
//! Kruskal-Wallis (k independent groups), with the asymptotic significance and the correction
//! for ties. Ties share their mean rank; case weights count as frequencies.

use mksigma_core::{Dataset, Value};
use mksigma_i18n::tr;
use mksigma_output::{Cell, Item, Table};
use statrs::distribution::{ContinuousCDF, Normal};

use crate::groups::{numeric, valid};
use crate::{StatsError, dist, n_cell, num, value_text};

#[derive(Debug, Clone, PartialEq)]
pub enum NparTest {
    /// `/M-W = vars BY group(a b)`
    MannWhitney { variables: Vec<String>, group: String, first: f64, second: f64 },
    /// `/WILCOXON = a WITH b (PAIRED)`: the differences are second − first.
    Wilcoxon { pairs: Vec<(String, String)> },
    /// `/K-W = vars BY group(low high)`: groups with values from low to high.
    KruskalWallis { variables: Vec<String>, group: String, low: f64, high: f64 },
}

/// Mean ranks of weighted values (ties share the mean of the ranks they span), in input order,
/// and Σ(t³ − t) over the tied groups.
fn ranks(values: &[(f64, f64)]) -> (Vec<f64>, f64) {
    let mut order: Vec<usize> = (0..values.len()).collect();
    order.sort_by(|a, b| values.get(*a).map(|v| v.0).unwrap_or(0.0).total_cmp(&values.get(*b).map(|v| v.0).unwrap_or(0.0)));
    let mut out = vec![0.0; values.len()];
    let (mut below, mut ties, mut i) = (0.0, 0.0, 0);
    while i < order.len() {
        let x = order.get(i).and_then(|&k| values.get(k)).map_or(0.0, |v| v.0);
        let mut j = i;
        let mut weight = 0.0;
        while let Some(&k) = order.get(j) {
            match values.get(k) {
                // `total_cmp`, not `==`: a NaN equals itself here, so the loop always advances.
                Some((v, w)) if v.total_cmp(&x).is_eq() => weight += w,
                _ => break,
            }
            j += 1;
        }
        let rank = below + (weight + 1.0) / 2.0;
        for &k in order.get(i..j).unwrap_or_default() {
            if let Some(r) = out.get_mut(k) {
                *r = rank;
            }
        }
        ties += weight.powi(3) - weight;
        below += weight;
        i = j;
    }
    (out, ties)
}

pub(crate) fn two_sided(z: f64) -> Option<f64> {
    let normal = Normal::new(0.0, 1.0).ok()?;
    Some(2.0 * normal.sf(z.abs()))
}

fn table(title: String, columns: Vec<String>, rows: Vec<(String, Vec<Cell>)>) -> Item {
    Item::Table(Table { title, columns, rows, categories: Vec::new() })
}

pub(crate) fn group_values(ds: &Dataset, var: usize, group: usize, keep: impl Fn(f64) -> bool) -> Vec<(f64, f64, f64)> {
    (0..ds.n_cases())
        .filter_map(|c| {
            let g = valid(ds, group, c).filter(|g| keep(*g))?;
            let w = ds.case_weight(c);
            (w > 0.0).then_some((valid(ds, var, c)?, w, g))
        })
        .collect()
}

fn mann_whitney(ds: &Dataset, variables: &[String], group: &str, first: f64, second: f64) -> Result<Vec<Item>, StatsError> {
    let (gi, gvar) = numeric(ds, group)?;
    let (mut rank_rows, mut columns, mut stats) = (Vec::new(), Vec::new(), vec![Vec::new(), Vec::new(), Vec::new(), Vec::new()]);
    for name in variables {
        let (vi, var) = numeric(ds, name)?;
        let data = group_values(ds, vi, gi, |g| g == first || g == second);
        let (r, ties) = ranks(&data.iter().map(|(x, w, _)| (*x, *w)).collect::<Vec<_>>());
        let sum = |g: f64| data.iter().zip(&r).filter(|((_, _, gg), _)| *gg == g).fold((0.0, 0.0), |(n, s), ((_, w, _), rank)| (n + w, s + w * rank));
        let ((n1, r1), (n2, r2)) = (sum(first), sum(second));
        let n = n1 + n2;
        for (k, (g, gn, gr)) in [(first, n1, r1), (second, n2, r2)].into_iter().enumerate() {
            let label = if k == 0 { var.display_name().to_string() } else { String::new() };
            rank_rows
                .push((label, vec![Cell::Text(value_text(gvar, &Value::Num(g))), n_cell(gn), num((gn > 0.0).then(|| gr / gn), 2), num(Some(gr), 2)]));
        }
        rank_rows.push((String::new(), vec![Cell::Text(tr!("o-total")), n_cell(n), Cell::Empty, Cell::Empty]));
        // U counts the pairs where the first group's value is higher; report the smaller U and
        // the rank sum of the group it belongs to.
        let u1 = r1 - n1 * (n1 + 1.0) / 2.0;
        let u2 = n1 * n2 - u1;
        let (u, w) = if u1 <= u2 { (u1, r1) } else { (u2, r2) };
        let variance = n1 * n2 / (n * (n - 1.0)) * ((n.powi(3) - n) / 12.0 - ties / 12.0);
        let z = Some((u - n1 * n2 / 2.0) / variance.sqrt()).filter(|z| z.is_finite());
        columns.push(var.display_name().to_string());
        for (row, cell) in stats.iter_mut().zip([num(Some(u), 3), num(Some(w), 3), num(z, 3), num(z.and_then(two_sided), 3)]) {
            row.push(cell);
        }
    }
    let names = [tr!("o-mann-whitney-u"), tr!("o-wilcoxon-w"), tr!("o-z"), tr!("o-asymp-sig-2-tailed")];
    let rows = names.iter().zip(stats).map(|(n, cells)| (n.to_string(), cells)).collect();
    Ok(vec![
        table(tr!("o-ranks"), [String::new(), tr!("o-n"), tr!("o-mean-rank"), tr!("o-sum-of-ranks")].map(String::from).to_vec(), rank_rows),
        table(tr!("o-test-statistics"), columns, rows),
        Item::Text(tr!("o-grouping-variable", name = gvar.display_name())),
    ])
}

fn wilcoxon(ds: &Dataset, pairs: &[(String, String)]) -> Result<Vec<Item>, StatsError> {
    let (mut rank_rows, mut columns, mut z_row, mut sig_row) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for (a, b) in pairs {
        let ((ai, avar), (bi, bvar)) = (numeric(ds, a)?, numeric(ds, b)?);
        let diffs: Vec<(f64, f64)> = (0..ds.n_cases())
            .filter_map(|c| {
                let w = ds.case_weight(c);
                (w > 0.0).then_some((valid(ds, bi, c)? - valid(ds, ai, c)?, w))
            })
            .collect();
        let zero: f64 = diffs.iter().filter(|(d, _)| *d == 0.0).map(|(_, w)| w).sum();
        let nonzero: Vec<(f64, f64)> = diffs.iter().filter(|(d, _)| *d != 0.0).copied().collect();
        let (r, ties) = ranks(&nonzero.iter().map(|(d, w)| (d.abs(), *w)).collect::<Vec<_>>());
        let side = |negative: bool| {
            nonzero.iter().zip(&r).filter(|((d, _), _)| (*d < 0.0) == negative).fold((0.0, 0.0), |(n, s), ((_, w), rank)| (n + w, s + w * rank))
        };
        let ((nn, tn), (np, tp)) = (side(true), side(false));
        let n = nn + np;
        let label = format!("{} - {}", bvar.display_name(), avar.display_name());
        let lines = [
            (tr!("o-negative-ranks"), nn, Some(tn)),
            (tr!("o-positive-ranks"), np, Some(tp)),
            (tr!("o-ties"), zero, None),
            (tr!("o-total"), n + zero, None),
        ];
        for (k, (name, count, sum)) in lines.into_iter().enumerate() {
            rank_rows.push((
                if k == 0 { label.clone() } else { String::new() },
                vec![Cell::Text(name), n_cell(count), num(sum.filter(|_| count > 0.0).map(|s| s / count), 2), num(sum, 2)],
            ));
        }
        let variance = n * (n + 1.0) * (2.0 * n + 1.0) / 24.0 - ties / 48.0;
        let z = Some((tn.min(tp) - n * (n + 1.0) / 4.0) / variance.sqrt()).filter(|z| z.is_finite());
        columns.push(label);
        z_row.push(num(z, 3));
        sig_row.push(num(z.and_then(two_sided), 3));
    }
    Ok(vec![
        table(tr!("o-ranks"), [String::new(), tr!("o-n"), tr!("o-mean-rank"), tr!("o-sum-of-ranks")].map(String::from).to_vec(), rank_rows),
        table(tr!("o-test-statistics"), columns, vec![(tr!("o-z"), z_row), (tr!("o-asymp-sig-2-tailed"), sig_row)]),
        Item::Text(tr!("o-wilcoxon-signed-ranks-test-negative-ranks-the-second-variable-is-lower")),
    ])
}

fn kruskal_wallis(ds: &Dataset, variables: &[String], group: &str, low: f64, high: f64) -> Result<Vec<Item>, StatsError> {
    let (gi, gvar) = numeric(ds, group)?;
    let (mut rank_rows, mut columns, mut stats) = (Vec::new(), Vec::new(), vec![Vec::new(), Vec::new(), Vec::new()]);
    for name in variables {
        let (vi, var) = numeric(ds, name)?;
        let data = group_values(ds, vi, gi, |g| low <= g && g <= high);
        let (r, ties) = ranks(&data.iter().map(|(x, w, _)| (*x, *w)).collect::<Vec<_>>());
        let mut levels: Vec<f64> = data.iter().map(|(_, _, g)| *g).collect();
        levels.sort_by(f64::total_cmp);
        levels.dedup();
        let n: f64 = data.iter().map(|(_, w, _)| w).sum();
        let mut h = 0.0;
        for (k, level) in levels.iter().enumerate() {
            let (gn, gr) =
                data.iter().zip(&r).filter(|((_, _, g), _)| g == level).fold((0.0, 0.0), |(n, s), ((_, w, _), rank)| (n + w, s + w * rank));
            h += gr * gr / gn;
            let label = if k == 0 { var.display_name().to_string() } else { String::new() };
            rank_rows.push((label, vec![Cell::Text(value_text(gvar, &Value::Num(*level))), n_cell(gn), num(Some(gr / gn), 2)]));
        }
        rank_rows.push((String::new(), vec![Cell::Text(tr!("o-total")), n_cell(n), Cell::Empty]));
        let h = (12.0 / (n * (n + 1.0)) * h - 3.0 * (n + 1.0)) / (1.0 - ties / (n.powi(3) - n));
        let df = levels.len() as f64 - 1.0;
        let h = Some(h).filter(|h| h.is_finite() && df >= 1.0);
        columns.push(var.display_name().to_string());
        for (row, cell) in stats.iter_mut().zip([num(h, 3), n_cell(df.max(0.0)), num(h.and_then(|h| dist::chi_square_sig(h, df)), 3)]) {
            row.push(cell);
        }
    }
    let rows = [tr!("o-kruskal-wallis-h"), tr!("o-df"), tr!("o-asymp-sig")].iter().zip(stats).map(|(n, cells)| (n.to_string(), cells)).collect();
    Ok(vec![
        table(tr!("o-ranks"), [String::new(), tr!("o-n"), tr!("o-mean-rank")].map(String::from).to_vec(), rank_rows),
        table(tr!("o-test-statistics"), columns, rows),
        Item::Text(tr!("o-kruskal-wallis-note", name = gvar.display_name())),
    ])
}

pub fn npar_tests(ds: &Dataset, tests: &[NparTest]) -> Result<Vec<Item>, StatsError> {
    let mut items = Vec::new();
    for test in tests {
        items.extend(match test {
            NparTest::MannWhitney { variables, group, first, second } => mann_whitney(ds, variables, group, *first, *second)?,
            NparTest::Wilcoxon { pairs } => wilcoxon(ds, pairs)?,
            NparTest::KruskalWallis { variables, group, low, high } => kruskal_wallis(ds, variables, group, *low, *high)?,
        });
    }
    Ok(items)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mksigma_core::Variable;

    #[test]
    fn mean_ranks_for_ties() {
        let (r, ties) = ranks(&[(3.0, 1.0), (1.0, 1.0), (3.0, 1.0), (2.0, 1.0)]);
        assert_eq!(r, [3.5, 1.0, 3.5, 2.0]);
        assert_eq!(ties, 6.0, "one tie of two: 2³ − 2");
        // A weight of 2 counts as two tied cases.
        let (r, _) = ranks(&[(5.0, 2.0), (1.0, 1.0)]);
        assert_eq!(r, [2.5, 1.0]);
    }

    #[test]
    fn ranking_a_nan_terminates() {
        // A NaN never equals itself with `==`; ranking used to loop forever on one (#21).
        let (r, _) = ranks(&[(f64::NAN, 1.0), (1.0, 1.0), (f64::NAN, 1.0), (f64::INFINITY, 1.0)]);
        assert_eq!(r.len(), 4);
        assert_eq!(r[1], 1.0);
    }

    fn stat(items: &[Item], row: &str) -> f64 {
        let Some(Item::Table(t)) = items.get(1) else { panic!() };
        match t.rows.iter().find(|(n, _)| n == row).map(|(_, c)| &c[0]) {
            Some(Cell::Num { value, .. }) => *value,
            Some(Cell::Int(n)) => *n as f64,
            other => panic!("{row}: {other:?}"),
        }
    }

    #[test]
    fn mann_whitney_and_kruskal_wallis_by_hand() {
        let mut ds = Dataset::new();
        ds.add_variable(Variable::numeric("x")).unwrap();
        ds.add_variable(Variable::numeric("g")).unwrap();
        // Group 1: 1 2 3; group 2: 4 5 6 7. No ties: U = 0, W = 6.
        for (x, g) in [(1.0, 1.0), (2.0, 1.0), (3.0, 1.0), (4.0, 2.0), (5.0, 2.0), (6.0, 2.0), (7.0, 2.0)] {
            ds.push_case(vec![Value::Num(x), Value::Num(g)]).unwrap();
        }
        let mw = npar_tests(&ds, &[NparTest::MannWhitney { variables: vec!["x".into()], group: "g".into(), first: 1.0, second: 2.0 }]).unwrap();
        assert_eq!(stat(&mw, "Mann-Whitney U"), 0.0);
        assert_eq!(stat(&mw, "Wilcoxon W"), 6.0);
        // Z = (0 − 6) / √(3·4·8/12) = −6/√8.
        assert!((stat(&mw, "Z") + 6.0 / 8f64.sqrt()).abs() < 1e-12);

        let kw = npar_tests(&ds, &[NparTest::KruskalWallis { variables: vec!["x".into()], group: "g".into(), low: 1.0, high: 2.0 }]).unwrap();
        // H = 12/(7·8) (6²/3 + 22²/4) − 3·8 = 4.5.
        assert!((stat(&kw, "Kruskal-Wallis H") - 4.5).abs() < 1e-12);
        assert_eq!(stat(&kw, "df"), 1.0);
    }

    #[test]
    fn wilcoxon_signed_ranks() {
        let mut ds = Dataset::new();
        ds.add_variable(Variable::numeric("before")).unwrap();
        ds.add_variable(Variable::numeric("after")).unwrap();
        // Differences: +1, +2, +3, −4, 0 (dropped).
        for (b, a) in [(10.0, 11.0), (10.0, 12.0), (10.0, 13.0), (10.0, 6.0), (5.0, 5.0)] {
            ds.push_case(vec![Value::Num(b), Value::Num(a)]).unwrap();
        }
        let items = npar_tests(&ds, &[NparTest::Wilcoxon { pairs: vec![("before".into(), "after".into())] }]).unwrap();
        let Item::Table(ranks) = &items[0] else { panic!() };
        assert_eq!(ranks.rows[0].1[3], Cell::Num { value: 4.0, decimals: 2 }, "negative ranks sum to 4");
        assert_eq!(ranks.rows[2].1[1], Cell::Int(1), "one tie");
        // T = 4, n = 4: Z = (4 − 5) / √(4·5·9/24).
        assert!((stat(&items, "Z") - (4.0 - 5.0) / 7.5f64.sqrt()).abs() < 1e-12);
    }
}
