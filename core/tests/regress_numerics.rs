mod common;
#[allow(unused_imports)]
use common::*;

use sdoe::linalg::{LsqDecomp, Mat, cholesky, inverse, sym_eigen};
use sdoe::stats::*;

fn rel_err(actual: f64, expected: f64) -> f64 {
    ((actual - expected) / expected).abs()
}

fn assert_rel(what: &str, actual: f64, expected: f64, tol: f64) {
    assert!(
        actual.is_finite() && rel_err(actual, expected) < tol,
        "{what}: actual {actual:e}, expected {expected:e} (rel tol {tol:e})"
    );
}

fn assert_nan(what: &str, actual: f64) {
    assert!(actual.is_nan(), "{what}: actual {actual:e}, expected NaN");
}

/// normal_cdf loses relative precision in the lower tail (x < 0).
#[test]
fn normal_cdf_lower_tail_is_accurate() {
    // R: pnorm(c(-5,-7,-8,-8.5,-10,-20)), digits=17
    let cases = [
        (-5.0, 2.8665157187919391e-07),
        (-7.0, 1.2798125438858352e-12),
        (-8.0, 6.2209605742717849e-16),
        (-8.5, 9.4795348222033192e-18),
        (-10.0, 7.6198530241605269e-24),
        (-20.0, 2.7536241186062337e-89),
    ];
    let bad: Vec<String> = cases
        .iter()
        .filter_map(|&(x, want)| {
            let got = normal_cdf(x);
            (!(got.is_finite() && rel_err(got, want) < 1e-10))
                .then(|| format!("x={x}: actual {got:e}, expected {want:e}"))
        })
        .collect();
    assert!(bad.is_empty(), "normal_cdf lower tail: {bad:?}");
}

/// normal_quantile is inaccurate for tiny p because the normal_cdf lower tail is inaccurate.
#[test]
fn normal_quantile_tiny_p_is_accurate() {
    // R: qnorm(1e-15), digits=17
    assert_rel("normal_quantile(1e-15)", normal_quantile(1e-15), -7.9413453261709952, 1e-10);
}

/// t_quantile returns -Inf for p below about 1e-16 because it negates the quantile of 1-p.
#[test]
fn t_quantile_tiny_p_is_finite() {
    // R: qt(1e-20, 10)
    assert_rel("t_quantile(1e-20,10)", t_quantile(1e-20, 10.0), -256.43469931852621, 1e-8);
    // R: qt(1e-300, 100)
    assert_rel("t_quantile(1e-300,100)", t_quantile(1e-300, 100.0), -9682.7342870695647, 1e-8);
}

/// t_quantile caps its search bracket at 1e12, so heavy-tailed quantiles are truncated.
#[test]
fn t_quantile_heavy_tail_exceeds_bracket_cap() {
    // R: qt(1e-10, 0.5)
    assert_rel("t_quantile(1e-10,0.5)", t_quantile(1e-10, 0.5), -10284911563163566080.0, 1e-8);
}

/// t_quantile loses accuracy for p close to 1 because 1-p cancels.
#[test]
fn t_quantile_upper_end_is_accurate() {
    // R: qt(1-1e-12, 1)
    assert_rel("t_quantile(1-1e-12,1)", t_quantile(1.0 - 1e-12, 1.0), 318316927901.77966, 1e-8);
}

/// f_quantile(1, ..) returns a finite value instead of +Inf.
#[test]
fn f_quantile_p_one_is_infinite() {
    let got = f_quantile(1.0, 2.0, 5.0);
    assert!(got == f64::INFINITY, "f_quantile(1,2,5): actual {got:e}, expected inf");
}

/// f_quantile caps its search bracket, so extreme upper quantiles are truncated.
#[test]
fn f_quantile_extreme_upper_exceeds_bracket_cap() {
    // R: qf(1-1e-12, 1, 1)
    assert_rel("f_quantile(1-1e-12,1,1)", f_quantile(1.0 - 1e-12, 1.0, 1.0), 4.0530266635530702e+23, 1e-8);
}

/// f_quantile is inaccurate for small p because of cancellation.
#[test]
fn f_quantile_small_p_is_accurate() {
    // Closed form for F(1,1): CDF = (2/π) atan(√f), so f = tan²(π p / 2).
    // R: tan(pi*1e-6/2)^2 (R's qf(1e-6,1,1) is off by 1.7e-5 relative here;
    // pf() of the closed-form value gives back 1e-6).
    assert_rel("f_quantile(1e-6,1,1)", f_quantile(1e-6, 1.0, 1.0), 2.4674011002763979e-12, 1e-8);
}

/// f_quantile accepts invalid degrees of freedom and NaN probabilities instead of returning NaN.
#[test]
fn f_quantile_invalid_arguments_give_nan() {
    assert_nan("f_quantile(0.5,0,5)", f_quantile(0.5, 0.0, 5.0));
    assert_nan("f_quantile(NaN,2,5)", f_quantile(f64::NAN, 2.0, 5.0));
}

/// t_cdf returns a number for zero degrees of freedom instead of NaN.
#[test]
fn t_cdf_zero_df_is_nan() {
    assert_nan("t_cdf(1,0)", t_cdf(1.0, 0.0));
}

/// t_cdf returns 0 for a NaN statistic instead of NaN.
#[test]
fn t_cdf_nan_statistic_is_nan() {
    assert_nan("t_cdf(NaN,5)", t_cdf(f64::NAN, 5.0));
}

/// t_cdf returns NaN for infinite df instead of the normal limit.
#[test]
fn t_cdf_infinite_df_is_normal_limit() {
    // R: pt(2, Inf)
    assert_rel("t_cdf(2,inf)", t_cdf(2.0, f64::INFINITY), 0.97724986805182079, 1e-10);
}

/// t_two_sided_p returns NaN for infinite df instead of the normal limit.
#[test]
fn t_two_sided_p_infinite_df_is_normal_limit() {
    // R: 2*pt(-2, Inf)
    assert_rel("t_two_sided_p(2,inf)", t_two_sided_p(2.0, f64::INFINITY), 0.045500263896358424, 1e-10);
}

/// t_quantile returns a number for a NaN probability instead of NaN.
#[test]
fn t_quantile_nan_probability_is_nan() {
    assert_nan("t_quantile(NaN,5)", t_quantile(f64::NAN, 5.0));
}

/// t_two_sided_p returns 0 for a NaN statistic, so saturated fits report p = 0.
#[test]
fn t_two_sided_p_nan_statistic_is_nan() {
    assert_nan("t_two_sided_p(NaN,5)", t_two_sided_p(f64::NAN, 5.0));
}

/// inc_gamma_p is off by about 1e-3 for large shape a.
#[test]
fn inc_gamma_p_large_shape_is_accurate() {
    // R: pgamma(1e7, 1e7)
    assert_rel("inc_gamma_p(1e7,1e7)", inc_gamma_p(1e7, 1e7), 0.50004205220872378, 1e-6);
}

/// ln_gamma returns NaN for negative non-integer x instead of ln|Gamma(x)|.
#[test]
fn ln_gamma_negative_argument_is_log_abs_gamma() {
    // R: lgamma(-0.5) = log(2*sqrt(pi))
    assert_rel("ln_gamma(-0.5)", ln_gamma(-0.5), 1.2655121234846454, 1e-10);
}

/// inverse rejects diag(1, 1e-14) because its singularity test uses a global scale.
#[test]
fn inverse_accepts_badly_scaled_diagonal() {
    let a = Mat::from_rows(&[vec![1.0, 0.0], vec![0.0, 1e-14]]);
    let inv = inverse(&a).expect("inverse(diag(1,1e-14)) returned None, expected Some");
    assert_rel("inv[0,0]", inv[(0, 0)], 1.0, 1e-12);
    assert_rel("inv[1,1]", inv[(1, 1)], 1e14, 1e-12);
}

/// inverse rejects diag(1e8, 1e-5) because its singularity test uses a global scale.
#[test]
fn inverse_accepts_wide_range_diagonal() {
    let a = Mat::from_rows(&[vec![1e8, 0.0], vec![0.0, 1e-5]]);
    let inv = inverse(&a).expect("inverse(diag(1e8,1e-5)) returned None, expected Some");
    assert_rel("inv[0,0]", inv[(0, 0)], 1e-8, 1e-12);
    assert_rel("inv[1,1]", inv[(1, 1)], 1e5, 1e-12);
}

/// cholesky rejects diag(1, 1e-14) and diag(1e8, 1e-5) because its pivot test uses a global scale.
#[test]
fn cholesky_accepts_badly_scaled_diagonals() {
    let a = Mat::from_rows(&[vec![1.0, 0.0], vec![0.0, 1e-14]]);
    let l = cholesky(&a).expect("cholesky(diag(1,1e-14)) returned None, expected Some");
    assert_rel("L[1,1] of diag(1,1e-14)", l[(1, 1)], 1e-7, 1e-12);
    let b = Mat::from_rows(&[vec![1e8, 0.0], vec![0.0, 1e-5]]);
    let l = cholesky(&b).expect("cholesky(diag(1e8,1e-5)) returned None, expected Some");
    assert_rel("L[0,0] of diag(1e8,1e-5)", l[(0, 0)], 1e4, 1e-12);
}

/// Guard (passes today): the singular matrix 1..9 must stay rejected by inverse.
#[test]
fn inverse_still_rejects_singular_matrix() {
    let a = Mat::from_rows(&[
        vec![1.0, 2.0, 3.0],
        vec![4.0, 5.0, 6.0],
        vec![7.0, 8.0, 9.0],
    ]);
    assert!(inverse(&a).is_none(), "inverse of the singular 1..9 matrix returned Some");
}

/// LsqDecomp::new drops a small-magnitude but independent column as aliased because the rank test is absolute.
#[test]
fn lsq_decomp_keeps_small_scale_independent_column() {
    let x = Mat::from_rows(&[
        vec![1e-10, 1.0],
        vec![2e-10, 0.0],
        vec![3e-10, 1.0],
        vec![4e-10, 0.0],
    ]);
    let d = LsqDecomp::new(&x, 1e-9);
    assert!(
        d.kept == vec![0, 1] && d.dropped.is_empty(),
        "kept {:?}, dropped {:?}; expected kept [0, 1], dropped []",
        d.kept,
        d.dropped
    );
}

/// sym_eigen stops at an absolute threshold, so tiny-scale matrices return the diagonal unrotated.
#[test]
fn sym_eigen_converges_for_tiny_scale_matrix() {
    let a = Mat::from_rows(&[vec![2e-12, 1e-12], vec![1e-12, 2e-12]]);
    let (vals, _) = sym_eigen(&a);
    assert_eq!(vals.len(), 2);
    assert_rel("eigenvalue 0", vals[0], 3e-12, 1e-9);
    assert_rel("eigenvalue 1", vals[1], 1e-12, 1e-9);
}
