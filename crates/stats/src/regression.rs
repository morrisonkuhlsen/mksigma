//! `REGRESSION` with `/METHOD=ENTER`: ordinary least squares of one dependent variable on the
//! predictors, cases missing any of them excluded (listwise), weights as frequencies.
//!
//! Solved by a QR decomposition (Householder reflections) of the cases' design matrix, each row
//! scaled by the square root of its weight, as accurate software does: it never forms `XᵀX`,
//! whose condition number is the square of the data's, so ill-conditioned predictors (powers of
//! one variable, say) keep their digits. The residual and regression sums of squares are read
//! off `Qᵀy`; the standard errors come from `R⁻¹R⁻ᵀ = (XᵀX)⁻¹` and the residual mean square.

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

/// A least-squares fit: the coefficients (the constant first), `(XᵀX)⁻¹`, and the regression
/// and residual sums of squares.
pub(crate) struct LeastSquares {
    pub coefficients: Vec<f64>,
    pub inverse: Vec<Vec<f64>>,
    pub ss_regression: f64,
    pub ss_residual: f64,
}

/// Least squares of `y` on a constant and `x`, each case weighted, by Householder QR. `None`
/// when a column is (numerically) a combination of the ones before it.
pub(crate) fn least_squares(cases: &[(f64, Vec<f64>, f64)], p: usize) -> Option<LeastSquares> {
    let k = p + 1;
    let n = cases.len();
    // Fewer cases than coefficients (heavy weights can hide it) leave the fit undetermined.
    if n < k {
        return None;
    }
    // Column by column: the constant, then each predictor; and y.
    let mut a: Vec<Vec<f64>> =
        (0..k).map(|j| cases.iter().map(|(_, x, w)| w.sqrt() * if j == 0 { 1.0 } else { x.get(j - 1).copied().unwrap_or(0.0) }).collect()).collect();
    let mut qty: Vec<f64> = cases.iter().map(|(y, _, w)| w.sqrt() * y).collect();
    let norms: Vec<f64> = a.iter().map(|c| c.iter().map(|v| v * v).sum::<f64>().sqrt()).collect();
    let mut diagonal = vec![0.0; k];
    for j in 0..k {
        let column: Vec<f64> = a.get(j)?.get(j..)?.to_vec();
        let norm = column.iter().map(|v| v * v).sum::<f64>().sqrt();
        // What is left of the column after the ones before it: next to nothing is collinear.
        if norm <= 1e-13 * norms.get(j).copied().unwrap_or(0.0).max(f64::MIN_POSITIVE) {
            return None;
        }
        let head = column.first().copied().unwrap_or(0.0);
        let alpha = if head > 0.0 { -norm } else { norm };
        let mut v = column;
        if let Some(v0) = v.first_mut() {
            *v0 -= alpha;
        }
        let vv: f64 = v.iter().map(|x| x * x).sum();
        let reflect = |target: &mut [f64]| {
            let s: f64 = v.iter().zip(target.iter()).map(|(a, b)| a * b).sum();
            let f = 2.0 * s / vv;
            for (t, vi) in target.iter_mut().zip(&v) {
                *t -= f * vi;
            }
        };
        if vv > 0.0 {
            for c in a.iter_mut().skip(j) {
                if let Some(tail) = c.get_mut(j..) {
                    reflect(tail);
                }
            }
            if let Some(tail) = qty.get_mut(j..) {
                reflect(tail);
            }
        }
        *diagonal.get_mut(j)? = alpha;
    }
    // R is upper triangular: R[i][j] = a[j][i] for i ≤ j, its diagonal the alphas.
    let r =
        |i: usize, j: usize| if i == j { diagonal.get(i).copied().unwrap_or(0.0) } else { a.get(j).and_then(|c| c.get(i)).copied().unwrap_or(0.0) };
    let mut coefficients = vec![0.0; k];
    for i in (0..k).rev() {
        let s: f64 = ((i + 1)..k).map(|j| r(i, j) * coefficients.get(j).copied().unwrap_or(0.0)).sum();
        *coefficients.get_mut(i)? = (qty.get(i).copied().unwrap_or(0.0) - s) / r(i, i);
    }
    // R⁻¹, column by column, then (XᵀX)⁻¹ = R⁻¹R⁻ᵀ.
    let mut r_inv = vec![vec![0.0; k]; k];
    for c in 0..k {
        for i in (0..=c).rev() {
            let s: f64 = ((i + 1)..=c).map(|j| r(i, j) * r_inv.get(j).and_then(|row| row.get(c)).copied().unwrap_or(0.0)).sum();
            let identity = if i == c { 1.0 } else { 0.0 };
            *r_inv.get_mut(i)?.get_mut(c)? = (identity - s) / r(i, i);
        }
    }
    let inverse = (0..k)
        .map(|i| (0..k).map(|j| r_inv.get(i).zip(r_inv.get(j)).map_or(0.0, |(a, b)| a.iter().zip(b).map(|(x, y)| x * y).sum())).collect())
        .collect();
    // Qᵀy: the constant's part, the predictors' (the regression), the rest (the residuals).
    let ss_regression = qty.iter().take(k).skip(1).map(|v| v * v).sum();
    let ss_residual = qty.iter().skip(k).map(|v| v * v).sum();
    Some(LeastSquares { coefficients, inverse, ss_regression, ss_residual })
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
    // The dependent's and each predictor's sum of squared deviations, for R² and the betas.
    let syy: f64 = cases.iter().map(|(y, _, w)| w * (y - my).powi(2)).sum();
    let sxx: Vec<f64> =
        mx.iter().enumerate().map(|(j, m)| cases.iter().map(|(_, x, w)| w * (x.get(j).copied().unwrap_or(0.0) - m).powi(2)).sum()).collect();
    let Some(LeastSquares { coefficients, inverse, ss_regression, ss_residual }) = least_squares(&cases, p) else {
        return Ok(vec![Item::Warning(tr!("o-regression-collinear"))]);
    };
    let constant = coefficients.first().copied().unwrap_or(f64::NAN);
    let b: Vec<f64> = coefficients.iter().skip(1).copied().collect();
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
    // Var(B) is the residual mean square times the diagonal of (XᵀX)⁻¹, the constant's first.
    let se = |j: usize| (ms_residual * inverse.get(j).and_then(|r| r.get(j)).copied().unwrap_or(f64::NAN)).sqrt();
    let mut coefficients = vec![("1".to_string(), row(tr!("o-constant"), constant, se(0), None))];
    for ((j, name), (bj, sj)) in names.iter().enumerate().zip(b.iter().zip(&sxx)) {
        let sd_x = (sj / df_total).sqrt();
        let beta = bj * sd_x / sd_y;
        coefficients.push((String::new(), row(name.clone(), *bj, se(j + 1), Some(beta).filter(|b| b.is_finite()))));
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

    #[test]
    fn two_heavy_cases_cannot_fit_three_coefficients() {
        // N = 20 by the weights, but two points can't place a plane.
        let mut ds = Dataset::new();
        for name in ["y", "a", "b", "w"] {
            ds.add_variable(Variable::numeric(name)).unwrap();
        }
        for row in [[1.0, 2.0, 3.0, 10.0], [2.0, 5.0, 1.0, 10.0]] {
            ds.push_case(row.iter().map(|v| Value::Num(*v)).collect()).unwrap();
        }
        ds.weight = Some(3);
        let items = regression(&ds, &Options { dependent: "y".into(), predictors: vec!["a".into(), "b".into()], ci: None }).unwrap();
        assert!(matches!(&items[..], [Item::Warning(_)]), "{items:?}");
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
