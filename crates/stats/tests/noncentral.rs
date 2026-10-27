//! The noncentral distributions against reference values computed for these tests with mpmath
//! at 40 digits (BSD; χ², beta and F from their Poisson mixtures, the t by quadrature over the
//! χ² of its denominator, so independently of the series MK Sigma sums) and checked against
//! scipy.stats `ncx2`, `ncf` and `nct` (BSD), re-typed here as constants. Below four degrees of
//! freedom the t's quadrature runs over w = s^ν, where the density has no pole. Each probability is
//! held to 1e-10 relative, both tails, across the ranges power analysis uses.
#![allow(clippy::excessive_precision)]

use mksigma_stats::noncentral as nc;

const RELATIVE: f64 = 1e-10;

fn close(name: &str, got: Option<f64>, want: f64) {
    let Some(got) = got else {
        panic!("{name}: no value, wanted {want:e}");
    };
    let error = (got - want).abs();
    assert!(error <= RELATIVE * want.abs() || error < 1e-300, "{name}: {got:e}, wanted {want:e} (relative error {:.1e})", error / want.abs());
}

/// (x, df, λ, P(X ≤ x), P(X > x)).
const CHI_SQUARE: [(f64, f64, f64, f64, f64); 12] = [
    (0.5, 1.0, 0.5, 0.42135039647485745, 0.5786496035251426),
    (3.0, 2.0, 1.0, 0.6206436532195436, 0.3793563467804564),
    (10.0, 5.0, 3.0, 0.7172368464311432, 0.28276315356885684),
    (50.0, 10.0, 40.0, 0.5287105342177714, 0.4712894657822287),
    (1000.0, 100.0, 900.0, 0.5063589811559768, 0.4936410188440233),
    (11000.0, 1000.0, 10000.0, 0.5019157623999378, 0.4980842376000621),
    (1010000.0, 1000000.0, 10000.0, 0.5001880359033944, 0.49981196409660555),
    (5.0, 3.0, 20.0, 0.005351168063011945, 0.994648831936988),
    (100.0, 4.0, 1.0, 1.0, 3.488176713111085e-18),
    (20000.0, 10.0, 10000.0, 1.0, 0.0),
    (10000.0, 10.0, 10000.0, 0.48205335570736285, 0.5179466442926371),
    (0.01, 1.0, 30.0, 2.56019459347329e-08, 0.9999999743980541),
];

/// (f, df1, df2, λ, P(F ≤ f), P(F > f)).
const F: [(f64, f64, f64, f64, f64, f64); 9] = [
    (1.0, 1.0, 10.0, 1.0, 0.46343819462597113, 0.5365618053740289),
    (2.5, 3.0, 20.0, 5.0, 0.5163549992552664, 0.4836450007447336),
    (4.0, 2.0, 100.0, 10.0, 0.3080274533759656, 0.6919725466240344),
    (1.5, 10.0, 1000000.0, 50.0, 2.5298141856120465e-05, 0.9999747018581439),
    (1.2, 100.0, 1000.0, 200.0, 3.051172145694454e-12, 0.9999999999969488),
    (3.0, 5.0, 30.0, 10000.0, 0.0, 1.0),
    (0.2, 4.0, 12.0, 0.5, 0.053897290157938094, 0.9461027098420619),
    (20.0, 2.0, 40.0, 2.0, 0.9999483104934522, 5.168950654774669e-05),
    (1.01, 1000.0, 1000000.0, 10000.0, 0.0, 1.0),
];

/// (x, a, b, λ, P(X ≤ x), P(X > x)).
const BETA: [(f64, f64, f64, f64, f64, f64); 5] = [
    (0.5, 2.0, 3.0, 1.0, 0.5977904448184806, 0.4022095551815193),
    (0.9, 5.0, 2.0, 10.0, 0.6983930640447402, 0.3016069359552597),
    (0.1, 1.0, 1.0, 4.0, 0.016529888822158657, 0.9834701111778413),
    (0.99, 0.5, 0.5, 100.0, 0.31731050786291387, 0.6826894921370861),
    (0.3, 10.0, 20.0, 1000.0, 3.165647655314586e-135, 1.0),
];

/// (t, df, δ, P(T ≤ t), P(T > t)).
const T: [(f64, f64, f64, f64, f64); 25] = [
    (1.0, 10.0, 1.0, 0.4902400513954507, 0.5097599486045493),
    (2.0, 5.0, 1.5, 0.6314492472556718, 0.3685507527443283),
    (-1.0, 8.0, 0.5, 0.07672309311130025, 0.9232769068886998),
    (3.0, 20.0, -1.0, 0.9998469152055283, 0.00015308479447168896),
    (0.5, 1.0, 2.0, 0.06327688747994675, 0.9367231125200532),
    (2.5, 1000000.0, 2.0, 0.6914619661828861, 0.30853803381711387),
    (10.0, 30.0, 12.0, 0.1016883658492792, 0.8983116341507208),
    (-2.0, 15.0, 1.0, 0.0025682523293202465, 0.9974317476706798),
    (15.0, 50.0, 10.0, 0.9974938219693124, 0.002506178030687692),
    (2.0, 10.0, 0.0, 0.9633059826146299, 0.03669401738537018),
    (1.7, 25.0, 3.3, 0.057963070409263594, 0.9420369295907364),
    (40.0, 100.0, 30.0, 0.9996822979723854, 0.0003177020276146001),
    (0.5, 0.3, -1.0, 0.8859623949889958, 0.11403760501100421),
    (0.5, 0.3, 1.0, 0.24959424404092423, 0.7504057559590758),
    (2.0, 0.3, 3.0, 0.1433622809875715, 0.8566377190124285),
    (0.5, 0.7, 0.5, 0.4479811360722299, 0.5520188639277701),
    (1.0, 1.0, -2.0, 0.9938142398653379, 0.006185760134662118),
    (2.0, 1000000.0, -1.0, 0.9986500864568566, 0.001349913543143393),
    (140.0, 10000.0, 141.0, 0.23781093777820614, 0.7621890622217938),
    (-1.0, 3.0, 4.0, 1.5878344612792132e-06, 0.9999984121655388),
    // The tail away from δ, where the series cancels: these come by quadrature.
    (1.0, 10.0, -5.0, 0.9999999975110271, 2.4889728823491635e-09),
    (0.5, 30.0, -8.0, 1.0, 1.1379733103147325e-17),
    (2.0, 5.0, -3.0, 0.9999922003639081, 7.799636091832038e-06),
    (0.1, 50.0, -12.0, 1.0, 5.3516697581272795e-34),
    (0.0, 10.0, -6.0, 0.9999999990134123, 9.86587645037698e-10),
];

#[test]
fn chi_square() {
    for (x, df, ncp, cdf, sf) in CHI_SQUARE {
        let name = format!("χ²({x}; {df}, {ncp})");
        close(&format!("{name} cdf"), nc::chi_square_cdf(x, df, ncp), cdf);
        close(&format!("{name} sf"), nc::chi_square_sf(x, df, ncp), sf);
    }
}

#[test]
fn f() {
    for (f, df1, df2, ncp, cdf, sf) in F {
        let name = format!("F({f}; {df1}, {df2}, {ncp})");
        close(&format!("{name} cdf"), nc::f_cdf(f, df1, df2, ncp), cdf);
        close(&format!("{name} sf"), nc::f_sf(f, df1, df2, ncp), sf);
    }
}

#[test]
fn beta() {
    for (x, a, b, ncp, cdf, sf) in BETA {
        let name = format!("beta({x}; {a}, {b}, {ncp})");
        close(&format!("{name} cdf"), nc::beta_cdf(x, a, b, ncp), cdf);
        close(&format!("{name} sf"), nc::beta_sf(x, a, b, ncp), sf);
    }
}

#[test]
fn t() {
    for (t, df, delta, cdf, sf) in T {
        let name = format!("t({t}; {df}, {delta})");
        close(&format!("{name} cdf"), nc::t_cdf(t, df, delta), cdf);
        close(&format!("{name} sf"), nc::t_sf(t, df, delta), sf);
    }
}

#[test]
fn quantiles_invert_the_distribution_functions() {
    for p in [1e-9, 0.025, 0.5, 0.8, 0.975, 1.0 - 1e-9] {
        let x = nc::chi_square_quantile(p, 7.0, 12.5);
        close(&format!("χ² quantile {p}"), x.and_then(|x| nc::chi_square_cdf(x, 7.0, 12.5)), p);
        let f = nc::f_quantile(p, 3.0, 40.0, 9.0);
        close(&format!("F quantile {p}"), f.and_then(|f| nc::f_cdf(f, 3.0, 40.0, 9.0)), p);
        let x = nc::beta_quantile(p, 2.5, 6.0, 4.0);
        close(&format!("beta quantile {p}"), x.and_then(|x| nc::beta_cdf(x, 2.5, 6.0, 4.0)), p);
        let t = nc::t_quantile(p, 12.0, 2.2);
        close(&format!("t quantile {p}"), t.and_then(|t| nc::t_cdf(t, 12.0, 2.2)), p);
    }
}

#[test]
fn noncentralities_give_the_probability_asked() {
    // The power of a test: the λ that leaves 1 − power below the critical value (11.07 and
    // 2.87 are the 5% points, so P = 0.95 at λ = 0 and any p below it can be reached; for the
    // beta, 0.6 is about its 96th percentile).
    for p in [0.05, 0.2, 0.5, 0.9] {
        let l = nc::chi_square_ncp(11.07, 5.0, p);
        close(&format!("χ² ncp {p}"), l.and_then(|l| nc::chi_square_cdf(11.07, 5.0, l)), p);
        let l = nc::f_ncp(2.87, 3.0, 36.0, p);
        close(&format!("F ncp {p}"), l.and_then(|l| nc::f_cdf(2.87, 3.0, 36.0, l)), p);
        let l = nc::beta_ncp(0.6, 2.0, 5.0, p);
        close(&format!("beta ncp {p}"), l.and_then(|l| nc::beta_cdf(0.6, 2.0, 5.0, l)), p);
    }
    // A confidence interval for δ: the noncentralities that put the t seen at the 97.5th and
    // 2.5th percentiles, either sign.
    for (t, p) in [(2.5, 0.975), (2.5, 0.025), (-0.4, 0.975), (-0.4, 0.025)] {
        let d = nc::t_ncp(t, 18.0, p);
        close(&format!("t ncp {t} {p}"), d.and_then(|d| nc::t_cdf(t, 18.0, d)), p);
    }
}
