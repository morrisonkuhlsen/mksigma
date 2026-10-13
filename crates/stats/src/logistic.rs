//! `LOGISTIC REGRESSION` with `/METHOD=ENTER`: a binary dependent variable (its lower value
//! coded 0, the higher 1) on numeric covariates, fitted by maximum likelihood with
//! Newton–Raphson. Cases missing any variable are left out; weights count as frequencies.

use mksigma_core::{Dataset, Value};
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

fn probability(x: &[f64], beta: &[f64]) -> f64 {
    let eta: f64 = x.iter().zip(beta).map(|(a, b)| a * b).sum();
    1.0 / (1.0 + (-eta).exp())
}

/// Weighted log-likelihood of 0/1 outcomes.
fn log_likelihood(rows: &[(f64, Vec<f64>, f64)], beta: &[f64]) -> f64 {
    rows.iter()
        .map(|(y, x, w)| {
            let p = probability(x, beta).clamp(1e-300, 1.0 - 1e-16);
            w * (y * p.ln() + (1.0 - y) * (1.0 - p).ln())
        })
        .sum()
}

pub fn logistic(ds: &Dataset, options: &Options) -> Result<Vec<Item>, StatsError> {
    let (yi, yvar) = numeric(ds, &options.dependent)?;
    let xs = options.covariates.iter().map(|n| numeric(ds, n)).collect::<Result<Vec<_>, _>>()?;
    // Listwise; x gets a leading 1 for the constant.
    let raw: Vec<(f64, Vec<f64>, f64)> = (0..ds.n_cases())
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
    let [low, high] = levels[..] else {
        return Ok(vec![Item::Warning(format!(
            "LOGISTIC REGRESSION needs a dependent variable with exactly two values; {} has {}",
            yvar.name,
            levels.len()
        ))]);
    };
    let rows: Vec<(f64, Vec<f64>, f64)> = raw.into_iter().map(|(y, x, w)| (if y == high { 1.0 } else { 0.0 }, x, w)).collect();
    let n: f64 = rows.iter().map(|(_, _, w)| w).sum();
    let ones: f64 = rows.iter().map(|(y, _, w)| y * w).sum();
    let k = xs.len() + 1;

    // The constant-only model has a closed form.
    let p0 = ones / n;
    let ll0 = ones * p0.ln() + (n - ones) * (1.0 - p0).ln();
    let mut beta = vec![0.0; k];
    if let Some(b0) = beta.first_mut() {
        *b0 = (p0 / (1.0 - p0)).ln();
    }
    let mut ll = log_likelihood(&rows, &beta);
    let (mut iterations, mut converged, mut information) = (0, false, None);
    while iterations < options.iterations.max(1) {
        iterations += 1;
        let mut gradient = vec![0.0; k];
        let mut hessian = vec![vec![0.0; k]; k];
        for (y, x, w) in &rows {
            let p = probability(x, &beta);
            let v = w * p * (1.0 - p);
            for ((g, row), xa) in gradient.iter_mut().zip(hessian.iter_mut()).zip(x) {
                *g += w * (y - p) * xa;
                for (h, xb) in row.iter_mut().zip(x) {
                    *h += v * xa * xb;
                }
            }
        }
        let Some(inverse) = invert(&hessian) else { break };
        let step: Vec<f64> = inverse.iter().map(|row| row.iter().zip(&gradient).map(|(a, g)| a * g).sum()).collect();
        for (b, s) in beta.iter_mut().zip(&step) {
            *b += s;
        }
        let new_ll = log_likelihood(&rows, &beta);
        // SPSS's default: stop when no estimate changes by more than 0.1%.
        let small = step.iter().zip(&beta).all(|(s, b)| s.abs() <= 0.001 * b.abs().max(1e-8));
        let flat = (new_ll - ll).abs() <= 1e-10 * new_ll.abs().max(1.0);
        ll = new_ll;
        information = Some(inverse);
        if small || flat {
            converged = true;
            break;
        }
    }
    // Standard errors from the information at the final estimates.
    let mut hessian = vec![vec![0.0; k]; k];
    for (_, x, w) in &rows {
        let p = probability(x, &beta);
        for (row, xa) in hessian.iter_mut().zip(x) {
            for (h, xb) in row.iter_mut().zip(x) {
                *h += w * p * (1.0 - p) * xa * xb;
            }
        }
    }
    let covariance = invert(&hessian).or(information);
    let Some(covariance) = covariance else {
        return Ok(vec![Item::Warning("LOGISTIC REGRESSION: the covariates are collinear (or constant); no model".into())]);
    };

    let chi = 2.0 * (ll - ll0);
    let df = xs.len() as f64;
    let chi_sig = dist::chi_square_sig(chi, df);
    let cox_snell = 1.0 - (2.0 / n * (ll0 - ll)).exp();
    let nagelkerke = cox_snell / (1.0 - (2.0 * ll0 / n).exp());

    let encoding = Table {
        title: "Dependent Variable Encoding".into(),
        columns: vec!["Internal Value".into()],
        rows: vec![(value_text(yvar, &Value::Num(low)), vec![Cell::Int(0)]), (value_text(yvar, &Value::Num(high)), vec![Cell::Int(1)])],
    };
    let omnibus_row = || vec![num(Some(chi), 3), n_cell(df), num(chi_sig, 3)];
    let omnibus = Table {
        title: "Omnibus Tests of Model Coefficients".into(),
        columns: ["Chi-square", "df", "Sig."].map(String::from).to_vec(),
        rows: vec![("Step".into(), omnibus_row()), ("Block".into(), omnibus_row()), ("Model".into(), omnibus_row())],
    };
    let summary = Table {
        title: "Model Summary".into(),
        columns: ["-2 Log likelihood", "Cox & Snell R Square", "Nagelkerke R Square"].map(String::from).to_vec(),
        rows: vec![("1".into(), vec![num(Some(-2.0 * ll), 3), num(Some(cox_snell), 3), num(Some(nagelkerke), 3)])],
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
        title: format!("Classification Table (cut value {})", options.cut),
        columns: vec![
            "Observed".into(),
            format!("Predicted {}", value_text(yvar, &Value::Num(low))),
            format!("Predicted {}", value_text(yvar, &Value::Num(high))),
            "Percentage Correct".into(),
        ],
        rows: vec![
            (
                yvar.display_name().to_string(),
                vec![Cell::Text(value_text(yvar, &Value::Num(low))), n_cell(counts[0][0]), n_cell(counts[0][1]), correct(counts[0], 0)],
            ),
            (String::new(), vec![Cell::Text(value_text(yvar, &Value::Num(high))), n_cell(counts[1][0]), n_cell(counts[1][1]), correct(counts[1], 1)]),
            ("Overall Percentage".into(), vec![Cell::Empty, Cell::Empty, Cell::Empty, num(Some(overall), 1)]),
        ],
    };

    let z = options.ci.and_then(|level| Normal::new(0.0, 1.0).ok().map(|d| d.inverse_cdf(0.5 + level / 2.0)));
    let mut names: Vec<String> = xs.iter().map(|(_, v)| v.display_name().to_string()).collect();
    names.push("Constant".into());
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
    let mut columns = ["B", "S.E.", "Wald", "df", "Sig.", "Exp(B)"].map(String::from).to_vec();
    if let Some(level) = options.ci {
        columns.extend([format!("{:.0}% CI Exp(B) Lower", level * 100.0), format!("{:.0}% CI Exp(B) Upper", level * 100.0)]);
    }
    let note = if converged {
        format!("Estimation terminated at iteration number {iterations} because parameter estimates changed by less than .001.")
    } else {
        format!("Estimation stopped after {iterations} iterations without converging; the estimates are unreliable (complete separation?).")
    };
    let mut items = vec![
        Item::Table(encoding),
        Item::Table(omnibus),
        Item::Table(summary),
        Item::Text(note),
        Item::Table(classification),
        Item::Table(Table { title: "Variables in the Equation".into(), columns, rows: coefficients }),
    ];
    if !converged {
        items.insert(0, Item::Warning("LOGISTIC REGRESSION did not converge".into()));
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
}
