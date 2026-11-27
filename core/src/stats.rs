//! Probability distributions and descriptive statistics.

use std::f64::consts::PI;

/// Natural logarithm of the absolute value of the gamma function, ln|Γ(x)|
/// (Lanczos approximation, g = 7). Non-positive integers give +∞.
pub fn ln_gamma(x: f64) -> f64 {
    const G: [f64; 9] = [
        0.999_999_999_999_809_9,
        676.520_368_121_885_1,
        -1_259.139_216_722_402_8,
        771.323_428_777_653_1,
        -176.615_029_162_140_6,
        12.507_343_278_686_905,
        -0.138_571_095_265_720_12,
        9.984_369_578_019_572e-6,
        1.505_632_735_149_311_6e-7,
    ];
    if x < 0.5 {
        if x == x.floor() {
            return f64::INFINITY;
        }
        // Reflection: Γ(x) Γ(1 - x) = π / sin(πx).
        (PI / (PI * x).sin().abs()).ln() - ln_gamma(1.0 - x)
    } else {
        let x = x - 1.0;
        let mut a = G[0];
        let t = x + 7.5;
        for (i, g) in G.iter().enumerate().skip(1) {
            a += g / (x + i as f64);
        }
        0.5 * (2.0 * PI).ln() + (x + 0.5) * t.ln() - t + a.ln()
    }
}

/// Continued fraction for the incomplete beta function (modified Lentz).
fn beta_cf(a: f64, b: f64, x: f64) -> f64 {
    const FPMIN: f64 = 1e-300;
    let qab = a + b;
    let qap = a + 1.0;
    let qam = a - 1.0;
    let mut c = 1.0;
    let mut d = 1.0 - qab * x / qap;
    if d.abs() < FPMIN {
        d = FPMIN;
    }
    d = 1.0 / d;
    let mut h = d;
    for m in 1..10_000 {
        let m = m as f64;
        let m2 = 2.0 * m;
        let aa = m * (b - m) * x / ((qam + m2) * (a + m2));
        d = 1.0 + aa * d;
        if d.abs() < FPMIN {
            d = FPMIN;
        }
        c = 1.0 + aa / c;
        if c.abs() < FPMIN {
            c = FPMIN;
        }
        d = 1.0 / d;
        h *= d * c;
        let aa = -(a + m) * (qab + m) * x / ((a + m2) * (qap + m2));
        d = 1.0 + aa * d;
        if d.abs() < FPMIN {
            d = FPMIN;
        }
        c = 1.0 + aa / c;
        if c.abs() < FPMIN {
            c = FPMIN;
        }
        d = 1.0 / d;
        let del = d * c;
        h *= del;
        if (del - 1.0).abs() < 1e-15 {
            break;
        }
    }
    h
}

/// Regularised incomplete beta function I_x(a, b).
pub fn inc_beta(a: f64, b: f64, x: f64) -> f64 {
    if x.is_nan() || a.is_nan() || b.is_nan() {
        return f64::NAN;
    }
    if x <= 0.0 {
        return 0.0;
    }
    if x >= 1.0 {
        return 1.0;
    }
    let ln_bt = ln_gamma(a + b) - ln_gamma(a) - ln_gamma(b) + a * x.ln() + b * (1.0 - x).ln();
    let bt = ln_bt.exp();
    if x < (a + 1.0) / (a + b + 2.0) {
        bt * beta_cf(a, b, x) / a
    } else {
        1.0 - bt * beta_cf(b, a, 1.0 - x) / b
    }
}

/// Iteration limit of the incomplete gamma series and continued fraction:
/// both need on the order of √a terms for large shape `a`.
fn gamma_iterations(a: f64) -> usize {
    10_000 + (100.0 * a.max(0.0).sqrt()).min(1e8) as usize
}

/// Series for P(a, x), used for x < a + 1.
fn gamma_series(a: f64, x: f64) -> f64 {
    let mut ap = a;
    let mut sum = 1.0 / a;
    let mut del = sum;
    for _ in 0..gamma_iterations(a) {
        ap += 1.0;
        del *= x / ap;
        sum += del;
        if del.abs() < sum.abs() * 1e-16 {
            break;
        }
    }
    sum * (-x + a * x.ln() - ln_gamma(a)).exp()
}

/// Continued fraction for Q(a, x) (modified Lentz), used for x >= a + 1.
fn gamma_cf(a: f64, x: f64) -> f64 {
    const FPMIN: f64 = 1e-300;
    let mut b = x + 1.0 - a;
    let mut c = 1.0 / FPMIN;
    let mut d = 1.0 / b;
    let mut h = d;
    for i in 1..gamma_iterations(a) {
        let an = -(i as f64) * (i as f64 - a);
        b += 2.0;
        d = an * d + b;
        if d.abs() < FPMIN {
            d = FPMIN;
        }
        c = b + an / c;
        if c.abs() < FPMIN {
            c = FPMIN;
        }
        d = 1.0 / d;
        let del = d * c;
        h *= del;
        if (del - 1.0).abs() < 1e-16 {
            break;
        }
    }
    (-x + a * x.ln() - ln_gamma(a)).exp() * h
}

/// Regularised lower incomplete gamma function P(a, x).
pub fn inc_gamma_p(a: f64, x: f64) -> f64 {
    if x.is_nan() || a.is_nan() || a <= 0.0 {
        return f64::NAN;
    }
    if x <= 0.0 {
        return 0.0;
    }
    if x.is_infinite() {
        return 1.0;
    }
    if x < a + 1.0 { gamma_series(a, x) } else { 1.0 - gamma_cf(a, x) }
}

/// Regularised upper incomplete gamma function Q(a, x) = 1 - P(a, x),
/// computed without cancellation where Q is small.
pub fn inc_gamma_q(a: f64, x: f64) -> f64 {
    if x.is_nan() || a.is_nan() || a <= 0.0 {
        return f64::NAN;
    }
    if x <= 0.0 {
        return 1.0;
    }
    if x.is_infinite() {
        return 0.0;
    }
    if x < a + 1.0 { 1.0 - gamma_series(a, x) } else { gamma_cf(a, x) }
}

pub fn erf(x: f64) -> f64 {
    if x >= 0.0 { inc_gamma_p(0.5, x * x) } else { -inc_gamma_p(0.5, x * x) }
}

/// Complementary error function 1 - erf(x), accurate in relative terms for
/// large positive x.
pub fn erfc(x: f64) -> f64 {
    if x >= 0.0 { inc_gamma_q(0.5, x * x) } else { 1.0 + inc_gamma_p(0.5, x * x) }
}

/// Standard normal CDF. The tail beyond |x| comes from Q(1/2, x²/2), so
/// lower-tail values keep full relative precision.
pub fn normal_cdf(x: f64) -> f64 {
    if x.is_nan() {
        return f64::NAN;
    }
    let tail = 0.5 * inc_gamma_q(0.5, 0.5 * x * x);
    if x < 0.0 { tail } else { 1.0 - tail }
}

pub fn normal_pdf(x: f64) -> f64 {
    (-0.5 * x * x).exp() / (2.0 * PI).sqrt()
}

/// Inverse of the standard normal CDF (Acklam's algorithm with one Halley
/// step). Upper-half probabilities use the symmetry q(p) = -q(1 - p); 1 - p
/// is exact for p >= 0.5.
pub fn normal_quantile(p: f64) -> f64 {
    if p.is_nan() {
        return f64::NAN;
    }
    if p <= 0.0 {
        return f64::NEG_INFINITY;
    }
    if p >= 1.0 {
        return f64::INFINITY;
    }
    if p > 0.5 {
        return -normal_quantile(1.0 - p);
    }
    const A: [f64; 6] = [
        -3.969683028665376e+01,
        2.209460984245205e+02,
        -2.759285104469687e+02,
        1.383577518672690e+02,
        -3.066479806614716e+01,
        2.506628277459239e+00,
    ];
    const B: [f64; 5] = [
        -5.447609879822406e+01,
        1.615858368580409e+02,
        -1.556989798598866e+02,
        6.680131188771972e+01,
        -1.328068155288572e+01,
    ];
    const C: [f64; 6] = [
        -7.784894002430293e-03,
        -3.223964580411365e-01,
        -2.400758277161838e+00,
        -2.549732539343734e+00,
        4.374664141464968e+00,
        2.938163982698783e+00,
    ];
    const D: [f64; 4] = [
        7.784695709041462e-03,
        3.224671290700398e-01,
        2.445134137142996e+00,
        3.754408661907416e+00,
    ];
    let plow = 0.02425;
    let x = if p < plow {
        let q = (-2.0 * p.ln()).sqrt();
        (((((C[0] * q + C[1]) * q + C[2]) * q + C[3]) * q + C[4]) * q + C[5])
            / ((((D[0] * q + D[1]) * q + D[2]) * q + D[3]) * q + 1.0)
    } else {
        let q = p - 0.5;
        let r = q * q;
        (((((A[0] * r + A[1]) * r + A[2]) * r + A[3]) * r + A[4]) * r + A[5]) * q
            / (((((B[0] * r + B[1]) * r + B[2]) * r + B[3]) * r + B[4]) * r + 1.0)
    };
    let e = normal_cdf(x) - p;
    let u = e * (2.0 * PI).sqrt() * (x * x / 2.0).exp();
    let x = x - u / (1.0 + x * u / 2.0);
    if x.is_finite() { x } else { f64::NEG_INFINITY }
}

/// Probability that a Student t variable with `df` degrees of freedom
/// exceeds |t| (one tail).
fn t_tail(t: f64, df: f64) -> f64 {
    if df.is_infinite() {
        return normal_cdf(-t.abs());
    }
    if t.is_infinite() {
        return 0.0;
    }
    0.5 * inc_beta(df / 2.0, 0.5, df / (df + t * t))
}

/// Student t cumulative distribution.
pub fn t_cdf(t: f64, df: f64) -> f64 {
    if t.is_nan() || df.is_nan() || df <= 0.0 {
        return f64::NAN;
    }
    let tail = t_tail(t, df);
    if t > 0.0 { 1.0 - tail } else { tail }
}

/// Two-sided p-value of a t statistic. A NaN statistic (e.g. from a zero
/// standard error) gives NaN.
pub fn t_two_sided_p(t: f64, df: f64) -> f64 {
    if t.is_nan() || df.is_nan() || df <= 0.0 {
        return f64::NAN;
    }
    (2.0 * t_tail(t, df)).min(1.0)
}

/// Upper tail probability of the F distribution.
pub fn f_sf(f: f64, d1: f64, d2: f64) -> f64 {
    if d1.is_nan() || d2.is_nan() || d1 <= 0.0 || d2 <= 0.0 || f.is_nan() {
        return f64::NAN;
    }
    if f <= 0.0 {
        return 1.0;
    }
    if f.is_infinite() {
        return 0.0;
    }
    inc_beta(d2 / 2.0, d1 / 2.0, d2 / (d2 + d1 * f))
}

/// Lower tail probability (CDF) of the F distribution, computed directly so
/// that small values keep their relative precision.
pub fn f_cdf(f: f64, d1: f64, d2: f64) -> f64 {
    if d1.is_nan() || d2.is_nan() || d1 <= 0.0 || d2 <= 0.0 || f.is_nan() {
        return f64::NAN;
    }
    if f <= 0.0 {
        return 0.0;
    }
    if f.is_infinite() {
        return 1.0;
    }
    inc_beta(d1 / 2.0, d2 / 2.0, d1 * f / (d1 * f + d2))
}

/// Solves `g(x) = target` for x > 0, where `g` is non-decreasing, by
/// bisection on a logarithmic scale while the bracket spans more than a
/// factor of 4 and on a linear scale after that. Returns +∞ when no finite x
/// reaches the target and 0 when every positive x exceeds it.
fn solve_positive<G: Fn(f64) -> f64>(g: G, target: f64) -> f64 {
    let (mut lo, mut hi) = (1.0, 1.0);
    if g(hi) < target {
        while g(hi) < target {
            lo = hi;
            hi *= 2.0;
            if !hi.is_finite() {
                return f64::INFINITY;
            }
        }
    } else {
        while g(lo) >= target {
            hi = lo;
            lo *= 0.5;
            if lo == 0.0 {
                return 0.0;
            }
        }
    }
    for _ in 0..4000 {
        let mid = if hi > 4.0 * lo { (lo * hi).sqrt() } else { 0.5 * (lo + hi) };
        if mid <= lo || mid >= hi {
            break;
        }
        if g(mid) < target {
            lo = mid;
        } else {
            hi = mid;
        }
        if hi - lo <= 2.0 * f64::EPSILON * hi {
            break;
        }
    }
    0.5 * (lo + hi)
}

/// Quantile of the Student t distribution. The search runs on the tail
/// probability min(p, 1 - p), so both tails keep their relative precision.
pub fn t_quantile(p: f64, df: f64) -> f64 {
    if p.is_nan() || df.is_nan() || df <= 0.0 {
        return f64::NAN;
    }
    if p <= 0.0 {
        return f64::NEG_INFINITY;
    }
    if p >= 1.0 {
        return f64::INFINITY;
    }
    if p == 0.5 {
        return 0.0;
    }
    let q = if p < 0.5 { p } else { 1.0 - p };
    let t = solve_positive(|t| -t_tail(t, df), -q);
    if p < 0.5 { -t } else { t }
}

/// Quantile of the F distribution. Lower-half probabilities are solved on
/// the CDF and upper-half ones on the upper tail, so both keep their
/// relative precision.
pub fn f_quantile(p: f64, d1: f64, d2: f64) -> f64 {
    if p.is_nan() || d1.is_nan() || d2.is_nan() || d1 <= 0.0 || d2 <= 0.0 {
        return f64::NAN;
    }
    if p <= 0.0 {
        return 0.0;
    }
    if p >= 1.0 {
        return f64::INFINITY;
    }
    if p <= 0.5 {
        solve_positive(|f| f_cdf(f, d1, d2), p)
    } else {
        let q = 1.0 - p;
        solve_positive(|f| -f_sf(f, d1, d2), -q)
    }
}

/// Upper tail of the noncentral F distribution with noncentrality `lambda`.
pub fn noncentral_f_sf(f: f64, d1: f64, d2: f64, lambda: f64) -> f64 {
    if lambda <= 0.0 {
        return f_sf(f, d1, d2);
    }
    if f <= 0.0 {
        return 1.0;
    }
    let x = d1 * f / (d1 * f + d2);
    let half = lambda / 2.0;
    let jmax = (half + 12.0 * half.sqrt() + 60.0) as usize;
    let mut cdf = 0.0;
    for j in 0..=jmax {
        let jf = j as f64;
        let lw = -half + jf * half.ln() - ln_gamma(jf + 1.0);
        let w = lw.exp();
        if w < 1e-18 && jf > half {
            break;
        }
        cdf += w * inc_beta(d1 / 2.0 + jf, d2 / 2.0, x);
    }
    (1.0 - cdf).clamp(0.0, 1.0)
}

pub fn mean(v: &[f64]) -> f64 {
    if v.is_empty() { f64::NAN } else { v.iter().sum::<f64>() / v.len() as f64 }
}

/// Sample variance (n - 1 denominator).
pub fn variance(v: &[f64]) -> f64 {
    if v.len() < 2 {
        return f64::NAN;
    }
    let m = mean(v);
    v.iter().map(|x| (x - m).powi(2)).sum::<f64>() / (v.len() - 1) as f64
}

pub fn std_dev(v: &[f64]) -> f64 {
    variance(v).sqrt()
}

pub fn median(v: &[f64]) -> f64 {
    if v.is_empty() {
        return f64::NAN;
    }
    let mut s = v.to_vec();
    s.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = s.len();
    if n % 2 == 1 { s[n / 2] } else { 0.5 * (s[n / 2 - 1] + s[n / 2]) }
}

pub fn correlation(a: &[f64], b: &[f64]) -> f64 {
    let ma = mean(a);
    let mb = mean(b);
    let mut sab = 0.0;
    let mut saa = 0.0;
    let mut sbb = 0.0;
    for (x, y) in a.iter().zip(b) {
        sab += (x - ma) * (y - mb);
        saa += (x - ma).powi(2);
        sbb += (y - mb).powi(2);
    }
    if saa <= 0.0 || sbb <= 0.0 { 0.0 } else { sab / (saa * sbb).sqrt() }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() <= tol * b.abs().max(1e-300) || (a - b).abs() < 1e-14
    }

    #[test]
    fn reference_values() {
        // Reference values from scipy.stats.
        assert!(close(normal_cdf(1.96), 0.9750021048517795, 1e-12));
        assert!(close(normal_quantile(0.975), 1.959963984540054, 1e-12));
        assert!(close(t_quantile(0.975, 10.0), 2.228138851986274, 1e-10));
        assert!(close(t_two_sided_p(2.5, 7.0), 0.04099221858575289, 1e-10));
        assert!(close(f_sf(3.2, 3.0, 12.0), 0.062277550565349664, 1e-9));
        assert!(close(f_quantile(0.95, 2.0, 10.0), 4.1028210151304, 1e-9));
        assert!(close(noncentral_f_sf(4.0, 2.0, 10.0, 5.0), 0.3990889477193408, 1e-7));
        assert!(close(ln_gamma(4.5), 2.4537365708424423, 1e-12));
    }
}
