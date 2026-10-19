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

use crate::frequencies::{Order, Statistic, count};
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
fn pearson(pairs: &[(f64, f64, f64)], w: bool, how: &str, one_tailed: bool, full: bool, test: bool) -> Vec<Step> {
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
    if test && total > 2.0 && r.abs() < 1.0 {
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
            let steps = pearson(&pairs, w, how, one_tailed, full, true);
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
                sections.push((format!("Pair {}: the correlation", p + 1), pearson(&rows, w, &x_and_y, false, full, true)));
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

/// `CROSSTABS`' chi-square tests for each row × column pair: the counts and their totals, the
/// expected counts, Pearson's chi-square, the likelihood ratio, for a 2 × 2 table the
/// continuity correction and Fisher's exact test, and the linear-by-linear association.
pub fn crosstabs(ds: &Dataset, rows: &[String], columns: &[String], full: bool) -> Result<Explanation, StatsError> {
    let lookup = |name: &String| ds.variable(name).ok_or_else(|| StatsError::UnknownVariable(name.clone()));
    let mut sections = Vec::new();
    for row_name in rows {
        let (ri, row_var) = lookup(row_name)?;
        for column_name in columns {
            let (ci, column_var) = lookup(column_name)?;
            let counts = crate::crosstabs::count(ds, ri, ci);
            sections
                .push((format!("{} * {}", row_var.display_name(), column_var.display_name()), chi_square_steps(row_var, column_var, &counts, full)));
        }
    }
    Ok(Explanation { title: "Crosstabs".into(), sections })
}

fn chi_square_steps(
    row_var: &mksigma_core::Variable,
    column_var: &mksigma_core::Variable,
    counts: &crate::crosstabs::Counts,
    full: bool,
) -> Vec<Step> {
    let (nr, nc) = (counts.rows.len(), counts.columns.len());
    let labels = |var: &mksigma_core::Variable, values: &[mksigma_core::Value]| {
        values.iter().enumerate().map(|(k, v)| format!("{} = {}", k + 1, value_text(var, v))).collect::<Vec<_>>().join(", ")
    };
    let mut steps = vec![say(&format!(
        "The cases valid on both variables, counted in each cell (O; weighted counts are rounded to whole numbers). Rows: {}; columns: {}.",
        labels(row_var, &counts.rows),
        labels(column_var, &counts.columns),
    ))];
    if nr < 2 || nc < 2 {
        steps.push(say("One of the variables has a single value here, so there are no chi-square tests."));
        return steps;
    }
    let big_n = counts.total();
    let cell = |letter: &str, r: usize, c: usize| sub(var(letter), text(&format!("{}{}", r + 1, c + 1)));
    let row_totals: Vec<Math> = (0..nr)
        .map(|r| {
            equals(vec![sub(var("R"), text(&(r + 1).to_string())), plus((0..nc).map(|c| n(counts.get(r, c))).collect()), n(counts.row_total(r))])
        })
        .collect();
    steps.extend(several("Each row's total adds its counts:", row_totals));
    let column_totals: Vec<Math> = (0..nc)
        .map(|c| {
            equals(vec![sub(var("C"), text(&(c + 1).to_string())), plus((0..nr).map(|r| n(counts.get(r, c))).collect()), n(counts.column_total(c))])
        })
        .collect();
    steps.extend(several("And each column's:", column_totals));
    steps.push(step("The total:", equals(vec![var("N"), plus((0..nr).map(|r| n(counts.row_total(r))).collect()), n(big_n)])));
    let cells: Vec<(usize, usize)> = (0..nr).flat_map(|r| (0..nc).map(move |c| (r, c))).collect();
    let expected: Vec<Math> = cells
        .iter()
        .map(|&(r, c)| {
            equals(vec![
                cell("E", r, c),
                frac(row(vec![sub(var("R"), text(&(r + 1).to_string())), text("·"), sub(var("C"), text(&(c + 1).to_string()))]), var("N")),
                frac(row(vec![n(counts.row_total(r)), text("·"), n(counts.column_total(c))]), n(big_n)),
                n(counts.expected(r, c)),
            ])
        })
        .collect();
    steps.extend(several("The count each cell would expect if the variables were independent:", expected));

    let df = ((nr - 1) * (nc - 1)) as f64;
    let chi2: f64 = cells.iter().map(|&(r, c)| (counts.get(r, c) - counts.expected(r, c)).powi(2) / counts.expected(r, c)).sum();
    let symbol = || frac(sup(paren(row(vec![var("O"), text("−"), var("E")])), text("2")), var("E"));
    let each: Vec<Math> = cells
        .iter()
        .map(|&(r, c)| frac(sup(paren(row(vec![n(counts.get(r, c)), text("−"), n(counts.expected(r, c))])), text("2")), n(counts.expected(r, c))))
        .collect();
    let parts: Vec<Math> = cells.iter().map(|&(r, c)| n((counts.get(r, c) - counts.expected(r, c)).powi(2) / counts.expected(r, c))).collect();
    steps.push(step(
        "Pearson's chi-square adds each cell's squared distance from its expected count, relative to it:",
        equals(vec![sup(var("χ"), text("2")), sum(None, None, symbol()), terms(each, full), terms(parts, full), approx(chi2)]),
    ));
    steps.push(step(
        "Its degrees of freedom:",
        equals(vec![
            var("df"),
            row(vec![paren(row(vec![var("r"), text("−"), text("1")])), paren(row(vec![var("c"), text("−"), text("1")]))]),
            row(vec![paren(row(vec![n(nr as f64), text("−"), text("1")])), text("·"), paren(row(vec![n(nc as f64), text("−"), text("1")]))]),
            n(df),
        ]),
    ));
    let chi_sig = |x: f64, df: f64, what: &str| {
        dist::chi_square_sig(x, df).map(|p| {
            step(
                &format!(
                    "{what}: the probability of a chi-square at least this large with {} degrees of freedom, from that distribution:",
                    number_text(df)
                ),
                equals(vec![var("p"), approx(p)]),
            )
        })
    };
    steps.extend(chi_sig(chi2, df, "Its significance"));

    let lr: f64 =
        cells.iter().map(|&(r, c)| (counts.get(r, c), counts.expected(r, c))).filter(|(o, _)| *o > 0.0).map(|(o, e)| 2.0 * o * (o / e).ln()).sum();
    let lr_terms: Vec<Math> = cells
        .iter()
        .filter(|&&(r, c)| counts.get(r, c) > 0.0)
        .map(|&(r, c)| row(vec![n(counts.get(r, c)), text("·"), text("ln"), frac(n(counts.get(r, c)), n(counts.expected(r, c)))]))
        .collect();
    steps.push(step(
        "The likelihood ratio, over the cells with a count:",
        equals(vec![
            sup(var("G"), text("2")),
            row(vec![text("2"), sum(None, None, row(vec![var("O"), text("·"), text("ln"), frac(var("O"), var("E"))]))]),
            row(vec![text("2"), paren(terms(lr_terms, full))]),
            approx(lr),
        ]),
    ));
    steps.extend(chi_sig(lr, df, "Its significance"));

    if nr == 2 && nc == 2 {
        let (a, b, c, d) = (counts.get(0, 0), counts.get(0, 1), counts.get(1, 0), counts.get(1, 1));
        let (r1, r2, c1, c2) = (a + b, c + d, a + c, b + d);
        let inner = ((a * d - b * c).abs() - big_n / 2.0).max(0.0);
        let yates = big_n * inner.powi(2) / (r1 * r2 * c1 * c2);
        steps.push(step(
            "The continuity correction (Yates), for a 2 × 2 table with cells a b / c d:",
            equals(vec![
                sup(sub(var("χ"), text("Yates")), text("2")),
                frac(
                    row(vec![
                        var("N"),
                        sup(
                            paren(row(vec![
                                text("|"),
                                var("a"),
                                var("d"),
                                text("−"),
                                var("b"),
                                var("c"),
                                text("|"),
                                text("−"),
                                frac(var("N"), text("2")),
                            ])),
                            text("2"),
                        ),
                    ]),
                    row(vec![sub(var("R"), text("1")), sub(var("R"), text("2")), sub(var("C"), text("1")), sub(var("C"), text("2"))]),
                ),
                frac(
                    row(vec![
                        n(big_n),
                        text("·"),
                        sup(
                            paren(row(vec![
                                text("|"),
                                n(a),
                                text("·"),
                                n(d),
                                text("−"),
                                n(b),
                                text("·"),
                                n(c),
                                text("|"),
                                text("−"),
                                n(big_n / 2.0),
                            ])),
                            text("2"),
                        ),
                    ]),
                    row(vec![n(r1), text("·"), n(r2), text("·"), n(c1), text("·"), n(c2)]),
                ),
                approx(yates),
            ]),
        ));
        steps.extend(chi_sig(yates, 1.0, "Its significance"));
        steps.extend(fisher_steps([a, b, c, d], full));
    }
    // Linear-by-linear: the correlation of the row and column values, each cell counted O times.
    let scores = |values: &[mksigma_core::Value]| values.iter().map(mksigma_core::Value::as_num).collect::<Option<Vec<f64>>>();
    if row_var.var_type == mksigma_core::VarType::Numeric
        && column_var.var_type == mksigma_core::VarType::Numeric
        && let (Some(rs), Some(cs)) = (scores(&counts.rows), scores(&counts.columns))
    {
        let pairs: Vec<(f64, f64, f64)> =
            cells.iter().filter(|&&(r, c)| counts.get(r, c) > 0.0).filter_map(|&(r, c)| Some((*rs.get(r)?, *cs.get(c)?, counts.get(r, c)))).collect();
        steps.push(say("The linear-by-linear association uses the correlation r of the row and column values, each cell counted as often as its count (the w below):"));
        steps.extend(pearson(&pairs, true, "in the table", false, full, false));
        let mx = pairs.iter().map(|(x, _, w)| x * w).sum::<f64>() / big_n;
        let my = pairs.iter().map(|(_, y, w)| y * w).sum::<f64>() / big_n;
        let sxy: f64 = pairs.iter().map(|(x, y, w)| w * (x - mx) * (y - my)).sum();
        let sxx: f64 = pairs.iter().map(|(x, _, w)| w * (x - mx).powi(2)).sum();
        let syy: f64 = pairs.iter().map(|(_, y, w)| w * (y - my).powi(2)).sum();
        let r = sxy / (sxx * syy).sqrt();
        if r.is_finite() {
            let m2 = (big_n - 1.0) * r * r;
            steps.push(step(
                "The association, with 1 degree of freedom:",
                equals(vec![
                    sup(var("M"), text("2")),
                    row(vec![paren(row(vec![var("N"), text("−"), text("1")])), sup(var("r"), text("2"))]),
                    row(vec![paren(row(vec![n(big_n), text("−"), text("1")])), text("·"), sup(val(r), text("2"))]),
                    approx(m2),
                ]),
            ));
            steps.extend(chi_sig(m2, 1.0, "Its significance"));
        }
    }
    steps
}

/// Fisher's exact test for `[a, b, c, d]`: every table with the same totals, its probability
/// from the hypergeometric distribution, and the ones at most as likely as this table.
fn fisher_steps([a, b, c, d]: [f64; 4], full: bool) -> Vec<Step> {
    let (r1, r2, c1, c2, big_n) = (a + b, c + d, a + c, b + d, a + b + c + d);
    let ln_fact = |x: f64| statrs::function::gamma::ln_gamma(x + 1.0);
    let p_of = |x: f64| {
        (ln_fact(r1) + ln_fact(r2) + ln_fact(c1) + ln_fact(c2)
            - ln_fact(big_n)
            - ln_fact(x)
            - ln_fact(r1 - x)
            - ln_fact(c1 - x)
            - ln_fact(r2 - c1 + x))
        .exp()
    };
    let fact = |x: f64| row(vec![n(x), text("!")]);
    let mut steps = vec![step(
        "Fisher's exact test: with the totals fixed, the probability of a table whose first cell is x:",
        equals(vec![
            row(vec![var("P"), paren(var("x"))]),
            frac(
                row(vec![
                    sub(var("R"), text("1")),
                    text("!"),
                    sub(var("R"), text("2")),
                    text("!"),
                    sub(var("C"), text("1")),
                    text("!"),
                    sub(var("C"), text("2")),
                    text("!"),
                ]),
                row(vec![
                    var("N"),
                    text("!"),
                    var("x"),
                    text("!"),
                    paren(row(vec![sub(var("R"), text("1")), text("−"), var("x")])),
                    text("!"),
                    paren(row(vec![sub(var("C"), text("1")), text("−"), var("x")])),
                    text("!"),
                    paren(row(vec![sub(var("R"), text("2")), text("−"), sub(var("C"), text("1")), text("+"), var("x")])),
                    text("!"),
                ]),
            ),
        ]),
    )];
    let observed = p_of(a);
    steps.push(step(
        "This table's:",
        equals(vec![
            row(vec![var("P"), paren(n(a))]),
            frac(
                row(vec![fact(r1), text("·"), fact(r2), text("·"), fact(c1), text("·"), fact(c2)]),
                row(vec![fact(big_n), text("·"), fact(a), text("·"), fact(b), text("·"), fact(c), text("·"), fact(d)]),
            ),
            n(observed),
        ]),
    ));
    let (low, high) = ((c1 - r2).max(0.0) as i64, r1.min(c1) as i64);
    let (mut two, mut lower, mut upper) = (Vec::new(), Vec::new(), Vec::new());
    let (mut two_sum, mut lower_sum, mut upper_sum) = (0.0, 0.0, 0.0);
    let mut all = Vec::new();
    for x in low..=high {
        let x = x as f64;
        let p = p_of(x);
        all.push(equals(vec![row(vec![var("P"), paren(n(x))]), n(p)]));
        if p <= observed * (1.0 + 1e-7) {
            two.push(n(p));
            two_sum += p;
        }
        if x <= a {
            lower.push(n(p));
            lower_sum += p;
        }
        if x >= a {
            upper.push(n(p));
            upper_sum += p;
        }
    }
    let mut listed = Vec::new();
    for (k, m) in all.into_iter().enumerate() {
        if k > 0 {
            listed.push(text(",   "));
        }
        listed.push(m);
    }
    if !full && listed.len() > SHOWN * 2 {
        let count = listed.len();
        let mut short: Vec<Math> = listed.iter().take(10).cloned().collect();
        short.push(text("⋯"));
        short.extend(listed.iter().skip(count - 3).cloned());
        listed = short;
    }
    steps.push(step(&format!("Every table with these totals, x from {low} to {high}:"), row(listed)));
    steps.push(step(
        "Two-sided: the tables at most as likely as this one, added up:",
        equals(vec![var("p"), terms(two, full), approx(two_sum.min(1.0))]),
    ));
    let (one, one_sum, side) = if lower_sum <= upper_sum { (lower, lower_sum, "x at most") } else { (upper, upper_sum, "x at least") };
    steps.push(step(
        &format!("One-sided: the smaller tail, the tables with {side} {}:", number_text(a)),
        equals(vec![var("p"), terms(one, full), approx(one_sum.min(1.0))]),
    ));
    steps
}

fn number_text(x: f64) -> String {
    mksigma_output::math::number(x, PLACES)
}

/// `x` to four decimals, then to `places` as the table shows it (when that differs).
fn approx_to(x: f64, places: usize) -> Math {
    if number_text(x) == mksigma_output::math::number(x, places) { n(x) } else { row(vec![n(x), text("≈"), num(x, places)]) }
}

/// `FREQUENCIES`' Statistics table: N valid and missing, then each statistic asked for, from
/// the frequencies (each value `x` counted `f` times), with what it needs before it.
pub fn frequency_statistics(ds: &Dataset, variables: &[String], wanted: &[Statistic], full: bool) -> Result<Explanation, StatsError> {
    let weights: Vec<f64> = (0..ds.n_cases()).map(|c| ds.case_weight(c)).collect();
    let asked = |s: &[Statistic]| s.iter().any(|x| wanted.contains(x));
    let mut sections = Vec::new();
    for name in variables {
        let (index, variable) = ds.variable(name).ok_or_else(|| StatsError::UnknownVariable(name.clone()))?;
        let counts = count(variable, ds.column(index).unwrap_or_default(), &weights, Order::AscendingValue);
        let (valid_n, missing) = (counts.valid_total(), counts.missing_total());
        let mut steps = several(
            "N counts the cases with a valid value, and apart those whose value is missing:",
            vec![equals(vec![sub(var("N"), text("valid")), n(valid_n)]), equals(vec![sub(var("N"), text("missing")), n(missing)])],
        );
        let xs: Vec<(f64, f64)> = counts.valid.iter().filter_map(|(v, f)| Some((v.as_num()?, *f))).collect();
        if wanted.is_empty() {
            sections.push((variable.display_name().to_string(), steps));
            continue;
        }
        if variable.var_type != mksigma_core::VarType::Numeric {
            steps.push(say("A string variable has none of the other statistics."));
            sections.push((variable.display_name().to_string(), steps));
            continue;
        }
        if xs.is_empty() {
            steps.push(say("With no valid values there is nothing more to compute."));
            sections.push((variable.display_name().to_string(), steps));
            continue;
        }
        // Each distinct value once, with its frequency.
        let f_x = |x: f64, f: f64| row(vec![n(f), text("·"), val(x)]);
        let body = row(vec![indexed("f", "k"), indexed("x", "k")]);
        let total: f64 = xs.iter().map(|(x, f)| x * f).sum();
        let mean = total / valid_n;
        if asked(&[Statistic::Sum, Statistic::Mean, Statistic::SeMean, Statistic::StdDev, Statistic::Variance]) {
            steps.push(step(
                "The sum: each distinct value times its frequency f, added up:",
                equals(vec![sum(None, None, body.clone()), terms(xs.iter().map(|(x, f)| f_x(*x, *f)).collect(), full), approx_to(total, 2)]),
            ));
        }
        if asked(&[Statistic::Mean, Statistic::SeMean, Statistic::StdDev, Statistic::Variance]) {
            steps.push(step(
                "The mean divides it by N:",
                equals(vec![
                    bar(var("x")),
                    frac(sum(None, None, body), sub(var("N"), text("valid"))),
                    frac(n(total), n(valid_n)),
                    approx_to(mean, 2),
                ]),
            ));
        }
        if asked(&[Statistic::SeMean, Statistic::StdDev, Statistic::Variance]) {
            let ss: f64 = xs.iter().map(|(x, f)| f * (x - mean).powi(2)).sum();
            let devs: Vec<Math> = xs.iter().map(|(x, f)| row(vec![n(*f), text("·"), dev_sq(*x, mean)])).collect();
            steps.push(step(
                "The squared deviations from the mean, each times its frequency:",
                equals(vec![
                    sum(None, None, row(vec![indexed("f", "k"), sup(paren(row(vec![indexed("x", "k"), text("−"), bar(var("x"))])), text("2"))])),
                    terms(devs, full),
                    n(ss),
                ]),
            ));
            if valid_n <= 1.0 {
                steps.push(say("With N of 1 or less the variance and the standard deviation are undefined."));
            } else {
                let variance = ss / (valid_n - 1.0);
                let sd = variance.sqrt();
                steps.extend(several(
                    "The variance divides that by N − 1; the standard deviation is its square root:",
                    vec![
                        equals(vec![sup(var("s"), text("2")), frac(n(ss), row(vec![n(valid_n), text("−"), text("1")])), approx_to(variance, 2)]),
                        equals(vec![var("s"), sqrt(n(variance)), approx_to(sd, 2)]),
                    ],
                ));
                if wanted.contains(&Statistic::SeMean) {
                    steps.push(step(
                        "The standard error of the mean:",
                        equals(vec![
                            var("SE"),
                            frac(var("s"), sqrt(sub(var("N"), text("valid")))),
                            frac(n(sd), sqrt(n(valid_n))),
                            approx_to(sd / valid_n.sqrt(), 2),
                        ]),
                    ));
                }
            }
        }
        if wanted.contains(&Statistic::Mode) {
            let top = xs.iter().map(|(_, f)| *f).fold(0.0, f64::max);
            let modes: Vec<f64> = xs.iter().filter(|(_, f)| *f == top).map(|(x, _)| *x).collect();
            let smallest = modes.iter().copied().fold(f64::INFINITY, f64::min);
            let text_of = if modes.len() > 1 {
                format!(
                    "The mode is the most frequent value. {} values share the largest frequency, {}; the table shows the smallest:",
                    modes.len(),
                    number_text(top)
                )
            } else {
                format!("The mode is the most frequent value, the one with the largest frequency, {}:", number_text(top))
            };
            steps.push(step(&text_of, equals(vec![text("Mode"), approx_to(smallest, 2)])));
        }
        let (lo, hi) = xs.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), (x, _)| (a.min(*x), b.max(*x)));
        if asked(&[Statistic::Minimum, Statistic::Maximum, Statistic::Range]) {
            steps.extend(several(
                "The smallest and largest valid values:",
                vec![equals(vec![text("Minimum"), approx_to(lo, 2)]), equals(vec![text("Maximum"), approx_to(hi, 2)])],
            ));
        }
        if wanted.contains(&Statistic::Range) {
            steps.push(step(
                "The range is their difference:",
                equals(vec![text("Range"), row(vec![n(hi), text("−"), val(lo)]), approx_to(hi - lo, 2)]),
            ));
        }
        sections.push((variable.display_name().to_string(), steps));
    }
    Ok(Explanation { title: "Statistics".into(), sections })
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
    fn frequency_statistics_by_hand_agree_with_the_table() {
        let ds = t_data();
        let all = [
            Statistic::Mean,
            Statistic::SeMean,
            Statistic::Mode,
            Statistic::StdDev,
            Statistic::Variance,
            Statistic::Range,
            Statistic::Minimum,
            Statistic::Maximum,
            Statistic::Sum,
        ];
        let names: Vec<String> = vec!["before".into(), "after".into()];
        let tex = latex(&frequency_statistics(&ds, &names, &all, false).unwrap());
        let options = crate::FrequenciesOptions { variables: names, table: false, order: Order::AscendingValue, statistics: all.to_vec() };
        let items = crate::frequencies(&ds, &options).unwrap();
        let t = tables(&items)[0];
        for (_, cells) in t.rows.iter().skip(2) {
            for cell in cells {
                let x = match cell {
                    Cell::Num { value, .. } => *value,
                    Cell::Text(s) => s.trim_end_matches('a').parse().unwrap(),
                    other => panic!("{other:?}"),
                };
                let shown = number(x, 2);
                assert!(tex.contains(&format!("\\approx {shown}")) || tex.contains(&format!("= {shown}")), "{x} in {tex}");
            }
        }
        // before: 4 4 5 6 6 7 8, in frequencies: 2 · 4 + 5 + 2 · 6 + 7 + 8 = 40.
        assert!(tex.contains("2 \\cdot 4 + 1 \\cdot 5 + 2 \\cdot 6 + 1 \\cdot 7 + 1 \\cdot 8 = 40"), "{tex}");
        // Two values share the largest frequency: the smallest is shown.
        assert!(tex.contains("\\text{Mode} = 4"), "{tex}");
    }

    #[test]
    fn chi_square_by_hand_agrees_with_the_table() {
        // Drug: 12 improved, 3 not; placebo: 5 and 10.
        let mut ds = Dataset::new();
        for name in ["treatment", "improved"] {
            ds.add_variable(Variable::numeric(name)).unwrap();
        }
        for (t, i, k) in [(1.0, 1.0, 12), (1.0, 2.0, 3), (2.0, 1.0, 5), (2.0, 2.0, 10)] {
            for _ in 0..k {
                ds.push_case(vec![Value::Num(t), Value::Num(i)]).unwrap();
            }
        }
        let (rows, columns): (Vec<String>, Vec<String>) = (vec!["treatment".into()], vec!["improved".into()]);
        let tex = latex(&crosstabs(&ds, &rows, &columns, false).unwrap());
        let options = crate::CrosstabsOptions { rows, columns, cells: crate::CrosstabsCells::default(), chisq: true };
        let items = crate::crosstabs(&ds, &options).unwrap();
        let t = tables(&items).into_iter().find(|t| t.title == "Chi-Square Tests").unwrap();
        // Every value, significance and exact significance of the table.
        for (name, cells) in &t.rows {
            if name == "N of Valid Cases" {
                continue;
            }
            for k in [0, 2, 3, 4] {
                if let Some(cell @ Cell::Num { .. }) = cells.get(k) {
                    ends_on(&tex, &[cell]);
                }
            }
        }
        // Expected 15 · 17 / 30 = 8.5; Yates 30 (|12 · 10 − 3 · 5| − 15)² / (15 · 15 · 17 · 13).
        assert!(tex.contains("\\frac{15 \\cdot 17}{30} = 8.5"), "{tex}");
        assert!(tex.contains("\\left| 12 \\cdot 10 - 3 \\cdot 5 \\right|") || tex.contains("| 12 \\cdot 10 - 3 \\cdot 5 |"), "{tex}");
        assert!(tex.contains("\\frac{15 ! \\cdot 15 ! \\cdot 17 ! \\cdot 13 !}{30 ! \\cdot 12 ! \\cdot 3 ! \\cdot 5 ! \\cdot 10 !}"), "{tex}");
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
