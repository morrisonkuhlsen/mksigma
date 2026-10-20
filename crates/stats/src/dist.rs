//! Significance levels from the chi-square, t and F distributions (via `statrs`), and the
//! hypergeometric probabilities behind Fisher's exact test.

use statrs::distribution::{ChiSquared, ContinuousCDF, FisherSnedecor, StudentsT};

// statrs panics on a NaN argument: every function here returns `None` for one instead, and
// treats an infinite statistic as the limit it is (a significance of 0).

/// P(χ² ≥ x) with `df` degrees of freedom.
pub(crate) fn chi_square_sig(x: f64, df: f64) -> Option<f64> {
    if x.is_nan() || df.is_nan() {
        return None;
    }
    if x == f64::INFINITY {
        return Some(0.0);
    }
    let d = ChiSquared::new(df).ok()?;
    Some(d.sf(x))
}

/// Two-sided P(|T| ≥ |t|) with `df` degrees of freedom.
pub(crate) fn t_sig(t: f64, df: f64) -> Option<f64> {
    if t.is_nan() || df.is_nan() {
        return None;
    }
    if t.is_infinite() {
        return Some(0.0);
    }
    let d = StudentsT::new(0.0, 1.0, df).ok()?;
    Some(2.0 * d.sf(t.abs()))
}

/// The t value with `p` of the distribution below it, for confidence intervals.
pub(crate) fn t_quantile(p: f64, df: f64) -> Option<f64> {
    if !(0.0..=1.0).contains(&p) || df.is_nan() {
        return None;
    }
    let d = StudentsT::new(0.0, 1.0, df).ok()?;
    Some(d.inverse_cdf(p))
}

/// The standard normal value with `p` of the distribution below it.
pub(crate) fn normal_quantile(p: f64) -> Option<f64> {
    if !(0.0..=1.0).contains(&p) {
        return None;
    }
    let d = statrs::distribution::Normal::new(0.0, 1.0).ok()?;
    Some(d.inverse_cdf(p))
}

/// P(F ≥ f) with `df1`, `df2` degrees of freedom.
pub(crate) fn f_sig(f: f64, df1: f64, df2: f64) -> Option<f64> {
    if f.is_nan() || df1.is_nan() || df2.is_nan() {
        return None;
    }
    if f == f64::INFINITY {
        return Some(0.0);
    }
    let d = FisherSnedecor::new(df1, df2).ok()?;
    Some(d.sf(f))
}

fn ln_factorial(n: f64) -> f64 {
    statrs::function::gamma::ln_gamma(n + 1.0)
}

/// Fisher's exact test for the 2×2 table `[[a, b], [c, d]]` (whole counts): the two-sided
/// significance (tables at most as likely as the one seen) and the one-sided one (the smaller
/// tail).
pub(crate) fn fisher_exact(a: f64, b: f64, c: f64, d: f64) -> (f64, f64) {
    let (r1, r2, c1, n) = (a + b, c + d, a + c, a + b + c + d);
    let ln_p = |x: f64| {
        ln_factorial(r1) + ln_factorial(r2) + ln_factorial(c1) + ln_factorial(n - c1)
            - ln_factorial(n)
            - ln_factorial(x)
            - ln_factorial(r1 - x)
            - ln_factorial(c1 - x)
            - ln_factorial(r2 - c1 + x)
    };
    let low = (c1 - r2).max(0.0) as i64;
    let high = r1.min(c1) as i64;
    let observed = ln_p(a).exp();
    let (mut two, mut lower, mut upper) = (0.0, 0.0, 0.0);
    for x in low..=high {
        let x = x as f64;
        let p = ln_p(x).exp();
        // Relative tolerance: tables as likely as the observed one count, despite rounding.
        if p <= observed * (1.0 + 1e-7) {
            two += p;
        }
        if x <= a {
            lower += p;
        }
        if x >= a {
            upper += p;
        }
    }
    (two.min(1.0), lower.min(upper).min(1.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statistics_that_arent_numbers_dont_crash() {
        // An exact fit gives t = b / 0: infinite, or NaN for 0 / 0.
        assert_eq!(t_sig(f64::NAN, 5.0), None);
        assert_eq!(t_sig(f64::INFINITY, 5.0), Some(0.0));
        assert_eq!(t_sig(f64::NEG_INFINITY, 5.0), Some(0.0));
        assert_eq!(f_sig(f64::NAN, 1.0, 5.0), None);
        assert_eq!(f_sig(f64::INFINITY, 1.0, 5.0), Some(0.0));
        assert_eq!(chi_square_sig(f64::NAN, 1.0), None);
        assert_eq!(chi_square_sig(f64::INFINITY, 1.0), Some(0.0));
        assert_eq!(t_quantile(f64::NAN, 5.0), None);
        assert_eq!(t_sig(1.0, f64::NAN), None);
        assert_eq!(normal_quantile(f64::NAN), None);
    }
}
