//! The noncentral χ², F, beta and t distributions: their distribution functions, survival
//! functions and quantiles, and the noncentrality that gives a probability. Power and sample
//! size, the run length of control charts and confidence intervals for effect sizes rest on
//! them.
//!
//! Each is a Poisson mixture of central ones. χ² with `k` degrees of freedom and noncentrality
//! λ: P(X ≤ x) = Σⱼ wⱼ P(k/2 + j, x/2), with wⱼ the Poisson(λ/2) weights and P the regularized
//! incomplete gamma function (Ding 1992, *Applied Statistics* AS 275). Beta(a, b, λ):
//! Σⱼ wⱼ I_x(a + j, b), and F through it (Frick 1990, AS R84). t with ν degrees of freedom and
//! noncentrality δ, for t ≥ 0: Φ(−δ) + ½ Σⱼ [wⱼ I_x(j + ½, ν/2) + sign(δ) w_{j+½} I_x(j + 1, ν/2)],
//! with x = t²/(t² + ν) and weights at δ²/2 (Lenth 1989, AS 243). The series are summed from the
//! largest weight outwards, until a bound on what is left is below the precision of the sum;
//! the upper tails are summed as series of their own, not as 1 − P, so a small one keeps its
//! digits.
//!
//! Every function returns `None` for a NaN or out-of-range argument, or a series or search that
//! doesn't converge within its cap; none panics.

use statrs::function::gamma::ln_gamma;

/// What is left of a series, relative to its sum, when it stops.
const PRECISION: f64 = 1e-17;
/// Terms summed on each side of the largest weight, at most.
const MAX_TERMS: u32 = 1_000_000;
/// Halvings of an interval in a search, at most.
const MAX_HALVINGS: u32 = 2_000;
/// Doublings of a step while bracketing a root, at most.
const MAX_DOUBLINGS: u32 = 2_000;

/// Σⱼ w(j + offset)·f(j) for j = 0, 1, …, with w(m) = e^(−μ) μ^m / Γ(m + 1): the Poisson
/// weights, at half-integers too for the t. `f` is a probability, decreasing in j when `falls`
/// and increasing otherwise, which bounds the terms not yet summed.
fn mixture(mu: f64, offset: f64, falls: bool, f: impl Fn(f64) -> Option<f64>) -> Option<f64> {
    if mu == 0.0 {
        return if offset == 0.0 { f(0.0) } else { Some(0.0) };
    }
    let start = (mu - offset).floor().max(0.0);
    let first = (-mu + (start + offset) * mu.ln() - ln_gamma(start + offset + 1.0)).exp();
    if !first.is_finite() {
        return None;
    }
    let mut sum = 0.0;
    let small = |left: f64, sum: f64| left <= PRECISION * sum || left < f64::MIN_POSITIVE;

    // Upwards from the largest weight; after it, each weight shrinks by at least `ratio`.
    let (mut j, mut w) = (start, first);
    for n in 0.. {
        if n > MAX_TERMS {
            return None;
        }
        let v = f(j)?;
        sum += w * v;
        let ratio = mu / (j + offset + 1.0);
        let left = w * ratio / (1.0 - ratio) * if falls { v } else { 1.0 };
        if ratio < 1.0 && small(left, sum) {
            break;
        }
        w *= ratio;
        j += 1.0;
    }

    // Downwards, to j = 0.
    let (mut j, mut w) = (start, first);
    let mut n = 0;
    while j > 0.0 {
        n += 1;
        if n > MAX_TERMS {
            return None;
        }
        w *= (j + offset) / mu;
        j -= 1.0;
        let v = f(j)?;
        sum += w * v;
        let ratio = (j + offset) / mu;
        let left = w * ratio / (1.0 - ratio) * if falls { 1.0 } else { v };
        if ratio < 1.0 && small(left, sum) {
            break;
        }
    }
    sum.is_finite().then_some(sum)
}

fn degrees(df: f64) -> bool {
    df > 0.0 && df.is_finite()
}

fn noncentrality(ncp: f64) -> bool {
    ncp >= 0.0 && ncp.is_finite()
}

fn probability(p: f64) -> bool {
    (0.0..=1.0).contains(&p)
}

/// The standard normal distribution function, as ½ Q(½, z²/2) below 0 (erfc through the
/// incomplete gamma function).
fn phi(z: f64) -> Option<f64> {
    let tail = 0.5 * gamma_upper(0.5, z * z / 2.0)?;
    Some(if z < 0.0 { tail } else { 1.0 - tail })
}

/// ln Γ(z) − [(z − ½) ln z − z + ½ ln 2π]: what Stirling's formula leaves out of ln Γ(z), small
/// and computed apart, so that the big terms of Stirling's formula can cancel exactly.
fn stirling_rest(z: f64) -> f64 {
    if z >= 10.0 {
        // The asymptotic series (Abramowitz & Stegun 6.1.41).
        let r = 1.0 / (z * z);
        (1.0 / 12.0 - r * (1.0 / 360.0 - r * (1.0 / 1260.0 - r * (1.0 / 1680.0 - r / 1188.0)))) / z
    } else {
        ln_gamma(z) - ((z - 0.5) * z.ln() - z + 0.5 * (2.0 * std::f64::consts::PI).ln())
    }
}

/// ln(1 + t) − t, accurate for a small t: with u = t/(2 + t), ln(1 + t) = 2 atanh u, so
/// ln(1 + t) − t = −t·u + 2(u³/3 + u⁵/5 + …).
fn ln1p_minus(t: f64) -> f64 {
    if t.abs() > 0.5 {
        return t.ln_1p() - t;
    }
    let u = t / (2.0 + t);
    let u2 = u * u;
    let (mut sum, mut power) = (0.0, u * u2);
    for k in 1..60 {
        let term = power / f64::from(2 * k + 1);
        sum += term;
        if term.abs() <= f64::EPSILON * sum.abs() {
            break;
        }
        power *= u2;
    }
    -t * u + 2.0 * sum
}

/// x^a (1 − x)^b / B(a, b), given 1 − x as `cx`. With e = x·b − (1 − x)·a, its logarithm is
/// a·[ln(1 + e/a) − e/a] + b·[ln(1 − e/b) + e/b] + ½ ln(ab / (2π(a + b))) plus the Stirling
/// rests: no two large terms cancel, so it keeps its digits when a or b is in the millions.
fn beta_front(a: f64, b: f64, x: f64, cx: f64) -> f64 {
    let e = x * b - cx * a;
    let ln = a * ln1p_minus(e / a) + b * ln1p_minus(-e / b) + 0.5 * (a * b / (2.0 * std::f64::consts::PI * (a + b))).ln() + stirling_rest(a + b)
        - stirling_rest(a)
        - stirling_rest(b);
    ln.exp()
}

/// The continued fraction of I_x(a, b) (Abramowitz & Stegun 26.5.8), evaluated by Lentz's
/// method; it converges quickly for x < (a + 1)/(a + b + 2).
fn beta_fraction(a: f64, b: f64, x: f64) -> Option<f64> {
    const TINY: f64 = 1e-300;
    let mut c = 1.0;
    let mut d = 1.0 - (a + b) * x / (a + 1.0);
    if d.abs() < TINY {
        d = TINY;
    }
    d = 1.0 / d;
    let mut fraction = d;
    for m in 1..100_000 {
        let m = f64::from(m);
        let m2 = 2.0 * m;
        for coefficient in [m * (b - m) * x / ((a + m2 - 1.0) * (a + m2)), -(a + m) * (a + b + m) * x / ((a + m2) * (a + m2 + 1.0))] {
            d = 1.0 + coefficient * d;
            if d.abs() < TINY {
                d = TINY;
            }
            c = 1.0 + coefficient / c;
            if c.abs() < TINY {
                c = TINY;
            }
            d = 1.0 / d;
            fraction *= d * c;
        }
        if (d * c - 1.0).abs() <= f64::EPSILON {
            return fraction.is_finite().then_some(fraction);
        }
    }
    None
}

/// I_x(a, b), the regularized incomplete beta function, given 1 − x as `cx` (computed apart by
/// the caller, so an x near 1 keeps the digits of its complement).
fn beta_reg(a: f64, b: f64, x: f64, cx: f64) -> Option<f64> {
    if x <= 0.0 {
        return Some(0.0);
    }
    if cx <= 0.0 {
        return Some(1.0);
    }
    let p = if x < (a + 1.0) / (a + b + 2.0) {
        beta_front(a, b, x, cx) * beta_fraction(a, b, x)? / a
    } else {
        1.0 - beta_front(b, a, cx, x) * beta_fraction(b, a, cx)? / b
    };
    (p.is_finite()).then_some(p.clamp(0.0, 1.0))
}

/// x^a e^(−x) / Γ(a), with its logarithm written as a·[ln(1 + t) − t] + ½ ln(a/2π) minus the
/// Stirling rest, t = (x − a)/a: no large terms cancel when a is in the millions.
fn gamma_front(a: f64, x: f64) -> f64 {
    (a * ln1p_minus((x - a) / a) + 0.5 * (a / (2.0 * std::f64::consts::PI)).ln() - stirling_rest(a)).exp()
}

/// The regularized incomplete gamma functions (P(a, x), Q(a, x)): the series for P below
/// x = a + 1 (Abramowitz & Stegun 6.5.29), Legendre's continued fraction for Q above it (6.5.31,
/// by Lentz's method); the other one as the complement.
fn gamma_reg(a: f64, x: f64) -> Option<(f64, f64)> {
    if x <= 0.0 {
        return Some((0.0, 1.0));
    }
    let front = gamma_front(a, x);
    if x < a + 1.0 {
        let (mut term, mut sum) = (1.0 / a, 1.0 / a);
        for n in 1..1_000_000 {
            term *= x / (a + f64::from(n));
            sum += term;
            if term <= f64::EPSILON * sum {
                let p = (front * sum).clamp(0.0, 1.0);
                return p.is_finite().then_some((p, 1.0 - p));
            }
        }
        None
    } else {
        const TINY: f64 = 1e-300;
        let mut b = x + 1.0 - a;
        let mut c = 1.0 / TINY;
        let mut d = 1.0 / b;
        let mut fraction = d;
        for n in 1..1_000_000 {
            let n = f64::from(n);
            let coefficient = -n * (n - a);
            b += 2.0;
            d = coefficient * d + b;
            if d.abs() < TINY {
                d = TINY;
            }
            c = b + coefficient / c;
            if c.abs() < TINY {
                c = TINY;
            }
            d = 1.0 / d;
            fraction *= d * c;
            if (d * c - 1.0).abs() <= f64::EPSILON {
                let q = (front * fraction).clamp(0.0, 1.0);
                return q.is_finite().then_some((1.0 - q, q));
            }
        }
        None
    }
}

fn gamma_lower(a: f64, x: f64) -> Option<f64> {
    gamma_reg(a, x).map(|(p, _)| p)
}

fn gamma_upper(a: f64, x: f64) -> Option<f64> {
    gamma_reg(a, x).map(|(_, q)| q)
}

// χ².

fn chi_square(x: f64, df: f64, ncp: f64, upper: bool) -> Option<f64> {
    if x.is_nan() || !degrees(df) || !noncentrality(ncp) {
        return None;
    }
    if x <= 0.0 {
        return Some(if upper { 1.0 } else { 0.0 });
    }
    if x == f64::INFINITY {
        return Some(if upper { 0.0 } else { 1.0 });
    }
    let p = if upper {
        mixture(ncp / 2.0, 0.0, false, |j| gamma_upper(df / 2.0 + j, x / 2.0))?
    } else {
        mixture(ncp / 2.0, 0.0, true, |j| gamma_lower(df / 2.0 + j, x / 2.0))?
    };
    Some(p.clamp(0.0, 1.0))
}

/// P(X ≤ x) for the χ² with `df` degrees of freedom and noncentrality `ncp` (λ).
pub fn chi_square_cdf(x: f64, df: f64, ncp: f64) -> Option<f64> {
    chi_square(x, df, ncp, false)
}

/// P(X > x) for the χ² with `df` degrees of freedom and noncentrality `ncp`.
pub fn chi_square_sf(x: f64, df: f64, ncp: f64) -> Option<f64> {
    chi_square(x, df, ncp, true)
}

/// The x with P(X ≤ x) = `p`.
pub fn chi_square_quantile(p: f64, df: f64, ncp: f64) -> Option<f64> {
    if !probability(p) || !degrees(df) || !noncentrality(ncp) {
        return None;
    }
    quantile(p, 0.0, f64::INFINITY, df + ncp, |x| chi_square_cdf(x, df, ncp), |x| chi_square_sf(x, df, ncp))
}

/// The noncentrality λ ≥ 0 with P(X ≤ x) = `p`; `None` when even λ = 0 gives less than `p`.
pub fn chi_square_ncp(x: f64, df: f64, p: f64) -> Option<f64> {
    if x.is_nan() || !degrees(df) {
        return None;
    }
    ncp(p, |l| chi_square_cdf(x, df, l), |l| chi_square_sf(x, df, l))
}

// Beta and F.

/// The beta series at x, given 1 − x too (computed apart, so it keeps its digits).
fn beta_series(x: f64, cx: f64, a: f64, b: f64, ncp: f64, upper: bool) -> Option<f64> {
    if x <= 0.0 {
        return Some(if upper { 1.0 } else { 0.0 });
    }
    if cx <= 0.0 {
        return Some(if upper { 0.0 } else { 1.0 });
    }
    let p = if upper {
        mixture(ncp / 2.0, 0.0, false, |j| beta_reg(b, a + j, cx, x))?
    } else {
        mixture(ncp / 2.0, 0.0, true, |j| beta_reg(a + j, b, x, cx))?
    };
    Some(p.clamp(0.0, 1.0))
}

fn beta(x: f64, a: f64, b: f64, ncp: f64, upper: bool) -> Option<f64> {
    if x.is_nan() || !degrees(a) || !degrees(b) || !noncentrality(ncp) {
        return None;
    }
    beta_series(x, 1.0 - x, a, b, ncp, upper)
}

/// P(X ≤ x) for the beta with shapes `a`, `b` and noncentrality `ncp`.
pub fn beta_cdf(x: f64, a: f64, b: f64, ncp: f64) -> Option<f64> {
    beta(x, a, b, ncp, false)
}

/// P(X > x) for the beta with shapes `a`, `b` and noncentrality `ncp`.
pub fn beta_sf(x: f64, a: f64, b: f64, ncp: f64) -> Option<f64> {
    beta(x, a, b, ncp, true)
}

/// The x with P(X ≤ x) = `p`.
pub fn beta_quantile(p: f64, a: f64, b: f64, ncp: f64) -> Option<f64> {
    if !probability(p) || !degrees(a) || !degrees(b) || !noncentrality(ncp) {
        return None;
    }
    quantile(p, 0.0, 1.0, 0.5, |x| beta_cdf(x, a, b, ncp), |x| beta_sf(x, a, b, ncp))
}

/// The noncentrality λ ≥ 0 with P(X ≤ x) = `p`; `None` when even λ = 0 gives less than `p`.
pub fn beta_ncp(x: f64, a: f64, b: f64, p: f64) -> Option<f64> {
    if x.is_nan() || !degrees(a) || !degrees(b) {
        return None;
    }
    ncp(p, |l| beta_cdf(x, a, b, l), |l| beta_sf(x, a, b, l))
}

fn f_dist(f: f64, df1: f64, df2: f64, ncp: f64, upper: bool) -> Option<f64> {
    if f.is_nan() || !degrees(df1) || !degrees(df2) || !noncentrality(ncp) {
        return None;
    }
    if f <= 0.0 {
        return Some(if upper { 1.0 } else { 0.0 });
    }
    if f == f64::INFINITY {
        return Some(if upper { 0.0 } else { 1.0 });
    }
    // F = (X₁/df1) / (X₂/df2) gives the beta at df1·f / (df1·f + df2).
    let scaled = df1 * f;
    let total = scaled + df2;
    beta_series(scaled / total, df2 / total, df1 / 2.0, df2 / 2.0, ncp, upper)
}

/// P(F ≤ f) for the F with `df1`, `df2` degrees of freedom and noncentrality `ncp`.
pub fn f_cdf(f: f64, df1: f64, df2: f64, ncp: f64) -> Option<f64> {
    f_dist(f, df1, df2, ncp, false)
}

/// P(F > f) for the F with `df1`, `df2` degrees of freedom and noncentrality `ncp`.
pub fn f_sf(f: f64, df1: f64, df2: f64, ncp: f64) -> Option<f64> {
    f_dist(f, df1, df2, ncp, true)
}

/// The f with P(F ≤ f) = `p`.
pub fn f_quantile(p: f64, df1: f64, df2: f64, ncp: f64) -> Option<f64> {
    if !probability(p) || !degrees(df1) || !degrees(df2) || !noncentrality(ncp) {
        return None;
    }
    quantile(p, 0.0, f64::INFINITY, 1.0 + ncp / df1, |x| f_cdf(x, df1, df2, ncp), |x| f_sf(x, df1, df2, ncp))
}

/// The noncentrality λ ≥ 0 with P(F ≤ f) = `p`; `None` when even λ = 0 gives less than `p`.
pub fn f_ncp(f: f64, df1: f64, df2: f64, p: f64) -> Option<f64> {
    if f.is_nan() || !degrees(df1) || !degrees(df2) {
        return None;
    }
    ncp(p, |l| f_cdf(f, df1, df2, l), |l| f_sf(f, df1, df2, l))
}

// t.

/// P(T ≤ t), or P(T > t) when `upper`, for t ≥ 0.
fn t_right(t: f64, df: f64, delta: f64, upper: bool) -> Option<f64> {
    let mu = delta * delta / 2.0;
    if t == 0.0 {
        return if upper { phi(delta) } else { phi(-delta) };
    }
    if upper && delta < 0.0 {
        return t_far_tail(t, df, delta);
    }
    // x = t²/(t² + ν) and 1 − x, without forming t², which overflows for a large t.
    let r = df / t / t;
    let (x, cx) = (1.0 / (1.0 + r), r / (1.0 + r));
    let half = df / 2.0;
    let sign = if delta < 0.0 { -1.0 } else { 1.0 };
    let p = if upper {
        let even = mixture(mu, 0.0, false, |j| beta_reg(half, j + 0.5, cx, x))?;
        let odd = mixture(mu, 0.5, false, |j| beta_reg(half, j + 1.0, cx, x))?;
        0.5 * (even + sign * odd)
    } else {
        let even = mixture(mu, 0.0, true, |j| beta_reg(j + 0.5, half, x, cx))?;
        let odd = mixture(mu, 0.5, true, |j| beta_reg(j + 1.0, half, x, cx))?;
        phi(-delta)? + 0.5 * (even + sign * odd)
    };
    Some(p.clamp(0.0, 1.0))
}

/// P(T > t) for t > 0 and δ < 0, the tail away from the noncentrality, where the series'
/// terms change sign and cancel. T = (Z + δ)/S with S = √(V/ν), V a χ² with ν degrees of
/// freedom, so P(T > t) = ∫ Φ(δ − t·s) f_S(s) ds, an integral of positive terms: summed by
/// Gauss–Legendre over the range where the integrand is within e⁻⁶⁰ of its peak, with the
/// panels doubled until the sum settles (halving towards 0 when the range reaches it). Below one degree of freedom the integral is taken
/// over w = s^ν, where the density has no pole at 0.
fn t_far_tail(t: f64, df: f64, delta: f64) -> Option<f64> {
    // Every Φ(δ − t·s) is at most Φ(δ).
    if phi(delta)? == 0.0 {
        return Some(0.0);
    }
    let a = df / 2.0;
    // ln f_S(s) = ln 2 + (ν/2) ln(ν/2) − ln Γ(ν/2) + (ν − 1) ln s − ν s²/2, with ln Γ by
    // Stirling so that its large terms cancel: ln 2 + ½ ln(a/2π) − rest(a) + (2a − 1) ln s
    // + a·[ln s² − (s² − 1)].
    let front = std::f64::consts::LN_2 + 0.5 * (a / (2.0 * std::f64::consts::PI)).ln() - stirling_rest(a);
    let pole = df < 1.0;
    let ln_integrand = |u: f64| -> Option<f64> {
        if u <= 0.0 {
            return Some(f64::NEG_INFINITY);
        }
        let s = if pole { u.powf(1.0 / df) } else { u };
        let tail = phi(delta - t * s)?;
        if tail == 0.0 {
            return Some(f64::NEG_INFINITY);
        }
        let density = if pole {
            // f_S(s) ds = f_S(s)·s/(ν w) dw.
            front - df.ln() + a * (1.0 - s * s)
        } else if s < 0.5 {
            front + (2.0 * a - 1.0) * s.ln() + a * (1.0 - s * s)
        } else {
            front - s.ln() + a * ln1p_minus((s - 1.0) * (s + 1.0))
        };
        Some(tail.ln() + density)
    };

    // The integrand is log-concave: its peak by golden section, then where it falls by 60.
    let reach = 1.0 + (200.0 / df).sqrt();
    let end = if pole { reach.powf(df) } else { reach };
    let golden = (5.0_f64.sqrt() - 1.0) / 2.0;
    let (mut lo, mut hi) = (0.0, end);
    for _ in 0..200 {
        let (m1, m2) = (hi - golden * (hi - lo), lo + golden * (hi - lo));
        if ln_integrand(m1)? < ln_integrand(m2)? {
            lo = m1;
        } else {
            hi = m2;
        }
        if hi - lo <= 1e-12 * end {
            break;
        }
    }
    let top_at = (lo + hi) / 2.0;
    let top = ln_integrand(top_at)?;
    if !top.is_finite() {
        return None;
    }
    let edge = |from: f64, to: f64| -> Option<f64> {
        // Where the integrand falls to e⁻⁶⁰ of its peak, between the peak `from` and `to`.
        if ln_integrand(to)? >= top - 60.0 {
            return Some(to);
        }
        let (mut inside, mut outside) = (from, to);
        for _ in 0..200 {
            let mid = (inside + outside) / 2.0;
            if ln_integrand(mid)? >= top - 60.0 {
                inside = mid;
            } else {
                outside = mid;
            }
        }
        Some(outside)
    };
    let (from, to) = (edge(top_at, 0.0)?, edge(top_at, end)?);

    // When the range reaches down to 0, where the density may behave like a fractional power of
    // s, the pieces halve towards it, so each one sees a smooth integrand.
    let span = to - from;
    let pieces: Vec<(f64, f64)> = if from < 1e-3 * span {
        let cut = |k: i32| from + span * 0.5_f64.powi(k);
        std::iter::once((from, cut(64))).chain((0..64).rev().map(|k| (cut(k + 1), cut(k)))).collect()
    } else {
        vec![(from, to)]
    };
    let nodes = gauss_legendre::<20>();
    let integrate = |panels: u32| -> Option<f64> {
        let mut sum = 0.0;
        for &(start, stop) in &pieces {
            let width = (stop - start) / f64::from(panels);
            for panel in 0..panels {
                let middle = start + width * (f64::from(panel) + 0.5);
                for (x, w) in nodes {
                    sum += w * width / 2.0 * (ln_integrand(middle + x * width / 2.0)? - top).exp();
                }
            }
        }
        Some(sum)
    };
    let mut panels = 1;
    let mut last = integrate(panels)?;
    while panels < 1 << 10 {
        panels *= 2;
        let next = integrate(panels)?;
        if (next - last).abs() <= 1e-14 * next {
            let p = next * top.exp();
            return p.is_finite().then_some(p.clamp(0.0, 1.0));
        }
        last = next;
    }
    None
}

/// The nodes and weights of the N-point Gauss–Legendre rule on [−1, 1]: the roots of the
/// Legendre polynomial P_N by Newton's method, from the usual cosine guesses, and the weights
/// 2 / ((1 − x²) P_N′(x)²).
fn gauss_legendre<const N: usize>() -> [(f64, f64); N] {
    let n = N as f64;
    let legendre = |x: f64| {
        let (mut p0, mut p1) = (1.0, x);
        for k in 2..=N {
            let k = k as f64;
            (p0, p1) = (p1, ((2.0 * k - 1.0) * x * p1 - (k - 1.0) * p0) / k);
        }
        // P_N(x) and P_N′(x).
        (p1, n * (x * p1 - p0) / (x * x - 1.0))
    };
    let mut rule = [(0.0, 0.0); N];
    for (i, node) in rule.iter_mut().enumerate() {
        let mut x = (std::f64::consts::PI * (i as f64 + 0.75) / (n + 0.5)).cos();
        for _ in 0..100 {
            let (p, dp) = legendre(x);
            let step = p / dp;
            x -= step;
            if step.abs() <= 1e-16 {
                break;
            }
        }
        let (_, dp) = legendre(x);
        *node = (x, 2.0 / ((1.0 - x * x) * dp * dp));
    }
    rule
}

fn t_dist(t: f64, df: f64, delta: f64, upper: bool) -> Option<f64> {
    if t.is_nan() || !degrees(df) || !delta.is_finite() {
        return None;
    }
    if t.is_infinite() {
        return Some(if (t > 0.0) == upper { 0.0 } else { 1.0 });
    }
    // −T is the t with noncentrality −δ.
    if t < 0.0 { t_right(-t, df, -delta, !upper) } else { t_right(t, df, delta, upper) }
}

/// P(T ≤ t) for the t with `df` degrees of freedom and noncentrality `delta` (δ).
pub fn t_cdf(t: f64, df: f64, delta: f64) -> Option<f64> {
    t_dist(t, df, delta, false)
}

/// P(T > t) for the t with `df` degrees of freedom and noncentrality `delta`.
pub fn t_sf(t: f64, df: f64, delta: f64) -> Option<f64> {
    t_dist(t, df, delta, true)
}

/// The t with P(T ≤ t) = `p`.
pub fn t_quantile(p: f64, df: f64, delta: f64) -> Option<f64> {
    if !probability(p) || !degrees(df) || !delta.is_finite() {
        return None;
    }
    quantile(p, f64::NEG_INFINITY, f64::INFINITY, delta, |x| t_cdf(x, df, delta), |x| t_sf(x, df, delta))
}

/// The noncentrality δ with P(T ≤ t) = `p`, for 0 < p < 1.
pub fn t_ncp(t: f64, df: f64, p: f64) -> Option<f64> {
    if t.is_nan() || !t.is_finite() || !degrees(df) || !(p > 0.0 && p < 1.0) {
        return None;
    }
    // P(T ≤ t) falls as δ grows, from 1 to 0.
    let rise = |d: f64| -> Option<f64> { if p <= 0.5 { Some(p - t_cdf(t, df, d)?) } else { Some(t_sf(t, df, d)? - (1.0 - p)) } };
    root(rise, t, f64::NEG_INFINITY, f64::INFINITY)
}

// Searches.

/// The x in [`low`, `high`] with cdf(x) = `p`, starting from `guess`; the upper tail is matched
/// above the median, so a p near 1 keeps its digits.
fn quantile(p: f64, low: f64, high: f64, guess: f64, cdf: impl Fn(f64) -> Option<f64>, sf: impl Fn(f64) -> Option<f64>) -> Option<f64> {
    if p == 0.0 {
        return Some(low);
    }
    if p == 1.0 {
        return Some(high);
    }
    let rise = |x: f64| -> Option<f64> { if p <= 0.5 { Some(cdf(x)? - p) } else { Some((1.0 - p) - sf(x)?) } };
    root(rise, guess, low, high)
}

/// The noncentrality λ ≥ 0 with cdf(λ) = `p` (cdf falling in λ).
fn ncp(p: f64, cdf: impl Fn(f64) -> Option<f64>, sf: impl Fn(f64) -> Option<f64>) -> Option<f64> {
    if !(p > 0.0 && p < 1.0) {
        return None;
    }
    let rise = |l: f64| -> Option<f64> { if p <= 0.5 { Some(p - cdf(l)?) } else { Some(sf(l)? - (1.0 - p)) } };
    let at_zero = rise(0.0)?;
    if at_zero > 0.0 {
        return None;
    }
    if at_zero == 0.0 {
        return Some(0.0);
    }
    root(rise, 1.0, 0.0, f64::INFINITY)
}

/// The root of `rise`, an increasing function on (`low`, `high`): bracketed by steps doubling
/// away from `guess`, then halved down to the last digits.
fn root(rise: impl Fn(f64) -> Option<f64>, guess: f64, low: f64, high: f64) -> Option<f64> {
    let guess = if guess > low && guess < high {
        guess
    } else if low.is_finite() {
        low + 1.0
    } else {
        0.0
    };
    let (mut lo, mut hi) = if rise(guess)? < 0.0 {
        // Upwards: towards `high`.
        let mut step = guess.abs().max(1.0);
        let mut lo = guess;
        let mut found = None;
        for _ in 0..MAX_DOUBLINGS {
            let next = (guess + step).min(high);
            if next == high || !next.is_finite() {
                found = Some(high);
                break;
            }
            if rise(next)? >= 0.0 {
                found = Some(next);
                break;
            }
            lo = next;
            step *= 2.0;
        }
        (lo, found?)
    } else {
        let mut step = guess.abs().max(1.0);
        let mut hi = guess;
        let mut found = None;
        for _ in 0..MAX_DOUBLINGS {
            let next = (guess - step).max(low);
            if next == low || !next.is_finite() {
                found = Some(low);
                break;
            }
            if rise(next)? < 0.0 {
                found = Some(next);
                break;
            }
            hi = next;
            step *= 2.0;
        }
        (found?, hi)
    };
    if !lo.is_finite() || !hi.is_finite() {
        // The root lies beyond every finite bracket.
        return None;
    }
    for _ in 0..MAX_HALVINGS {
        let mid = lo + (hi - lo) / 2.0;
        if mid <= lo || mid >= hi {
            return Some(mid);
        }
        if rise(mid)? < 0.0 {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hostile_arguments_give_none() {
        assert_eq!(chi_square_cdf(f64::NAN, 2.0, 1.0), None);
        assert_eq!(chi_square_cdf(1.0, 0.0, 1.0), None);
        assert_eq!(chi_square_cdf(1.0, 2.0, -1.0), None);
        assert_eq!(chi_square_cdf(1.0, f64::INFINITY, 1.0), None);
        assert_eq!(f_sf(1.0, 2.0, f64::NAN, 1.0), None);
        assert_eq!(beta_cdf(0.5, -1.0, 2.0, 1.0), None);
        assert_eq!(t_cdf(1.0, 5.0, f64::INFINITY), None);
        assert_eq!(t_quantile(1.5, 5.0, 1.0), None);
        assert_eq!(chi_square_ncp(1.0, 2.0, 0.0), None);
        assert_eq!(t_ncp(1.0, 5.0, 1.0), None);
    }

    #[test]
    fn the_ends_of_the_ranges() {
        assert_eq!(chi_square_cdf(0.0, 3.0, 2.0), Some(0.0));
        assert_eq!(chi_square_sf(f64::INFINITY, 3.0, 2.0), Some(0.0));
        assert_eq!(f_cdf(-1.0, 3.0, 4.0, 2.0), Some(0.0));
        assert_eq!(beta_sf(1.0, 2.0, 3.0, 1.0), Some(0.0));
        assert_eq!(t_cdf(f64::NEG_INFINITY, 3.0, 2.0), Some(0.0));
        assert_eq!(t_sf(f64::NEG_INFINITY, 3.0, 2.0), Some(1.0));
        assert_eq!(chi_square_quantile(0.0, 3.0, 2.0), Some(0.0));
        assert_eq!(t_quantile(1.0, 3.0, 2.0), Some(f64::INFINITY));
        // λ = 0 is the central distribution.
        let central = statrs::function::gamma::gamma_lr(1.5, 1.0);
        assert!((chi_square_cdf(2.0, 3.0, 0.0).unwrap_or(f64::NAN) - central).abs() < 1e-15);
    }

    #[test]
    fn at_zero_the_t_is_the_normal_at_delta() {
        assert_eq!(t_cdf(0.0, 7.0, 1.3), phi(-1.3));
        // Φ(−1.3), from mpmath at 30 digits.
        let p = phi(-1.3).unwrap_or(f64::NAN);
        assert!((p - 0.09680048458561033).abs() < 1e-16);
    }

    #[test]
    fn the_gauss_legendre_rule_integrates_polynomials() {
        let rule = gauss_legendre::<20>();
        let weights: f64 = rule.iter().map(|(_, w)| w).sum();
        assert!((weights - 2.0).abs() < 1e-14);
        // ∫ x³⁸ dx over [−1, 1] = 2/39: exact for a 20-point rule.
        let high: f64 = rule.iter().map(|(x, w)| w * x.powi(38)).sum();
        assert!((high - 2.0 / 39.0).abs() < 1e-14);
    }

    #[test]
    fn a_noncentrality_no_value_can_reach_is_none() {
        // With λ = 0, P(χ²₃ ≤ 1) ≈ 0.199: no λ ≥ 0 makes it 0.5.
        assert_eq!(chi_square_ncp(1.0, 3.0, 0.5), None);
    }
}
