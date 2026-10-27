//! `LOGISTIC REGRESSION` with `/METHOD=ENTER`: a binary dependent variable (its lower value
//! coded 0, the higher 1) on numeric covariates, fitted by maximum likelihood with
//! Newton–Raphson. Cases missing any variable are left out; weights count as frequencies.

use mksigma_core::{Dataset, Value, Variable};
use mksigma_i18n::tr;
use mksigma_output::{Cell, Item, Table};
use statrs::distribution::{ContinuousCDF, Normal};

use crate::groups::{numeric, valid};
use crate::regression::invert;
use crate::{StatsError, dist, n_cell, num, value_text};

#[derive(Debug, Clone)]
pub struct Options {
    pub dependent: String,
    pub covariates: Vec<String>,
    /// Confidence intervals for Exp(B) at this level (`/PRINT=CI(95)`), if asked.
    pub ci: Option<f64>,
    /// Most Newton–Raphson iterations (`/CRITERIA=ITERATE(20)`).
    pub iterations: usize,
    /// Predicted probability at or above which a case is classified as 1 (`CUT(.5)`).
    pub cut: f64,
}

impl Default for Options {
    fn default() -> Self {
        Self { dependent: String::new(), covariates: Vec::new(), ci: None, iterations: 20, cut: 0.5 }
    }
}

pub(crate) fn probability(x: &[f64], beta: &[f64]) -> f64 {
    let eta: f64 = x.iter().zip(beta).map(|(a, b)| a * b).sum();
    1.0 / (1.0 + (-eta).exp())
}

/// Weighted log-likelihood of 0/1 outcomes.
fn log_likelihood(rows: &[Row], beta: &[f64]) -> f64 {
    rows.iter()
        .map(|(y, x, w)| {
            let p = probability(x, beta).clamp(1e-300, 1.0 - 1e-16);
            w * (y * p.ln() + (1.0 - y) * (1.0 - p).ln())
        })
        .sum()
}

/// A case: y coded 0 or 1, x with a leading 1 for the constant, and its weight.
pub(crate) type Row = (f64, Vec<f64>, f64);

/// The cases a model is fitted on, and what they were coded from.
pub(crate) struct Model<'a> {
    pub yvar: &'a Variable,
    pub xs: Vec<(usize, &'a Variable)>,
    /// The dependent's values coded 0 and 1.
    pub low: f64,
    pub high: f64,
    pub rows: Vec<Row>,
}

/// The cases valid on every variable (listwise) with a positive weight, the dependent coded 0
/// and 1; `Ok(Err((name, n)))` when the dependent has `n` values instead of two.
pub(crate) fn model<'a>(ds: &'a Dataset, options: &Options) -> Result<Result<Model<'a>, (String, usize)>, StatsError> {
    let (yi, yvar) = numeric(ds, &options.dependent)?;
    let xs = options.covariates.iter().map(|n| numeric(ds, n)).collect::<Result<Vec<_>, _>>()?;
    let raw: Vec<Row> = (0..ds.n_cases())
        .filter_map(|c| {
            let w = ds.case_weight(c);
            let y = valid(ds, yi, c)?;
            let mut x = vec![1.0];
            for (i, _) in &xs {
                x.push(valid(ds, *i, c)?);
            }
            (w > 0.0).then_some((y, x, w))
        })
        .collect();
    let mut levels: Vec<f64> = raw.iter().map(|(y, _, _)| *y).collect();
    levels.sort_by(f64::total_cmp);
    levels.dedup();
    let [low, high] = levels[..] else { return Ok(Err((yvar.name.clone(), levels.len()))) };
    let rows = raw.into_iter().map(|(y, x, w)| (if y == high { 1.0 } else { 0.0 }, x, w)).collect();
    Ok(Ok(Model { yvar, xs, low, high, rows }))
}

/// The gradient of the log-likelihood, Σ w (y − p) x, and the information matrix,
/// Σ w p (1 − p) x xᵀ, at `beta`.
pub(crate) fn derivatives(rows: &[Row], beta: &[f64]) -> (Vec<f64>, Vec<Vec<f64>>) {
    let k = beta.len();
    let mut gradient = vec![0.0; k];
    let mut information = vec![vec![0.0; k]; k];
    for (y, x, w) in rows {
        let p = probability(x, beta);
        let v = w * p * (1.0 - p);
        for ((g, row), xa) in gradient.iter_mut().zip(information.iter_mut()).zip(x) {
            *g += w * (y - p) * xa;
            for (h, xb) in row.iter_mut().zip(x) {
                *h += v * xa * xb;
            }
        }
    }
    (gradient, information)
}

/// One Newton–Raphson iteration.
pub(crate) struct Iteration {
    pub gradient: Vec<f64>,
    pub information: Vec<Vec<f64>>,
    /// `None` when the information can't be inverted: the iterations stop there.
    pub inverse: Option<Vec<Vec<f64>>>,
    /// The inverse times the gradient, added to the estimates.
    pub step: Vec<f64>,
    pub to: Vec<f64>,
    pub ll: f64,
}

/// The two values of the dependent variable split apart by the covariates, so the likelihood
/// keeps rising as the estimates grow and no maximum exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Separation {
    /// Every case: a line through the covariates puts all the 0s on one side, all the 1s on the
    /// other.
    Complete,
    /// Some cases are predicted exactly, the others not.
    Quasi,
}

/// A model fitted by maximum likelihood.
pub(crate) struct Fit {
    /// The weighted number of cases and of ones.
    pub n: f64,
    pub ones: f64,
    /// The constant-only model: its probability and log-likelihood.
    pub p0: f64,
    pub ll0: f64,
    pub iterations: Vec<Iteration>,
    pub beta: Vec<f64>,
    pub ll: f64,
    pub converged: bool,
    pub separation: Option<Separation>,
    /// The information at the final estimates, and its inverse (the estimates' covariance; the
    /// last iteration's inverse when this one has none).
    pub information: Vec<Vec<f64>>,
    pub covariance: Option<Vec<Vec<f64>>>,
}

/// Newton–Raphson from the constant-only model, at most `max` iterations.
pub(crate) fn fit(rows: &[Row], k: usize, max: usize) -> Fit {
    let n: f64 = rows.iter().map(|(_, _, w)| w).sum();
    let ones: f64 = rows.iter().map(|(y, _, w)| y * w).sum();
    // The constant-only model has a closed form.
    let p0 = ones / n;
    let ll0 = ones * p0.ln() + (n - ones) * (1.0 - p0).ln();
    let mut beta = vec![0.0; k];
    if let Some(b0) = beta.first_mut() {
        *b0 = (p0 / (1.0 - p0)).ln();
    }
    let mut ll = log_likelihood(rows, &beta);
    let (mut iterations, mut converged, mut last_inverse) = (Vec::new(), false, None);
    while iterations.len() < max.max(1) {
        let (gradient, information) = derivatives(rows, &beta);
        let Some(inverse) = invert(&information) else {
            iterations.push(Iteration { gradient, information, inverse: None, step: Vec::new(), to: beta.clone(), ll });
            break;
        };
        let step: Vec<f64> = inverse.iter().map(|row| row.iter().zip(&gradient).map(|(a, g)| a * g).sum()).collect();
        for (b, s) in beta.iter_mut().zip(&step) {
            *b += s;
        }
        ll = log_likelihood(rows, &beta);
        // SPSS's default: stop when no estimate changes by more than 0.1%. (Its criterion on the
        // log-likelihood, LCON, is off by default: under separation the log-likelihood flattens
        // towards 0 while the estimates keep growing.)
        let small = step.iter().zip(&beta).all(|(s, b)| s.abs() <= 0.001 * b.abs().max(1e-8));
        last_inverse = Some(inverse.clone());
        iterations.push(Iteration { gradient, information, inverse: Some(inverse), step, to: beta.clone(), ll });
        if small {
            converged = true;
            break;
        }
    }
    let separation = separation(rows, &beta, converged);
    if separation == Some(Separation::Complete) {
        // The estimates may have stopped changing by 0.1% only because they are huge.
        converged = false;
    }
    // Standard errors from the information at the final estimates.
    let (_, information) = derivatives(rows, &beta);
    let covariance = invert(&information).or(last_inverse);
    Fit { n, ones, p0, ll0, iterations, beta, ll, converged, separation, information, covariance }
}

/// Whether the covariates separate the two values, judged at the last estimates (Albert &
/// Anderson 1984): complete when every case falls on its own side of 0 on the linear
/// predictor and the fit is exact or still moving; quasi-complete when the iterations didn't
/// converge and some cases are already predicted within 1e-6 of their value.
fn separation(rows: &[Row], beta: &[f64], converged: bool) -> Option<Separation> {
    let miss = |(y, x, _): &Row| (y - probability(x, beta)).abs();
    let sides = rows.iter().all(|(y, x, _)| {
        let eta: f64 = x.iter().zip(beta).map(|(a, b)| a * b).sum();
        (eta > 0.0) == (*y == 1.0) && eta != 0.0
    });
    let exact = rows.iter().all(|r| miss(r) < 1e-8);
    if !rows.is_empty() && sides && (exact || !converged) {
        Some(Separation::Complete)
    } else if !converged && rows.iter().any(|r| miss(r) < 1e-6) {
        Some(Separation::Quasi)
    } else {
        None
    }
}

/// What to say about a separation.
pub(crate) fn separation_text(separation: Separation) -> String {
    match separation {
        Separation::Complete => tr!("o-logistic-complete-separation"),
        Separation::Quasi => tr!("o-logistic-quasi-separation"),
    }
}

pub fn logistic(ds: &Dataset, options: &Options) -> Result<Vec<Item>, StatsError> {
    let Model { yvar, xs, low, high, rows } = match model(ds, options)? {
        Ok(m) => m,
        Err((name, levels)) => return Ok(vec![Item::Warning(tr!("o-logistic-two-values", name = name.as_str(), n = levels))]),
    };
    let k = xs.len() + 1;
    let Fit { n, p0: _, ll0, iterations, beta, ll, converged, separation, covariance, .. } = fit(&rows, k, options.iterations);
    let iterations = iterations.len();
    let Some(covariance) = covariance else {
        // Under separation the information vanishes as the estimates grow: not collinearity.
        return Ok(vec![Item::Warning(separation.map_or_else(|| tr!("o-logistic-collinear"), separation_text))]);
    };

    let chi = 2.0 * (ll - ll0);
    let df = xs.len() as f64;
    let chi_sig = dist::chi_square_sig(chi, df);
    let cox_snell = 1.0 - (2.0 / n * (ll0 - ll)).exp();
    let nagelkerke = cox_snell / (1.0 - (2.0 * ll0 / n).exp());

    let encoding = Table {
        title: tr!("o-dependent-variable-encoding"),
        columns: vec![tr!("o-internal-value")],
        rows: vec![(value_text(yvar, &Value::Num(low)), vec![Cell::Int(0)]), (value_text(yvar, &Value::Num(high)), vec![Cell::Int(1)])],
        categories: Vec::new(),
    };
    let omnibus_row = || vec![num(Some(chi), 3), n_cell(df), num(chi_sig, 3)];
    let omnibus = Table {
        title: tr!("o-omnibus-tests-of-model-coefficients"),
        columns: [tr!("o-chi-square"), tr!("o-df"), tr!("o-sig")].map(String::from).to_vec(),
        rows: vec![(tr!("o-step"), omnibus_row()), (tr!("o-block"), omnibus_row()), (tr!("o-model"), omnibus_row())],
        categories: Vec::new(),
    };
    let summary = Table {
        title: tr!("o-model-summary"),
        columns: [tr!("o-2-log-likelihood"), tr!("o-cox-snell-r-square"), tr!("o-nagelkerke-r-square")].map(String::from).to_vec(),
        rows: vec![("1".into(), vec![num(Some(-2.0 * ll), 3), num(Some(cox_snell), 3), num(Some(nagelkerke), 3)])],
        categories: Vec::new(),
    };

    // Classification at the cut value.
    let mut counts = [[0.0; 2]; 2];
    for (y, x, w) in &rows {
        let predicted = usize::from(probability(x, &beta) >= options.cut);
        let observed = usize::from(*y == 1.0);
        if let Some(cell) = counts.get_mut(observed).and_then(|r| r.get_mut(predicted)) {
            *cell += w;
        }
    }
    let correct = |row: [f64; 2], hit: usize| {
        let total = row[0] + row[1];
        num((total > 0.0).then(|| row.get(hit).copied().unwrap_or(0.0) / total * 100.0), 1)
    };
    let overall = (counts[0][0] + counts[1][1]) / n * 100.0;
    let classification = Table {
        title: tr!("o-classification-table", cut = format!("{}", options.cut)),
        columns: vec![
            tr!("o-observed"),
            tr!("o-predicted-value", value = value_text(yvar, &Value::Num(low))),
            tr!("o-predicted-value", value = value_text(yvar, &Value::Num(high))),
            tr!("o-percentage-correct"),
        ],
        rows: vec![
            (
                yvar.display_name().to_string(),
                vec![Cell::Text(value_text(yvar, &Value::Num(low))), n_cell(counts[0][0]), n_cell(counts[0][1]), correct(counts[0], 0)],
            ),
            (String::new(), vec![Cell::Text(value_text(yvar, &Value::Num(high))), n_cell(counts[1][0]), n_cell(counts[1][1]), correct(counts[1], 1)]),
            (tr!("o-overall-percentage"), vec![Cell::Empty, Cell::Empty, Cell::Empty, num(Some(overall), 1)]),
        ],
        categories: Vec::new(),
    };

    let z = options.ci.and_then(|level| Normal::new(0.0, 1.0).ok().map(|d| d.inverse_cdf(0.5 + level / 2.0)));
    let mut names: Vec<String> = xs.iter().map(|(_, v)| v.display_name().to_string()).collect();
    names.push(tr!("o-constant"));
    // The constant is listed last, as SPSS does.
    let order: Vec<usize> = (1..k).chain(std::iter::once(0)).collect();
    let coefficients = order
        .iter()
        .zip(&names)
        .map(|(&j, name)| {
            let b = beta.get(j).copied().unwrap_or(f64::NAN);
            let se = covariance.get(j).and_then(|r| r.get(j)).map(|v| v.sqrt());
            let wald = se.map(|se| (b / se).powi(2));
            let mut cells = vec![
                num(Some(b), 3),
                num(se, 3),
                num(wald, 3),
                Cell::Int(1),
                num(wald.and_then(|w| dist::chi_square_sig(w, 1.0)), 3),
                num(Some(b.exp()), 3),
            ];
            if let Some(z) = z {
                cells.push(num(se.map(|se| (b - z * se).exp()), 3));
                cells.push(num(se.map(|se| (b + z * se).exp()), 3));
            }
            (name.clone(), cells)
        })
        .collect();
    let mut columns = [tr!("o-b"), tr!("o-s-e"), tr!("o-wald"), tr!("o-df"), tr!("o-sig"), tr!("o-exp-b")].map(String::from).to_vec();
    if let Some(level) = options.ci {
        columns.extend([
            tr!("o-ci-exp-b-lower", level = format!("{:.0}", level * 100.0)),
            tr!("o-ci-exp-b-upper", level = format!("{:.0}", level * 100.0)),
        ]);
    }
    let note = if converged { tr!("o-logistic-converged", n = iterations) } else { tr!("o-logistic-stopped", n = iterations) };
    let mut items = vec![
        Item::Table(encoding),
        Item::Table(omnibus),
        Item::Table(summary),
        Item::Text(note),
        Item::Table(classification),
        Item::Table(Table { title: tr!("o-variables-in-the-equation"), columns, rows: coefficients, categories: Vec::new() }),
    ];
    if let Some(separation) = separation {
        items.insert(0, Item::Warning(separation_text(separation)));
    } else if !converged {
        items.insert(0, Item::Warning(tr!("o-logistic-no-convergence")));
    }
    Ok(items)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mksigma_core::Variable;

    fn value(c: &Cell) -> f64 {
        match c {
            Cell::Num { value, .. } => *value,
            Cell::Int(n) => *n as f64,
            other => panic!("not a number: {other:?}"),
        }
    }

    #[test]
    fn a_single_binary_covariate_matches_the_odds_ratio() {
        // 2×2: x = 0 → 3 of 10 have y = 1; x = 1 → 7 of 10. The MLE odds ratio is (7/3)/(3/7).
        let mut ds = Dataset::new();
        ds.add_variable(Variable::numeric("y")).unwrap();
        ds.add_variable(Variable::numeric("x")).unwrap();
        for (x, ones) in [(0.0, 3), (1.0, 7)] {
            for i in 0..10 {
                ds.push_case(vec![Value::Num(if i < ones { 1.0 } else { 0.0 }), Value::Num(x)]).unwrap();
            }
        }
        let items = logistic(&ds, &Options { dependent: "y".into(), covariates: vec!["x".into()], ci: Some(0.95), ..Options::default() }).unwrap();
        let Some(Item::Table(vars)) = items.last() else { panic!() };
        let odds = (7.0 / 3.0) / (3.0 / 7.0);
        assert!((value(&vars.rows[0].1[0]) - f64::ln(odds)).abs() < 1e-6);
        assert!((value(&vars.rows[0].1[5]) - odds).abs() < 1e-5);
        // SE of a log odds ratio: √(1/3 + 1/7 + 1/7 + 1/3).
        let se = (2.0 / 3.0 + 2.0 / 7.0f64).sqrt();
        assert!((value(&vars.rows[0].1[1]) - se).abs() < 1e-6);
        // Constant: log odds at x = 0, ln(3/7).
        assert!((value(&vars.rows[1].1[0]) - (3.0f64 / 7.0).ln()).abs() < 1e-6);
        let Item::Table(class) = &items[4] else { panic!() };
        assert_eq!(value(&class.rows[2].1[3]), 70.0, "14 of 20 classified correctly");
    }

    #[test]
    fn a_dependent_with_three_values_is_refused() {
        let mut ds = Dataset::new();
        ds.add_variable(Variable::numeric("y")).unwrap();
        ds.add_variable(Variable::numeric("x")).unwrap();
        for i in 0..6 {
            ds.push_case(vec![Value::Num(f64::from(i % 3)), Value::Num(f64::from(i))]).unwrap();
        }
        let items = logistic(&ds, &Options { dependent: "y".into(), covariates: vec!["x".into()], ..Options::default() }).unwrap();
        assert!(matches!(&items[0], Item::Warning(s) if s.contains("exactly two values")));
    }

    fn data(rows: &[(f64, f64)]) -> Dataset {
        let mut ds = Dataset::new();
        ds.add_variable(Variable::numeric("y")).unwrap();
        ds.add_variable(Variable::numeric("x")).unwrap();
        for (y, x) in rows {
            ds.push_case(vec![Value::Num(*y), Value::Num(*x)]).unwrap();
        }
        ds
    }

    fn warnings(items: &[Item]) -> Vec<&str> {
        items.iter().filter_map(|i| if let Item::Warning(s) = i { Some(s.as_str()) } else { None }).collect()
    }

    fn texts(items: &[Item]) -> Vec<&str> {
        items.iter().filter_map(|i| if let Item::Text(s) = i { Some(s.as_str()) } else { None }).collect()
    }

    #[test]
    fn complete_separation_is_reported_not_converged() {
        // x = 1..6 with y = 0 0 0 1 1 1: x > 3.5 predicts y exactly, the slope has no maximum.
        // With ITERATE(30) it used to "converge" at iteration 26 with B = 48.6 (#29).
        let ds = data(&[(0.0, 1.0), (0.0, 2.0), (0.0, 3.0), (1.0, 4.0), (1.0, 5.0), (1.0, 6.0)]);
        for iterations in [20, 30, 1000] {
            let options = Options { dependent: "y".into(), covariates: vec!["x".into()], iterations, ..Options::default() };
            let items = logistic(&ds, &options).unwrap();
            assert_eq!(warnings(&items), [tr!("o-logistic-complete-separation")], "ITERATE({iterations}): {items:?}");
            assert!(texts(&items).iter().all(|t| !t.contains("changed by less than")), "ITERATE({iterations}): {items:?}");
            let explained = crate::explain::logistic(&ds, &options, false).unwrap();
            assert!(format!("{:?}", explained.sections).contains(&tr!("o-logistic-complete-separation")));
        }
    }

    #[test]
    fn quasi_complete_separation_is_reported() {
        // x ≤ 3 are all 0 and x ≥ 5 all 1; only at x = 4 are both values seen.
        let ds = data(&[(0.0, 1.0), (0.0, 2.0), (0.0, 3.0), (0.0, 4.0), (1.0, 4.0), (1.0, 5.0), (1.0, 6.0), (1.0, 7.0)]);
        let items = logistic(&ds, &Options { dependent: "y".into(), covariates: vec!["x".into()], ..Options::default() }).unwrap();
        assert_eq!(warnings(&items), [tr!("o-logistic-quasi-separation")], "{items:?}");
    }

    #[test]
    fn overlapping_values_still_converge() {
        let ds = data(&[(0.0, 1.0), (1.0, 2.0), (0.0, 3.0), (1.0, 4.0), (0.0, 5.0), (1.0, 6.0), (1.0, 7.0), (0.0, 2.5)]);
        let items = logistic(&ds, &Options { dependent: "y".into(), covariates: vec!["x".into()], ..Options::default() }).unwrap();
        assert!(warnings(&items).is_empty(), "{items:?}");
        assert!(texts(&items).iter().any(|t| t.contains("changed by less than")), "{items:?}");
    }
}
