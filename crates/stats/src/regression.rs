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

#[derive(Debug, Clone)]
pub struct Options {
    pub dependent: String,
    pub predictors: Vec<String>,
    /// Confidence intervals for B at this level (`/STATISTICS CI(95)`), if asked.
    pub ci: Option<f64>,
    /// A predictor enters only if its tolerance, 1 − R² on those already in, reaches this
    /// (`/CRITERIA=TOLERANCE(.0001)`, the default).
    pub tolerance: f64,
    /// Through the origin, without a constant (`/ORIGIN`): sums of squares about 0, not the
    /// means.
    pub origin: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self { dependent: String::new(), predictors: Vec::new(), ci: None, tolerance: TOLERANCE, origin: false }
    }
}

/// SPSS's default tolerance criterion.
pub const TOLERANCE: f64 = 0.0001;

/// A case: the dependent, the predictors, the weight.
pub(crate) type Case = (f64, Vec<f64>, f64);

/// The residuals of `target` (a function of a case) on the predictors `set` (and a constant,
/// with `constant`), with their weights; `None` when those predictors are collinear.
pub(crate) fn residuals(cases: &[Case], set: &[usize], constant: bool, target: impl Fn(&Case) -> f64) -> Option<Vec<(f64, f64)>> {
    let sub: Vec<Case> = cases.iter().map(|c| (target(c), set.iter().map(|&j| c.1.get(j).copied().unwrap_or(0.0)).collect(), c.2)).collect();
    let fit = least_squares(&sub, set.len(), constant)?;
    let (intercept, slopes) = if constant {
        (fit.coefficients.first().copied().unwrap_or(0.0), fit.coefficients.get(1..).unwrap_or(&[]))
    } else {
        (0.0, fit.coefficients.as_slice())
    };
    Some(sub.iter().map(|(v, x, w)| (v - intercept - slopes.iter().zip(x).map(|(c, xi)| c * xi).sum::<f64>(), *w)).collect())
}

/// The weighted sum of squares of `f` about its mean, or about 0 without a constant.
fn spread_of(cases: &[Case], constant: bool, f: impl Fn(&Case) -> f64) -> f64 {
    let total_w: f64 = cases.iter().map(|c| c.2).sum();
    let mean = if constant { cases.iter().map(|c| f(c) * c.2).sum::<f64>() / total_w } else { 0.0 };
    cases.iter().map(|c| c.2 * (f(c) - mean).powi(2)).sum()
}

/// Predictor `j`'s tolerance next to `set`: the share of its variance (about the mean, or about
/// 0 through the origin) the others leave unexplained, 1 − R². 0 for a predictor with none.
pub(crate) fn tolerance(cases: &[Case], set: &[usize], j: usize, constant: bool) -> f64 {
    let x = |c: &Case| c.1.get(j).copied().unwrap_or(0.0);
    let total = spread_of(cases, constant, x);
    if total <= 0.0 {
        return 0.0;
    }
    if set.is_empty() {
        return 1.0;
    }
    residuals(cases, set, constant, x).map_or(0.0, |e| e.iter().map(|(r, w)| w * r * r).sum::<f64>() / total)
}

/// `/METHOD=ENTER` with the tolerance test: in the order listed, each predictor enters if its
/// tolerance next to those already in reaches `criterion`. The entered, and the left out.
pub(crate) fn entry(cases: &[Case], p: usize, criterion: f64, constant: bool) -> (Vec<usize>, Vec<usize>) {
    let (mut entered, mut excluded) = (Vec::new(), Vec::new());
    for j in 0..p {
        if tolerance(cases, &entered, j, constant) >= criterion { entered.push(j) } else { excluded.push(j) }
    }
    (entered, excluded)
}

/// For a predictor left out: its standardized coefficient if it entered (Beta In), t, the
/// partial correlation with the dependent and its tolerance, next to the entered ones. `None`
/// for the first three when it adds nothing at all (a tolerance of 0).
pub(crate) struct Excluded {
    pub beta_in: Option<f64>,
    pub t: Option<f64>,
    pub partial: Option<f64>,
    pub tolerance: f64,
    pub df: f64,
}

pub(crate) fn excluded(cases: &[Case], entered: &[usize], j: usize, constant: bool) -> Excluded {
    let n: f64 = cases.iter().map(|c| c.2).sum();
    let df = n - entered.len() as f64 - 1.0 - f64::from(u8::from(constant));
    let tolerance = tolerance(cases, entered, j, constant);
    let ey = residuals(cases, entered, constant, |c| c.0);
    let ex = residuals(cases, entered, constant, |c| c.1.get(j).copied().unwrap_or(0.0));
    let none = Excluded { beta_in: None, t: None, partial: None, tolerance, df };
    let (Some(ey), Some(ex)) = (ey, ex) else { return none };
    let (syy, sxx, sxy) = ey.iter().zip(&ex).fold((0.0, 0.0, 0.0), |(a, b, c), ((y, w), (x, _))| (a + w * y * y, b + w * x * x, c + w * x * y));
    if tolerance < 1e-12 || sxx <= 0.0 || syy <= 0.0 {
        return none;
    }
    let partial = sxy / (sxx * syy).sqrt();
    // The coefficient in standard deviations; through the origin there is none.
    let beta_in = constant.then(|| {
        let sd = |f: &dyn Fn(&Case) -> f64| (spread_of(cases, true, f) / (n - 1.0)).sqrt();
        sxy / sxx * sd(&|c| c.1.get(j).copied().unwrap_or(0.0)) / sd(&|c| c.0)
    });
    let t = partial * (df / (1.0 - partial * partial)).sqrt();
    Excluded { beta_in, t: Some(t), partial: Some(partial), tolerance, df }
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

/// A least-squares fit: the coefficients (the constant first, when there is one), `(XᵀX)⁻¹`,
/// and the regression and residual sums of squares (about the mean, or about 0 without a
/// constant).
pub(crate) struct LeastSquares {
    pub coefficients: Vec<f64>,
    pub inverse: Vec<Vec<f64>>,
    pub ss_regression: f64,
    pub ss_residual: f64,
}

/// Least squares of `y` on `x` (and a constant, with `constant`), each case weighted, by
/// Householder QR. `None` when a column is (numerically) a combination of the ones before it.
pub(crate) fn least_squares(cases: &[Case], p: usize, constant: bool) -> Option<LeastSquares> {
    let first = usize::from(constant);
    let k = p + first;
    let n = cases.len();
    // Fewer cases than coefficients (heavy weights can hide it) leave the fit undetermined.
    if n < k {
        return None;
    }
    // Column by column: the constant (if any), then each predictor; and y.
    let mut a: Vec<Vec<f64>> = (0..k)
        .map(|j| cases.iter().map(|(_, x, w)| w.sqrt() * if j < first { 1.0 } else { x.get(j - first).copied().unwrap_or(0.0) }).collect())
        .collect();
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
    let ss_regression = qty.iter().take(k).skip(first).map(|v| v * v).sum();
    let ss_residual = qty.iter().skip(k).map(|v| v * v).sum();
    Some(LeastSquares { coefficients, inverse, ss_regression, ss_residual })
}

pub fn regression(ds: &Dataset, options: &Options) -> Result<Vec<Item>, StatsError> {
    let (yi, yvar) = numeric(ds, &options.dependent)?;
    let xs = options.predictors.iter().map(|n| numeric(ds, n)).collect::<Result<Vec<_>, _>>()?;
    let p = xs.len();
    // Listwise: cases valid on the dependent and every predictor.
    let cases: Vec<Case> = (0..ds.n_cases())
        .filter_map(|c| {
            let w = ds.case_weight(c);
            let y = valid(ds, yi, c)?;
            let x = xs.iter().map(|(i, _)| valid(ds, *i, c)).collect::<Option<Vec<f64>>>()?;
            (w > 0.0).then_some((y, x, w))
        })
        .collect();
    let n: f64 = cases.iter().map(|(_, _, w)| w).sum();
    // With a constant it counts as a coefficient too.
    let with_constant = !options.origin;
    let first = usize::from(with_constant);
    if p == 0 || n - (p + first) as f64 <= 0.0 {
        return Ok(vec![Item::Warning(tr!("o-regression-needs-cases", n = n, needed = p + first))]);
    }
    // The tolerance test; the model holds the predictors that entered.
    let (entered, left_out) = entry(&cases, p, options.tolerance, with_constant);
    if entered.is_empty() {
        return Ok(vec![Item::Warning(tr!("o-regression-collinear"))]);
    }
    let all_cases = cases;
    let cases: Vec<Case> = all_cases.iter().map(|(y, x, w)| (*y, entered.iter().map(|&j| x.get(j).copied().unwrap_or(0.0)).collect(), *w)).collect();
    let all_names: Vec<String> = xs.iter().map(|(_, v)| v.display_name().to_string()).collect();
    let xs: Vec<_> = entered.iter().filter_map(|&j| xs.get(j).copied()).collect();
    let p = xs.len();
    let df_residual = n - (p + first) as f64;
    // The dependent's and each predictor's sum of squares — about the mean, or about 0 through
    // the origin — for R² and the betas.
    let syy = spread_of(&cases, with_constant, |c| c.0);
    let sxx: Vec<f64> = (0..p).map(|j| spread_of(&cases, with_constant, |c| c.1.get(j).copied().unwrap_or(0.0))).collect();
    let Some(LeastSquares { coefficients, inverse, ss_regression, ss_residual }) = least_squares(&cases, p, with_constant) else {
        return Ok(vec![Item::Warning(tr!("o-regression-collinear"))]);
    };
    // An exact fit: no residual at all, not a rounding's −0, so that F is +∞ and its
    // significance 0.
    let ss_residual = if ss_residual > 0.0 { ss_residual } else { 0.0 };
    let constant = coefficients.first().copied().filter(|_| with_constant);
    let b: Vec<f64> = coefficients.iter().skip(first).copied().collect();
    let (df_regression, df_total) = (p as f64, n - first as f64);
    let ms_residual = ss_residual / df_residual;
    let ms_regression = ss_regression / df_regression;
    let f = ms_regression / ms_residual;
    let r2 = if syy > 0.0 { (ss_regression / syy).clamp(0.0, 1.0) } else { 0.0 };
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
    let mut coefficients: Vec<(String, Vec<Cell>)> =
        constant.map(|c| ("1".to_string(), row(tr!("o-constant"), c, se(0), None))).into_iter().collect();
    for ((j, name), (bj, sj)) in names.iter().enumerate().zip(b.iter().zip(&sxx)) {
        // In standard deviations; through the origin there is no Beta.
        let beta = with_constant.then(|| bj * (sj / df_total).sqrt() / sd_y).filter(|b| b.is_finite());
        let label = if coefficients.is_empty() { "1".to_string() } else { String::new() };
        coefficients.push((label, row(name.clone(), *bj, se(j + first), beta)));
    }
    let mut columns = [String::new(), tr!("o-b"), tr!("o-std-error"), tr!("o-beta"), tr!("o-t"), tr!("o-sig")].map(String::from).to_vec();
    if let Some(level) = options.ci {
        columns.extend([tr!("o-ci-lower", level = format!("{:.0}", level * 100.0)), tr!("o-ci-upper", level = format!("{:.0}", level * 100.0))]);
    }
    let mut items = vec![
        Item::Table(summary),
        Item::Text(if with_constant {
            tr!("o-regression-note", predictors = names.join(", "), dependent = yvar.display_name())
        } else {
            tr!("o-regression-note-origin", predictors = names.join(", "), dependent = yvar.display_name())
        }),
        Item::Table(anova),
        Item::Table(Table { title: tr!("o-coefficients"), columns, rows: coefficients, categories: Vec::new() }),
    ];
    if !left_out.is_empty() {
        let rows = left_out
            .iter()
            .map(|&j| {
                let e = excluded(&all_cases, &entered, j, with_constant);
                let sig = e.t.and_then(|t| dist::t_sig(t, e.df));
                let name = all_names.get(j).cloned().unwrap_or_default();
                (name, vec![num(e.beta_in, 3), num(e.t, 3), num(sig, 3), num(e.partial, 3), num(Some(e.tolerance), 3)])
            })
            .collect();
        let columns = [tr!("o-beta-in"), tr!("o-t"), tr!("o-sig"), tr!("o-partial-correlation"), tr!("o-tolerance")].map(String::from).to_vec();
        let names: Vec<String> = left_out.iter().filter_map(|&j| all_names.get(j).cloned()).collect();
        items.push(Item::Table(Table { title: tr!("o-excluded-variables"), columns, rows, categories: Vec::new() }));
        items.push(Item::Text(tr!(
            "o-regression-excluded-note",
            names = names.join(", "),
            tolerance = mksigma_i18n::decimal(&format!("{}", options.tolerance))
        )));
    }
    Ok(items)
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
    fn two_heavy_cases_fit_a_line_and_leave_the_plane_out() {
        // N = 20 by the weights, but two points place a line, not a plane: b can't enter after
        // a, and the line fits them exactly.
        let mut ds = Dataset::new();
        for name in ["y", "a", "b", "w"] {
            ds.add_variable(Variable::numeric(name)).unwrap();
        }
        for row in [[1.0, 2.0, 3.0, 10.0], [2.0, 5.0, 1.0, 10.0]] {
            ds.push_case(row.iter().map(|v| Value::Num(*v)).collect()).unwrap();
        }
        ds.weight = Some(3);
        let items = regression(
            &ds,
            &Options { dependent: "y".into(), predictors: vec!["a".into(), "b".into()], ci: None, tolerance: TOLERANCE, origin: false },
        )
        .unwrap();
        let Item::Table(anova) = &items[2] else { panic!("{items:?}") };
        assert_eq!((value(&anova.rows[1].1[1]), value(&anova.rows[0].1[5])), (0.0, 0.0), "no residual; F infinite, its significance 0");
        let Item::Table(summary) = &items[0] else { panic!() };
        assert_eq!(value(&summary.rows[0].1[1]), 1.0, "R² stays within 1");
        assert!(matches!(&items[4], Item::Table(t) if t.rows[0].0 == "b"));
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
        let items =
            regression(&ds, &Options { dependent: "y".into(), predictors: vec!["x1".into()], ci: Some(0.95), tolerance: TOLERANCE, origin: false })
                .unwrap();
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
        let items = regression(
            &dataset(&rows),
            &Options { dependent: "y".into(), predictors: vec!["x1".into(), "x2".into()], ci: None, tolerance: TOLERANCE, origin: false },
        )
        .unwrap();
        let Item::Table(coef) = &items[3] else { panic!() };
        assert!((value(&coef.rows[0].1[1]) - 1.0).abs() < 1e-9);
        assert!((value(&coef.rows[1].1[1]) - 2.0).abs() < 1e-9);
        assert!((value(&coef.rows[2].1[1]) + 3.0).abs() < 1e-9);

        let collinear: Vec<(f64, f64, f64)> = (1..8).map(|i| (f64::from(i) * 1.5, f64::from(i), f64::from(i) * 2.0)).collect();
        let items = regression(
            &dataset(&collinear),
            &Options { dependent: "y".into(), predictors: vec!["x1".into(), "x2".into()], ci: None, tolerance: TOLERANCE, origin: false },
        )
        .unwrap();
        // x2 = 2 x1 can't enter after x1: the model is y on x1 alone, an exact fit, whose t and
        // significance are infinite and 0 (a NaN t once crashed the significance), and x2 is
        // listed as excluded, its tolerance 0.
        let Item::Table(coef) = &items[3] else { panic!("{items:?}") };
        assert_eq!(coef.rows.len(), 2);
        assert!((value(&coef.rows[1].1[1]) - 1.5).abs() < 1e-9);
        let Item::Table(excluded) = &items[4] else { panic!("{items:?}") };
        assert_eq!((excluded.title.as_str(), excluded.rows[0].0.as_str()), ("Excluded Variables", "x2"));
        assert!(value(&excluded.rows[0].1[4]) < 1e-12 && excluded.rows[0].1[0] == Cell::Empty);
        // Every predictor constant: nothing enters.
        let flat: Vec<(f64, f64, f64)> = (1..6).map(|i| (f64::from(i), 1.0, 2.0)).collect();
        let items = regression(
            &dataset(&flat),
            &Options { dependent: "y".into(), predictors: vec!["x1".into(), "x2".into()], ci: None, tolerance: TOLERANCE, origin: false },
        )
        .unwrap();
        assert!(matches!(&items[..], [Item::Warning(s)] if s.contains("tolerance")), "{items:?}");
    }

    #[test]
    fn a_predictor_under_the_tolerance_is_listed_with_its_partial_correlation() {
        // x2 tracks x1 closely; with TOLERANCE(.5) it stays out.
        let rows: Vec<(f64, f64, f64)> =
            [(3.0, 1.0, 1.2), (5.0, 2.0, 1.9), (4.0, 3.0, 3.3), (8.0, 4.0, 3.8), (9.0, 5.0, 5.1), (12.0, 6.0, 6.4)].to_vec();
        let options = Options { dependent: "y".into(), predictors: vec!["x1".into(), "x2".into()], ci: None, tolerance: 0.5, origin: false };
        let items = regression(&dataset(&rows), &options).unwrap();
        let Some(Item::Table(excluded)) = items.iter().find(|i| matches!(i, Item::Table(t) if t.title == "Excluded Variables")) else {
            panic!("{items:?}")
        };
        let cells = &excluded.rows[0].1;
        // The partial correlation from the simple ones: (r_y2 − r_y1 r_12) / √((1 − r_y1²)(1 − r_12²)).
        let r = |a: &dyn Fn(&(f64, f64, f64)) -> f64, b: &dyn Fn(&(f64, f64, f64)) -> f64| {
            let n = rows.len() as f64;
            let (ma, mb) = (rows.iter().map(a).sum::<f64>() / n, rows.iter().map(b).sum::<f64>() / n);
            let sab: f64 = rows.iter().map(|q| (a(q) - ma) * (b(q) - mb)).sum();
            let saa: f64 = rows.iter().map(|q| (a(q) - ma).powi(2)).sum();
            let sbb: f64 = rows.iter().map(|q| (b(q) - mb).powi(2)).sum();
            sab / (saa * sbb).sqrt()
        };
        let (ry1, ry2, r12) = (r(&|q| q.0, &|q| q.1), r(&|q| q.0, &|q| q.2), r(&|q| q.1, &|q| q.2));
        let partial = (ry2 - ry1 * r12) / ((1.0 - ry1 * ry1) * (1.0 - r12 * r12)).sqrt();
        assert!((value(&cells[3]) - partial).abs() < 1e-12, "{} against {partial}", value(&cells[3]));
        assert!((value(&cells[4]) - (1.0 - r12 * r12)).abs() < 1e-12, "the tolerance is 1 − r12² with one predictor in");
        // t from the partial correlation, with n − 1 − 2 degrees of freedom.
        assert!((value(&cells[1]) - partial * (3.0 / (1.0 - partial * partial)).sqrt()).abs() < 1e-12);
    }
}
