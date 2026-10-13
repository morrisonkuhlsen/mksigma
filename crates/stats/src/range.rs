//! The studentized range distribution, for Tukey's HSD. From its definition: with `k` groups and
//! `df` error degrees of freedom,
//!
//! P(Q ≤ q) = ∫₀^∞ f(s) · W(q·s) ds,   W(x) = k ∫ φ(z) [Φ(z) − Φ(z − x)]^(k−1) dz,
//!
//! where `f` is the density of s = √(χ²_df / df). Both integrals are taken with composite
//! Simpson's rule over ranges that hold all but a negligible part of the mass.

use statrs::distribution::{ChiSquared, Continuous, ContinuousCDF, Normal};

fn simpson(a: f64, b: f64, intervals: usize, f: impl Fn(f64) -> f64) -> f64 {
    let n = intervals + intervals % 2;
    let h = (b - a) / n as f64;
    let mut sum = f(a) + f(b);
    for i in 1..n {
        let weight = if i % 2 == 1 { 4.0 } else { 2.0 };
        sum += weight * f(a + i as f64 * h);
    }
    sum * h / 3.0
}

/// P(range of k standard normals ≤ x): the distribution with infinite degrees of freedom.
fn w(x: f64, k: f64, normal: &Normal) -> f64 {
    if x <= 0.0 {
        return 0.0;
    }
    let inner = |z: f64| normal.pdf(z) * (normal.cdf(z) - normal.cdf(z - x)).max(0.0).powf(k - 1.0);
    (k * simpson(-8.5, 8.5 + x, 200, inner)).min(1.0)
}

/// P(Q ≤ q) for the studentized range with `k` means and `df` degrees of freedom.
pub(crate) fn cdf(q: f64, k: f64, df: f64) -> Option<f64> {
    if !(q.is_finite() && k >= 2.0 && df >= 1.0) {
        return None;
    }
    if q <= 0.0 {
        return Some(0.0);
    }
    let normal = Normal::new(0.0, 1.0).ok()?;
    if df > 50_000.0 {
        return Some(w(q, k, &normal));
    }
    // s = √(χ²/df): integrate over the central 1 − 2·10⁻¹² of its distribution.
    let chi = ChiSquared::new(df).ok()?;
    let (lo, hi) = ((chi.inverse_cdf(1e-12) / df).sqrt(), (chi.inverse_cdf(1.0 - 1e-12) / df).sqrt());
    let density = |s: f64| {
        // f(s) = 2 s df · χ²_pdf(df s²), in logs: the χ² density itself overflows for large df.
        if s <= 0.0 { 0.0 } else { (chi.ln_pdf(df * s * s) + (2.0 * s * df).ln()).exp() }
    };
    Some(simpson(lo, hi, 120, |s| density(s) * w(q * s, k, &normal)).clamp(0.0, 1.0))
}

/// The q with P(Q ≤ q) = p, by bisection.
pub(crate) fn quantile(p: f64, k: f64, df: f64) -> Option<f64> {
    if !(p > 0.0 && p < 1.0) {
        return None;
    }
    let (mut lo, mut hi) = (0.0, 1.0);
    while cdf(hi, k, df)? < p {
        hi *= 2.0;
        if hi > 1e4 {
            return None;
        }
    }
    for _ in 0..60 {
        let mid = (lo + hi) / 2.0;
        if cdf(mid, k, df)? < p {
            lo = mid
        } else {
            hi = mid
        }
        if hi - lo < 1e-7 {
            break;
        }
    }
    Some((lo + hi) / 2.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Reference values from the published table of upper 5% points (q such that P = 0.95).
    #[test]
    fn matches_published_critical_values() {
        for (k, df, q) in [(3.0, 10.0, 3.877), (4.0, 20.0, 3.958), (2.0, 5.0, 3.635), (5.0, 60.0, 3.977), (3.0, 237.0, 3.3355)] {
            let p = cdf(q, k, df).unwrap();
            assert!((p - 0.95).abs() < 2e-4, "k={k} df={df}: P({q}) = {p}");
            let back = quantile(0.95, k, df).unwrap();
            assert!((back - q).abs() < 2e-3, "k={k} df={df}: q = {back}");
        }
    }

    /// Reference values computed independently with scipy.stats.studentized_range.cdf.
    #[test]
    fn matches_an_independent_implementation() {
        let cases = [
            (1.5, 3.0, 10.0, 0.4419518273117344),
            (3.2, 4.0, 20.0, 0.8592442649743044),
            (5.0, 6.0, 8.0, 0.941516675336415),
            (2.5, 3.0, 200.0, 0.8168705211325364),
            (4.1, 10.0, 30.0, 0.8499665698844004),
            // Large df, where the χ² density overflows unless taken in logs.
            (2.0, 3.0, 237.0, 0.6648114828400998),
        ];
        for (q, k, df, expected) in cases {
            let p = cdf(q, k, df).unwrap();
            assert!((p - expected).abs() < 1e-5, "q={q} k={k} df={df}: {p} vs {expected}");
        }
    }
}
