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
use crate::groups::{numeric, valid, valid_value};
use crate::{StatsError, TTest, dist, value_text};

/// Terms shown in full up to this many; beyond, the first five and the last two.
const SHOWN: usize = 12;
/// Decimals for intermediate figures.
const PLACES: usize = 4;

fn step(text: &str, math: Math) -> Step {
    Step { text: text.to_string(), math: Some(math) }
}

/// Several equations under one sentence, each on its own: a document typesets them better
/// apart than joined by commas.
fn several(text: &str, maths: Vec<Math>) -> Vec<Step> {
    maths.into_iter().enumerate().map(|(k, m)| Step { text: if k == 0 { text.to_string() } else { String::new() }, math: Some(m) }).collect()
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
        steps.extend(several(
            "The minimum and maximum are the smallest and largest values:",
            vec![equals(vec![text("min"), n(lo)]), equals(vec![text("max"), n(hi)])],
        ));
        sections.push((variable.display_name().to_string(), steps));
    }
    Ok(Explanation { title: "Descriptive Statistics".into(), sections })
}

/// Pearson's r of `(x, y, weight)` pairs, step by step, then its t and significance; `how`
/// says which cases they are.
fn pearson(pairs: &[(f64, f64, f64)], w: bool, how: &str, one_tailed: bool, full: bool) -> Vec<Step> {
    let mut steps = Vec::new();
    let total: f64 = pairs.iter().map(|(_, _, w)| w).sum();
    steps.push(step(&format!("The cases {how}{}:", if w { ", weighted" } else { "" }), equals(vec![var("N"), n(total)])));
    if total <= 0.0 {
        steps.push(say("With no cases there is no correlation."));
        return steps;
    }
    let mx = pairs.iter().map(|(x, _, w)| x * w).sum::<f64>() / total;
    let my = pairs.iter().map(|(_, y, w)| y * w).sum::<f64>() / total;
    steps.extend(several("The means of x and y:", vec![equals(vec![bar(var("x")), n(mx)]), equals(vec![bar(var("y")), n(my)])]));
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
        .map(|(x, y, wi)| {
            let product = row(vec![paren(row(vec![val(*x), text("−"), n(mx)])), paren(row(vec![val(*y), text("−"), n(my)]))]);
            if w { row(vec![n(*wi), text("·"), product]) } else { product }
        })
        .collect();
    steps.push(step(
        "The sum of the products of the deviations:",
        equals(vec![sub(var("S"), var("xy")), sum(None, None, weight(row(vec![dx(), dy()]))), terms(cross, full), n(sxy)]),
    ));
    let xs: Vec<Math> = pairs.iter().map(|(x, _, wi)| weighted_term(w, *wi, dev_sq(*x, mx))).collect();
    steps.push(step(
        "The sum of the squared deviations of x:",
        equals(vec![sub(var("S"), var("xx")), sum(None, None, weight(sup(dx(), text("2")))), terms(xs, full), n(sxx)]),
    ));
    let ys: Vec<Math> = pairs.iter().map(|(_, y, wi)| weighted_term(w, *wi, dev_sq(*y, my))).collect();
    steps.push(step("And of y:", equals(vec![sub(var("S"), var("yy")), sum(None, None, weight(sup(dy(), text("2")))), terms(ys, full), n(syy)])));
    let r = sxy / (sxx * syy).sqrt();
    if !r.is_finite() {
        steps.push(say("One of the variables doesn't vary here, so r is undefined."));
        return steps;
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
    steps
}

/// A term of a weighted sum: `w · term`, or the term alone without weights.
fn weighted_term(w: bool, weight: f64, term: Math) -> Math {
    if w { row(vec![n(weight), text("·"), term]) } else { term }
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
            let how = if listwise { "valid on every variable listed (listwise)" } else { "valid on both (pairwise)" };
            let steps = pearson(&pairs, w, how, one_tailed, full);
            sections.push((format!("{} with {}", rvar.display_name(), cvar.display_name()), steps));
        }
    }
    Ok(Explanation { title: "Correlations".into(), sections })
}

/// The symbols of one sample's statistics: its size, its values, their mean and standard
/// deviation (`n₁`, `x₁ᵢ`, `x̄₁`, `s₁`).
struct Names {
    n: Math,
    item: Math,
    mean: Math,
    s: Math,
}

impl Names {
    /// For `x`, or for group `k` of `x` (`x₁`) when `k` isn't empty.
    fn of(letter: &str, k: &str) -> Self {
        let tag = |m: Math| if k.is_empty() { m } else { sub(m, text(k)) };
        let item = if k.is_empty() { indexed(letter, "i") } else { sub(var(letter), row(vec![text(k), var("i")])) };
        Names { n: tag(var("n")), item, mean: tag(bar(var(letter))), s: tag(var("s")) }
    }
}

/// One sample's size, mean and variance.
struct Sample {
    n: f64,
    mean: f64,
    /// `None` with a size of 1 or less.
    variance: Option<f64>,
}

/// A sample's size, mean, variance and standard deviation, step by step. `None` when it has no
/// valid values.
fn sample(steps: &mut Vec<Step>, xs: &[(f64, f64)], names: &Names, w: bool, full: bool) -> Option<Sample> {
    let total: f64 = xs.iter().map(|(_, w)| w).sum();
    if w {
        steps.push(step("Its size is the sum of the weights:", equals(vec![names.n.clone(), sum(None, None, indexed("w", "i")), n(total)])));
    } else {
        steps.push(step("Its size counts the cases with a valid value:", equals(vec![names.n.clone(), n(total)])));
    }
    if total <= 0.0 {
        steps.push(say("With no valid values there is nothing more to compute."));
        return None;
    }
    let s: f64 = xs.iter().map(|(x, w)| x * w).sum();
    let body = if w { row(vec![indexed("w", "i"), names.item.clone()]) } else { names.item.clone() };
    let each: Vec<Math> = xs.iter().map(|(x, wi)| weighted_term(w, *wi, val(*x))).collect();
    steps.push(step("The sum of the values:", equals(vec![sum(None, None, body.clone()), terms(each, full), n(s)])));
    let mean = s / total;
    steps.push(step("The mean:", equals(vec![names.mean.clone(), frac(sum(None, None, body), names.n.clone()), frac(n(s), n(total)), approx(mean)])));
    let ss: f64 = xs.iter().map(|(x, wi)| wi * (x - mean).powi(2)).sum();
    let devs: Vec<Math> = xs.iter().map(|(x, wi)| weighted_term(w, *wi, dev_sq(*x, mean))).collect();
    let squares: Vec<Math> = xs.iter().map(|(x, wi)| n(wi * (x - mean).powi(2))).collect();
    let dev_body = sup(paren(row(vec![names.item.clone(), text("−"), names.mean.clone()])), text("2"));
    let dev_body = if w { row(vec![indexed("w", "i"), dev_body]) } else { dev_body };
    steps.push(step(
        "The squared deviations from the mean, added up:",
        equals(vec![sum(None, None, dev_body), terms(devs, full), terms(squares, full), n(ss)]),
    ));
    if total <= 1.0 {
        steps.push(say("With a size of 1 or less the variance is undefined."));
        return Some(Sample { n: total, mean, variance: None });
    }
    let variance = ss / (total - 1.0);
    steps.extend(several(
        "The variance divides that by the size minus 1, and the standard deviation is its square root:",
        vec![
            equals(vec![sup(names.s.clone(), text("2")), frac(n(ss), row(vec![n(total), text("−"), text("1")])), n(variance)]),
            equals(vec![names.s.clone(), sqrt(n(variance)), approx(variance.sqrt())]),
        ],
    ));
    Some(Sample { n: total, mean, variance: Some(variance) })
}

/// `x` to four decimals, then as the tables show it, to three (when that differs).
fn approx(x: f64) -> Math {
    if number_text(x) == mksigma_output::math::number(x, 3) { n(x) } else { row(vec![n(x), text("≈"), num(x, 3)]) }
}

/// The two-tailed significance of `t` with `df` degrees of freedom.
fn t_significance(t: f64, df: f64) -> Option<Step> {
    let p = dist::t_sig(t, df)?;
    Some(step(
        &format!(
            "The significance (two-tailed) is the probability of a t at least this far from 0 in Student's t distribution with {} degrees of freedom: a figure from the distribution's function, not from arithmetic.",
            number_text(df)
        ),
        equals(vec![var("p"), approx(p)]),
    ))
}

/// t from a difference and its standard error, its degrees of freedom, significance and the
/// confidence interval of the difference.
fn t_and_interval(steps: &mut Vec<Step>, difference: (Math, f64), se: (Math, f64), df: (Math, f64), ci: f64) {
    let ((d_name, d), (se_name, se), (df_name, df)) = (difference, se, df);
    let t = d / se;
    steps.push(step("t divides the difference by its standard error:", equals(vec![var("t"), frac(d_name, se_name), frac(n(d), n(se)), approx(t)])));
    let whole = df.fract() == 0.0;
    steps.push(step("Its degrees of freedom:", equals(vec![var("df"), df_name, if whole { n(df) } else { approx(df) }])));
    steps.extend(t_significance(t, df));
    if let Some(critical) = dist::t_quantile(0.5 + ci / 2.0, df) {
        let level = number_text(ci * 100.0);
        let tail = number_text((1.0 - ci) / 2.0 * 100.0);
        steps.push(step(
            &format!("For the {level}% confidence interval, the t that leaves {tail}% in each tail, from the same distribution:"),
            equals(vec![sub(var("t"), num(0.5 + ci / 2.0, 4)), approx(critical)]),
        ));
        let half = critical * se;
        steps.extend(several(
            "The interval is the difference plus or minus that t times the standard error:",
            vec![
                equals(vec![row(vec![n(d), text("±"), n(critical), text("·"), n(se)]), row(vec![n(d), text("±"), n(half)])]),
                equals(vec![text("Lower"), row(vec![n(d), text("−"), n(half)]), approx(d - half)]),
                equals(vec![text("Upper"), row(vec![n(d), text("+"), n(half)]), approx(d + half)]),
            ],
        ));
    }
}

/// `T-TEST`: one sample against its test value, two independent groups (Levene's test, then
/// the pooled and the Welch versions) or paired samples, with intervals at confidence `ci`.
pub fn t_test(ds: &Dataset, test: &TTest, ci: f64, full: bool) -> Result<Explanation, StatsError> {
    let w = weighted(ds);
    let mut sections = Vec::new();
    match test {
        TTest::OneSample { variables, test_value } => {
            for name in variables {
                let (i, variable) = numeric(ds, name)?;
                let (xs, _) = values(ds, i);
                let mut steps = Vec::new();
                let names = Names::of("x", "");
                if let Some(Sample { n: size, mean, variance: Some(variance) }) = sample(&mut steps, &xs, &names, w, full) {
                    let diff = mean - test_value;
                    let mu = sub(var("μ"), text("0"));
                    steps.push(step(
                        "The difference from the test value:",
                        equals(vec![
                            row(vec![names.mean.clone(), text("−"), mu.clone()]),
                            row(vec![n(mean), text("−"), val(*test_value)]),
                            approx(diff),
                        ]),
                    ));
                    let se = (variance / size).sqrt();
                    steps.push(step(
                        "The standard error of the mean:",
                        equals(vec![var("SE"), frac(names.s.clone(), sqrt(names.n.clone())), frac(n(variance.sqrt()), sqrt(n(size))), approx(se)]),
                    ));
                    let df_name = row(vec![names.n.clone(), text("−"), text("1")]);
                    t_and_interval(&mut steps, (row(vec![names.mean, text("−"), mu]), diff), (var("SE"), se), (df_name, size - 1.0), ci);
                }
                sections.push((variable.display_name().to_string(), steps));
            }
        }
        TTest::Independent { variables, group, first, second } => {
            let (gi, gvar) = ds.variable(group).ok_or_else(|| StatsError::UnknownVariable(group.clone()))?;
            for name in variables {
                let (i, variable) = numeric(ds, name)?;
                let mut groups: [Vec<(f64, f64)>; 2] = [Vec::new(), Vec::new()];
                for case in 0..ds.n_cases() {
                    let weight = ds.case_weight(case);
                    let (Some(g), Some(x)) = (valid_value(ds, gi, case), valid(ds, i, case)) else { continue };
                    if weight <= 0.0 {
                        continue;
                    }
                    if g == first {
                        groups[0].push((x, weight));
                    } else if g == second {
                        groups[1].push((x, weight));
                    }
                }
                let heading = |part: &str| format!("{} by {}: {part}", variable.display_name(), gvar.display_name());
                let mut steps = Vec::new();
                let mut stats = Vec::new();
                for (k, (values, value)) in groups.iter().zip([first, second]).enumerate() {
                    let tag = (k + 1).to_string();
                    steps.push(say(&format!("Group {tag}: {} = {}.", gvar.display_name(), value_text(gvar, value))));
                    stats.push(sample(&mut steps, values, &Names::of("x", &tag), w, full));
                }
                let both = match (stats.first(), stats.get(1)) {
                    (Some(Some(a)), Some(Some(b))) => a.variance.zip(b.variance).map(|(v1, v2)| (a.n, a.mean, v1, b.n, b.mean, v2)),
                    _ => None,
                };
                let Some((n1, m1, v1, n2, m2, v2)) = both else {
                    steps.push(say("Each group needs two cases or more for the test."));
                    sections.push((heading("the groups"), steps));
                    continue;
                };
                let d = m1 - m2;
                let (g1, g2) = (Names::of("x", "1"), Names::of("x", "2"));
                steps.push(step(
                    "The difference of the means:",
                    equals(vec![row(vec![g1.mean.clone(), text("−"), g2.mean.clone()]), row(vec![n(m1), text("−"), n(m2)]), approx(d)]),
                ));
                sections.push((heading("the groups"), steps));
                sections.push((heading("Levene's test of equal variances"), levene_steps(&groups, [m1, m2], w, full)));

                // Equal variances: one variance for both, pooled.
                let mut steps = Vec::new();
                let df = n1 + n2 - 2.0;
                let pooled = ((n1 - 1.0) * v1 + (n2 - 1.0) * v2) / df;
                let s2 = |names: &Names| sup(names.s.clone(), text("2"));
                steps.push(step(
                    "The pooled variance weighs each group's variance by its degrees of freedom:",
                    equals(vec![
                        sup(sub(var("s"), var("p")), text("2")),
                        frac(
                            row(vec![
                                paren(row(vec![g1.n.clone(), text("−"), text("1")])),
                                s2(&g1),
                                text("+"),
                                paren(row(vec![g2.n.clone(), text("−"), text("1")])),
                                s2(&g2),
                            ]),
                            row(vec![g1.n.clone(), text("+"), g2.n.clone(), text("−"), text("2")]),
                        ),
                        frac(
                            row(vec![
                                paren(row(vec![n(n1), text("−"), text("1")])),
                                text("·"),
                                n(v1),
                                text("+"),
                                paren(row(vec![n(n2), text("−"), text("1")])),
                                text("·"),
                                n(v2),
                            ]),
                            row(vec![n(n1), text("+"), n(n2), text("−"), text("2")]),
                        ),
                        n(pooled),
                    ]),
                ));
                let se = (pooled * (1.0 / n1 + 1.0 / n2)).sqrt();
                let one_over = |x: Math| frac(text("1"), x);
                steps.push(step(
                    "The standard error of the difference:",
                    equals(vec![
                        var("SE"),
                        sqrt(row(vec![
                            sup(sub(var("s"), var("p")), text("2")),
                            paren(row(vec![one_over(g1.n.clone()), text("+"), one_over(g2.n.clone())])),
                        ])),
                        sqrt(row(vec![n(pooled), text("·"), paren(row(vec![one_over(n(n1)), text("+"), one_over(n(n2))]))])),
                        approx(se),
                    ]),
                ));
                let df_name = row(vec![g1.n.clone(), text("+"), g2.n.clone(), text("−"), text("2")]);
                let diff_name = || row(vec![g1.mean.clone(), text("−"), g2.mean.clone()]);
                t_and_interval(&mut steps, (diff_name(), d), (var("SE"), se), (df_name, df), ci);
                sections.push((heading("equal variances assumed"), steps));

                // Unequal variances: Welch's standard error and Satterthwaite's degrees of freedom.
                let mut steps = Vec::new();
                let (a, b) = (v1 / n1, v2 / n2);
                steps.extend(several(
                    "Each group's variance of its mean:",
                    vec![
                        equals(vec![var("a"), frac(s2(&g1), g1.n.clone()), frac(n(v1), n(n1)), n(a)]),
                        equals(vec![var("b"), frac(s2(&g2), g2.n.clone()), frac(n(v2), n(n2)), n(b)]),
                    ],
                ));
                let se = (a + b).sqrt();
                steps.push(step(
                    "The standard error of the difference adds them, without pooling:",
                    equals(vec![var("SE"), sqrt(row(vec![var("a"), text("+"), var("b")])), sqrt(row(vec![n(a), text("+"), n(b)])), approx(se)]),
                ));
                let (da, db) = (a * a / (n1 - 1.0), b * b / (n2 - 1.0));
                let df = (a + b).powi(2) / (da + db);
                steps.push(step(
                    "Welch–Satterthwaite's degrees of freedom, usually not a whole number:",
                    equals(vec![
                        var("df"),
                        frac(
                            sup(paren(row(vec![var("a"), text("+"), var("b")])), text("2")),
                            row(vec![
                                frac(sup(var("a"), text("2")), row(vec![g1.n.clone(), text("−"), text("1")])),
                                text("+"),
                                frac(sup(var("b"), text("2")), row(vec![g2.n.clone(), text("−"), text("1")])),
                            ]),
                        ),
                        frac(
                            sup(paren(row(vec![n(a), text("+"), n(b)])), text("2")),
                            row(vec![frac(sup(n(a), text("2")), n(n1 - 1.0)), text("+"), frac(sup(n(b), text("2")), n(n2 - 1.0))]),
                        ),
                        frac(n((a + b).powi(2)), row(vec![n(da), text("+"), n(db)])),
                        approx(df),
                    ]),
                ));
                t_and_interval(&mut steps, (diff_name(), d), (var("SE"), se), (text("Welch–Satterthwaite"), df), ci);
                sections.push((heading("equal variances not assumed"), steps));
            }
        }
        TTest::Paired { pairs } => {
            for (p, (a, b)) in pairs.iter().enumerate() {
                let ((ai, avar), (bi, bvar)) = (numeric(ds, a)?, numeric(ds, b)?);
                let rows: Vec<(f64, f64, f64)> = (0..ds.n_cases())
                    .filter_map(|c| Some((valid(ds, ai, c)?, valid(ds, bi, c)?, ds.case_weight(c))))
                    .filter(|(_, _, w)| *w > 0.0)
                    .collect();
                let pair = format!("Pair {}: {} − {}", p + 1, avar.display_name(), bvar.display_name());
                let mut steps = Vec::new();
                let each: Vec<Math> = rows.iter().map(|(x, y, _)| equals(vec![row(vec![val(*x), text("−"), val(*y)]), val(x - y)])).collect();
                let mut listed = Vec::new();
                for (k, m) in each.into_iter().enumerate() {
                    if k > 0 {
                        listed.push(text(",   "));
                    }
                    listed.push(m);
                }
                if !full && listed.len() > SHOWN * 2 {
                    let n_items = listed.len();
                    let mut short: Vec<Math> = listed.iter().take(10).cloned().collect();
                    short.push(text("⋯"));
                    short.extend(listed.iter().skip(n_items - 3).cloned());
                    listed = short;
                }
                steps.push(step(
                    &format!("The differences, case by case, on the cases valid on both ({} minus {}):", avar.display_name(), bvar.display_name()),
                    row(listed),
                ));
                let diffs: Vec<(f64, f64)> = rows.iter().map(|(x, y, w)| (x - y, *w)).collect();
                let names = Names::of("d", "");
                if let Some(Sample { n: size, mean, variance: Some(variance) }) = sample(&mut steps, &diffs, &names, w, full) {
                    let se = (variance / size).sqrt();
                    steps.push(step(
                        "The standard error of the mean difference:",
                        equals(vec![var("SE"), frac(names.s.clone(), sqrt(names.n.clone())), frac(n(variance.sqrt()), sqrt(n(size))), approx(se)]),
                    ));
                    let df_name = row(vec![names.n.clone(), text("−"), text("1")]);
                    t_and_interval(&mut steps, (names.mean, mean), (var("SE"), se), (df_name, size - 1.0), ci);
                }
                sections.push((pair.clone(), steps));
                let x_and_y = format!("x is {}, y is {}; the cases valid on both", avar.display_name(), bvar.display_name());
                sections.push((format!("Pair {}: the correlation", p + 1), pearson(&rows, w, &x_and_y, false, full)));
            }
        }
    }
    Ok(Explanation { title: "T-Test".into(), sections })
}

/// Levene's test, based on the mean: a one-way analysis of variance on each value's distance
/// from its group's mean.
fn levene_steps(groups: &[Vec<(f64, f64)>; 2], means: [f64; 2], w: bool, full: bool) -> Vec<Step> {
    let mut steps =
        vec![say("Levene's test asks whether the groups' variances differ: an analysis of variance on each value's distance from its group's mean.")];
    let z_of: Vec<Vec<(f64, f64)>> = groups.iter().zip(means).map(|(g, m)| g.iter().map(|(x, wi)| ((x - m).abs(), *wi)).collect()).collect();
    let mut z_means = Vec::new();
    for (k, ((g, z), m)) in groups.iter().zip(&z_of).zip(means).enumerate() {
        let tag = (k + 1).to_string();
        let size: f64 = z.iter().map(|(_, w)| w).sum();
        let total: f64 = z.iter().map(|(z, w)| z * w).sum();
        let distances: Vec<Math> = g.iter().map(|(x, wi)| weighted_term(w, *wi, row(vec![text("|"), val(*x), text("−"), n(m), text("|")]))).collect();
        let zs: Vec<Math> = z.iter().map(|(z, wi)| weighted_term(w, *wi, n(*z))).collect();
        let mean = total / size;
        z_means.push((size, mean));
        steps.extend(several(
            &format!("Group {tag}: the distances, their sum and mean:"),
            vec![
                equals(vec![sum(None, None, sub(var("z"), text(&format!("{tag}i")))), terms(distances, full), terms(zs, full), n(total)]),
                equals(vec![sub(bar(var("z")), text(&tag)), frac(n(total), n(size)), n(mean)]),
            ],
        ));
    }
    let big_n: f64 = z_means.iter().map(|(s, _)| s).sum();
    let grand = z_means.iter().map(|(s, m)| s * m).sum::<f64>() / big_n;
    steps.push(step(
        "The mean of all the distances:",
        equals(vec![bar(var("z")), frac(plus(z_means.iter().map(|(s, m)| row(vec![n(*s), text("·"), n(*m)])).collect()), n(big_n)), n(grand)]),
    ));
    let between: f64 = z_means.iter().map(|(s, m)| s * (m - grand).powi(2)).sum();
    steps.push(step(
        "Between the groups: each group's mean distance from that, squared and weighed by its size:",
        equals(vec![
            sub(var("SS"), text("between")),
            plus(z_means.iter().map(|(s, m)| row(vec![n(*s), text("·"), dev_sq(*m, grand)])).collect()),
            n(between),
        ]),
    ));
    let within_terms: Vec<Math> =
        z_of.iter().zip(&z_means).flat_map(|(z, (_, m))| z.iter().map(move |(z, wi)| weighted_term(w, *wi, dev_sq(*z, *m)))).collect();
    let within: f64 = z_of.iter().zip(&z_means).map(|(z, (_, m))| z.iter().map(|(z, wi)| wi * (z - m).powi(2)).sum::<f64>()).sum();
    steps.push(step(
        "Within the groups: each distance's deviation from its group's mean distance, squared:",
        equals(vec![sub(var("SS"), text("within")), terms(within_terms, full), n(within)]),
    ));
    let df2 = big_n - 2.0;
    if df2 <= 0.0 || within <= 0.0 {
        steps.push(say("With no spread within the groups F is undefined."));
        return steps;
    }
    let f = between / (within / df2);
    steps.push(step(
        "F divides the mean squares, with 1 and N − 2 degrees of freedom:",
        equals(vec![
            var("F"),
            frac(
                row(vec![sub(var("SS"), text("between")), text("/"), text("1")]),
                row(vec![sub(var("SS"), text("within")), text("/"), paren(row(vec![var("N"), text("−"), text("2")]))]),
            ),
            frac(n(between), row(vec![n(within), text("/"), n(df2)])),
            approx(f),
        ]),
    ));
    if let Some(p) = dist::f_sig(f, 1.0, df2) {
        steps.push(step(
            &format!("Its significance, from the F distribution with 1 and {} degrees of freedom:", number_text(df2)),
            equals(vec![var("p"), approx(p)]),
        ));
        steps.push(say(if p > 0.05 {
            "Above 0.05: no reason to doubt equal variances, so the row with equal variances assumed is the one to read."
        } else {
            "0.05 or below: the variances differ, so the row with equal variances not assumed is the one to read."
        }));
    }
    steps
}

fn number_text(x: f64) -> String {
    mksigma_output::math::number(x, PLACES)
}

/// `FREQUENCIES`: each value's percent, valid percent and cumulative percent, in the table's
/// `order`.
pub fn frequencies(ds: &Dataset, variables: &[String], order: Order, full: bool) -> Result<Explanation, StatsError> {
    let weights: Vec<f64> = (0..ds.n_cases()).map(|c| ds.case_weight(c)).collect();
    let mut sections = Vec::new();
    for name in variables {
        let (index, variable) = ds.variable(name).ok_or_else(|| StatsError::UnknownVariable(name.clone()))?;
        let counts = count(variable, ds.column(index).unwrap_or_default(), &weights, order);
        let (valid_n, missing) = (counts.valid_total(), counts.missing_total());
        let total = valid_n + missing;
        let mut steps = Vec::new();
        steps.extend(several(
            "Every case counts in the total; the valid ones are those whose value isn't missing:",
            vec![
                equals(vec![var("N"), row(vec![n(valid_n), text("+"), n(missing)]), n(total)]),
                equals(vec![sub(var("N"), text("valid")), n(valid_n)]),
            ],
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

    /// The t-test data of `ttest`'s tests: g, before, after.
    fn t_data() -> Dataset {
        let mut ds = Dataset::new();
        for name in ["g", "before", "after"] {
            ds.add_variable(Variable::numeric(name)).unwrap();
        }
        for (g, b, a) in [(1.0, 5.0, 7.0), (1.0, 6.0, 8.0), (1.0, 7.0, 7.0), (1.0, 8.0, 11.0), (2.0, 4.0, 5.0), (2.0, 4.0, 4.0), (2.0, 6.0, 9.0)] {
            ds.push_case(vec![Value::Num(g), Value::Num(b), Value::Num(a)]).unwrap();
        }
        ds
    }

    /// Every number of `cells` that the hand calculation must end on, to the table's 3 decimals.
    fn ends_on(tex: &str, cells: &[&Cell]) {
        for cell in cells {
            let x = match cell {
                Cell::Num { value, .. } => *value,
                // Whole numbers (a df) show as they are.
                Cell::Int(k) => {
                    assert!(tex.contains(&format!("= {k}")), "{k} in {tex}");
                    continue;
                }
                other => panic!("{other:?}"),
            };
            // As the table shows it: rounded, or exact already.
            let shown = number(x, 3);
            assert!(tex.contains(&format!("\\approx {shown}")) || tex.contains(&format!("= {shown}")), "{x} in {tex}");
        }
    }

    fn tables(items: &[Item]) -> Vec<&mksigma_output::Table> {
        items.iter().filter_map(|i| if let Item::Table(t) = i { Some(t) } else { None }).collect()
    }

    #[test]
    fn independent_t_test_by_hand_agrees_with_the_table() {
        let ds = t_data();
        let test = TTest::Independent { variables: vec!["before".into()], group: "g".into(), first: Value::Num(1.0), second: Value::Num(2.0) };
        let tex = latex(&t_test(&ds, &test, 0.95, false).unwrap());
        let items = crate::t_test(&ds, &test, 0.95).unwrap();
        let t = tables(&items)[1];
        // Levene F and p, t, df, p, difference's SE, interval: both rows.
        let (equal, unequal) = (&t.rows[0].1, &t.rows[1].1);
        ends_on(&tex, &[&equal[1], &equal[2], &equal[3], &equal[5], &equal[7], &equal[8], &equal[9]]);
        ends_on(&tex, &[&unequal[3], &unequal[4], &unequal[5], &unequal[7], &unequal[8], &unequal[9]]);
        // By hand: 5 6 7 8 (mean 6.5, variance 5/3) and 4 4 6 (mean 14/3, variance 4/3).
        assert!(tex.contains("\\frac{26}{4} = 6.5") && tex.contains("\\frac{5}{4 - 1} = 1.6667"), "{tex}");
        assert!(tex.contains("\\frac{2.6667}{3 - 1} = 1.3333"), "{tex}");
        // Pooled: (3 · 5/3 + 2 · 4/3) / 5 = 23/15.
        assert!(tex.contains("= 1.5333"), "{tex}");
        // Levene: distances 1.5 .5 .5 1.5 (mean 1) and 2/3 2/3 4/3 (mean 8/9); F = 0.0816.
        assert!(tex.contains("\\left| 5 - 6.5 \\right|") || tex.contains("| 5 - 6.5 |"), "{tex}");
        assert!(tex.contains("\\approx 0.082"), "{tex}");
    }

    #[test]
    fn one_sample_and_paired_t_tests_by_hand_agree_with_the_tables() {
        let ds = t_data();
        let test = TTest::OneSample { variables: vec!["before".into()], test_value: 5.0 };
        let tex = latex(&t_test(&ds, &test, 0.95, false).unwrap());
        let items = crate::t_test(&ds, &test, 0.95).unwrap();
        let row = &tables(&items)[1].rows[0].1;
        // t, df, p, difference, interval.
        ends_on(&tex, &[&row[0], &row[1], &row[2], &row[3], &row[4], &row[5]]);
        assert!(tex.contains("{\\mu}_{0}"), "{tex}");

        let test = TTest::Paired { pairs: vec![("after".into(), "before".into())] };
        let e = t_test(&ds, &test, 0.95, false).unwrap();
        assert_eq!(e.sections.len(), 2, "the test and the correlation");
        let tex = latex(&e);
        let items = crate::t_test(&ds, &test, 0.95).unwrap();
        let tables = tables(&items);
        let row = &tables[2].rows[0].1;
        // Mean, standard deviation, SE, interval, t, df, p; and the pair's r.
        ends_on(&tex, &[&row[1], &row[2], &row[3], &row[4], &row[5], &row[6], &row[7], &row[8]]);
        ends_on(&tex, &[&tables[1].rows[0].1[2]]);
        // Differences 2 2 0 3 1 0 3, case by case: sum 11.
        assert!(tex.contains("7 - 5 = 2") && tex.contains("+ 3 = 11"), "{tex}");
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
        let tex = latex(&frequencies(&ds, &["g".into()], Order::AscendingValue, false).unwrap());
        // Two 1s of four cases (50%), of three valid (66.7%).
        assert!(tex.contains("\\frac{2}{4} \\cdot 100 = 50") && tex.contains("\\frac{2}{3} \\cdot 100 = 66.7"), "{tex}");
        assert!(tex.contains("\\frac{3}{3} \\cdot 100 = 100"), "cumulative ends at 100: {tex}");
    }
}
