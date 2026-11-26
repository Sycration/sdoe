//! Probability distributions and descriptive statistics.

use std::f64::consts::PI;

/// Natural logarithm of the gamma function (Lanczos approximation, g = 7).
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
        (PI / (PI * x).sin()).ln() - ln_gamma(1.0 - x)
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

/// Regularised lower incomplete gamma function P(a, x).
pub fn inc_gamma_p(a: f64, x: f64) -> f64 {
    if x <= 0.0 {
        return 0.0;
    }
    let gln = ln_gamma(a);
    if x < a + 1.0 {
        let mut ap = a;
        let mut sum = 1.0 / a;
        let mut del = sum;
        for _ in 0..10_000 {
            ap += 1.0;
            del *= x / ap;
            sum += del;
            if del.abs() < sum.abs() * 1e-16 {
                break;
            }
        }
        sum * (-x + a * x.ln() - gln).exp()
    } else {
        const FPMIN: f64 = 1e-300;
        let mut b = x + 1.0 - a;
        let mut c = 1.0 / FPMIN;
        let mut d = 1.0 / b;
        let mut h = d;
        for i in 1..10_000 {
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
        1.0 - (-x + a * x.ln() - gln).exp() * h
    }
}

pub fn erf(x: f64) -> f64 {
    if x >= 0.0 { inc_gamma_p(0.5, x * x) } else { -inc_gamma_p(0.5, x * x) }
}

pub fn normal_cdf(x: f64) -> f64 {
    if x < -8.0 {
        // Asymptotic tail formula: accurate in relative terms where erf underflows.
        let z = -x;
        let pdf = (-0.5 * z * z).exp() / (2.0 * PI).sqrt();
        return pdf / z * (1.0 - 1.0 / (z * z) + 3.0 / z.powi(4));
    }
    0.5 * (1.0 + erf(x / std::f64::consts::SQRT_2))
}

pub fn normal_pdf(x: f64) -> f64 {
    (-0.5 * x * x).exp() / (2.0 * PI).sqrt()
}

/// Inverse of the standard normal CDF (Acklam's algorithm with one Halley step).
pub fn normal_quantile(p: f64) -> f64 {
    if p <= 0.0 {
        return f64::NEG_INFINITY;
    }
    if p >= 1.0 {
        return f64::INFINITY;
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
    } else if p <= 1.0 - plow {
        let q = p - 0.5;
        let r = q * q;
        (((((A[0] * r + A[1]) * r + A[2]) * r + A[3]) * r + A[4]) * r + A[5]) * q
            / (((((B[0] * r + B[1]) * r + B[2]) * r + B[3]) * r + B[4]) * r + 1.0)
    } else {
        let q = (-2.0 * (1.0 - p).ln()).sqrt();
        -(((((C[0] * q + C[1]) * q + C[2]) * q + C[3]) * q + C[4]) * q + C[5])
            / ((((D[0] * q + D[1]) * q + D[2]) * q + D[3]) * q + 1.0)
    };
    let e = normal_cdf(x) - p;
    let u = e * (2.0 * PI).sqrt() * (x * x / 2.0).exp();
    x - u / (1.0 + x * u / 2.0)
}

/// Student t cumulative distribution.
pub fn t_cdf(t: f64, df: f64) -> f64 {
    if !t.is_finite() {
        return if t > 0.0 { 1.0 } else { 0.0 };
    }
    let x = df / (df + t * t);
    let tail = 0.5 * inc_beta(df / 2.0, 0.5, x);
    if t > 0.0 { 1.0 - tail } else { tail }
}

/// Two-sided p-value of a t statistic.
pub fn t_two_sided_p(t: f64, df: f64) -> f64 {
    if df <= 0.0 || !t.is_finite() {
        return if t.is_finite() { f64::NAN } else { 0.0 };
    }
    inc_beta(df / 2.0, 0.5, df / (df + t * t))
}

/// Upper tail probability of the F distribution.
pub fn f_sf(f: f64, d1: f64, d2: f64) -> f64 {
    if d1 <= 0.0 || d2 <= 0.0 || f.is_nan() {
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

fn bisect<F: Fn(f64) -> f64>(f: F, target: f64, mut lo: f64, mut hi: f64) -> f64 {
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        if f(mid) < target {
            lo = mid;
        } else {
            hi = mid;
        }
        if (hi - lo).abs() <= 1e-14 * mid.abs().max(1e-12) {
            break;
        }
    }
    0.5 * (lo + hi)
}

/// Quantile of the Student t distribution.
pub fn t_quantile(p: f64, df: f64) -> f64 {
    if p <= 0.0 {
        return f64::NEG_INFINITY;
    }
    if p >= 1.0 {
        return f64::INFINITY;
    }
    if (p - 0.5).abs() < 1e-16 {
        return 0.0;
    }
    if p < 0.5 {
        return -t_quantile(1.0 - p, df);
    }
    let mut hi = 2.0;
    while t_cdf(hi, df) < p && hi < 1e12 {
        hi *= 2.0;
    }
    bisect(|t| t_cdf(t, df), p, 0.0, hi)
}

/// Quantile of the F distribution.
pub fn f_quantile(p: f64, d1: f64, d2: f64) -> f64 {
    if p <= 0.0 {
        return 0.0;
    }
    let cdf = |f: f64| 1.0 - f_sf(f, d1, d2);
    let mut hi = 2.0;
    while cdf(hi) < p && hi < 1e12 {
        hi *= 2.0;
    }
    bisect(cdf, p, 0.0, hi)
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
