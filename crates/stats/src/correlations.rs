//! `CORRELATIONS`: Pearson correlations with their significance and N. Missing values are
//! excluded pair by pair (the default) or listwise; case weights count as frequencies.
//! Significant correlations are flagged `*` (p < .05) and `**` (p < .01), as SPSS does by default.

use mksigma_core::Dataset;
use mksigma_i18n::tr;
use mksigma_output::{Cell, Item, Table};

use crate::groups::{numeric, valid};
use crate::{StatsError, dist, n_cell, num};

#[derive(Debug, Clone, Default)]
pub struct Options {
    pub variables: Vec<String>,
    /// Variables after `WITH`: rows are `variables`, columns these. Empty: a square matrix.
    pub with: Vec<String>,
    /// One-tailed significance instead of two-tailed.
    pub one_tailed: bool,
    /// Flag significant correlations with asterisks.
    pub flag: bool,
    /// Use only cases valid on every variable listed.
    pub listwise: bool,
}

/// Pearson's r, its significance and the sum of weights over `rows` of (x, y, weight).
fn pearson(rows: &[(f64, f64, f64)], one_tailed: bool) -> (Option<f64>, Option<f64>, f64) {
    let n: f64 = rows.iter().map(|(_, _, w)| w).sum();
    if n <= 0.0 {
        return (None, None, 0.0);
    }
    // A constant has no correlation: told by its values, not by its sum of squares, which
    // rounding leaves a hair above 0 for a value such as 3.7.
    let constant = |f: fn(&(f64, f64, f64)) -> f64| rows.first().is_some_and(|first| rows.iter().all(|row| f(row) == f(first)));
    if constant(|r| r.0) || constant(|r| r.1) {
        return (None, None, n);
    }
    let mx = rows.iter().map(|(x, _, w)| x * w).sum::<f64>() / n;
    let my = rows.iter().map(|(_, y, w)| y * w).sum::<f64>() / n;
    let (mut sxy, mut sxx, mut syy) = (0.0, 0.0, 0.0);
    for (x, y, w) in rows {
        sxy += w * (x - mx) * (y - my);
        sxx += w * (x - mx).powi(2);
        syy += w * (y - my).powi(2);
    }
    let r = Some((sxy / (sxx * syy).sqrt()).clamp(-1.0, 1.0)).filter(|r| r.is_finite());
    let sig = r.filter(|r| r.abs() < 1.0 && n > 2.0).and_then(|r| {
        let t = r * ((n - 2.0) / (1.0 - r * r)).sqrt();
        dist::t_sig(t, n - 2.0).map(|p| if one_tailed { p / 2.0 } else { p })
    });
    // A perfect correlation is as significant as it gets.
    let sig = sig.or_else(|| r.filter(|r| r.abs() >= 1.0 && n > 2.0).map(|_| 0.0));
    (r, sig, n)
}

pub fn correlations(ds: &Dataset, options: &Options) -> Result<Vec<Item>, StatsError> {
    let rows_vars = options.variables.iter().map(|n| numeric(ds, n)).collect::<Result<Vec<_>, _>>()?;
    let column_vars =
        if options.with.is_empty() { rows_vars.clone() } else { options.with.iter().map(|n| numeric(ds, n)).collect::<Result<Vec<_>, _>>()? };
    let all: Vec<usize> = rows_vars.iter().chain(&column_vars).map(|(i, _)| *i).collect();
    let usable = |case: usize| ds.case_weight(case) > 0.0 && (!options.listwise || all.iter().all(|&v| valid(ds, v, case).is_some()));

    let tail = if options.one_tailed { tr!("o-sig-1-tailed") } else { tr!("o-sig-2-tailed") };
    let (mut flagged_05, mut flagged_01, mut constant) = (false, false, false);
    let mut rows = Vec::new();
    for (ri, rvar) in &rows_vars {
        let mut r_cells = vec![Cell::Text(tr!("o-pearson-correlation"))];
        let mut sig_cells = vec![Cell::Text(tail.clone())];
        let mut n_cells = vec![Cell::Text(tr!("o-n"))];
        for (ci, _) in &column_vars {
            let pairs: Vec<(f64, f64, f64)> =
                (0..ds.n_cases()).filter(|&c| usable(c)).filter_map(|c| Some((valid(ds, *ri, c)?, valid(ds, *ci, c)?, ds.case_weight(c)))).collect();
            let (r, sig, n) = pearson(&pairs, options.one_tailed);
            constant |= r.is_none() && n > 0.0;
            let same = ri == ci;
            let stars = match sig {
                Some(p) if options.flag && !same && p < 0.01 => "**",
                Some(p) if options.flag && !same && p < 0.05 => "*",
                _ => "",
            };
            flagged_01 |= stars == "**";
            flagged_05 |= stars == "*";
            r_cells.push(match (r, stars) {
                (Some(r), "") => num(Some(r), 3),
                (Some(r), stars) => Cell::Text(format!("{}{stars}", mksigma_i18n::decimal(&format!("{r:.3}")))),
                (None, _) => Cell::Empty,
            });
            sig_cells.push(if same { Cell::Empty } else { num(sig, 3) });
            n_cells.push(n_cell(n));
        }
        let label = rvar.display_name().to_string();
        rows.push((label, r_cells));
        rows.push((String::new(), sig_cells));
        rows.push((String::new(), n_cells));
    }
    let mut columns = vec![String::new()];
    columns.extend(column_vars.iter().map(|(_, v)| v.display_name().to_string()));
    let mut items = vec![Item::Table(Table { title: tr!("o-correlations"), columns, rows, categories: Vec::new() })];
    let tails = if options.one_tailed { tr!("o-1-tailed") } else { tr!("o-2-tailed") };
    if flagged_01 {
        items.push(Item::Text(tr!("o-correlation-significant-01", tails = tails.as_str())));
    }
    if flagged_05 {
        items.push(Item::Text(tr!("o-correlation-significant-05", tails = tails.as_str())));
    }
    if constant {
        items.push(Item::Text(tr!("o-correlation-constant")));
    }
    if options.listwise {
        let n: f64 = (0..ds.n_cases()).filter(|&c| usable(c)).map(|c| ds.case_weight(c)).sum();
        items.push(Item::Text(tr!("o-listwise-n", n = n.round())));
    }
    Ok(items)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mksigma_core::{Value, Variable};

    #[test]
    fn matrix_flags_and_pairwise_missing() {
        let mut ds = Dataset::new();
        for name in ["x", "y", "z"] {
            ds.add_variable(Variable::numeric(name)).unwrap();
        }
        // y = 2x exactly; z unrelated, missing once.
        let rows = [(1.0, 2.0, Some(5.0)), (2.0, 4.0, Some(1.0)), (3.0, 6.0, None), (4.0, 8.0, Some(4.0)), (5.0, 10.0, Some(2.0))];
        for (x, y, z) in rows {
            ds.push_case(vec![Value::Num(x), Value::Num(y), z.map_or(Value::SysMis, Value::Num)]).unwrap();
        }
        let options = Options { variables: vec!["x".into(), "y".into(), "z".into()], flag: true, ..Options::default() };
        let items = correlations(&ds, &options).unwrap();
        let Item::Table(t) = &items[0] else { panic!() };
        assert_eq!(t.rows[0].1[2], Cell::Text("1.000**".into()), "x with y: perfect");
        assert_eq!(t.rows[0].1[1], Cell::Num { value: 1.0, decimals: 3 }, "the diagonal isn't flagged");
        assert_eq!(t.rows[1].1[1], Cell::Empty, "no significance on the diagonal");
        assert_eq!(t.rows[2].1[3], Cell::Int(4), "x with z: pairwise N");
        assert_eq!(t.rows[2].1[2], Cell::Int(5));
        assert!(matches!(&items[1], Item::Text(s) if s.starts_with("**.")));

        let listwise = Options { listwise: true, ..options };
        let items = correlations(&ds, &listwise).unwrap();
        let Item::Table(t) = &items[0] else { panic!() };
        assert_eq!(t.rows[2].1[2], Cell::Int(4), "listwise: the case missing z goes everywhere");
        assert!(items.iter().any(|i| matches!(i, Item::Text(s) if s == "Listwise N = 4")));
    }

    #[test]
    fn a_constant_has_no_correlation() {
        // 3.7 isn't exact in binary: its sum of squares about the mean comes out a hair above 0,
        // which used to give r = 1.000** for two constants and r = .000 against another (#27).
        let mut ds = Dataset::new();
        for name in ["a", "b", "x"] {
            ds.add_variable(Variable::numeric(name)).unwrap();
        }
        for x in [1.0, 4.0, 2.0, 8.0, 5.0, 7.0, 3.0, 6.0, 9.0, 0.5, 2.5, 7.5] {
            ds.push_case(vec![Value::Num(3.7), Value::Num(0.1), Value::Num(x)]).unwrap();
        }
        let options = Options { variables: vec!["a".into(), "b".into(), "x".into()], flag: true, ..Options::default() };
        let items = correlations(&ds, &options).unwrap();
        let Item::Table(t) = &items[0] else { panic!() };
        for (row, column) in [(0, 1), (0, 2), (0, 3), (3, 1), (6, 1), (6, 2)] {
            assert_eq!(t.rows[row].1[column], Cell::Empty, "r of row {row}, column {column}");
            assert_eq!(t.rows[row + 1].1[column], Cell::Empty, "its Sig");
        }
        assert_eq!(t.rows[6].1[3], Cell::Num { value: 1.0, decimals: 3 }, "x with itself");
        assert_eq!(t.rows[2].1[2], Cell::Int(12), "N still shown");
        assert!(items.iter().any(|i| matches!(i, Item::Text(s) if s == &tr!("o-correlation-constant"))));
        // Its Σ says so too, instead of an r from the rounding.
        let explained = crate::explain::correlations(&ds, &options.variables, &[], false, false, false).unwrap();
        let (_, steps) = &explained.sections[0];
        assert!(format!("{steps:?}").contains(&tr!("ex-one-of-the-variables-doesn-t-vary-here-so-r-is-undefined")));
    }
}
