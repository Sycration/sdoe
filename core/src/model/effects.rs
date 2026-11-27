//! Effect screening (Lenth's method, half-normal plots) and Box-Cox analysis.

use super::fit::{ALIAS_TOL, Fit};
use super::terms::{ColPart, ModelLayout};
use crate::linalg::LsqDecomp;
use crate::stats;

#[derive(Clone, Debug)]
pub struct Effect {
    pub label: String,
    pub column: usize,
    /// Effect = change in the fitted response when the column goes from its
    /// lowest to its highest coded value: 2 x coefficient for linear terms,
    /// interactions and two-level categorical factors, 1 x coefficient for
    /// squares (0 to 1).
    pub effect: f64,
    /// Standardised effect: t statistic when residual df exist, Lenth t otherwise.
    pub t: f64,
}

#[derive(Clone, Debug)]
pub struct EffectScreen {
    pub effects: Vec<Effect>,
    pub pse: f64,
    pub me: f64,
    pub sme: f64,
    /// Reference line for standardised effects.
    pub t_crit: f64,
    pub uses_lenth: bool,
}

/// Difference between the highest and the lowest value of a model column
/// over the coded region: process factors and categorical effect columns
/// span -1 to 1 (even powers 0 to 1), mixture components 0 to 1.
fn column_span(layout: &ModelLayout, parts: &[ColPart]) -> f64 {
    let (mut lo, mut hi) = (1.0f64, 1.0f64);
    for part in parts {
        let (a, b) = match *part {
            ColPart::Num { factor, .. } if layout.mixture.get(factor).copied().unwrap_or(false) => (0.0, 1.0),
            ColPart::Num { power, .. } if power % 2 == 0 => (0.0, 1.0),
            _ => (-1.0, 1.0),
        };
        let c = [lo * a, lo * b, hi * a, hi * b];
        lo = c.iter().cloned().fold(f64::INFINITY, f64::min);
        hi = c.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    }
    hi - lo
}

/// Screens the model columns of a fit.
pub fn screen(fit: &Fit, alpha: f64) -> EffectScreen {
    let layout = fit.layout();
    let mut effects = Vec::new();
    for (ci, col) in layout.columns.iter().enumerate() {
        if col.term.is_none() || fit.aliased[ci] {
            continue;
        }
        effects.push(Effect { label: col.label.clone(), column: ci, effect: column_span(layout, &col.parts) * fit.coef[ci], t: fit.t[ci] });
    }
    let m = effects.len();
    let abs: Vec<f64> = effects.iter().map(|e| e.effect.abs()).collect();
    let s0 = 1.5 * stats::median(&abs);
    let trimmed: Vec<f64> = abs.iter().cloned().filter(|a| *a < 2.5 * s0).collect();
    let pse = if trimmed.is_empty() { s0 } else { 1.5 * stats::median(&trimmed) };
    let d = (m as f64 / 3.0).max(1.0);
    let me = stats::t_quantile(1.0 - alpha / 2.0, d) * pse;
    let gamma = (1.0 + (1.0 - alpha).powf(1.0 / m.max(1) as f64)) / 2.0;
    let sme = stats::t_quantile(gamma, d) * pse;
    let uses_lenth = !(fit.df_resid > 0.0);
    if uses_lenth {
        for e in &mut effects {
            e.t = if pse > 0.0 { e.effect / pse } else { f64::NAN };
        }
    }
    let t_crit = if uses_lenth {
        if pse > 0.0 { me / pse } else { f64::NAN }
    } else {
        stats::t_quantile(1.0 - alpha / 2.0, fit.df_resid)
    };
    EffectScreen { effects, pse, me, sme, t_crit, uses_lenth }
}

/// Half-normal quantiles for `m` ordered absolute effects.
pub fn half_normal_quantiles(m: usize) -> Vec<f64> {
    (1..=m).map(|i| stats::normal_quantile(0.5 + 0.5 * (i as f64 - 0.5) / m as f64)).collect()
}

/// Normal quantiles (Blom) for `m` ordered values.
pub fn normal_quantiles(m: usize) -> Vec<f64> {
    (1..=m).map(|i| stats::normal_quantile((i as f64 - 0.375) / (m as f64 + 0.25))).collect()
}

#[derive(Clone, Debug)]
pub struct BoxCox {
    pub lambdas: Vec<f64>,
    pub ln_sse: Vec<f64>,
    pub best: f64,
    pub ci: (f64, f64),
    pub recommended: f64,
    pub threshold: f64,
}

/// Powers that a Box-Cox analysis recommends when one lies in the
/// confidence interval; the transform list offers each of them.
pub const LADDER: [f64; 9] = [1.0, 0.5, 0.0, -0.5, -1.0, 2.0, -2.0, 3.0, -3.0];

/// Labels of the transforms offered for a response: Log10 and the
/// transforms of the Box-Cox ladder.
pub fn transform_labels() -> Vec<String> {
    use crate::project::Transform;
    let mut v = vec![Transform::None, Transform::Ln, Transform::Log10, Transform::Sqrt, Transform::Inverse];
    for &l in &LADDER {
        let t = lambda_transform(l);
        if !v.contains(&t) {
            v.push(t);
        }
    }
    v.into_iter().map(Transform::label).collect()
}

/// Box-Cox profile for the model structure of `fit` (on the raw response).
pub fn box_cox(fit: &Fit, alpha: f64) -> Result<BoxCox, String> {
    let y = &fit.input.y_raw;
    if y.iter().any(|v| *v <= 0.0) {
        return Err("Box-Cox requires all response values to be positive".into());
    }
    let n = y.len() as f64;
    let g = (y.iter().map(|v| v.ln()).sum::<f64>() / n).exp();
    let d = LsqDecomp::new(&fit.input.x, ALIAS_TOL);
    let df = n - d.rank() as f64;
    if df <= 0.0 {
        return Err("Box-Cox requires residual degrees of freedom".into());
    }
    let mut lambdas = Vec::new();
    let mut ln_sse = Vec::new();
    for i in 0..=120 {
        let l = -3.0 + 0.05 * i as f64;
        let l = if l.abs() < 1e-9 { 0.0 } else { l };
        let yl: Vec<f64> = y
            .iter()
            .map(|v| if l == 0.0 { g * v.ln() } else { (v.powf(l) - 1.0) / (l * g.powf(l - 1.0)) })
            .collect();
        // Explicit residuals avoid cancellation from the large constant offset of y(lambda).
        let m = yl.iter().sum::<f64>() / n;
        let yc: Vec<f64> = yl.iter().map(|v| v - m).collect();
        let qty = d.q.t_mul_vec(&yc);
        let fitted = d.q.mul_vec(&qty);
        let sse: f64 = yc.iter().zip(&fitted).map(|(a, b)| (a - b).powi(2)).sum();
        lambdas.push(l);
        ln_sse.push(sse.max(1e-300).ln());
    }
    let (bi, bmin) = ln_sse
        .iter()
        .enumerate()
        .fold((0, f64::INFINITY), |acc, (i, &v)| if v < acc.1 { (i, v) } else { acc });
    let tq = stats::t_quantile(1.0 - alpha / 2.0, df);
    let threshold = bmin + (1.0 + tq * tq / df).ln();
    let inside: Vec<f64> = lambdas.iter().zip(&ln_sse).filter(|(_, s)| **s <= threshold).map(|(l, _)| *l).collect();
    let ci = (
        inside.iter().cloned().fold(f64::INFINITY, f64::min),
        inside.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
    );
    let best = lambdas[bi];
    let recommended = LADDER
        .iter()
        .cloned()
        .filter(|s| *s >= ci.0 - 1e-9 && *s <= ci.1 + 1e-9)
        .min_by(|a, b| {
            if *a == 1.0 {
                return std::cmp::Ordering::Less;
            }
            if *b == 1.0 {
                return std::cmp::Ordering::Greater;
            }
            (a - best).abs().partial_cmp(&(b - best).abs()).unwrap()
        })
        .unwrap_or(best);
    Ok(BoxCox { lambdas, ln_sse, best, ci, recommended, threshold })
}

/// Transform corresponding to a Box-Cox lambda.
pub fn lambda_transform(l: f64) -> crate::project::Transform {
    use crate::project::Transform;
    match l {
        x if x == 1.0 => Transform::None,
        x if x == 0.0 => Transform::Ln,
        x if x == 0.5 => Transform::Sqrt,
        x if x == -1.0 => Transform::Inverse,
        x => Transform::Power(x),
    }
}
