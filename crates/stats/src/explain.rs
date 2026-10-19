//! The calculations behind a table, by hand: each statistic's formula, then the same formula
//! with the data's numbers in it, every term and every sum, down to the figure the table shows.
//! Built from the same cases, weights and missing values as the table, so a reader can check
//! every number with a calculator. Tests hold each final figure against the table's.
//!
//! With many cases the terms of a sum show as the first five, `⋯` and the last two; `full`
//! writes every one.

use mksigma_core::Dataset;
use mksigma_output::math::build::*;
use mksigma_output::math::{Explanation, Math, Step};

use crate::frequencies::{Order, count};
use crate::groups::{numeric, valid};
use crate::{StatsError, dist, value_text};

/// Terms shown in full up to this many; beyond, the first five and the last two.
const SHOWN: usize = 12;
/// Decimals for intermediate figures.
const PLACES: usize = 4;

fn step(text: &str, math: Math) -> Step {
    Step { text: text.to_string(), math: Some(math) }
}

fn say(text: &str) -> Step {
    Step { text: text.to_string(), math: None }
}

/// `t₁ + t₂ + ⋯ + tₙ`, all of them when `full` or few.
fn terms(items: Vec<Math>, full: bool) -> Math {
    if full || items.len() <= SHOWN {
        return plus(items);
    }
    let n = items.len();
    let mut shown: Vec<Math> = items.iter().take(5).cloned().collect();
    shown.push(text("⋯"));
    shown.extend(items.iter().skip(n - 2).cloned());
    plus(shown)
}

fn n(x: f64) -> Math {
    num(x, PLACES)
}

/// A value as it would be written: negatives in parentheses inside an expression.
fn val(x: f64) -> Math {
    if x < 0.0 { paren(n(x)) } else { n(x) }
}

/// `(v − m)²`.
fn dev_sq(v: f64, m: f64) -> Math {
    sup(paren(row(vec![val(v), text("−"), n(m)])), text("2"))
}

/// The valid (value, weight) pairs of a numeric variable, and how many cases were left out.
fn values(ds: &Dataset, index: usize) -> (Vec<(f64, f64)>, usize) {
    let mut out = Vec::new();
    let mut left = 0;
    for case in 0..ds.n_cases() {
        let w = ds.case_weight(case);
        match valid(ds, index, case) {
            Some(x) if w > 0.0 => out.push((x, w)),
            _ => left += 1,
        }
    }
    (out, left)
}

fn weighted(ds: &Dataset) -> bool {
    ds.weight.is_some()
}

/// `DESCRIPTIVES`: N, minimum, maximum, mean and standard deviation of each variable.
pub fn descriptives(ds: &Dataset, variables: &[String], full: bool) -> Result<Explanation, StatsError> {
    let mut sections = Vec::new();
    let w = weighted(ds);
    for name in variables {
        let (index, variable) = numeric(ds, name)?;
        let (xs, left) = values(ds, index);
        let mut steps = Vec::new();
        let total: f64 = xs.iter().map(|(_, w)| w).sum();
        let note =
            if left > 0 { format!(" ({left} cases are left out: their value is missing or their weight isn't positive)") } else { String::new() };
        if w {
            steps.push(step(
                &format!("The cases are weighted: N is the sum of the weights{note}."),
                equals(vec![var("N"), sum(None, None, indexed("w", "i")), n(total)]),
            ));
        } else {
            steps.push(step(&format!("N counts the cases with a valid value{note}."), equals(vec![var("N"), n(total)])));
        }
        if xs.is_empty() {
            steps.push(say("With no valid values there is nothing more to compute."));
            sections.push((variable.display_name().to_string(), steps));
            continue;
        }
        let xi = || indexed("x", "i");
        let range = || (Some(equals(vec![var("i"), text("1")])), Some(var("n")));
        // The sum.
        let s: f64 = xs.iter().map(|(x, w)| x * w).sum();
        let each: Vec<Math> = xs.iter().map(|(x, wi)| if w { row(vec![n(*wi), text("·"), val(*x)]) } else { val(*x) }).collect();
        let body = if w { row(vec![indexed("w", "i"), xi()]) } else { xi() };
        let (from, to) = range();
        steps.push(step("The sum of the values:", equals(vec![sum(from, to, body.clone()), terms(each, full), n(s)])));
        // The mean.
        let mean = s / total;
        let (from, to) = range();
        steps.push(step(
            "The mean divides the sum by N:",
            equals(vec![bar(var("x")), frac(sum(from, to, body), var("N")), frac(n(s), n(total)), n(mean)]),
        ));
        // Squared deviations.
        let ss: f64 = xs.iter().map(|(x, wi)| wi * (x - mean).powi(2)).sum();
        let devs: Vec<Math> = xs.iter().map(|(x, wi)| if w { row(vec![n(*wi), text("·"), dev_sq(*x, mean)]) } else { dev_sq(*x, mean) }).collect();
        let squares: Vec<Math> = xs.iter().map(|(x, wi)| n(wi * (x - mean).powi(2))).collect();
        let dev_body = sup(paren(row(vec![xi(), text("−"), bar(var("x"))])), text("2"));
        let dev_body = if w { row(vec![indexed("w", "i"), dev_body]) } else { dev_body };
        let (from, to) = range();
        steps.push(step(
            "The squared deviations from the mean, added up:",
            equals(vec![sum(from, to, dev_body), terms(devs, full), terms(squares, full), n(ss)]),
        ));
        if total <= 1.0 {
            steps.push(say("With N of 1 or less the variance and standard deviation are undefined."));
        } else {
            let variance = ss / (total - 1.0);
            steps.push(step(
                "The variance divides that by N − 1:",
                equals(vec![sup(var("s"), text("2")), frac(n(ss), row(vec![n(total), text("−"), text("1")])), n(variance)]),
            ));
            let sd = variance.sqrt();
            steps.push(step(
                "The standard deviation is its square root:",
                equals(vec![var("s"), sqrt(n(variance)), row(vec![n(sd), text("≈"), num(sd, 2)])]),
            ));
        }
        let (lo, hi) = xs.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), (x, _)| (a.min(*x), b.max(*x)));
        steps.push(step("The minimum and maximum are the smallest and largest values:", row(vec![text("min = "), n(lo), text(",   max = "), n(hi)])));
        sections.push((variable.display_name().to_string(), steps));
    }
    Ok(Explanation { title: "Descriptive Statistics".into(), sections })
}

/// `CORRELATIONS`: Pearson's r and its significance for each pair of different variables.
pub fn correlations(
    ds: &Dataset,
    variables: &[String],
    with: &[String],
    listwise: bool,
    one_tailed: bool,
    full: bool,
) -> Result<Explanation, StatsError> {
    let rows = variables.iter().map(|v| numeric(ds, v)).collect::<Result<Vec<_>, _>>()?;
    let columns = if with.is_empty() { rows.clone() } else { with.iter().map(|v| numeric(ds, v)).collect::<Result<Vec<_>, _>>()? };
    let all: Vec<usize> = rows.iter().chain(&columns).map(|(i, _)| *i).collect();
    let usable = |case: usize| ds.case_weight(case) > 0.0 && (!listwise || all.iter().all(|&v| valid(ds, v, case).is_some()));
    let w = weighted(ds);
    let mut sections = Vec::new();
    let mut done: Vec<(usize, usize)> = Vec::new();
    for (ri, rvar) in &rows {
        for (ci, cvar) in &columns {
            // Each pair once, and not a variable with itself.
            if ri == ci || done.contains(&(*ci, *ri)) {
                continue;
            }
            done.push((*ri, *ci));
            let pairs: Vec<(f64, f64, f64)> =
                (0..ds.n_cases()).filter(|&c| usable(c)).filter_map(|c| Some((valid(ds, *ri, c)?, valid(ds, *ci, c)?, ds.case_weight(c)))).collect();
            let mut steps = Vec::new();
            let total: f64 = pairs.iter().map(|(_, _, w)| w).sum();
            let how = if listwise { "valid on every variable listed (listwise)" } else { "valid on both (pairwise)" };
            steps.push(step(&format!("The cases {how}{}:", if w { ", weighted" } else { "" }), equals(vec![var("N"), n(total)])));
            if total <= 0.0 {
                steps.push(say("With no cases there is no correlation."));
                sections.push((format!("{} with {}", rvar.display_name(), cvar.display_name()), steps));
                continue;
            }
            let mx = pairs.iter().map(|(x, _, w)| x * w).sum::<f64>() / total;
            let my = pairs.iter().map(|(_, y, w)| y * w).sum::<f64>() / total;
            steps
                .push(step("The means of x and y:", row(vec![equals(vec![bar(var("x")), n(mx)]), text(",   "), equals(vec![bar(var("y")), n(my)])])));
            let xi = || indexed("x", "i");
            let yi = || indexed("y", "i");
            let weight = |m: Math| if w { row(vec![indexed("w", "i"), m]) } else { m };
            let dx = || paren(row(vec![xi(), text("−"), bar(var("x"))]));
            let dy = || paren(row(vec![yi(), text("−"), bar(var("y"))]));
            let sxy: f64 = pairs.iter().map(|(x, y, w)| w * (x - mx) * (y - my)).sum();
            let sxx: f64 = pairs.iter().map(|(x, _, w)| w * (x - mx).powi(2)).sum();
            let syy: f64 = pairs.iter().map(|(_, y, w)| w * (y - my).powi(2)).sum();
            let cross: Vec<Math> = pairs
                .iter()
                .map(|(x, y, _)| row(vec![paren(row(vec![val(*x), text("−"), n(mx)])), paren(row(vec![val(*y), text("−"), n(my)]))]))
                .collect();
            steps.push(step(
                "The sum of the products of the deviations:",
                equals(vec![sub(var("S"), var("xy")), sum(None, None, weight(row(vec![dx(), dy()]))), terms(cross, full), n(sxy)]),
            ));
            let xs: Vec<Math> = pairs.iter().map(|(x, _, _)| dev_sq(*x, mx)).collect();
            steps.push(step(
                "The sum of the squared deviations of x:",
                equals(vec![sub(var("S"), var("xx")), sum(None, None, weight(sup(dx(), text("2")))), terms(xs, full), n(sxx)]),
            ));
            let ys: Vec<Math> = pairs.iter().map(|(_, y, _)| dev_sq(*y, my)).collect();
            steps.push(step(
                "And of y:",
                equals(vec![sub(var("S"), var("yy")), sum(None, None, weight(sup(dy(), text("2")))), terms(ys, full), n(syy)]),
            ));
            let r = sxy / (sxx * syy).sqrt();
            if !r.is_finite() {
                steps.push(say("One of the variables doesn't vary here, so r is undefined."));
                sections.push((format!("{} with {}", rvar.display_name(), cvar.display_name()), steps));
                continue;
            }
            let r = r.clamp(-1.0, 1.0);
            steps.push(step(
                "Pearson's r:",
                equals(vec![
                    var("r"),
                    frac(sub(var("S"), var("xy")), sqrt(row(vec![sub(var("S"), var("xx")), text("·"), sub(var("S"), var("yy"))]))),
                    frac(n(sxy), sqrt(row(vec![n(sxx), text("·"), n(syy)]))),
                    row(vec![n(r), text("≈"), num(r, 3)]),
                ]),
            ));
            if total > 2.0 && r.abs() < 1.0 {
                let df = total - 2.0;
                let t = r * (df / (1.0 - r * r)).sqrt();
                steps.push(step(
                    "Its t statistic, with N − 2 degrees of freedom:",
                    equals(vec![
                        var("t"),
                        frac(
                            row(vec![var("r"), sqrt(row(vec![var("N"), text("−"), text("2")]))]),
                            sqrt(row(vec![text("1"), text("−"), sup(var("r"), text("2"))])),
                        ),
                        frac(row(vec![n(r), sqrt(n(df))]), sqrt(row(vec![text("1"), text("−"), sup(val(r), text("2"))]))),
                        n(t),
                    ]),
                ));
                if let Some(p) = dist::t_sig(t, df) {
                    let (p, tails) = if one_tailed { (p / 2.0, "one-tailed") } else { (p, "two-tailed") };
                    steps.push(step(
                        &format!("The significance ({tails}) is the probability of a t at least this far from 0 in Student's t distribution with {} degrees of freedom: the one figure here that comes from the distribution's function, not from arithmetic.", number_text(df)),
                        row(vec![var("p"), text("="), n(p), text("≈"), num(p, 3)]),
                    ));
                }
            }
            sections.push((format!("{} with {}", rvar.display_name(), cvar.display_name()), steps));
        }
    }
    Ok(Explanation { title: "Correlations".into(), sections })
}

fn number_text(x: f64) -> String {
    mksigma_output::math::number(x, PLACES)
}

/// `FREQUENCIES`: each value's percent, valid percent and cumulative percent.
pub fn frequencies(ds: &Dataset, variables: &[String], full: bool) -> Result<Explanation, StatsError> {
    let weights: Vec<f64> = (0..ds.n_cases()).map(|c| ds.case_weight(c)).collect();
    let mut sections = Vec::new();
    for name in variables {
        let (index, variable) = ds.variable(name).ok_or_else(|| StatsError::UnknownVariable(name.clone()))?;
        let counts = count(variable, ds.column(index).unwrap_or_default(), &weights, Order::AscendingValue);
        let (valid_n, missing) = (counts.valid_total(), counts.missing_total());
        let total = valid_n + missing;
        let mut steps = Vec::new();
        steps.push(step(
            "Every case counts in the total; the valid ones are those whose value isn't missing:",
            row(vec![
                equals(vec![var("N"), row(vec![n(valid_n), text("+"), n(missing)]), n(total)]),
                text(",   "),
                equals(vec![sub(var("N"), text("valid")), n(valid_n)]),
            ]),
        ));
        let mut cumulative = 0.0;
        let shown = if full { counts.valid.len() } else { counts.valid.len().min(SHOWN * 2) };
        for (value, f) in counts.valid.iter().take(shown) {
            cumulative += f;
            let label = value_text(variable, value);
            let frac100 = |a: Math, b: f64, r: f64| equals(vec![row(vec![frac(a, n(b)), text("×"), text("100")]), num(r, 1)]);
            steps.push(step(
                &format!("{label}: f = {}", number_text(*f)),
                row(vec![
                    text("Percent:  "),
                    frac100(n(*f), total, f / total * 100.0),
                    text(",   Valid:  "),
                    frac100(n(*f), valid_n, f / valid_n * 100.0),
                    text(",   Cumulative:  "),
                    frac100(n(cumulative), valid_n, cumulative / valid_n * 100.0),
                ]),
            ));
        }
        if shown < counts.valid.len() {
            steps.push(say(&format!("And {} more values, the same way (Show every term lists them).", counts.valid.len() - shown)));
        }
        sections.push((variable.display_name().to_string(), steps));
    }
    Ok(Explanation { title: "Frequencies".into(), sections })
}

#[cfg(test)]
mod tests {
    use super::*;
    use mksigma_core::{Value, Variable};
    use mksigma_output::math::number;
    use mksigma_output::{Cell, Item};

    fn sample() -> Dataset {
        let mut ds = Dataset::new();
        for name in ["age", "income", "score"] {
            ds.add_variable(Variable::numeric(name)).unwrap();
        }
        for (a, i, s) in [
            (23.0, 1800.0, 61.0),
            (35.0, 4200.0, 74.0),
            (41.0, 5100.0, 69.0),
            (29.0, 2600.0, 80.0),
            (52.0, 6900.0, 58.0),
            (47.0, 3900.0, 72.0),
            (31.0, 3100.0, 88.0),
            (26.0, 2200.0, 77.0),
        ] {
            ds.push_case(vec![Value::Num(a), Value::Num(i), Value::Num(s)]).unwrap();
        }
        ds
    }

    fn latex(e: &Explanation) -> String {
        e.sections.iter().flat_map(|(_, steps)| steps.iter().filter_map(|s| s.math.as_ref().map(Math::to_latex))).collect::<Vec<_>>().join("\n")
    }

    #[test]
    fn descriptives_by_hand_agree_with_the_table() {
        let ds = sample();
        let names: Vec<String> = ["age", "income", "score"].map(String::from).to_vec();
        let e = descriptives(&ds, &names, false).unwrap();
        let table = crate::descriptives(&ds, &names).unwrap();
        for (k, (_, steps)) in e.sections.iter().enumerate() {
            let tex: Vec<String> = steps.iter().filter_map(|s| s.math.as_ref().map(Math::to_latex)).collect();
            // The table's mean and standard deviation, as the hand calculation ends.
            let (_, cells) = &table.rows[k];
            let (Cell::Num { value: mean, .. }, Cell::Num { value: sd, .. }) = (&cells[3], &cells[4]) else { panic!() };
            assert!(tex.iter().any(|t| t.ends_with(&format!("= {}", number(*mean, 4)))), "mean {mean} in {tex:?}");
            assert!(tex.iter().any(|t| t.ends_with(&format!("\\approx {}", number(*sd, 2)))), "sd {sd} in {tex:?}");
        }
        // Ages 23 35 41 29 52 47 31 26: sum 284, mean 35.5, squared deviations 744.
        let age = latex(&Explanation { title: String::new(), sections: vec![e.sections[0].clone()] });
        assert!(age.contains("= 284") && age.contains("\\frac{284}{8} = 35.5") && age.contains("= 744"), "{age}");
        assert!(age.contains("{\\left( 23 - 35.5 \\right)}^{2}"), "every term, with the numbers: {age}");
        assert!(age.contains("\\frac{744}{8 - 1} = 106.2857"), "{age}");
    }

    #[test]
    fn many_terms_are_shortened_unless_asked() {
        let mut ds = Dataset::new();
        ds.add_variable(Variable::numeric("x")).unwrap();
        for k in 0..40 {
            ds.push_case(vec![Value::Num(f64::from(k))]).unwrap();
        }
        let short = latex(&descriptives(&ds, &["x".into()], false).unwrap());
        let long = latex(&descriptives(&ds, &["x".into()], true).unwrap());
        assert!(short.contains("\\cdots") && !long.contains("\\cdots"));
        assert!(long.len() > short.len() * 2);
        // 0 + 1 + ... + 39 = 780.
        assert!(short.contains("= 780"));
    }

    #[test]
    fn correlations_by_hand_agree_with_the_table() {
        let ds = sample();
        let names: Vec<String> = ["age", "income"].map(String::from).to_vec();
        let e = correlations(&ds, &names, &[], false, false, false).unwrap();
        assert_eq!(e.sections.len(), 1, "each pair once");
        let items = crate::correlations(&ds, &crate::CorrelationsOptions { variables: names.clone(), ..Default::default() }).unwrap();
        let Item::Table(t) = &items[0] else { panic!() };
        // The r of age with income: row 0 (Pearson), column 2 (column 0 names the statistic).
        let r = match &t.rows[0].1[2] {
            Cell::Num { value, .. } => *value,
            Cell::Text(s) => s.trim_end_matches('*').parse().unwrap(),
            other => panic!("{other:?}"),
        };
        let p = match &t.rows[1].1[2] {
            Cell::Num { value, .. } => *value,
            other => panic!("{other:?}"),
        };
        let tex = latex(&e);
        assert!(tex.contains(&format!("\\approx {}", number(r, 3))), "r {r}: {tex}");
        assert!(tex.contains(&format!("\\approx {}", number(p, 3))), "p {p}: {tex}");
    }

    #[test]
    fn frequencies_percentages() {
        let mut ds = Dataset::new();
        let mut g = Variable::numeric("g");
        g.missing = mksigma_core::MissingValues::Discrete(vec![Value::Num(9.0)]);
        ds.add_variable(g).unwrap();
        for x in [1.0, 1.0, 2.0, 9.0] {
            ds.push_case(vec![Value::Num(x)]).unwrap();
        }
        let tex = latex(&frequencies(&ds, &["g".into()], false).unwrap());
        // Two 1s of four cases (50%), of three valid (66.7%).
        assert!(tex.contains("\\frac{2}{4} \\cdot 100 = 50") && tex.contains("\\frac{2}{3} \\cdot 100 = 66.7"), "{tex}");
        assert!(tex.contains("\\frac{3}{3} \\cdot 100 = 100"), "cumulative ends at 100: {tex}");
    }
}
