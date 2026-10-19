//! The calculations behind a table, by hand: each statistic's formula, then the same formula
//! with the data's numbers in it, every term and every sum, down to the figure the table shows.
//! Built from the same cases, weights and missing values as the table, so a reader can check
//! every number with a calculator. Tests hold each final figure against the table's.
//!
//! With many cases the terms of a sum show as the first five, `⋯` and the last two; `full`
//! writes every one.

use mksigma_core::Dataset;
use mksigma_i18n::tr;
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
    num(x, places(x))
}

/// Four decimals; a small number (under 0.01) four significant digits instead, so that the
/// entries of an inverse matrix, say, don't show as 0.
fn places(x: f64) -> usize {
    let size = x.abs();
    if size == 0.0 || size >= 0.01 || !size.is_finite() {
        return PLACES;
    }
    ((-size.log10()).ceil() as usize + 3).min(15)
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
        let note = if left > 0 { format!(" {}", tr!("ex-left-out", n = left)) } else { String::new() };
        if w {
            steps.push(step(&tr!("ex-weighted-n", note = note.as_str()), equals(vec![var("N"), sum(None, None, indexed("w", "i")), n(total)])));
        } else {
            steps.push(step(&tr!("ex-valid-n", note = note), equals(vec![var("N"), n(total)])));
        }
        if xs.is_empty() {
            steps.push(say(&tr!("ex-with-no-valid-values-there-is-nothing-more-to-compute")));
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
        steps.push(step(&tr!("ex-the-sum-of-the-values"), equals(vec![sum(from, to, body.clone()), terms(each, full), n(s)])));
        // The mean.
        let mean = s / total;
        let (from, to) = range();
        steps.push(step(
            &tr!("ex-the-mean-divides-the-sum-by-n"),
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
            &tr!("ex-the-squared-deviations-from-the-mean-added-up"),
            equals(vec![sum(from, to, dev_body), terms(devs, full), terms(squares, full), n(ss)]),
        ));
        if total <= 1.0 {
            steps.push(say(&tr!("ex-with-n-of-1-or-less-the-variance-and-standard-deviation-are")));
        } else {
            let variance = ss / (total - 1.0);
            steps.push(step(
                &tr!("ex-the-variance-divides-that-by-n-1"),
                equals(vec![sup(var("s"), text("2")), frac(n(ss), row(vec![n(total), text("−"), text("1")])), n(variance)]),
            ));
            let sd = variance.sqrt();
            steps.push(step(
                &tr!("ex-the-standard-deviation-is-its-square-root"),
                equals(vec![var("s"), sqrt(n(variance)), row(vec![n(sd), text("≈"), num(sd, 2)])]),
            ));
        }
        let (lo, hi) = xs.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), (x, _)| (a.min(*x), b.max(*x)));
        steps.extend(several(
            &tr!("ex-the-minimum-and-maximum-are-the-smallest-and-largest-values"),
            vec![equals(vec![text(&tr!("ex-word-min")), n(lo)]), equals(vec![text(&tr!("ex-word-max")), n(hi)])],
        ));
        sections.push((variable.display_name().to_string(), steps));
    }
    Ok(Explanation { title: tr!("ex-title-descriptive-statistics"), sections })
}

/// Pearson's r of `(x, y, weight)` pairs, step by step, then its t and significance; `how`
/// says which cases they are.
fn pearson(pairs: &[(f64, f64, f64)], w: bool, how: &str, one_tailed: bool, full: bool, test: bool) -> Vec<Step> {
    let mut steps = Vec::new();
    let total: f64 = pairs.iter().map(|(_, _, w)| w).sum();
    let cases = if w { tr!("ex-cases-weighted", how = how) } else { tr!("ex-cases", how = how) };
    steps.push(step(&cases, equals(vec![var("N"), n(total)])));
    if total <= 0.0 {
        steps.push(say(&tr!("ex-with-no-cases-there-is-no-correlation")));
        return steps;
    }
    let mx = pairs.iter().map(|(x, _, w)| x * w).sum::<f64>() / total;
    let my = pairs.iter().map(|(_, y, w)| y * w).sum::<f64>() / total;
    steps.extend(several(&tr!("ex-the-means-of-x-and-y"), vec![equals(vec![bar(var("x")), n(mx)]), equals(vec![bar(var("y")), n(my)])]));
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
        &tr!("ex-the-sum-of-the-products-of-the-deviations"),
        equals(vec![sub(var("S"), var("xy")), sum(None, None, weight(row(vec![dx(), dy()]))), terms(cross, full), n(sxy)]),
    ));
    let xs: Vec<Math> = pairs.iter().map(|(x, _, wi)| weighted_term(w, *wi, dev_sq(*x, mx))).collect();
    steps.push(step(
        &tr!("ex-the-sum-of-the-squared-deviations-of-x"),
        equals(vec![sub(var("S"), var("xx")), sum(None, None, weight(sup(dx(), text("2")))), terms(xs, full), n(sxx)]),
    ));
    let ys: Vec<Math> = pairs.iter().map(|(_, y, wi)| weighted_term(w, *wi, dev_sq(*y, my))).collect();
    steps.push(step(
        &tr!("ex-and-of-y"),
        equals(vec![sub(var("S"), var("yy")), sum(None, None, weight(sup(dy(), text("2")))), terms(ys, full), n(syy)]),
    ));
    let r = sxy / (sxx * syy).sqrt();
    if !r.is_finite() {
        steps.push(say(&tr!("ex-one-of-the-variables-doesn-t-vary-here-so-r-is-undefined")));
        return steps;
    }
    let r = r.clamp(-1.0, 1.0);
    steps.push(step(
        &tr!("ex-pearson-s-r"),
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
            &tr!("ex-its-t-statistic-with-n-2-degrees-of-freedom"),
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
            let (p, tails) = if one_tailed { (p / 2.0, tr!("ex-one-tailed")) } else { (p, tr!("ex-two-tailed")) };
            steps.push(step(
                &tr!("ex-r-significance", tails = tails, df = number_text(df)),
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
            let how = if listwise { tr!("ex-how-listwise") } else { tr!("ex-how-pairwise") };
            let steps = pearson(&pairs, w, &how, one_tailed, full, true);
            sections.push((tr!("ex-pair-with", a = rvar.display_name(), b = cvar.display_name()), steps));
        }
    }
    Ok(Explanation { title: tr!("ex-title-correlations"), sections })
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
        steps.push(step(&tr!("ex-its-size-is-the-sum-of-the-weights"), equals(vec![names.n.clone(), sum(None, None, indexed("w", "i")), n(total)])));
    } else {
        steps.push(step(&tr!("ex-its-size-counts-the-cases-with-a-valid-value"), equals(vec![names.n.clone(), n(total)])));
    }
    if total <= 0.0 {
        steps.push(say(&tr!("ex-with-no-valid-values-there-is-nothing-more-to-compute")));
        return None;
    }
    let s: f64 = xs.iter().map(|(x, w)| x * w).sum();
    let body = if w { row(vec![indexed("w", "i"), names.item.clone()]) } else { names.item.clone() };
    let each: Vec<Math> = xs.iter().map(|(x, wi)| weighted_term(w, *wi, val(*x))).collect();
    steps.push(step(&tr!("ex-the-sum-of-the-values"), equals(vec![sum(None, None, body.clone()), terms(each, full), n(s)])));
    let mean = s / total;
    steps.push(step(
        &tr!("ex-the-mean"),
        equals(vec![names.mean.clone(), frac(sum(None, None, body), names.n.clone()), frac(n(s), n(total)), approx(mean)]),
    ));
    let ss: f64 = xs.iter().map(|(x, wi)| wi * (x - mean).powi(2)).sum();
    let devs: Vec<Math> = xs.iter().map(|(x, wi)| weighted_term(w, *wi, dev_sq(*x, mean))).collect();
    let squares: Vec<Math> = xs.iter().map(|(x, wi)| n(wi * (x - mean).powi(2))).collect();
    let dev_body = sup(paren(row(vec![names.item.clone(), text("−"), names.mean.clone()])), text("2"));
    let dev_body = if w { row(vec![indexed("w", "i"), dev_body]) } else { dev_body };
    steps.push(step(
        &tr!("ex-the-squared-deviations-from-the-mean-added-up"),
        equals(vec![sum(None, None, dev_body), terms(devs, full), terms(squares, full), n(ss)]),
    ));
    if total <= 1.0 {
        steps.push(say(&tr!("ex-with-a-size-of-1-or-less-the-variance-is-undefined")));
        return Some(Sample { n: total, mean, variance: None });
    }
    let variance = ss / (total - 1.0);
    steps.extend(several(
        &tr!("ex-the-variance-divides-that-by-the-size-minus-1-and-the-standa"),
        vec![
            equals(vec![sup(names.s.clone(), text("2")), frac(n(ss), row(vec![n(total), text("−"), text("1")])), approx(variance)]),
            equals(vec![names.s.clone(), sqrt(n(variance)), approx(variance.sqrt())]),
        ],
    ));
    Some(Sample { n: total, mean, variance: Some(variance) })
}

/// `x` to four decimals, then as the tables show it, to three (when that differs).
fn approx(x: f64) -> Math {
    if mksigma_output::math::number(x, places(x)) == mksigma_output::math::number(x, 3) { n(x) } else { row(vec![n(x), text("≈"), num(x, 3)]) }
}

/// The two-tailed significance of `t` with `df` degrees of freedom.
fn t_significance(t: f64, df: f64) -> Option<Step> {
    let p = dist::t_sig(t, df)?;
    Some(step(&tr!("ex-t-significance", df = number_text(df)), equals(vec![var("p"), approx(p)])))
}

/// t from a difference and its standard error, its degrees of freedom, significance and the
/// confidence interval of the difference.
fn t_and_interval(steps: &mut Vec<Step>, difference: (Math, f64), se: (Math, f64), df: (Math, f64), ci: f64) {
    let ((d_name, d), (se_name, se), (df_name, df)) = (difference, se, df);
    let t = d / se;
    steps.push(step(
        &tr!("ex-t-divides-the-difference-by-its-standard-error"),
        equals(vec![var("t"), frac(d_name, se_name), frac(n(d), n(se)), approx(t)]),
    ));
    let whole = df.fract() == 0.0;
    steps.push(step(&tr!("ex-its-degrees-of-freedom"), equals(vec![var("df"), df_name, if whole { n(df) } else { approx(df) }])));
    steps.extend(t_significance(t, df));
    if let Some(critical) = dist::t_quantile(0.5 + ci / 2.0, df) {
        let level = number_text(ci * 100.0);
        let tail = number_text((1.0 - ci) / 2.0 * 100.0);
        steps.push(step(&tr!("ex-ci-t", level = level, tail = tail), equals(vec![sub(var("t"), num(0.5 + ci / 2.0, 4)), approx(critical)])));
        let half = critical * se;
        steps.extend(several(
            &tr!("ex-the-interval-is-the-difference-plus-or-minus-that-t-times-th"),
            vec![
                equals(vec![row(vec![n(d), text("±"), n(critical), text("·"), n(se)]), row(vec![n(d), text("±"), n(half)])]),
                equals(vec![text(&tr!("ex-word-lower-cap")), row(vec![n(d), text("−"), n(half)]), approx(d - half)]),
                equals(vec![text(&tr!("ex-word-upper-cap")), row(vec![n(d), text("+"), n(half)]), approx(d + half)]),
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
                        &tr!("ex-the-difference-from-the-test-value"),
                        equals(vec![
                            row(vec![names.mean.clone(), text("−"), mu.clone()]),
                            row(vec![n(mean), text("−"), val(*test_value)]),
                            approx(diff),
                        ]),
                    ));
                    let se = (variance / size).sqrt();
                    steps.push(step(
                        &tr!("ex-the-standard-error-of-the-mean"),
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
                let heading = |part: &str| tr!("ex-heading-by", variable = variable.display_name(), group = gvar.display_name(), part = part);
                let mut steps = Vec::new();
                let mut stats = Vec::new();
                for (k, (values, value)) in groups.iter().zip([first, second]).enumerate() {
                    let tag = (k + 1).to_string();
                    steps.push(say(&tr!("ex-group-is", tag = tag.as_str(), group = gvar.display_name(), value = value_text(gvar, value))));
                    stats.push(sample(&mut steps, values, &Names::of("x", &tag), w, full));
                }
                let both = match (stats.first(), stats.get(1)) {
                    (Some(Some(a)), Some(Some(b))) => a.variance.zip(b.variance).map(|(v1, v2)| (a.n, a.mean, v1, b.n, b.mean, v2)),
                    _ => None,
                };
                let Some((n1, m1, v1, n2, m2, v2)) = both else {
                    steps.push(say(&tr!("ex-each-group-needs-two-cases-or-more-for-the-test")));
                    sections.push((heading(&tr!("ex-the-groups")), steps));
                    continue;
                };
                let d = m1 - m2;
                let (g1, g2) = (Names::of("x", "1"), Names::of("x", "2"));
                steps.push(step(
                    &tr!("ex-the-difference-of-the-means"),
                    equals(vec![row(vec![g1.mean.clone(), text("−"), g2.mean.clone()]), row(vec![n(m1), text("−"), n(m2)]), approx(d)]),
                ));
                sections.push((heading(&tr!("ex-the-groups")), steps));
                sections.push((heading(&tr!("ex-levene-s-test-of-equal-variances")), levene_steps(&groups, &[m1, m2], w, full, true)));

                // Equal variances: one variance for both, pooled.
                let mut steps = Vec::new();
                let df = n1 + n2 - 2.0;
                let pooled = ((n1 - 1.0) * v1 + (n2 - 1.0) * v2) / df;
                let s2 = |names: &Names| sup(names.s.clone(), text("2"));
                steps.push(step(
                    &tr!("ex-the-pooled-variance-weighs-each-group-s-variance-by-its-degr"),
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
                    &tr!("ex-the-standard-error-of-the-difference"),
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
                sections.push((heading(&tr!("ex-equal-variances-assumed")), steps));

                // Unequal variances: Welch's standard error and Satterthwaite's degrees of freedom.
                let mut steps = Vec::new();
                let (a, b) = (v1 / n1, v2 / n2);
                steps.extend(several(
                    &tr!("ex-each-group-s-variance-of-its-mean"),
                    vec![
                        equals(vec![var("a"), frac(s2(&g1), g1.n.clone()), frac(n(v1), n(n1)), n(a)]),
                        equals(vec![var("b"), frac(s2(&g2), g2.n.clone()), frac(n(v2), n(n2)), n(b)]),
                    ],
                ));
                let se = (a + b).sqrt();
                steps.push(step(
                    &tr!("ex-the-standard-error-of-the-difference-adds-them-without-pooli"),
                    equals(vec![var("SE"), sqrt(row(vec![var("a"), text("+"), var("b")])), sqrt(row(vec![n(a), text("+"), n(b)])), approx(se)]),
                ));
                let (da, db) = (a * a / (n1 - 1.0), b * b / (n2 - 1.0));
                let df = (a + b).powi(2) / (da + db);
                steps.push(step(
                    &tr!("ex-welch-satterthwaite-s-degrees-of-freedom-usually-not-a-whole"),
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
                sections.push((heading(&tr!("ex-equal-variances-not-assumed")), steps));
            }
        }
        TTest::Paired { pairs } => {
            for (p, (a, b)) in pairs.iter().enumerate() {
                let ((ai, avar), (bi, bvar)) = (numeric(ds, a)?, numeric(ds, b)?);
                let rows: Vec<(f64, f64, f64)> = (0..ds.n_cases())
                    .filter_map(|c| Some((valid(ds, ai, c)?, valid(ds, bi, c)?, ds.case_weight(c))))
                    .filter(|(_, _, w)| *w > 0.0)
                    .collect();
                let pair = tr!("ex-pair-difference", n = p + 1, a = avar.display_name(), b = bvar.display_name());
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
                steps.push(step(&tr!("ex-differences", a = avar.display_name(), b = bvar.display_name()), row(listed)));
                let diffs: Vec<(f64, f64)> = rows.iter().map(|(x, y, w)| (x - y, *w)).collect();
                let names = Names::of("d", "");
                if let Some(Sample { n: size, mean, variance: Some(variance) }) = sample(&mut steps, &diffs, &names, w, full) {
                    let se = (variance / size).sqrt();
                    steps.push(step(
                        &tr!("ex-the-standard-error-of-the-mean-difference"),
                        equals(vec![var("SE"), frac(names.s.clone(), sqrt(names.n.clone())), frac(n(variance.sqrt()), sqrt(n(size))), approx(se)]),
                    ));
                    let df_name = row(vec![names.n.clone(), text("−"), text("1")]);
                    t_and_interval(&mut steps, (names.mean, mean), (var("SE"), se), (df_name, size - 1.0), ci);
                }
                sections.push((pair.clone(), steps));
                let x_and_y = tr!("ex-how-pair", x = avar.display_name(), y = bvar.display_name());
                sections.push((tr!("ex-pair-correlation", n = p + 1), pearson(&rows, w, &x_and_y, false, full, true)));
            }
        }
    }
    Ok(Explanation { title: tr!("ex-title-t-test"), sections })
}

/// Levene's test, based on the mean: a one-way analysis of variance on each value's distance
/// from its group's mean.
/// `t_test` says which row of its table to read.
fn levene_steps(groups: &[Vec<(f64, f64)>], means: &[f64], w: bool, full: bool, t_test: bool) -> Vec<Step> {
    let mut steps = vec![say(&tr!("ex-levene-s-test-asks-whether-the-groups-variances-differ-an-an"))];
    let z_of: Vec<Vec<(f64, f64)>> = groups.iter().zip(means).map(|(g, &m)| g.iter().map(|(x, wi)| ((x - m).abs(), *wi)).collect()).collect();
    let mut z_means = Vec::new();
    for (k, ((g, z), &m)) in groups.iter().zip(&z_of).zip(means).enumerate() {
        let tag = (k + 1).to_string();
        let size: f64 = z.iter().map(|(_, w)| w).sum();
        let total: f64 = z.iter().map(|(z, w)| z * w).sum();
        let distances: Vec<Math> = g.iter().map(|(x, wi)| weighted_term(w, *wi, row(vec![text("|"), val(*x), text("−"), n(m), text("|")]))).collect();
        let zs: Vec<Math> = z.iter().map(|(z, wi)| weighted_term(w, *wi, n(*z))).collect();
        let mean = total / size;
        z_means.push((size, mean));
        steps.extend(several(
            &tr!("ex-group-distances", tag = tag.as_str()),
            vec![
                equals(vec![sum(None, None, sub(var("z"), text(&format!("{tag}i")))), terms(distances, full), terms(zs, full), n(total)]),
                equals(vec![sub(bar(var("z")), text(&tag)), frac(n(total), n(size)), n(mean)]),
            ],
        ));
    }
    let big_n: f64 = z_means.iter().map(|(s, _)| s).sum();
    let grand = z_means.iter().map(|(s, m)| s * m).sum::<f64>() / big_n;
    steps.push(step(
        &tr!("ex-the-mean-of-all-the-distances"),
        equals(vec![bar(var("z")), frac(plus(z_means.iter().map(|(s, m)| row(vec![n(*s), text("·"), n(*m)])).collect()), n(big_n)), n(grand)]),
    ));
    let between: f64 = z_means.iter().map(|(s, m)| s * (m - grand).powi(2)).sum();
    steps.push(step(
        &tr!("ex-between-the-groups-each-group-s-mean-distance-from-that-squa"),
        equals(vec![
            sub(var("SS"), text(&tr!("ex-word-between"))),
            plus(z_means.iter().map(|(s, m)| row(vec![n(*s), text("·"), dev_sq(*m, grand)])).collect()),
            n(between),
        ]),
    ));
    let within_terms: Vec<Math> =
        z_of.iter().zip(&z_means).flat_map(|(z, (_, m))| z.iter().map(move |(z, wi)| weighted_term(w, *wi, dev_sq(*z, *m)))).collect();
    let within: f64 = z_of.iter().zip(&z_means).map(|(z, (_, m))| z.iter().map(|(z, wi)| wi * (z - m).powi(2)).sum::<f64>()).sum();
    steps.push(step(
        &tr!("ex-within-the-groups-each-distance-s-deviation-from-its-group-s"),
        equals(vec![sub(var("SS"), text(&tr!("ex-word-within"))), terms(within_terms, full), n(within)]),
    ));
    let df1 = z_means.len() as f64 - 1.0;
    let df2 = big_n - z_means.len() as f64;
    if df1 <= 0.0 || df2 <= 0.0 || within <= 0.0 {
        steps.push(say(&tr!("ex-with-no-spread-within-the-groups-f-is-undefined")));
        return steps;
    }
    let f = (between / df1) / (within / df2);
    steps.push(step(
        &tr!("ex-f-divides-the-mean-squares-with-k-1-and-n-k-degrees-of-freed"),
        equals(vec![
            var("F"),
            frac(
                row(vec![sub(var("SS"), text(&tr!("ex-word-between"))), text("/"), paren(row(vec![var("k"), text("−"), text("1")]))]),
                row(vec![sub(var("SS"), text(&tr!("ex-word-within"))), text("/"), paren(row(vec![var("N"), text("−"), var("k")]))]),
            ),
            frac(row(vec![n(between), text("/"), n(df1)]), row(vec![n(within), text("/"), n(df2)])),
            approx(f),
        ]),
    ));
    if let Some(p) = dist::f_sig(f, df1, df2) {
        steps.push(step(&tr!("ex-f-significance", df1 = number_text(df1), df2 = number_text(df2)), equals(vec![var("p"), approx(p)])));
        if !t_test {
            return steps;
        }
        steps.push(say(if p > 0.05 {
            "Above 0.05: no reason to doubt equal variances, so the row with equal variances assumed is the one to read."
        } else {
            "0.05 or below: the variances differ, so the row with equal variances not assumed is the one to read."
        }));
    }
    steps
}

/// `ONEWAY`: for each dependent variable, each group's mean and variance (with its interval
/// when Descriptives are asked for), Levene's test when asked for, the analysis of variance,
/// and each post hoc test's comparisons.
pub fn oneway(ds: &Dataset, options: &crate::OnewayOptions, full: bool) -> Result<Explanation, StatsError> {
    let (fi, fvar) = numeric(ds, &options.factor)?;
    let w = weighted(ds);
    let alpha = if options.alpha > 0.0 && options.alpha < 1.0 { options.alpha } else { 0.05 };
    let mut sections = Vec::new();
    for name in &options.dependents {
        let (i, variable) = numeric(ds, name)?;
        // Groups by factor value, ascending, as the tables.
        let mut groups: Vec<(f64, Vec<(f64, f64)>)> = Vec::new();
        for case in 0..ds.n_cases() {
            let weight = ds.case_weight(case);
            let (Some(level), Some(x)) = (valid(ds, fi, case), valid(ds, i, case)) else { continue };
            if weight <= 0.0 {
                continue;
            }
            match groups.iter_mut().find(|(l, _)| *l == level) {
                Some((_, g)) => g.push((x, weight)),
                None => groups.push((level, vec![(x, weight)])),
            }
        }
        groups.sort_by(|a, b| a.0.total_cmp(&b.0));
        let heading = |part: &str| tr!("ex-heading-by", variable = variable.display_name(), group = fvar.display_name(), part = part);
        let mut steps = Vec::new();
        let mut stats: Vec<(f64, f64, f64)> = Vec::new();
        for (k, (level, values)) in groups.iter().enumerate() {
            let tag = (k + 1).to_string();
            steps.push(say(&tr!(
                "ex-group-is",
                tag = tag.as_str(),
                group = fvar.display_name(),
                value = value_text(fvar, &mksigma_core::Value::Num(*level))
            )));
            let names = Names::of("x", &tag);
            if let Some(Sample { n: size, mean, variance }) = sample(&mut steps, values, &names, w, full) {
                let variance = variance.unwrap_or(0.0);
                if options.descriptives && size > 1.0 {
                    interval_steps(&mut steps, &names, size, mean, variance);
                    steps.extend(extremes(values));
                }
                stats.push((size, mean, variance));
            }
        }
        if options.descriptives {
            let all: Vec<(f64, f64)> = groups.iter().flat_map(|(_, g)| g.iter().copied()).collect();
            steps.push(say(&tr!("ex-all-the-groups-together-the-total-row")));
            let names = Names::of("x", "");
            if let Some(Sample { n: size, mean, variance: Some(variance) }) = sample(&mut steps, &all, &names, w, full) {
                interval_steps(&mut steps, &names, size, mean, variance);
                steps.extend(extremes(&all));
            }
        }
        sections.push((heading(&tr!("ex-the-groups")), steps));
        let k = stats.len() as f64;
        let big_n: f64 = stats.iter().map(|(s, _, _)| s).sum();
        if k < 2.0 || big_n <= k {
            sections
                .push((heading(&tr!("ex-the-analysis-of-variance")), vec![say(&tr!("ex-it-needs-two-groups-or-more-and-more-cases-than-groups"))]));
            continue;
        }
        if options.homogeneity {
            let values: Vec<Vec<(f64, f64)>> = groups.iter().map(|(_, g)| g.clone()).collect();
            let means: Vec<f64> = stats.iter().map(|(_, m, _)| *m).collect();
            sections.push((heading(&tr!("ex-levene-s-test-of-equal-variances")), levene_steps(&values, &means, w, full, false)));
        }

        let mut steps = Vec::new();
        let grand = stats.iter().map(|(s, m, _)| s * m).sum::<f64>() / big_n;
        let group_mean = |k: usize| sub(bar(var("x")), text(&(k + 1).to_string()));
        let group_n = |k: usize| sub(var("n"), text(&(k + 1).to_string()));
        steps.push(step(
            &tr!("ex-the-grand-mean-weighs-each-group-s-mean-by-its-size"),
            equals(vec![
                bar(var("x")),
                frac(plus((0..stats.len()).map(|k| row(vec![group_n(k), group_mean(k)])).collect()), var("N")),
                frac(plus(stats.iter().map(|(s, m, _)| row(vec![n(*s), text("·"), n(*m)])).collect()), n(big_n)),
                n(grand),
            ]),
        ));
        let between: f64 = stats.iter().map(|(s, m, _)| s * (m - grand).powi(2)).sum();
        steps.push(step(
            &tr!("ex-between-the-groups-each-group-s-mean-against-the-grand-mean"),
            equals(vec![
                sub(var("SS"), text(&tr!("ex-word-between"))),
                sum(
                    None,
                    None,
                    row(vec![indexed("n", "k"), sup(paren(row(vec![sub(bar(var("x")), var("k")), text("−"), bar(var("x"))])), text("2"))]),
                ),
                plus(stats.iter().map(|(s, m, _)| row(vec![n(*s), text("·"), dev_sq(*m, grand)])).collect()),
                approx(between),
            ]),
        ));
        let within: f64 = stats.iter().map(|(s, _, v)| (s - 1.0) * v).sum();
        steps.push(step(
            &tr!("ex-within-the-groups-each-group-s-squared-deviations-from-its-o"),
            equals(vec![
                sub(var("SS"), text(&tr!("ex-word-within"))),
                sum(None, None, row(vec![paren(row(vec![indexed("n", "k"), text("−"), text("1")])), sup(indexed("s", "k"), text("2"))])),
                plus(stats.iter().map(|(s, _, v)| row(vec![paren(row(vec![n(*s), text("−"), text("1")])), text("·"), n(*v)])).collect()),
                approx(within),
            ]),
        ));
        steps.push(step(
            &tr!("ex-the-total-sum-of-squares-is-the-two-together"),
            equals(vec![sub(var("SS"), text(&tr!("ex-word-total"))), row(vec![n(between), text("+"), n(within)]), approx(between + within)]),
        ));
        let (df1, df2) = (k - 1.0, big_n - k);
        steps.extend(several(
            &tr!("ex-the-degrees-of-freedom-with-k-groups-and-n-cases"),
            vec![
                equals(vec![sub(var("df"), text(&tr!("ex-word-between"))), row(vec![var("k"), text("−"), text("1")]), n(df1)]),
                equals(vec![
                    sub(var("df"), text(&tr!("ex-word-within"))),
                    row(vec![var("N"), text("−"), var("k")]),
                    row(vec![n(big_n), text("−"), n(k)]),
                    n(df2),
                ]),
            ],
        ));
        let (ms_b, ms_w) = (between / df1, within / df2);
        steps.extend(several(
            &tr!("ex-each-mean-square-divides-its-sum-of-squares-by-its-degrees-o"),
            vec![
                equals(vec![sub(var("MS"), text(&tr!("ex-word-between"))), frac(n(between), n(df1)), approx(ms_b)]),
                equals(vec![sub(var("MS"), text(&tr!("ex-word-within"))), frac(n(within), n(df2)), approx(ms_w)]),
            ],
        ));
        let f = ms_b / ms_w;
        steps.push(step(
            &tr!("ex-f-compares-them"),
            equals(vec![
                var("F"),
                frac(sub(var("MS"), text(&tr!("ex-word-between"))), sub(var("MS"), text(&tr!("ex-word-within")))),
                frac(n(ms_b), n(ms_w)),
                approx(f),
            ]),
        ));
        if let Some(p) = dist::f_sig(f, df1, df2) {
            steps.push(step(&tr!("ex-f-significance", df1 = number_text(df1), df2 = number_text(df2)), equals(vec![var("p"), approx(p)])));
        }
        sections.push((heading(&tr!("ex-the-analysis-of-variance")), steps));

        for test in &options.posthoc {
            sections.push((
                heading(match test {
                    crate::PostHoc::Tukey => "Tukey HSD",
                    crate::PostHoc::Bonferroni => "Bonferroni",
                }),
                posthoc_steps(*test, &stats, ms_w, df2, alpha),
            ));
        }
    }
    Ok(Explanation { title: tr!("ex-title-oneway"), sections })
}

/// The smallest and largest of the values.
fn extremes(values: &[(f64, f64)]) -> Vec<Step> {
    let (lo, hi) = values.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), (x, _)| (a.min(*x), b.max(*x)));
    several(
        &tr!("ex-the-smallest-and-largest-values"),
        vec![equals(vec![text(&tr!("ex-word-minimum-cap")), n(lo)]), equals(vec![text(&tr!("ex-word-maximum-cap")), n(hi)])],
    )
}

/// A group's standard error and the 95% interval of its mean, as ONEWAY's Descriptives show.
fn interval_steps(steps: &mut Vec<Step>, names: &Names, size: f64, mean: f64, variance: f64) {
    let se = (variance / size).sqrt();
    steps.push(step(
        &tr!("ex-its-standard-error"),
        equals(vec![var("SE"), frac(names.s.clone(), sqrt(names.n.clone())), frac(n(variance.sqrt()), sqrt(n(size))), approx(se)]),
    ));
    if let Some(critical) = dist::t_quantile(0.975, size - 1.0) {
        let half = critical * se;
        steps.extend(several(
            &tr!("ex-mean-interval", df = number_text(size - 1.0)),
            vec![
                equals(vec![sub(var("t"), text("0.975")), approx(critical)]),
                equals(vec![text(&tr!("ex-word-lower-cap")), row(vec![n(mean), text("−"), n(critical), text("·"), n(se)]), approx(mean - half)]),
                equals(vec![text(&tr!("ex-word-upper-cap")), row(vec![n(mean), text("+"), n(critical), text("·"), n(se)]), approx(mean + half)]),
            ],
        ));
    }
}

/// One post hoc test over every pair of groups (I < J; the table also lists J − I, the same
/// with the signs turned): the difference, its standard error, significance and interval.
fn posthoc_steps(test: crate::PostHoc, stats: &[(f64, f64, f64)], mse: f64, df: f64, alpha: f64) -> Vec<Step> {
    let k = stats.len() as f64;
    let m = k * (k - 1.0) / 2.0;
    let mut steps = Vec::new();
    let critical = match test {
        crate::PostHoc::Tukey => {
            let q = crate::range::quantile(1.0 - alpha, k, df);
            if let Some(q) = q {
                steps.push(step(
                    &tr!("ex-tukey", k = number_text(k), df = number_text(df), alpha = number_text(alpha)),
                    equals(vec![sub(var("q"), text(&number_text(1.0 - alpha))), approx(q)]),
                ));
            }
            q.map(|q| q / std::f64::consts::SQRT_2)
        }
        crate::PostHoc::Bonferroni => {
            let t = dist::t_quantile(1.0 - alpha / (2.0 * m), df);
            steps.push(step(
                &tr!("ex-bonferroni", m = number_text(m), alpha = number_text(alpha)),
                equals(vec![var("m"), frac(row(vec![var("k"), paren(row(vec![var("k"), text("−"), text("1")]))]), text("2")), n(m)]),
            ));
            if let Some(t) = t {
                steps.push(step(&tr!("ex-that-t"), equals(vec![sub(var("t"), text(&tr!("ex-word-crit"))), approx(t)])));
            }
            t
        }
    };
    steps.push(step(
        &tr!("ex-every-comparison-uses-the-mean-square-within-the-groups"),
        equals(vec![
            var("SE"),
            sqrt(row(vec![
                sub(var("MS"), text(&tr!("ex-word-within"))),
                paren(row(vec![frac(text("1"), sub(var("n"), var("I"))), text("+"), frac(text("1"), sub(var("n"), var("J")))])),
            ])),
        ]),
    ));
    for (i, (ni, mi, _)) in stats.iter().enumerate() {
        for (j, (nj, mj, _)) in stats.iter().enumerate().skip(i + 1) {
            let diff = mi - mj;
            let se = (mse * (1.0 / ni + 1.0 / nj)).sqrt();
            let mut maths = vec![
                equals(vec![
                    row(vec![sub(bar(var("x")), text(&(i + 1).to_string())), text("−"), sub(bar(var("x")), text(&(j + 1).to_string()))]),
                    row(vec![n(*mi), text("−"), n(*mj)]),
                    approx(diff),
                ]),
                equals(vec![
                    var("SE"),
                    sqrt(row(vec![n(mse), text("·"), paren(row(vec![frac(text("1"), n(*ni)), text("+"), frac(text("1"), n(*nj))]))])),
                    approx(se),
                ]),
            ];
            match test {
                crate::PostHoc::Tukey => {
                    let q = diff.abs() / se * std::f64::consts::SQRT_2;
                    maths.push(equals(vec![var("q"), frac(row(vec![text("|"), n(diff), text("|"), text("·"), sqrt(text("2"))]), n(se)), approx(q)]));
                    if let Some(p) = crate::range::cdf(q, k, df).map(|p| 1.0 - p) {
                        maths.push(equals(vec![var("p"), approx(p)]));
                    }
                }
                crate::PostHoc::Bonferroni => {
                    let t = diff / se;
                    maths.push(equals(vec![var("t"), frac(n(diff), n(se)), approx(t)]));
                    if let Some(p) = dist::t_sig(t, df) {
                        // A probability: at most 1.
                        let times = row(vec![n(m), text("·"), n(p)]);
                        let times =
                            if p * m > 1.0 { row(vec![text(&tr!("ex-word-min")), paren(row(vec![text("1"), text(","), times]))]) } else { times };
                        maths.push(equals(vec![var("p"), times, approx((p * m).min(1.0))]));
                    }
                }
            }
            if let Some(c) = critical {
                maths.push(equals(vec![
                    text(&tr!("ex-word-lower-cap")),
                    row(vec![n(diff), text("−"), n(c), text("·"), n(se)]),
                    approx(diff - c * se),
                ]));
                maths.push(equals(vec![
                    text(&tr!("ex-word-upper-cap")),
                    row(vec![n(diff), text("+"), n(c), text("·"), n(se)]),
                    approx(diff + c * se),
                ]));
            }
            steps.extend(several(&tr!("ex-groups-pair", i = i + 1, j = j + 1), maths));
        }
    }
    steps
}

/// Each distinct value once, ascending, with its weight and the rank it gets: ties share the
/// mean of the ranks they span.
fn ranked(values: &[(f64, f64)]) -> Vec<(f64, f64, f64)> {
    let mut sorted: Vec<(f64, f64)> = values.to_vec();
    sorted.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut out: Vec<(f64, f64, f64)> = Vec::new();
    for (x, w) in sorted {
        match out.last_mut() {
            Some((v, weight, _)) if *v == x => *weight += w,
            _ => out.push((x, w, 0.0)),
        }
    }
    let mut below = 0.0;
    for (_, w, rank) in out.iter_mut() {
        *rank = below + (*w + 1.0) / 2.0;
        below += *w;
    }
    out
}

/// The rank of `x` among `ranked`.
fn rank_of(ranked: &[(f64, f64, f64)], x: f64) -> f64 {
    ranked.iter().find(|(v, _, _)| *v == x).map_or(0.0, |(_, _, r)| *r)
}

/// The ranks, step by step: each distinct value, and for a tie the mean of the ranks it spans.
fn rank_steps(intro: &str, ranked: &[(f64, f64, f64)], full: bool) -> Vec<Step> {
    let mut below = 0.0;
    let mut items = Vec::new();
    for (k, (x, w, rank)) in ranked.iter().enumerate() {
        if k > 0 {
            items.push(text(",   "));
        }
        let (first, last) = (below + 1.0, below + w);
        items.push(if *w > 1.0 {
            row(vec![val(*x), text("→"), equals(vec![frac(row(vec![n(first), text("+"), n(last)]), text("2")), n(*rank)])])
        } else {
            row(vec![val(*x), text("→"), n(*rank)])
        });
        below += w;
    }
    if !full && items.len() > SHOWN * 2 {
        let count = items.len();
        let mut short: Vec<Math> = items.iter().take(10).cloned().collect();
        short.push(text("⋯"));
        short.extend(items.iter().skip(count - 3).cloned());
        items = short;
    }
    vec![step(intro, row(items))]
}

/// The correction for ties: Σ(t³ − t) over the values that t cases share.
fn ties_step(ranked: &[(f64, f64, f64)], full: bool) -> (Step, f64) {
    let tied: Vec<f64> = ranked.iter().map(|(_, w, _)| *w).filter(|w| *w > 1.0).collect();
    let total: f64 = tied.iter().map(|t| t.powi(3) - t).sum();
    let math = if tied.is_empty() {
        equals(vec![var("T"), text("0")])
    } else {
        equals(vec![
            var("T"),
            sum(None, None, paren(row(vec![sup(var("t"), text("3")), text("−"), var("t")]))),
            terms(tied.iter().map(|t| paren(row(vec![sup(n(*t), text("3")), text("−"), n(*t)]))).collect(), full),
            n(total),
        ])
    };
    let text_of = if tied.is_empty() { tr!("ex-no-ties") } else { tr!("ex-ties") };
    (step(&text_of, math), total)
}

/// The significance of `z` from the standard normal distribution, two-tailed.
fn z_significance(z: f64) -> Option<Step> {
    let p = crate::npar::two_sided(z)?;
    Some(step(&tr!("ex-its-significance-two-tailed-is-the-probability-of-a-z-at-lea"), equals(vec![var("p"), approx(p)])))
}

/// A sum of ranks, term by term, and the mean rank: the sum over the cases.
fn rank_sum(name: Math, size_name: Math, ranks: Vec<(f64, f64)>, full: bool) -> (Math, f64) {
    let total: f64 = ranks.iter().map(|(r, w)| r * w).sum();
    let size: f64 = ranks.iter().map(|(_, w)| w).sum();
    let each: Vec<Math> = ranks.iter().map(|(r, w)| if *w == 1.0 { n(*r) } else { row(vec![n(*w), text("·"), n(*r)]) }).collect();
    let sum_math = equals(vec![name.clone(), terms(each, full), n(total)]);
    if size <= 0.0 {
        return (sum_math, total);
    }
    let mean = equals(vec![bar(name), frac(n(total), n(size)), approx_to(total / size, 2)]);
    (row(vec![equals(vec![size_name, n(size)]), text(",   "), sum_math, text(",   "), mean]), total)
}

/// `NPAR TESTS`: Mann-Whitney, Wilcoxon signed ranks and Kruskal-Wallis, rank by rank.
pub fn npar_tests(ds: &Dataset, tests: &[crate::NparTest], full: bool) -> Result<Explanation, StatsError> {
    use crate::NparTest;
    let mut sections = Vec::new();
    for test in tests {
        match test {
            NparTest::MannWhitney { variables, group, first, second } => {
                let (gi, gvar) = numeric(ds, group)?;
                for name in variables {
                    let (vi, variable) = numeric(ds, name)?;
                    let data = crate::npar::group_values(ds, vi, gi, |g| g == *first || g == *second);
                    let ranked = ranked(&data.iter().map(|(x, w, _)| (*x, *w)).collect::<Vec<_>>());
                    let label = |g: f64| value_text(gvar, &mksigma_core::Value::Num(g));
                    let mut steps =
                        rank_steps(&tr!("ex-rank-both", group = gvar.display_name(), first = label(*first), second = label(*second)), &ranked, full);
                    let group_ranks =
                        |g: f64| data.iter().filter(|(_, _, gg)| *gg == g).map(|(x, w, _)| (rank_of(&ranked, *x), *w)).collect::<Vec<_>>();
                    let (r1_math, r1) = rank_sum(sub(var("R"), text("1")), sub(var("n"), text("1")), group_ranks(*first), full);
                    let (r2_math, r2) = rank_sum(sub(var("R"), text("2")), sub(var("n"), text("2")), group_ranks(*second), full);
                    let size = |g: f64| data.iter().filter(|(_, _, gg)| *gg == g).map(|(_, w, _)| w).sum::<f64>();
                    let (n1, n2) = (size(*first), size(*second));
                    let big_n = n1 + n2;
                    steps.extend(several(
                        &tr!("ex-rank-groups-two", first = label(*first), second = label(*second)),
                        vec![r1_math, r2_math, equals(vec![var("N"), row(vec![n(n1), text("+"), n(n2)]), n(big_n)])],
                    ));
                    let u1 = r1 - n1 * (n1 + 1.0) / 2.0;
                    let u2 = n1 * n2 - u1;
                    steps.extend(several(
                        &tr!("ex-u-counts-the-pairs-one-case-from-each-group-in-which-group-1"),
                        vec![
                            equals(vec![
                                sub(var("U"), text("1")),
                                row(vec![
                                    sub(var("R"), text("1")),
                                    text("−"),
                                    frac(
                                        row(vec![sub(var("n"), text("1")), paren(row(vec![sub(var("n"), text("1")), text("+"), text("1")]))]),
                                        text("2"),
                                    ),
                                ]),
                                row(vec![n(r1), text("−"), frac(row(vec![n(n1), text("·"), n(n1 + 1.0)]), text("2"))]),
                                n(u1),
                            ]),
                            equals(vec![
                                sub(var("U"), text("2")),
                                row(vec![sub(var("n"), text("1")), sub(var("n"), text("2")), text("−"), sub(var("U"), text("1"))]),
                                row(vec![n(n1), text("·"), n(n2), text("−"), n(u1)]),
                                n(u2),
                            ]),
                        ],
                    ));
                    let (u, w) = if u1 <= u2 { (u1, r1) } else { (u2, r2) };
                    steps.extend(several(
                        &tr!("ex-mann-whitney-s-u-and-wilcoxon-s-w-the-sum-of-ranks-of-the-gr"),
                        vec![equals(vec![var("U"), n(u)]), equals(vec![var("W"), n(w)])],
                    ));
                    let (ties, t) = ties_step(&ranked, full);
                    steps.push(ties);
                    let variance = n1 * n2 / (big_n * (big_n - 1.0)) * ((big_n.powi(3) - big_n) / 12.0 - t / 12.0);
                    steps.push(step(
                        &tr!("ex-u-s-variance-with-the-ties-taken-out"),
                        equals(vec![
                            sup(sub(var("σ"), var("U")), text("2")),
                            row(vec![
                                frac(
                                    row(vec![sub(var("n"), text("1")), sub(var("n"), text("2"))]),
                                    row(vec![var("N"), paren(row(vec![var("N"), text("−"), text("1")]))]),
                                ),
                                paren(row(vec![
                                    frac(row(vec![sup(var("N"), text("3")), text("−"), var("N")]), text("12")),
                                    text("−"),
                                    frac(var("T"), text("12")),
                                ])),
                            ]),
                            row(vec![
                                frac(row(vec![n(n1), text("·"), n(n2)]), row(vec![n(big_n), text("·"), n(big_n - 1.0)])),
                                paren(row(vec![
                                    frac(row(vec![sup(n(big_n), text("3")), text("−"), n(big_n)]), text("12")),
                                    text("−"),
                                    frac(n(t), text("12")),
                                ])),
                            ]),
                            n(variance),
                        ]),
                    ));
                    let z = (u - n1 * n2 / 2.0) / variance.sqrt();
                    if z.is_finite() {
                        steps.push(step(
                            &tr!("ex-z-measures-u-s-distance-from-its-expected-value-half-the-pro"),
                            equals(vec![
                                var("Z"),
                                frac(
                                    row(vec![var("U"), text("−"), frac(row(vec![sub(var("n"), text("1")), sub(var("n"), text("2"))]), text("2"))]),
                                    sqrt(sup(sub(var("σ"), var("U")), text("2"))),
                                ),
                                frac(row(vec![n(u), text("−"), n(n1 * n2 / 2.0)]), sqrt(n(variance))),
                                approx(z),
                            ]),
                        ));
                        steps.extend(z_significance(z));
                    }
                    sections.push((tr!("ex-heading-mann-whitney", variable = variable.display_name(), group = gvar.display_name()), steps));
                }
            }
            NparTest::Wilcoxon { pairs } => {
                for (a, b) in pairs {
                    let ((ai, avar), (bi, bvar)) = (numeric(ds, a)?, numeric(ds, b)?);
                    let rows: Vec<(f64, f64, f64)> = (0..ds.n_cases())
                        .filter_map(|c| {
                            let w = ds.case_weight(c);
                            (w > 0.0).then_some((valid(ds, ai, c)?, valid(ds, bi, c)?, w))
                        })
                        .collect();
                    let mut listed = Vec::new();
                    for (k, (x, y, _)) in rows.iter().enumerate() {
                        if k > 0 {
                            listed.push(text(",   "));
                        }
                        listed.push(equals(vec![row(vec![val(*y), text("−"), val(*x)]), val(y - x)]));
                    }
                    if !full && listed.len() > SHOWN * 2 {
                        let count = listed.len();
                        let mut short: Vec<Math> = listed.iter().take(10).cloned().collect();
                        short.push(text("⋯"));
                        short.extend(listed.iter().skip(count - 3).cloned());
                        listed = short;
                    }
                    let mut steps = vec![step(&tr!("ex-wilcoxon-differences", a = bvar.display_name(), b = avar.display_name()), row(listed))];
                    let nonzero: Vec<(f64, f64)> = rows.iter().map(|(x, y, w)| (y - x, *w)).filter(|(d, _)| *d != 0.0).collect();
                    let ranked = ranked(&nonzero.iter().map(|(d, w)| (d.abs(), *w)).collect::<Vec<_>>());
                    steps.extend(rank_steps(&tr!("ex-rank-sizes"), &ranked, full));
                    let side = |negative: bool| {
                        nonzero.iter().filter(|(d, _)| (*d < 0.0) == negative).map(|(d, w)| (rank_of(&ranked, d.abs()), *w)).collect::<Vec<_>>()
                    };
                    let (neg_math, tn) = rank_sum(sub(var("T"), text("−")), sub(var("n"), text("−")), side(true), full);
                    let (pos_math, tp) = rank_sum(sub(var("T"), text("+")), sub(var("n"), text("+")), side(false), full);
                    let big_n: f64 = nonzero.iter().map(|(_, w)| w).sum();
                    let zero: f64 = rows.iter().filter(|(x, y, _)| y == x).map(|(_, _, w)| w).sum();
                    steps.extend(several(
                        &tr!("ex-how-many-differences-are-negative-and-positive-their-sums-of"),
                        vec![
                            neg_math,
                            pos_math,
                            equals(vec![text(&tr!("ex-word-ties-cap")), n(zero)]),
                            equals(vec![text(&tr!("ex-word-total-cap")), row(vec![n(big_n), text("+"), n(zero)]), n(big_n + zero)]),
                        ],
                    ));
                    let (ties, t) = ties_step(&ranked, full);
                    steps.push(ties);
                    let variance = big_n * (big_n + 1.0) * (2.0 * big_n + 1.0) / 24.0 - t / 48.0;
                    steps.push(step(
                        &tr!("ex-wilcoxon-variance", n = number_text(big_n)),
                        equals(vec![
                            sup(var("σ"), text("2")),
                            row(vec![
                                frac(
                                    row(vec![
                                        var("n"),
                                        paren(row(vec![var("n"), text("+"), text("1")])),
                                        paren(row(vec![text("2"), var("n"), text("+"), text("1")])),
                                    ]),
                                    text("24"),
                                ),
                                text("−"),
                                frac(var("T"), text("48")),
                            ]),
                            row(vec![
                                frac(row(vec![n(big_n), text("·"), n(big_n + 1.0), text("·"), n(2.0 * big_n + 1.0)]), text("24")),
                                text("−"),
                                frac(n(t), text("48")),
                            ]),
                            n(variance),
                        ]),
                    ));
                    let smaller = tn.min(tp);
                    let z = (smaller - big_n * (big_n + 1.0) / 4.0) / variance.sqrt();
                    if z.is_finite() {
                        steps.push(step(
                            &tr!("ex-z-measures-the-smaller-sum-s-distance-from-its-expected-valu"),
                            equals(vec![
                                var("Z"),
                                frac(
                                    row(vec![
                                        text(&tr!("ex-word-min")),
                                        paren(row(vec![sub(var("T"), text("−")), text(","), sub(var("T"), text("+"))])),
                                        text("−"),
                                        frac(row(vec![var("n"), paren(row(vec![var("n"), text("+"), text("1")]))]), text("4")),
                                    ]),
                                    var("σ"),
                                ),
                                frac(row(vec![n(smaller), text("−"), n(big_n * (big_n + 1.0) / 4.0)]), sqrt(n(variance))),
                                approx(z),
                            ]),
                        ));
                        steps.extend(z_significance(z));
                    }
                    sections.push((tr!("ex-heading-wilcoxon", a = bvar.display_name(), b = avar.display_name()), steps));
                }
            }
            NparTest::KruskalWallis { variables, group, low, high } => {
                let (gi, gvar) = numeric(ds, group)?;
                for name in variables {
                    let (vi, variable) = numeric(ds, name)?;
                    let data = crate::npar::group_values(ds, vi, gi, |g| *low <= g && g <= *high);
                    let ranked = ranked(&data.iter().map(|(x, w, _)| (*x, *w)).collect::<Vec<_>>());
                    let mut steps = rank_steps(
                        &tr!("ex-rank-every", group = gvar.display_name(), low = number_text(*low), high = number_text(*high)),
                        &ranked,
                        full,
                    );
                    let mut levels: Vec<f64> = data.iter().map(|(_, _, g)| *g).collect();
                    levels.sort_by(f64::total_cmp);
                    levels.dedup();
                    let mut groups = Vec::new();
                    let mut sums = Vec::new();
                    for (k, level) in levels.iter().enumerate() {
                        let ranks: Vec<(f64, f64)> = data.iter().filter(|(_, _, g)| g == level).map(|(x, w, _)| (rank_of(&ranked, *x), *w)).collect();
                        let size: f64 = ranks.iter().map(|(_, w)| w).sum();
                        let tag = (k + 1).to_string();
                        let (math, total) = rank_sum(sub(var("R"), text(&tag)), sub(var("n"), text(&tag)), ranks, full);
                        sums.push(math);
                        groups.push((size, total));
                    }
                    let names: Vec<String> = levels
                        .iter()
                        .enumerate()
                        .map(|(k, l)| tr!("ex-group-label", n = k + 1, value = value_text(gvar, &mksigma_core::Value::Num(*l))))
                        .collect();
                    let big_n: f64 = groups.iter().map(|(s, _)| s).sum();
                    sums.push(equals(vec![var("N"), plus(groups.iter().map(|(s, _)| n(*s)).collect()), n(big_n)]));
                    steps.extend(several(&tr!("ex-rank-groups", groups = names.join(", ")), sums));
                    let inner: f64 = groups.iter().map(|(s, r)| r * r / s).sum();
                    let raw = 12.0 / (big_n * (big_n + 1.0)) * inner - 3.0 * (big_n + 1.0);
                    steps.push(step(
                        &tr!("ex-h-before-the-correction-for-ties"),
                        equals(vec![
                            var("H"),
                            row(vec![
                                frac(text("12"), row(vec![var("N"), paren(row(vec![var("N"), text("+"), text("1")]))])),
                                sum(None, None, frac(sup(indexed("R", "k"), text("2")), indexed("n", "k"))),
                                text("−"),
                                text("3"),
                                paren(row(vec![var("N"), text("+"), text("1")])),
                            ]),
                            row(vec![
                                frac(text("12"), row(vec![n(big_n), text("·"), n(big_n + 1.0)])),
                                paren(plus(groups.iter().map(|(s, r)| frac(sup(n(*r), text("2")), n(*s))).collect())),
                                text("−"),
                                text("3"),
                                text("·"),
                                n(big_n + 1.0),
                            ]),
                            n(raw),
                        ]),
                    ));
                    let (ties, t) = ties_step(&ranked, full);
                    steps.push(ties);
                    let correction = 1.0 - t / (big_n.powi(3) - big_n);
                    let h = raw / correction;
                    let df = levels.len() as f64 - 1.0;
                    steps.push(step(
                        &tr!("ex-corrected-for-ties"),
                        equals(vec![
                            var("H"),
                            frac(var("H"), row(vec![text("1"), text("−"), frac(var("T"), row(vec![sup(var("N"), text("3")), text("−"), var("N")]))])),
                            frac(n(raw), row(vec![text("1"), text("−"), frac(n(t), row(vec![sup(n(big_n), text("3")), text("−"), n(big_n)]))])),
                            approx(h),
                        ]),
                    ));
                    if h.is_finite() && df >= 1.0 {
                        steps.push(step(&tr!("ex-its-degrees-of-freedom-the-groups-minus-1"), equals(vec![var("df"), n(df)])));
                        if let Some(p) = dist::chi_square_sig(h, df) {
                            steps.push(step(&tr!("ex-chi-significance-df", df = number_text(df)), equals(vec![var("p"), approx(p)])));
                        }
                    }
                    sections.push((tr!("ex-heading-kruskal-wallis", variable = variable.display_name(), group = gvar.display_name()), steps));
                }
            }
        }
    }
    Ok(Explanation { title: tr!("ex-title-npar-tests"), sections })
}

/// `EXAMINE`'s Descriptives for each variable (and group of `by`): the mean with its standard
/// error and interval, the 5% trimmed mean, the median and the interquartile range by SPSS's
/// weighted average at (W + 1)p, the variance and standard deviation, the extremes, and the
/// skewness and kurtosis with their standard errors.
pub fn examine(ds: &Dataset, variables: &[String], by: Option<&str>, full: bool) -> Result<Explanation, StatsError> {
    let by = by.map(|b| ds.variable(b).ok_or_else(|| StatsError::UnknownVariable(b.to_string()))).transpose()?;
    let w = weighted(ds);
    let mut sections = Vec::new();
    for name in variables {
        let (vi, variable) = numeric(ds, name)?;
        let (groups, _) = crate::examine::groups_of(ds, vi, by.map(|(bi, _)| bi));
        for (key, values) in groups {
            let heading = match (&key, by) {
                (Some(v), Some((_, bvar))) => {
                    tr!("ex-heading-group", variable = variable.display_name(), group = bvar.display_name(), value = value_text(bvar, v))
                }
                _ => variable.display_name().to_string(),
            };
            let sorted = crate::examine::Sorted::new(values);
            sections.push((heading, examine_steps(&sorted, w, full)));
        }
    }
    Ok(Explanation { title: tr!("ex-title-explore"), sections })
}

fn examine_steps(sorted: &crate::examine::Sorted, w: bool, full: bool) -> Vec<Step> {
    let mut steps = Vec::new();
    let names = Names::of("x", "");
    let Some(Sample { n: big_w, mean, variance }) = sample(&mut steps, &sorted.0, &names, w, full) else { return steps };
    if let Some(variance) = variance {
        let se = (variance / big_w).sqrt();
        steps.push(step(
            &tr!("ex-the-standard-error-of-the-mean"),
            equals(vec![var("SE"), frac(var("s"), sqrt(var("n"))), frac(n(variance.sqrt()), sqrt(n(big_w))), approx(se)]),
        ));
        if let Some(critical) = dist::t_quantile(0.975, big_w - 1.0) {
            let half = critical * se;
            steps.extend(several(
                &format!(
                    "The 95% interval of the mean, with the t that leaves 2.5% in each tail at {} degrees of freedom:",
                    number_text(big_w - 1.0)
                ),
                vec![
                    equals(vec![sub(var("t"), text("0.975")), approx(critical)]),
                    equals(vec![text(&tr!("ex-word-lower-cap")), row(vec![n(mean), text("−"), n(critical), text("·"), n(se)]), approx(mean - half)]),
                    equals(vec![text(&tr!("ex-word-upper-cap")), row(vec![n(mean), text("+"), n(critical), text("·"), n(se)]), approx(mean + half)]),
                ],
            ));
        }
    }

    // The 5% trimmed mean: 5% of the weight off each end, a value at a cut keeping its share.
    let cut = big_w * 0.05;
    let mut below = 0.0;
    let mut kept: Vec<(f64, f64)> = Vec::new();
    for (x, wi) in &sorted.0 {
        let (start, end) = (below, below + wi);
        below = end;
        let keep = (end.min(big_w - cut) - start.max(cut)).max(0.0);
        if keep > 0.0 {
            kept.push((*x, keep));
        }
    }
    let kept_w: f64 = kept.iter().map(|(_, k)| k).sum();
    let kept_sum: f64 = kept.iter().map(|(x, k)| x * k).sum();
    steps.push(step(
        &tr!("ex-the-5-trimmed-mean-leaves-out-5-of-the-cases-at-each-end-a-c"),
        equals(vec![var("c"), row(vec![text("0.05"), text("·"), n(big_w)]), n(cut)]),
    ));
    steps.push(step(
        &tr!("ex-what-remains-each-value-times-the-part-of-it-kept-over-the-p"),
        equals(vec![
            frac(
                terms(kept.iter().map(|(x, k)| row(vec![n(*k), text("·"), val(*x)])).collect(), full),
                terms(kept.iter().map(|(_, k)| n(*k)).collect(), full),
            ),
            frac(n(kept_sum), n(kept_w)),
            approx(kept_sum / kept_w),
        ]),
    ));

    // Percentiles: the weighted average at (W + 1)p.
    let percentile = |p: f64, name: Math, label: &str, steps: &mut Vec<Step>| -> f64 {
        let value = sorted.percentile(p);
        let pos = (big_w + 1.0) * p;
        let at = equals(vec![
            row(vec![paren(row(vec![var("W"), text("+"), text("1")])), text("·"), num(p, 2)]),
            row(vec![n(big_w + 1.0), text("·"), num(p, 2)]),
            n(pos),
        ]);
        if pos <= 1.0 || pos >= big_w {
            let text_of = if pos <= 1.0 { tr!("ex-percentile-first", what = label) } else { tr!("ex-percentile-last", what = label) };
            steps.extend(several(&text_of, vec![at, equals(vec![name, n(value)])]));
        } else {
            let k = pos.floor();
            let (a, b) = (sorted.at_rank(k), sorted.at_rank(k + 1.0));
            steps.extend(several(
                &tr!("ex-percentile-between", what = label, k = number_text(k), next = number_text(k + 1.0), a = number_text(a), b = number_text(b)),
                vec![
                    at,
                    equals(vec![
                        name,
                        row(vec![val(a), text("+"), n(pos - k), text("·"), paren(row(vec![val(b), text("−"), val(a)]))]),
                        approx(value),
                    ]),
                ],
            ));
        }
        value
    };
    percentile(0.5, text(&tr!("ex-word-median-cap")), &tr!("ex-median"), &mut steps);
    let q1 = percentile(0.25, sub(var("Q"), text("1")), &tr!("ex-first-quartile"), &mut steps);
    let q3 = percentile(0.75, sub(var("Q"), text("3")), &tr!("ex-third-quartile"), &mut steps);
    steps.push(step(
        &tr!("ex-the-interquartile-range"),
        equals(vec![
            text("IQR"),
            row(vec![sub(var("Q"), text("3")), text("−"), sub(var("Q"), text("1"))]),
            row(vec![n(q3), text("−"), val(q1)]),
            approx(q3 - q1),
        ]),
    ));
    steps.extend(extremes(&sorted.0));
    if let (Some(lo), Some(hi)) = (sorted.0.first(), sorted.0.last()) {
        steps.push(step(
            &tr!("ex-the-range"),
            equals(vec![text(&tr!("ex-word-range-cap")), row(vec![n(hi.0), text("−"), val(lo.0)]), approx(hi.0 - lo.0)]),
        ));
    }

    // Skewness and kurtosis: the bias-corrected G1 and G2, from the third and fourth moments.
    let Some(variance) = variance else { return steps };
    let sd = variance.sqrt();
    if big_w > 2.0 {
        let cubes: Vec<Math> =
            sorted.0.iter().map(|(x, wi)| weighted_term(w, *wi, sup(paren(row(vec![val(*x), text("−"), n(mean)])), text("3")))).collect();
        let m3: f64 = sorted.0.iter().map(|(x, wi)| wi * (x - mean).powi(3)).sum();
        steps.push(step(
            &tr!("ex-the-cubed-deviations-from-the-mean-added-up"),
            equals(vec![sum(None, None, sup(paren(row(vec![indexed("x", "i"), text("−"), bar(var("x"))])), text("3"))), terms(cubes, full), n(m3)]),
        ));
        let g1 = big_w / ((big_w - 1.0) * (big_w - 2.0)) * m3 / sd.powi(3);
        steps.push(step(
            &tr!("ex-the-skewness"),
            equals(vec![
                sub(var("G"), text("1")),
                row(vec![
                    frac(var("W"), row(vec![paren(row(vec![var("W"), text("−"), text("1")])), paren(row(vec![var("W"), text("−"), text("2")]))])),
                    text("·"),
                    frac(sum(None, None, sup(paren(row(vec![indexed("x", "i"), text("−"), bar(var("x"))])), text("3"))), sup(var("s"), text("3"))),
                ]),
                row(vec![frac(n(big_w), row(vec![n(big_w - 1.0), text("·"), n(big_w - 2.0)])), text("·"), frac(n(m3), sup(n(sd), text("3")))]),
                approx(g1),
            ]),
        ));
        let se1 = (6.0 * big_w * (big_w - 1.0) / ((big_w - 2.0) * (big_w + 1.0) * (big_w + 3.0))).sqrt();
        steps.push(step(
            &tr!("ex-its-standard-error"),
            equals(vec![
                sub(var("SE"), sub(var("G"), text("1"))),
                sqrt(frac(
                    row(vec![text("6"), var("W"), paren(row(vec![var("W"), text("−"), text("1")]))]),
                    row(vec![
                        paren(row(vec![var("W"), text("−"), text("2")])),
                        paren(row(vec![var("W"), text("+"), text("1")])),
                        paren(row(vec![var("W"), text("+"), text("3")])),
                    ]),
                )),
                sqrt(frac(
                    row(vec![text("6"), text("·"), n(big_w), text("·"), n(big_w - 1.0)]),
                    row(vec![n(big_w - 2.0), text("·"), n(big_w + 1.0), text("·"), n(big_w + 3.0)]),
                )),
                approx(se1),
            ]),
        ));
        if big_w > 3.0 {
            let fourths: Vec<Math> =
                sorted.0.iter().map(|(x, wi)| weighted_term(w, *wi, sup(paren(row(vec![val(*x), text("−"), n(mean)])), text("4")))).collect();
            let m4: f64 = sorted.0.iter().map(|(x, wi)| wi * (x - mean).powi(4)).sum();
            steps.push(step(
                &tr!("ex-the-fourth-powers-of-the-deviations-added-up"),
                equals(vec![
                    sum(None, None, sup(paren(row(vec![indexed("x", "i"), text("−"), bar(var("x"))])), text("4"))),
                    terms(fourths, full),
                    n(m4),
                ]),
            ));
            let a = big_w * (big_w + 1.0) / ((big_w - 1.0) * (big_w - 2.0) * (big_w - 3.0));
            let b = 3.0 * (big_w - 1.0).powi(2) / ((big_w - 2.0) * (big_w - 3.0));
            let g2 = a * m4 / sd.powi(4) - b;
            steps.push(step(
                &tr!("ex-the-kurtosis"),
                equals(vec![
                    sub(var("G"), text("2")),
                    row(vec![
                        frac(
                            row(vec![var("W"), paren(row(vec![var("W"), text("+"), text("1")]))]),
                            row(vec![
                                paren(row(vec![var("W"), text("−"), text("1")])),
                                paren(row(vec![var("W"), text("−"), text("2")])),
                                paren(row(vec![var("W"), text("−"), text("3")])),
                            ]),
                        ),
                        text("·"),
                        frac(
                            sum(None, None, sup(paren(row(vec![indexed("x", "i"), text("−"), bar(var("x"))])), text("4"))),
                            sup(var("s"), text("4")),
                        ),
                        text("−"),
                        frac(
                            row(vec![text("3"), sup(paren(row(vec![var("W"), text("−"), text("1")])), text("2"))]),
                            row(vec![paren(row(vec![var("W"), text("−"), text("2")])), paren(row(vec![var("W"), text("−"), text("3")]))]),
                        ),
                    ]),
                    row(vec![n(a), text("·"), frac(n(m4), sup(n(sd), text("4"))), text("−"), n(b)]),
                    approx(g2),
                ]),
            ));
            let se2 = (4.0 * (big_w * big_w - 1.0) * se1 * se1 / ((big_w - 3.0) * (big_w + 5.0))).sqrt();
            steps.push(step(
                &tr!("ex-its-standard-error"),
                equals(vec![
                    sub(var("SE"), sub(var("G"), text("2"))),
                    sqrt(frac(
                        row(vec![
                            text("4"),
                            paren(row(vec![sup(var("W"), text("2")), text("−"), text("1")])),
                            sup(sub(var("SE"), sub(var("G"), text("1"))), text("2")),
                        ]),
                        row(vec![paren(row(vec![var("W"), text("−"), text("3")])), paren(row(vec![var("W"), text("+"), text("5")]))]),
                    )),
                    sqrt(frac(
                        row(vec![text("4"), text("·"), n(big_w * big_w - 1.0), text("·"), sup(n(se1), text("2"))]),
                        row(vec![n(big_w - 3.0), text("·"), n(big_w + 5.0)]),
                    )),
                    approx(se2),
                ]),
            ));
        }
    }
    steps
}

/// A matrix of numbers.
fn numbers(m: &[Vec<f64>]) -> Vec<Vec<Math>> {
    m.iter().map(|r| r.iter().map(|x| n(*x)).collect()).collect()
}

/// `[A | B]` of numbers.
fn side_by_side(a: &[Vec<f64>], b: &[Vec<f64>]) -> Math {
    let rows = a.iter().zip(b).map(|(ra, rb)| ra.iter().chain(rb).map(|x| n(*x)).collect()).collect();
    augmented(rows, a.first().map_or(0, Vec::len))
}

/// The inverse of `m` by Gauss–Jordan elimination, as the engine does it (the largest pivot of
/// each column first), with the augmented matrix after each column. `None` when a pivot
/// vanishes: a predictor that is a combination of the others.
fn gauss_jordan_steps(m: &[Vec<f64>], steps: &mut Vec<Step>) -> Option<Vec<Vec<f64>>> {
    let size = m.len();
    let mut a: Vec<Vec<f64>> = m.to_vec();
    let mut inv: Vec<Vec<f64>> = (0..size).map(|i| (0..size).map(|j| if i == j { 1.0 } else { 0.0 }).collect()).collect();
    let scale: Vec<f64> = (0..size).map(|i| m.get(i).and_then(|r| r.get(i)).copied().unwrap_or(0.0).abs()).collect();
    steps.push(step(&tr!("ex-its-inverse-by-gauss-jordan-elimination-beside-it-the-identi"), side_by_side(&a, &inv)));
    let at = |a: &Vec<Vec<f64>>, r: usize, c: usize| a.get(r).and_then(|row| row.get(c)).copied().unwrap_or(0.0);
    for col in 0..size {
        let pivot_row = (col..size).max_by(|&x, &y| at(&a, x, col).abs().total_cmp(&at(&a, y, col).abs()))?;
        let pivot = at(&a, pivot_row, col);
        if pivot.abs() <= 1e-10 * scale.get(col).copied().unwrap_or(0.0).max(f64::MIN_POSITIVE) {
            steps.push(say(&tr!("ex-a-pivot-is-0-a-predictor-is-a-combination-of-the-others-and")));
            return None;
        }
        let mut what = Vec::new();
        if pivot_row != col {
            a.swap(col, pivot_row);
            inv.swap(col, pivot_row);
            what.push(tr!("ex-gj-swap", a = col + 1, b = pivot_row + 1));
        }
        for j in 0..size {
            if let Some(x) = a.get_mut(col).and_then(|r| r.get_mut(j)) {
                *x /= pivot;
            }
            if let Some(x) = inv.get_mut(col).and_then(|r| r.get_mut(j)) {
                *x /= pivot;
            }
        }
        what.push(tr!("ex-gj-divide", row = col + 1, by = number_text(pivot)));
        for row in 0..size {
            let factor = at(&a, row, col);
            if row == col || factor == 0.0 {
                continue;
            }
            for j in 0..size {
                let (ac, ic) = (at(&a, col, j), at(&inv, col, j));
                if let Some(x) = a.get_mut(row).and_then(|r| r.get_mut(j)) {
                    *x -= factor * ac;
                }
                if let Some(x) = inv.get_mut(row).and_then(|r| r.get_mut(j)) {
                    *x -= factor * ic;
                }
            }
            what.push(tr!("ex-gj-subtract", factor = number_text(factor), from = col + 1, row = row + 1));
        }
        steps.push(step(&tr!("ex-gj-column", n = col + 1, what = what.join("; ")), side_by_side(&a, &inv)));
    }
    Some(inv)
}

/// `REGRESSION`: least squares on the deviations from the means, as the engine computes it: the
/// cross-products, the slopes from `Sxx b = Sxy` (Sxx inverted by Gauss–Jordan with two
/// predictors or more), the constant, the sums of squares, F, R², and each coefficient's
/// standard error, t, significance, standardized beta and interval.
pub fn regression(ds: &Dataset, options: &crate::RegressionOptions, full: bool) -> Result<Explanation, StatsError> {
    let (yi, yvar) = numeric(ds, &options.dependent)?;
    let xs = options.predictors.iter().map(|name| numeric(ds, name)).collect::<Result<Vec<_>, _>>()?;
    let p = xs.len();
    let cases: Vec<(f64, Vec<f64>, f64)> = (0..ds.n_cases())
        .filter_map(|c| {
            let w = ds.case_weight(c);
            let y = valid(ds, yi, c)?;
            let x = xs.iter().map(|(i, _)| valid(ds, *i, c)).collect::<Option<Vec<f64>>>()?;
            (w > 0.0).then_some((y, x, w))
        })
        .collect();
    let at = |m: &[Vec<f64>], j: usize, k: usize| m.get(j).and_then(|r| r.get(k)).copied().unwrap_or(0.0);
    let item = |v: &[f64], j: usize| v.get(j).copied().unwrap_or(0.0);
    let w = weighted(ds);
    let mut steps = Vec::new();
    let big_n: f64 = cases.iter().map(|(_, _, w)| w).sum();
    let legend: Vec<String> = xs.iter().enumerate().map(|(j, (_, v))| tr!("ex-x-is", n = j + 1, name = v.display_name())).collect();
    steps.push(step(
        &if w {
            tr!("ex-regression-cases-weighted", y = yvar.display_name(), predictors = legend.join(", "))
        } else {
            tr!("ex-regression-cases", y = yvar.display_name(), predictors = legend.join(", "))
        },
        equals(vec![var("N"), n(big_n)]),
    ));
    let df_residual = big_n - p as f64 - 1.0;
    if p == 0 || df_residual <= 0.0 {
        steps.push(say(&tr!("ex-it-needs-more-cases-than-predictors-plus-one")));
        return Ok(Explanation { title: tr!("ex-title-regression"), sections: vec![(tr!("ex-the-model"), steps)] });
    }
    let x_name = |j: usize| if p == 1 { var("x") } else { sub(var("x"), text(&(j + 1).to_string())) };
    let column = |j: usize| cases.iter().map(|(_, x, w)| (x.get(j).copied().unwrap_or(0.0), *w)).collect::<Vec<_>>();
    let ys: Vec<(f64, f64)> = cases.iter().map(|(y, _, w)| (*y, *w)).collect();
    // The means.
    let mean_of = |values: &[(f64, f64)], name: Math| -> (Math, f64) {
        let total: f64 = values.iter().map(|(x, w)| x * w).sum();
        let mean = total / big_n;
        let each: Vec<Math> = values.iter().map(|(x, wi)| weighted_term(w, *wi, val(*x))).collect();
        (equals(vec![bar(name), frac(terms(each, full), n(big_n)), frac(n(total), n(big_n)), n(mean)]), mean)
    };
    let (y_math, my) = mean_of(&ys, var("y"));
    let mut means = vec![y_math];
    let mut mx = Vec::new();
    for j in 0..p {
        let (math, m) = mean_of(&column(j), x_name(j));
        means.push(math);
        mx.push(m);
    }
    steps.extend(several(&tr!("ex-the-means"), means));
    let mut sections = vec![(tr!("ex-the-data"), steps)];

    // The cross-products of the deviations.
    let mut steps = Vec::new();
    let product = |a: &[(f64, f64)], ma: f64, b: &[(f64, f64)], mb: f64, name: Math| -> (Math, f64) {
        let total: f64 = a.iter().zip(b).map(|((x, wi), (y, _))| wi * (x - ma) * (y - mb)).sum();
        let each: Vec<Math> = a
            .iter()
            .zip(b)
            .map(|((x, wi), (y, _))| {
                weighted_term(w, *wi, row(vec![paren(row(vec![val(*x), text("−"), n(ma)])), paren(row(vec![val(*y), text("−"), n(mb)]))]))
            })
            .collect();
        (equals(vec![name, terms(each, full), n(total)]), total)
    };
    let sname = |a: &str, b: &str| sub(var("S"), text(&format!("{a}{b}")));
    let tag = |j: usize| if p == 1 { "x".to_string() } else { format!("{}", j + 1) };
    let mut sxx = vec![vec![0.0; p]; p];
    let mut sxy = vec![0.0; p];
    let mut maths = Vec::new();
    for j in 0..p {
        for k in j..p {
            let (math, total) = product(&column(j), item(&mx, j), &column(k), item(&mx, k), sname(&tag(j), &tag(k)));
            maths.push(math);
            for (r, c) in [(j, k), (k, j)] {
                if let Some(x) = sxx.get_mut(r).and_then(|row| row.get_mut(c)) {
                    *x = total;
                }
            }
        }
    }
    for j in 0..p {
        let (math, total) = product(&column(j), item(&mx, j), &ys, my, sname(&tag(j), "y"));
        maths.push(math);
        if let Some(x) = sxy.get_mut(j) {
            *x = total;
        }
    }
    let (syy_math, syy) = product(&ys, my, &ys, my, sname("y", "y"));
    maths.push(syy_math);
    steps.extend(several(&tr!("ex-the-sums-of-the-products-of-the-deviations-from-the-means-fo"), maths));
    sections.push((tr!("ex-sums-of-products"), steps));

    // The slopes.
    let mut steps = Vec::new();
    let inverse = if p == 1 {
        let s = at(&sxx, 0, 0);
        if s <= 0.0 {
            steps.push(say(&tr!("ex-the-predictor-doesn-t-vary-so-there-is-no-slope")));
            sections.push((tr!("ex-the-coefficients"), steps));
            return Ok(Explanation { title: tr!("ex-title-regression"), sections });
        }
        steps.push(step(
            &tr!("ex-the-slope-divides-the-products-of-x-and-y-by-the-squares-of"),
            equals(vec![var("b"), frac(sname("x", "y"), sname("x", "x")), frac(n(item(&sxy, 0)), n(s)), approx(item(&sxy, 0) / s)]),
        ));
        vec![vec![1.0 / s]]
    } else {
        steps.extend(several(
            &tr!("ex-the-slopes-solve-the-normal-equations-sxx-b-sxy-with-these-m"),
            vec![
                equals(vec![sub(var("S"), text("xx")), matrix(numbers(&sxx))]),
                equals(vec![sub(var("S"), text("xy")), matrix(sxy.iter().map(|x| vec![n(*x)]).collect())]),
            ],
        ));
        let Some(inverse) = gauss_jordan_steps(&sxx, &mut steps) else {
            sections.push((tr!("ex-the-coefficients"), steps));
            return Ok(Explanation { title: tr!("ex-title-regression"), sections });
        };
        steps.push(step(&tr!("ex-the-inverse"), equals(vec![sup(sub(var("S"), text("xx")), text("−1")), matrix(numbers(&inverse))])));
        let mut maths = Vec::new();
        for (j, row_of) in inverse.iter().enumerate() {
            let b: f64 = row_of.iter().zip(&sxy).map(|(a, s)| a * s).sum();
            maths.push(equals(vec![
                sub(var("b"), text(&(j + 1).to_string())),
                plus((0..p).map(|k| row(vec![val(at(&inverse, j, k)), text("·"), val(item(&sxy, k))])).collect()),
                approx(b),
            ]));
        }
        steps.extend(several(&tr!("ex-each-slope-is-a-row-of-the-inverse-times-sxy"), maths));
        inverse
    };
    let b: Vec<f64> = (0..p).map(|j| (0..p).map(|k| at(&inverse, j, k) * item(&sxy, k)).sum()).collect();
    let b_name = |j: usize| if p == 1 { var("b") } else { sub(var("b"), text(&(j + 1).to_string())) };
    let constant = my - b.iter().zip(&mx).map(|(b, m)| b * m).sum::<f64>();
    steps.push(step(
        &tr!("ex-the-constant-puts-the-line-through-the-means"),
        equals(vec![
            var("a"),
            row(vec![bar(var("y")), text("−"), plus((0..p).map(|j| row(vec![b_name(j), bar(x_name(j))])).collect())]),
            row(vec![n(my), text("−"), paren(plus((0..p).map(|j| row(vec![val(item(&b, j)), text("·"), val(item(&mx, j))])).collect()))]),
            approx(constant),
        ]),
    ));
    sections.push((tr!("ex-the-coefficients"), steps));

    // The analysis of variance and the model summary.
    let mut steps = Vec::new();
    let ss_regression: f64 = b.iter().zip(&sxy).map(|(b, s)| b * s).sum();
    let ss_residual = (syy - ss_regression).max(0.0);
    steps.extend(several(
        &tr!("ex-the-sum-of-squares-the-model-explains-the-rest-left-in-the-r"),
        vec![
            equals(vec![
                sub(var("SS"), text(&tr!("ex-word-regression"))),
                sum(None, None, row(vec![indexed("b", "j"), sub(var("S"), var("jy"))])),
                plus((0..p).map(|j| row(vec![val(item(&b, j)), text("·"), val(item(&sxy, j))])).collect()),
                approx(ss_regression),
            ]),
            equals(vec![
                sub(var("SS"), text(&tr!("ex-word-residual"))),
                row(vec![sname("y", "y"), text("−"), sub(var("SS"), text(&tr!("ex-word-regression")))]),
                row(vec![n(syy), text("−"), n(ss_regression)]),
                approx(ss_residual),
            ]),
            equals(vec![sub(var("SS"), text(&tr!("ex-word-total"))), sname("y", "y"), approx(syy)]),
        ],
    ));
    let (df_regression, df_total) = (p as f64, big_n - 1.0);
    let (ms_regression, ms_residual) = (ss_regression / df_regression, ss_residual / df_residual);
    steps.extend(several(
        &tr!("ex-their-degrees-of-freedom-predictors-cases-minus-predictors-m"),
        vec![
            equals(vec![sub(var("df"), text(&tr!("ex-word-regression"))), n(df_regression)]),
            equals(vec![
                sub(var("df"), text(&tr!("ex-word-residual"))),
                row(vec![n(big_n), text("−"), n(df_regression), text("−"), text("1")]),
                n(df_residual),
            ]),
            equals(vec![sub(var("MS"), text(&tr!("ex-word-regression"))), frac(n(ss_regression), n(df_regression)), approx(ms_regression)]),
            equals(vec![sub(var("MS"), text(&tr!("ex-word-residual"))), frac(n(ss_residual), n(df_residual)), approx(ms_residual)]),
        ],
    ));
    let f = ms_regression / ms_residual;
    steps.push(step(&tr!("ex-f-compares-them"), equals(vec![var("F"), frac(n(ms_regression), n(ms_residual)), approx(f)])));
    if let Some(sig) = dist::f_sig(f, df_regression, df_residual) {
        steps.push(step(
            &tr!("ex-f-significance", df1 = number_text(df_regression), df2 = number_text(df_residual)),
            equals(vec![var("p"), approx(sig)]),
        ));
    }
    let r2 = if syy > 0.0 { ss_regression / syy } else { 0.0 };
    let adjusted = 1.0 - (1.0 - r2) * df_total / df_residual;
    steps.extend(several(
        &tr!("ex-the-share-of-the-variance-the-model-explains-its-square-root"),
        vec![
            equals(vec![
                sup(var("R"), text("2")),
                frac(sub(var("SS"), text(&tr!("ex-word-regression"))), sub(var("SS"), text(&tr!("ex-word-total")))),
                frac(n(ss_regression), n(syy)),
                approx(r2),
            ]),
            equals(vec![var("R"), sqrt(n(r2)), approx(r2.sqrt())]),
            equals(vec![
                sup(sub(var("R"), text(&tr!("ex-word-adj"))), text("2")),
                row(vec![text("1"), text("−"), paren(row(vec![text("1"), text("−"), n(r2)])), text("·"), frac(n(df_total), n(df_residual))]),
                approx(adjusted),
            ]),
            equals(vec![text("SE of the estimate"), sqrt(n(ms_residual)), approx(ms_residual.sqrt())]),
        ],
    ));
    sections.push((tr!("ex-the-analysis-of-variance-and-the-model-summary"), steps));

    // Each coefficient.
    let mut steps = Vec::new();
    let t_crit = options.ci.and_then(|level| dist::t_quantile(0.5 + level / 2.0, df_residual));
    if let (Some(level), Some(tc)) = (options.ci, t_crit) {
        steps.push(step(
            &tr!(
                "ex-intervals-t",
                level = number_text(level * 100.0),
                tail = number_text((1.0 - level) / 2.0 * 100.0),
                df = number_text(df_residual)
            ),
            equals(vec![sub(var("t"), num(0.5 + level / 2.0, 4)), approx(tc)]),
        ));
    }
    let coefficient = |name: Math, value: f64, se_math: Vec<Math>, se: f64, extra: Vec<Math>| -> Vec<Math> {
        let t = value / se;
        let mut maths = vec![equals([vec![sub(var("SE"), name.clone())], se_math, vec![approx(se)]].concat())];
        maths.push(equals(vec![var("t"), frac(n(value), n(se)), approx(t)]));
        if let Some(sig) = dist::t_sig(t, df_residual) {
            maths.push(equals(vec![var("p"), approx(sig)]));
        }
        maths.extend(extra);
        if let Some(tc) = t_crit {
            maths.push(equals(vec![
                text(&tr!("ex-word-lower-cap")),
                row(vec![n(value), text("−"), n(tc), text("·"), n(se)]),
                approx(value - tc * se),
            ]));
            maths.push(equals(vec![
                text(&tr!("ex-word-upper-cap")),
                row(vec![n(value), text("+"), n(tc), text("·"), n(se)]),
                approx(value + tc * se),
            ]));
        }
        maths
    };
    let quad: f64 = (0..p).map(|j| (0..p).map(|k| item(&mx, j) * at(&inverse, j, k) * item(&mx, k)).sum::<f64>()).sum();
    let se_constant = (ms_residual * (1.0 / big_n + quad)).sqrt();
    let quad_math = if p == 1 {
        frac(sup(bar(var("x")), text("2")), sname("x", "x"))
    } else {
        row(vec![sup(bar(var("x")), text("T")), sup(sub(var("S"), text("xx")), text("−1")), bar(var("x"))])
    };
    steps.extend(several(
        &tr!("ex-constant-se", a = number_text(constant)),
        coefficient(
            var("a"),
            constant,
            vec![
                sqrt(row(vec![sub(var("MS"), text(&tr!("ex-word-residual"))), paren(row(vec![frac(text("1"), var("N")), text("+"), quad_math]))])),
                sqrt(row(vec![n(ms_residual), text("·"), paren(row(vec![frac(text("1"), n(big_n)), text("+"), n(quad)]))])),
            ],
            se_constant,
            Vec::new(),
        ),
    ));
    let sd_y = (syy / df_total).sqrt();
    for j in 0..p {
        let se = (ms_residual * at(&inverse, j, j)).sqrt();
        let sd_x = (at(&sxx, j, j) / df_total).sqrt();
        let beta = item(&b, j) * sd_x / sd_y;
        let diagonal = if p == 1 {
            frac(text("1"), sname("x", "x"))
        } else {
            sub(paren(sup(sub(var("S"), text("xx")), text("−1"))), text(&format!("{}{}", j + 1, j + 1)))
        };
        let beta_math = equals(vec![
            text(&tr!("ex-word-beta-cap")),
            row(vec![b_name(j), text("·"), frac(sub(var("s"), x_name(j)), sub(var("s"), var("y")))]),
            row(vec![val(item(&b, j)), text("·"), frac(sqrt(frac(n(at(&sxx, j, j)), n(df_total))), sqrt(frac(n(syy), n(df_total))))]),
            approx(beta),
        ]);
        let name = xs.get(j).map_or(String::new(), |(_, v)| v.display_name().to_string());
        steps.extend(several(
            &tr!("ex-coefficient-se", name = name.as_str(), b = number_text(item(&b, j))),
            coefficient(
                b_name(j),
                item(&b, j),
                vec![
                    sqrt(row(vec![sub(var("MS"), text(&tr!("ex-word-residual"))), text("·"), diagonal])),
                    sqrt(row(vec![n(ms_residual), text("·"), n(at(&inverse, j, j))])),
                ],
                se,
                vec![beta_math],
            ),
        ));
    }
    sections.push((tr!("ex-each-coefficient"), steps));
    Ok(Explanation { title: tr!("ex-title-regression"), sections })
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
            sections.push((
                tr!("ex-heading-crosstab", rows = row_var.display_name(), columns = column_var.display_name()),
                chi_square_steps(row_var, column_var, &counts, full),
            ));
        }
    }
    Ok(Explanation { title: tr!("ex-title-crosstabs"), sections })
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
    let mut steps = vec![say(&tr!("ex-crosstab-cases", rows = labels(row_var, &counts.rows), columns = labels(column_var, &counts.columns)))];
    if nr < 2 || nc < 2 {
        steps.push(say(&tr!("ex-one-of-the-variables-has-a-single-value-here-so-there-are-no")));
        return steps;
    }
    let big_n = counts.total();
    let cell = |letter: &str, r: usize, c: usize| sub(var(letter), text(&format!("{}{}", r + 1, c + 1)));
    let row_totals: Vec<Math> = (0..nr)
        .map(|r| {
            equals(vec![sub(var("R"), text(&(r + 1).to_string())), plus((0..nc).map(|c| n(counts.get(r, c))).collect()), n(counts.row_total(r))])
        })
        .collect();
    steps.extend(several(&tr!("ex-each-row-s-total-adds-its-counts"), row_totals));
    let column_totals: Vec<Math> = (0..nc)
        .map(|c| {
            equals(vec![sub(var("C"), text(&(c + 1).to_string())), plus((0..nr).map(|r| n(counts.get(r, c))).collect()), n(counts.column_total(c))])
        })
        .collect();
    steps.extend(several(&tr!("ex-and-each-column-s"), column_totals));
    steps.push(step(&tr!("ex-the-total"), equals(vec![var("N"), plus((0..nr).map(|r| n(counts.row_total(r))).collect()), n(big_n)])));
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
    steps.extend(several(&tr!("ex-the-count-each-cell-would-expect-if-the-variables-were-indep"), expected));

    let df = ((nr - 1) * (nc - 1)) as f64;
    let chi2: f64 = cells.iter().map(|&(r, c)| (counts.get(r, c) - counts.expected(r, c)).powi(2) / counts.expected(r, c)).sum();
    let symbol = || frac(sup(paren(row(vec![var("O"), text("−"), var("E")])), text("2")), var("E"));
    let each: Vec<Math> = cells
        .iter()
        .map(|&(r, c)| frac(sup(paren(row(vec![n(counts.get(r, c)), text("−"), n(counts.expected(r, c))])), text("2")), n(counts.expected(r, c))))
        .collect();
    let parts: Vec<Math> = cells.iter().map(|&(r, c)| n((counts.get(r, c) - counts.expected(r, c)).powi(2) / counts.expected(r, c))).collect();
    steps.push(step(
        &tr!("ex-pearson-s-chi-square-adds-each-cell-s-squared-distance-from"),
        equals(vec![sup(var("χ"), text("2")), sum(None, None, symbol()), terms(each, full), terms(parts, full), approx(chi2)]),
    ));
    steps.push(step(
        &tr!("ex-its-degrees-of-freedom"),
        equals(vec![
            var("df"),
            row(vec![paren(row(vec![var("r"), text("−"), text("1")])), paren(row(vec![var("c"), text("−"), text("1")]))]),
            row(vec![paren(row(vec![n(nr as f64), text("−"), text("1")])), text("·"), paren(row(vec![n(nc as f64), text("−"), text("1")]))]),
            n(df),
        ]),
    ));
    let chi_sig = |x: f64, df: f64, what: &str| {
        dist::chi_square_sig(x, df).map(|p| step(&tr!("ex-chi-significance", what = what, df = number_text(df)), equals(vec![var("p"), approx(p)])))
    };
    steps.extend(chi_sig(chi2, df, &tr!("ex-its-significance")));

    let lr: f64 =
        cells.iter().map(|&(r, c)| (counts.get(r, c), counts.expected(r, c))).filter(|(o, _)| *o > 0.0).map(|(o, e)| 2.0 * o * (o / e).ln()).sum();
    let lr_terms: Vec<Math> = cells
        .iter()
        .filter(|&&(r, c)| counts.get(r, c) > 0.0)
        .map(|&(r, c)| row(vec![n(counts.get(r, c)), text("·"), text("ln"), frac(n(counts.get(r, c)), n(counts.expected(r, c)))]))
        .collect();
    steps.push(step(
        &tr!("ex-the-likelihood-ratio-over-the-cells-with-a-count"),
        equals(vec![
            sup(var("G"), text("2")),
            row(vec![text("2"), sum(None, None, row(vec![var("O"), text("·"), text("ln"), frac(var("O"), var("E"))]))]),
            row(vec![text("2"), paren(terms(lr_terms, full))]),
            approx(lr),
        ]),
    ));
    steps.extend(chi_sig(lr, df, &tr!("ex-its-significance")));

    if nr == 2 && nc == 2 {
        let (a, b, c, d) = (counts.get(0, 0), counts.get(0, 1), counts.get(1, 0), counts.get(1, 1));
        let (r1, r2, c1, c2) = (a + b, c + d, a + c, b + d);
        let inner = ((a * d - b * c).abs() - big_n / 2.0).max(0.0);
        let yates = big_n * inner.powi(2) / (r1 * r2 * c1 * c2);
        steps.push(step(
            &tr!("ex-the-continuity-correction-yates-for-a-2-2-table-with-cells-a"),
            equals(vec![
                sup(sub(var("χ"), text(&tr!("ex-word-yates-cap"))), text("2")),
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
        steps.extend(chi_sig(yates, 1.0, &tr!("ex-its-significance")));
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
        steps.push(say(&tr!("ex-the-linear-by-linear-association-uses-the-correlation-r-of-t")));
        steps.extend(pearson(&pairs, true, &tr!("ex-how-table"), false, full, false));
        let mx = pairs.iter().map(|(x, _, w)| x * w).sum::<f64>() / big_n;
        let my = pairs.iter().map(|(_, y, w)| y * w).sum::<f64>() / big_n;
        let sxy: f64 = pairs.iter().map(|(x, y, w)| w * (x - mx) * (y - my)).sum();
        let sxx: f64 = pairs.iter().map(|(x, _, w)| w * (x - mx).powi(2)).sum();
        let syy: f64 = pairs.iter().map(|(_, y, w)| w * (y - my).powi(2)).sum();
        let r = sxy / (sxx * syy).sqrt();
        if r.is_finite() {
            let m2 = (big_n - 1.0) * r * r;
            steps.push(step(
                &tr!("ex-the-association-with-1-degree-of-freedom"),
                equals(vec![
                    sup(var("M"), text("2")),
                    row(vec![paren(row(vec![var("N"), text("−"), text("1")])), sup(var("r"), text("2"))]),
                    row(vec![paren(row(vec![n(big_n), text("−"), text("1")])), text("·"), sup(val(r), text("2"))]),
                    approx(m2),
                ]),
            ));
            steps.extend(chi_sig(m2, 1.0, &tr!("ex-its-significance")));
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
        &tr!("ex-fisher-s-exact-test-with-the-totals-fixed-the-probability-of"),
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
        &tr!("ex-this-table-s"),
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
    steps.push(step(&tr!("ex-fisher-tables", low = low, high = high), row(listed)));
    steps.push(step(
        &tr!("ex-two-sided-the-tables-at-most-as-likely-as-this-one-added-up"),
        equals(vec![var("p"), terms(two, full), approx(two_sum.min(1.0))]),
    ));
    let (one, one_sum, side) =
        if lower_sum <= upper_sum { (lower, lower_sum, "ex-fisher-at-most") } else { (upper, upper_sum, "ex-fisher-at-least") };
    steps.push(step(
        &mksigma_i18n::message(side, &[("a", number_text(a).into())]),
        equals(vec![var("p"), terms(one, full), approx(one_sum.min(1.0))]),
    ));
    steps
}

/// A number in a sentence, with the language's decimal mark.
fn number_text(x: f64) -> String {
    mksigma_i18n::decimal(&mksigma_output::math::number(x, places(x)))
}

/// `x` to four decimals, then to `places` as the table shows it (when that differs).
fn approx_to(x: f64, shown: usize) -> Math {
    if mksigma_output::math::number(x, places(x)) == mksigma_output::math::number(x, shown) {
        n(x)
    } else {
        row(vec![n(x), text("≈"), num(x, shown)])
    }
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
            &tr!("ex-n-counts-the-cases-with-a-valid-value-and-apart-those-whose"),
            vec![
                equals(vec![sub(var("N"), text(&tr!("ex-word-valid"))), n(valid_n)]),
                equals(vec![sub(var("N"), text(&tr!("ex-word-missing"))), n(missing)]),
            ],
        );
        let xs: Vec<(f64, f64)> = counts.valid.iter().filter_map(|(v, f)| Some((v.as_num()?, *f))).collect();
        if wanted.is_empty() {
            sections.push((variable.display_name().to_string(), steps));
            continue;
        }
        if variable.var_type != mksigma_core::VarType::Numeric {
            steps.push(say(&tr!("ex-a-string-variable-has-none-of-the-other-statistics")));
            sections.push((variable.display_name().to_string(), steps));
            continue;
        }
        if xs.is_empty() {
            steps.push(say(&tr!("ex-with-no-valid-values-there-is-nothing-more-to-compute")));
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
                &tr!("ex-the-sum-each-distinct-value-times-its-frequency-f-added-up"),
                equals(vec![sum(None, None, body.clone()), terms(xs.iter().map(|(x, f)| f_x(*x, *f)).collect(), full), approx_to(total, 2)]),
            ));
        }
        if asked(&[Statistic::Mean, Statistic::SeMean, Statistic::StdDev, Statistic::Variance]) {
            steps.push(step(
                &tr!("ex-the-mean-divides-it-by-n"),
                equals(vec![
                    bar(var("x")),
                    frac(sum(None, None, body), sub(var("N"), text(&tr!("ex-word-valid")))),
                    frac(n(total), n(valid_n)),
                    approx_to(mean, 2),
                ]),
            ));
        }
        if asked(&[Statistic::SeMean, Statistic::StdDev, Statistic::Variance]) {
            let ss: f64 = xs.iter().map(|(x, f)| f * (x - mean).powi(2)).sum();
            let devs: Vec<Math> = xs.iter().map(|(x, f)| row(vec![n(*f), text("·"), dev_sq(*x, mean)])).collect();
            steps.push(step(
                &tr!("ex-the-squared-deviations-from-the-mean-each-times-its-frequenc"),
                equals(vec![
                    sum(None, None, row(vec![indexed("f", "k"), sup(paren(row(vec![indexed("x", "k"), text("−"), bar(var("x"))])), text("2"))])),
                    terms(devs, full),
                    n(ss),
                ]),
            ));
            if valid_n <= 1.0 {
                steps.push(say(&tr!("ex-with-n-of-1-or-less-the-variance-and-the-standard-deviation")));
            } else {
                let variance = ss / (valid_n - 1.0);
                let sd = variance.sqrt();
                steps.extend(several(
                    &tr!("ex-the-variance-divides-that-by-n-1-the-standard-deviation-is-i"),
                    vec![
                        equals(vec![sup(var("s"), text("2")), frac(n(ss), row(vec![n(valid_n), text("−"), text("1")])), approx_to(variance, 2)]),
                        equals(vec![var("s"), sqrt(n(variance)), approx_to(sd, 2)]),
                    ],
                ));
                if wanted.contains(&Statistic::SeMean) {
                    steps.push(step(
                        &tr!("ex-the-standard-error-of-the-mean"),
                        equals(vec![
                            var("SE"),
                            frac(var("s"), sqrt(sub(var("N"), text(&tr!("ex-word-valid"))))),
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
            let text_of = if modes.len() > 1 { tr!("ex-modes", n = modes.len(), f = number_text(top)) } else { tr!("ex-mode", f = number_text(top)) };
            steps.push(step(&text_of, equals(vec![text(&tr!("ex-word-mode-cap")), approx_to(smallest, 2)])));
        }
        let (lo, hi) = xs.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), (x, _)| (a.min(*x), b.max(*x)));
        if asked(&[Statistic::Minimum, Statistic::Maximum, Statistic::Range]) {
            steps.extend(several(
                &tr!("ex-the-smallest-and-largest-valid-values"),
                vec![
                    equals(vec![text(&tr!("ex-word-minimum-cap")), approx_to(lo, 2)]),
                    equals(vec![text(&tr!("ex-word-maximum-cap")), approx_to(hi, 2)]),
                ],
            ));
        }
        if wanted.contains(&Statistic::Range) {
            steps.push(step(
                &tr!("ex-the-range-is-their-difference"),
                equals(vec![text(&tr!("ex-word-range-cap")), row(vec![n(hi), text("−"), val(lo)]), approx_to(hi - lo, 2)]),
            ));
        }
        sections.push((variable.display_name().to_string(), steps));
    }
    Ok(Explanation { title: tr!("ex-title-statistics"), sections })
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
            &tr!("ex-every-case-counts-in-the-total-the-valid-ones-are-those-whos"),
            vec![
                equals(vec![var("N"), row(vec![n(valid_n), text("+"), n(missing)]), n(total)]),
                equals(vec![sub(var("N"), text(&tr!("ex-word-valid"))), n(valid_n)]),
            ],
        ));
        let mut cumulative = 0.0;
        let shown = if full { counts.valid.len() } else { counts.valid.len().min(SHOWN * 2) };
        for (value, f) in counts.valid.iter().take(shown) {
            cumulative += f;
            let label = value_text(variable, value);
            let frac100 = |a: Math, b: f64, r: f64| equals(vec![row(vec![frac(a, n(b)), text("×"), text("100")]), num(r, 1)]);
            steps.push(step(
                &tr!("ex-value-f", label = label.as_str(), f = number_text(*f)),
                row(vec![
                    text(&tr!("ex-percent-label")),
                    frac100(n(*f), total, f / total * 100.0),
                    text(&format!(",   {}", tr!("ex-valid-label"))),
                    frac100(n(*f), valid_n, f / valid_n * 100.0),
                    text(&format!(",   {}", tr!("ex-cumulative-label"))),
                    frac100(n(cumulative), valid_n, cumulative / valid_n * 100.0),
                ]),
            ));
        }
        if shown < counts.valid.len() {
            steps.push(say(&tr!("ex-more-values", n = counts.valid.len() - shown)));
        }
        sections.push((variable.display_name().to_string(), steps));
    }
    Ok(Explanation { title: tr!("ex-title-frequencies"), sections })
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
    fn oneway_by_hand_agrees_with_the_tables() {
        let mut ds = Dataset::new();
        ds.add_variable(Variable::numeric("y")).unwrap();
        ds.add_variable(Variable::numeric("g")).unwrap();
        // Groups 1: 2 3 4 (mean 3), 2: 6 7 8 (mean 7), 3: 4 5 6 (mean 5); grand mean 5.
        for (g, ys) in [(1.0, [2.0, 3.0, 4.0]), (2.0, [6.0, 7.0, 8.0]), (3.0, [4.0, 5.0, 6.0])] {
            for y in ys {
                ds.push_case(vec![Value::Num(y), Value::Num(g)]).unwrap();
            }
        }
        let options = crate::OnewayOptions {
            dependents: vec!["y".into()],
            factor: "g".into(),
            descriptives: true,
            homogeneity: true,
            posthoc: vec![crate::PostHoc::Tukey, crate::PostHoc::Bonferroni],
            alpha: 0.05,
        };
        let tex = latex(&oneway(&ds, &options, false).unwrap());
        let items = crate::oneway(&ds, &options).unwrap();
        // Every number in every table, the post hoc ones with either sign (the table lists both
        // orders of each pair).
        for t in tables(&items) {
            for (_, cells) in &t.rows {
                for cell in cells {
                    let x = match cell {
                        Cell::Num { value, .. } => *value,
                        Cell::Text(s) if s.ends_with('*') => s.trim_end_matches('*').parse().unwrap(),
                        Cell::Int(k) => *k as f64,
                        _ => continue,
                    };
                    let found = [x, -x].iter().any(|x| {
                        let shown = number(*x, 3);
                        tex.contains(&format!("\\approx {shown}")) || tex.contains(&format!("= {shown}"))
                    });
                    assert!(found, "{x} of {} in {tex}", t.title);
                }
            }
        }
        // SSB = 3 · (4 + 4 + 0) = 24, SSW = 2 + 2 + 2 = 6, F = (24 / 2) / (6 / 6) = 12.
        assert!(tex.contains("= 24\n") || tex.contains("= 24"), "{tex}");
        assert!(tex.contains("\\frac{12}{1} = 12"), "{tex}");
    }

    fn npar_data() -> Dataset {
        let mut ds = Dataset::new();
        for name in ["x", "g", "after"] {
            ds.add_variable(Variable::numeric(name)).unwrap();
        }
        // Group 1: 1 2 2 3; group 2: 3 4 5 5 6. Ties: 2 2, 3 3 and 5 5.
        for (x, g, a) in [
            (1.0, 1.0, 2.0),
            (2.0, 1.0, 4.0),
            (2.0, 1.0, 1.0),
            (3.0, 1.0, 3.0),
            (3.0, 2.0, 6.0),
            (4.0, 2.0, 2.0),
            (5.0, 2.0, 8.0),
            (5.0, 2.0, 4.0),
            (6.0, 2.0, 9.0),
        ] {
            ds.push_case(vec![Value::Num(x), Value::Num(g), Value::Num(a)]).unwrap();
        }
        ds
    }

    #[test]
    fn nonparametric_tests_by_hand_agree_with_the_tables() {
        let ds = npar_data();
        for test in [
            crate::NparTest::MannWhitney { variables: vec!["x".into()], group: "g".into(), first: 1.0, second: 2.0 },
            crate::NparTest::KruskalWallis { variables: vec!["x".into()], group: "g".into(), low: 1.0, high: 2.0 },
            crate::NparTest::Wilcoxon { pairs: vec![("x".into(), "after".into())] },
        ] {
            let tex = latex(&npar_tests(&ds, std::slice::from_ref(&test), false).unwrap());
            let items = crate::npar_tests(&ds, &[test]).unwrap();
            for t in tables(&items) {
                for (_, cells) in &t.rows {
                    for cell in cells {
                        let (x, places) = match cell {
                            Cell::Num { value, decimals } => (*value, *decimals as usize),
                            Cell::Int(k) => (*k as f64, 0),
                            _ => continue,
                        };
                        let shown = number(x, places);
                        let found = tex.contains(&format!("\\approx {shown}")) || tex.contains(&format!("= {shown}"));
                        assert!(found, "{x} of {} in {tex}", t.title);
                    }
                }
            }
        }
        // Mann-Whitney's ranks: 1 → 1, 2 → (2 + 3) / 2 = 2.5, 3 → (4 + 5) / 2 = 4.5.
        let test = crate::NparTest::MannWhitney { variables: vec!["x".into()], group: "g".into(), first: 1.0, second: 2.0 };
        let tex = latex(&npar_tests(&ds, &[test], false).unwrap());
        assert!(tex.contains("2 \\to \\frac{2 + 3}{2} = 2.5") && tex.contains("3 \\to \\frac{4 + 5}{2} = 4.5"), "{tex}");
        // Three ties of two: T = 3 · (2³ − 2) = 18.
        assert!(tex.contains("= 18"), "{tex}");
    }

    #[test]
    fn examine_by_hand_agrees_with_the_table() {
        let ds = t_data();
        for by in [None, Some("g")] {
            let tex = latex(&examine(&ds, &["before".into(), "after".into()], by, false).unwrap());
            let options = crate::ExamineOptions {
                variables: vec!["before".into(), "after".into()],
                by: by.map(String::from),
                descriptives: true,
                boxplot: false,
            };
            let items = crate::examine(&ds, &options).unwrap();
            let t = tables(&items).into_iter().find(|t| t.title == "Descriptives").unwrap();
            for (_, cells) in &t.rows {
                for cell in cells {
                    let Cell::Num { value, .. } = cell else { continue };
                    let shown = number(*value, 3);
                    assert!(tex.contains(&format!("\\approx {shown}")) || tex.contains(&format!("= {shown}")), "{value} in {tex}");
                }
            }
        }
        // before: 4 4 5 6 6 7 8; the median at (7 + 1) · 0.5 = 4, between 6 and 6.
        let tex = latex(&examine(&ds, &["before".into()], None, false).unwrap());
        assert!(tex.contains("= 8 \\cdot 0.5 = 4") && tex.contains("\\text{Median} = 6 + 0 \\cdot \\left( 6 - 6 \\right) = 6"), "{tex}");
    }

    #[test]
    fn regression_by_hand_agrees_with_the_tables() {
        let ds = sample();
        for predictors in [vec!["age"], vec!["age", "income"]] {
            let options = crate::RegressionOptions {
                dependent: "score".into(),
                predictors: predictors.iter().map(|p| p.to_string()).collect(),
                ci: Some(0.95),
            };
            let tex = latex(&regression(&ds, &options, false).unwrap());
            let items = crate::regression(&ds, &options).unwrap();
            for t in tables(&items) {
                for (_, cells) in &t.rows {
                    for cell in cells {
                        let x = match cell {
                            Cell::Num { value, .. } => *value,
                            Cell::Int(k) => *k as f64,
                            _ => continue,
                        };
                        let shown = number(x, 3);
                        assert!(tex.contains(&format!("\\approx {shown}")) || tex.contains(&format!("= {shown}")), "{x} of {} in {tex}", t.title);
                    }
                }
            }
            if predictors.len() == 2 {
                assert!(tex.contains("\\begin{array}{cc|cc}"), "Gauss–Jordan's augmented matrix: {tex}");
            }
        }
        // x = 1..5, y = 2 4 5 4 5: Sxy = 6, Sxx = 10, b = 0.6, a = 4 − 0.6 · 3 = 2.2.
        let mut ds = Dataset::new();
        for name in ["y", "x"] {
            ds.add_variable(Variable::numeric(name)).unwrap();
        }
        for (y, x) in [(2.0, 1.0), (4.0, 2.0), (5.0, 3.0), (4.0, 4.0), (5.0, 5.0)] {
            ds.push_case(vec![Value::Num(y), Value::Num(x)]).unwrap();
        }
        let options = crate::RegressionOptions { dependent: "y".into(), predictors: vec!["x".into()], ci: None };
        let tex = latex(&regression(&ds, &options, false).unwrap());
        assert!(tex.contains("\\frac{6}{10} = 0.6") && tex.contains("= 2.2"), "{tex}");
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
