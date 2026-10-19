//! `REGRESSION` with `/METHOD=ENTER`: ordinary least squares of one dependent variable on the
//! predictors, cases missing any of them excluded (listwise), weights as frequencies.
//!
//! Computed on deviations from the weighted means, which keeps sums of squares accurate: the
//! slopes solve `Sxx b = Sxy`, the constant is `ȳ − b·x̄`. Standard errors come from the inverse
//! of `Sxx` and the residual mean square.

use mksigma_core::Dataset;
use mksigma_i18n::tr;
use mksigma_output::{Cell, Item, Table};

use crate::groups::{numeric, valid};
use crate::{StatsError, dist, n_cell, num};

#[derive(Debug, Clone, Default)]
pub struct Options {
    pub dependent: String,
    pub predictors: Vec<String>,
    /// Confidence intervals for B at this level (`/STATISTICS CI(95)`), if asked.
    pub ci: Option<f64>,
}

/// Inverts a symmetric positive (semi)definite matrix by Gauss–Jordan elimination with partial
/// pivoting. `None` when a pivot is negligible next to its diagonal: a predictor that is (almost)
/// a linear combination of the others.
pub(crate) fn invert(m: &[Vec<f64>]) -> Option<Vec<Vec<f64>>> {
    let n = m.len();
    let mut a: Vec<Vec<f64>> = m.to_vec();
    let mut inv: Vec<Vec<f64>> = (0..n).map(|i| (0..n).map(|j| if i == j { 1.0 } else { 0.0 }).collect()).collect();
    let scale: Vec<f64> = (0..n).map(|i| m.get(i).and_then(|r| r.get(i)).copied().unwrap_or(0.0).abs()).collect();
    for col in 0..n {
        let pivot_row = (col..n).max_by(|&x, &y| {
            let (vx, vy) = (a[x][col].abs(), a[y][col].abs());
            vx.total_cmp(&vy)
        })?;
        let pivot = a[pivot_row][col];
        if pivot.abs() <= 1e-10 * scale[col].max(f64::MIN_POSITIVE) {
            return None;
        }
        a.swap(col, pivot_row);
        inv.swap(col, pivot_row);
        for j in 0..n {
            a[col][j] /= pivot;
            inv[col][j] /= pivot;
        }
        for row in 0..n {
            if row != col {
                let factor = a[row][col];
                if factor != 0.0 {
                    for j in 0..n {
                        a[row][j] -= factor * a[col][j];
                        inv[row][j] -= factor * inv[col][j];
                    }
                }
            }
        }
    }
    Some(inv)
}

pub fn regression(ds: &Dataset, options: &Options) -> Result<Vec<Item>, StatsError> {
    let (yi, yvar) = numeric(ds, &options.dependent)?;
    let xs = options.predictors.iter().map(|n| numeric(ds, n)).collect::<Result<Vec<_>, _>>()?;
    let p = xs.len();
    // Listwise: cases valid on the dependent and every predictor.
    let cases: Vec<(f64, Vec<f64>, f64)> = (0..ds.n_cases())
        .filter_map(|c| {
            let w = ds.case_weight(c);
            let y = valid(ds, yi, c)?;
            let x = xs.iter().map(|(i, _)| valid(ds, *i, c)).collect::<Option<Vec<f64>>>()?;
            (w > 0.0).then_some((y, x, w))
        })
        .collect();
    let n: f64 = cases.iter().map(|(_, _, w)| w).sum();
    let df_residual = n - p as f64 - 1.0;
    if p == 0 || df_residual <= 0.0 {
        return Ok(vec![Item::Warning(tr!("o-regression-needs-cases", n = n, needed = p + 1))]);
    }
    let my = cases.iter().map(|(y, _, w)| y * w).sum::<f64>() / n;
    let mx: Vec<f64> = (0..p).map(|j| cases.iter().map(|(_, x, w)| x.get(j).copied().unwrap_or(0.0) * w).sum::<f64>() / n).collect();
    let dev = |x: &[f64], j: usize| x.get(j).copied().unwrap_or(0.0) - mx.get(j).copied().unwrap_or(0.0);
    let mut sxx = vec![vec![0.0; p]; p];
    let mut sxy = vec![0.0; p];
    let mut syy = 0.0;
    for (y, x, w) in &cases {
        let dy = y - my;
        syy += w * dy * dy;
        let d: Vec<f64> = (0..p).map(|j| dev(x, j)).collect();
        for ((row, s), dj) in sxx.iter_mut().zip(sxy.iter_mut()).zip(&d) {
            *s += w * dj * dy;
            for (cell, dk) in row.iter_mut().zip(&d) {
                *cell += w * dj * dk;
            }
        }
    }
    let Some(inverse) = invert(&sxx) else {
        return Ok(vec![Item::Warning(tr!("o-regression-collinear"))]);
    };
    let b: Vec<f64> = (0..p).map(|j| (0..p).map(|k| inverse[j][k] * sxy[k]).sum()).collect();
    let constant = my - b.iter().zip(&mx).map(|(b, m)| b * m).sum::<f64>();
    let ss_regression: f64 = b.iter().zip(&sxy).map(|(b, s)| b * s).sum();
    let ss_residual = (syy - ss_regression).max(0.0);
    let (df_regression, df_total) = (p as f64, n - 1.0);
    let ms_residual = ss_residual / df_residual;
    let ms_regression = ss_regression / df_regression;
    let f = ms_regression / ms_residual;
    let r2 = if syy > 0.0 { ss_regression / syy } else { 0.0 };
    let adjusted = 1.0 - (1.0 - r2) * df_total / df_residual;
    let sd_y = (syy / df_total).sqrt();

    let names: Vec<String> = xs.iter().map(|(_, v)| v.display_name().to_string()).collect();
    let summary = Table {
        title: tr!("o-model-summary"),
        columns: [tr!("o-r"), tr!("o-r-square"), tr!("o-adjusted-r-square"), tr!("o-std-error-of-the-estimate")].map(String::from).to_vec(),
        rows: vec![("1".into(), vec![num(Some(r2.sqrt()), 3), num(Some(r2), 3), num(Some(adjusted), 3), num(Some(ms_residual.sqrt()), 3)])],
        categories: Vec::new(),
    };
    let anova = Table {
        title: tr!("o-anova"),
        columns: [String::new(), tr!("o-sum-of-squares"), tr!("o-df"), tr!("o-mean-square"), tr!("o-f"), tr!("o-sig")].map(String::from).to_vec(),
        rows: vec![
            (
                "1".into(),
                vec![
                    Cell::Text(tr!("o-regression")),
                    num(Some(ss_regression), 3),
                    n_cell(df_regression),
                    num(Some(ms_regression), 3),
                    num(Some(f), 3),
                    num(dist::f_sig(f, df_regression, df_residual), 3),
                ],
            ),
            (
                String::new(),
                vec![
                    Cell::Text(tr!("o-residual")),
                    num(Some(ss_residual), 3),
                    n_cell(df_residual),
                    num(Some(ms_residual), 3),
                    Cell::Empty,
                    Cell::Empty,
                ],
            ),
            (String::new(), vec![Cell::Text(tr!("o-total")), num(Some(syy), 3), n_cell(df_total), Cell::Empty, Cell::Empty, Cell::Empty]),
        ],
        categories: Vec::new(),
    };

    let t_crit = options.ci.and_then(|level| dist::t_quantile(0.5 + level / 2.0, df_residual));
    let row = |name: String, coefficient: f64, se: f64, beta: Option<f64>| {
        let t = coefficient / se;
        let mut cells =
            vec![Cell::Text(name), num(Some(coefficient), 3), num(Some(se), 3), num(beta, 3), num(Some(t), 3), num(dist::t_sig(t, df_residual), 3)];
        if let Some(tc) = t_crit {
            cells.push(num(Some(coefficient - tc * se), 3));
            cells.push(num(Some(coefficient + tc * se), 3));
        }
        cells
    };
    // Var(constant) = MSE (1/n + x̄ᵀ Sxx⁻¹ x̄).
    let quad: f64 = (0..p).map(|j| (0..p).map(|k| mx[j] * inverse[j][k] * mx[k]).sum::<f64>()).sum();
    let mut coefficients = vec![("1".to_string(), row(tr!("o-constant"), constant, (ms_residual * (1.0 / n + quad)).sqrt(), None))];
    for j in 0..p {
        let se = (ms_residual * inverse[j][j]).sqrt();
        let sd_x = (sxx[j][j] / df_total).sqrt();
        let beta = b[j] * sd_x / sd_y;
        coefficients.push((String::new(), row(names[j].clone(), b[j], se, Some(beta).filter(|b| b.is_finite()))));
    }
    let mut columns = [String::new(), tr!("o-b"), tr!("o-std-error"), tr!("o-beta"), tr!("o-t"), tr!("o-sig")].map(String::from).to_vec();
    if let Some(level) = options.ci {
        columns.extend([tr!("o-ci-lower", level = format!("{:.0}", level * 100.0)), tr!("o-ci-upper", level = format!("{:.0}", level * 100.0))]);
    }
    Ok(vec![
        Item::Table(summary),
        Item::Text(tr!("o-regression-note", predictors = names.join(", "), dependent = yvar.display_name())),
        Item::Table(anova),
        Item::Table(Table { title: tr!("o-coefficients"), columns, rows: coefficients, categories: Vec::new() }),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use mksigma_core::{Value, Variable};

    fn value(c: &Cell) -> f64 {
        match c {
            Cell::Num { value, .. } => *value,
            Cell::Int(n) => *n as f64,
            other => panic!("not a number: {other:?}"),
        }
    }

    fn dataset(rows: &[(f64, f64, f64)]) -> Dataset {
        let mut ds = Dataset::new();
        for name in ["y", "x1", "x2"] {
            ds.add_variable(Variable::numeric(name)).unwrap();
        }
        for (y, a, b) in rows {
            ds.push_case(vec![Value::Num(*y), Value::Num(*a), Value::Num(*b)]).unwrap();
        }
        ds
    }

    #[test]
    fn simple_regression_by_hand() {
        // x = 1..5, y = 2, 4, 5, 4, 5: slope 0.6, intercept 2.2, R² = 0.6.
        let ds = dataset(&[(2.0, 1.0, 0.0), (4.0, 2.0, 0.0), (5.0, 3.0, 0.0), (4.0, 4.0, 0.0), (5.0, 5.0, 0.0)]);
        let items = regression(&ds, &Options { dependent: "y".into(), predictors: vec!["x1".into()], ci: Some(0.95) }).unwrap();
        let Item::Table(summary) = &items[0] else { panic!() };
        assert!((value(&summary.rows[0].1[1]) - 0.6).abs() < 1e-12);
        let Item::Table(coef) = &items[3] else { panic!() };
        assert!((value(&coef.rows[0].1[1]) - 2.2).abs() < 1e-12);
        assert!((value(&coef.rows[1].1[1]) - 0.6).abs() < 1e-12);
        // Beta of a single predictor is r = √R².
        assert!((value(&coef.rows[1].1[3]) - 0.6f64.sqrt()).abs() < 1e-12);
        // SE(b) = √(MSE / Sxx) = √((2.4 / 3) / 10).
        assert!((value(&coef.rows[1].1[2]) - (0.8f64 / 10.0).sqrt()).abs() < 1e-12);
        assert_eq!(coef.rows[1].1.len(), 8, "with CI columns");
    }

    #[test]
    fn exact_fit_with_two_predictors_and_collinearity() {
        // y = 1 + 2 x1 − 3 x2 exactly.
        let rows: Vec<(f64, f64, f64)> =
            [(1.0, 2.0), (2.0, 1.0), (3.0, 5.0), (4.0, 2.0), (5.0, 7.0), (6.0, 3.0)].iter().map(|&(a, b)| (1.0 + 2.0 * a - 3.0 * b, a, b)).collect();
        let items = regression(&dataset(&rows), &Options { dependent: "y".into(), predictors: vec!["x1".into(), "x2".into()], ci: None }).unwrap();
        let Item::Table(coef) = &items[3] else { panic!() };
        assert!((value(&coef.rows[0].1[1]) - 1.0).abs() < 1e-9);
        assert!((value(&coef.rows[1].1[1]) - 2.0).abs() < 1e-9);
        assert!((value(&coef.rows[2].1[1]) + 3.0).abs() < 1e-9);

        let collinear: Vec<(f64, f64, f64)> = (1..8).map(|i| (f64::from(i) * 1.5, f64::from(i), f64::from(i) * 2.0)).collect();
        let items =
            regression(&dataset(&collinear), &Options { dependent: "y".into(), predictors: vec!["x1".into(), "x2".into()], ci: None }).unwrap();
        assert!(matches!(&items[0], Item::Warning(s) if s.contains("linear combination")));
    }
}
