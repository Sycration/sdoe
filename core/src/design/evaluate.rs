//! Design evaluation: efficiencies, prediction variance, correlations,
//! variance inflation and power.

use super::optimal::Region;
use super::{designed, env_factors};
use crate::linalg::{LsqDecomp, Mat, inverse_spd};
use crate::model::fit::ALIAS_TOL;
use crate::model::terms::{ColPart, ModelLayout, Term};
use crate::project::{Coded, Project, Role};
use crate::rng::Rng;
use crate::stats;

#[derive(Clone, Debug)]
pub struct Evaluation {
    pub n: usize,
    pub p: usize,
    pub rank: usize,
    pub labels: Vec<String>,
    pub aliased: Vec<String>,
    pub d_eff: f64,
    pub a_value: f64,
    pub i_value: f64,
    pub g_eff: f64,
    /// Standard error of each kept coefficient in units of sigma.
    pub se_unit: Vec<f64>,
    pub vif: Vec<f64>,
    pub power: Vec<f64>,
    /// Labels of the kept columns (aligned with se_unit, vif, power).
    pub kept_labels: Vec<String>,
    /// Correlation matrix of the non-intercept model columns.
    pub corr_labels: Vec<String>,
    pub corr: Mat,
    /// Sorted scaled prediction variance samples (N x'(X'X)^-1 x).
    pub fds: Vec<f64>,
    pub warnings: Vec<String>,
    pub df_resid: f64,
}

pub fn evaluate(p: &Project, points: &[Vec<Option<Coded>>], terms: &[Term], snr: f64, alpha: f64) -> Result<Evaluation, String> {
    let layout = ModelLayout::new(p, terms);
    let rows: Vec<Vec<f64>> = points.iter().filter_map(|pt| layout.row(pt)).collect();
    let n = rows.len();
    let pc = layout.ncols();
    let mut warnings = Vec::new();
    if n < points.len() {
        warnings.push(format!("{} runs lack values for model factors and are not counted.", points.len() - n));
    }
    if n == 0 {
        return Err("no runs with values for all model factors".into());
    }
    let x = Mat::from_rows(&rows);
    let d = LsqDecomp::new(&x, ALIAS_TOL);
    let aliased: Vec<String> = d.dropped.iter().map(|&c| layout.columns[c].label.clone()).collect();
    if !aliased.is_empty() {
        warnings.push(format!("Not estimable (aliased): {}.", aliased.join(", ")));
    }
    let xk = x.select_cols(&d.kept);
    let g = xk.gram();
    let r = d.rank();
    let ginv = inverse_spd(&g).unwrap_or_else(|| d.xtx_inverse());
    let logdet: f64 = (0..r).map(|i| d.r[(i, i)].abs().ln()).sum::<f64>() * 2.0;
    let d_eff = if r == pc { 100.0 * (logdet / pc as f64).exp() / n as f64 } else { 0.0 };
    let a_value = ginv.trace();
    let kept_labels: Vec<String> = d.kept.iter().map(|&c| layout.columns[c].label.clone()).collect();
    let se_unit: Vec<f64> = (0..r).map(|i| ginv[(i, i)].sqrt()).collect();
    let df_resid = n as f64 - r as f64;

    // Power to detect a coefficient change of snr/2 sigma (an effect of snr
    // sigma across the coded range for numeric terms).
    let power: Vec<f64> = d
        .kept
        .iter()
        .enumerate()
        .map(|(i, &c)| {
            if df_resid <= 0.0 {
                return f64::NAN;
            }
            let numeric = layout.columns[c].parts.iter().all(|p| matches!(p, ColPart::Num { .. }));
            let beta = if numeric { snr / 2.0 } else { snr / 2.0 };
            let lambda = beta * beta / ginv[(i, i)];
            let fcrit = stats::f_quantile(1.0 - alpha, 1.0, df_resid);
            stats::noncentral_f_sf(fcrit, 1.0, df_resid, lambda)
        })
        .collect();

    // Variance inflation from centred columns.
    let pred: Vec<usize> = (0..r).filter(|&i| layout.columns[d.kept[i]].term.is_some()).collect();
    let mut vif = vec![f64::NAN; r];
    let mut corr_labels = Vec::new();
    let mut corr = Mat::zeros(0, 0);
    if !pred.is_empty() {
        let mut xc = xk.select_cols(&pred);
        if layout.intercept {
            for j in 0..xc.cols {
                let m = stats::mean(&xc.col(j));
                for i in 0..xc.rows {
                    xc[(i, j)] -= m;
                }
            }
        }
        let gc = xc.gram();
        if let Some(gi) = inverse_spd(&gc) {
            for (j, &i) in pred.iter().enumerate() {
                vif[i] = gi[(j, j)] * gc[(j, j)];
            }
        }
        // Correlations among all non-intercept columns (including aliased).
        let cols: Vec<usize> = (0..pc).filter(|&c| layout.columns[c].term.is_some()).collect();
        corr_labels = cols.iter().map(|&c| layout.columns[c].label.clone()).collect();
        corr = Mat::zeros(cols.len(), cols.len());
        let colv: Vec<Vec<f64>> = cols.iter().map(|&c| x.col(c)).collect();
        for i in 0..cols.len() {
            for j in 0..cols.len() {
                corr[(i, j)] = if i == j { 1.0 } else { stats::correlation(&colv[i], &colv[j]) };
            }
        }
        // Confounding warnings, with emphasis on environmental covariates.
        let env_col = |c: usize| {
            layout.columns[c].parts.iter().any(|part| {
                let f = match part {
                    ColPart::Num { factor, .. } | ColPart::Cat { factor, .. } => *factor,
                };
                p.factors[f].role == Role::Environmental
            })
        };
        for i in 0..cols.len() {
            for j in i + 1..cols.len() {
                let rij = corr[(i, j)];
                let envi = env_col(cols[i]) || env_col(cols[j]);
                if (envi && rij.abs() > 0.5) || rij.abs() > 0.9 {
                    warnings.push(format!(
                        "{} and {} are correlated (r = {}){}.",
                        corr_labels[i],
                        corr_labels[j],
                        crate::format::num(rij, 2),
                        if envi { "; the environmental effect is partly confounded with the factor effect" } else { "" }
                    ));
                }
            }
        }
    }

    // Prediction variance over the region.
    let mut fds = Vec::new();
    let mut i_value = f64::NAN;
    let mut g_eff = f64::NAN;
    if r == pc {
        let region = Region::new(p, &designed(p), 11);
        let env = env_factors(p);
        let mut rng = Rng::new(42);
        let mut base = vec![None; p.factors.len()];
        let used = layout.factors_used();
        let mut sampled: Vec<(f64, Vec<Option<Coded>>)> = Vec::new();
        for _ in 0..2000 {
            let mut pt = region.random_point(&mut rng, &base);
            for (f, fac) in p.factors.iter().enumerate() {
                if !used.contains(&f) {
                    continue;
                }
                if env.contains(&f) || fac.role == Role::Block {
                    pt[f] = Some(if fac.is_categorical() {
                        Coded::Level(rng.below(fac.level_count().max(1)))
                    } else {
                        Coded::Num(rng.uniform() * 2.0 - 1.0)
                    });
                }
                if matches!(fac.kind, crate::project::FactorKind::Continuous { .. }) && region.grid[f].len() > 0 {
                    pt[f] = Some(Coded::Num(rng.uniform() * 2.0 - 1.0));
                }
            }
            if let Some(row) = layout.row(&pt) {
                let v = n as f64 * ginv.quad_form(&row);
                fds.push(v);
                sampled.push((v, pt));
            }
            base = vec![None; p.factors.len()];
        }
        // Coordinate values searched for the maximum prediction variance:
        // the region grid (with the range ends) for designed factors, -1/0/1
        // or the levels for environmental and block factors.
        let mut coords: Vec<Vec<Coded>> = vec![Vec::new(); p.factors.len()];
        for (f, fac) in p.factors.iter().enumerate() {
            if !used.contains(&f) || fac.is_mixture() {
                continue;
            }
            coords[f] = if !region.grid[f].is_empty() {
                region.grid[f].clone()
            } else if env.contains(&f) || fac.role == Role::Block {
                if fac.is_categorical() {
                    (0..fac.level_count().max(1)).map(Coded::Level).collect()
                } else {
                    vec![Coded::Num(-1.0), Coded::Num(0.0), Coded::Num(1.0)]
                }
            } else {
                Vec::new()
            };
        }
        fds.sort_by(|a, b| a.total_cmp(b));
        if !fds.is_empty() {
            i_value = stats::mean(&fds) / n as f64;
            let maxv = max_prediction_variance(&layout, &ginv, n, &coords, &region, sampled);
            g_eff = 100.0 * pc as f64 / maxv;
        }
    }
    if df_resid <= 0.0 {
        warnings.push("No residual degrees of freedom: the design cannot estimate error for this model.".into());
    }
    Ok(Evaluation {
        n,
        p: pc,
        rank: r,
        labels: layout.columns.iter().map(|c| c.label.clone()).collect(),
        aliased,
        d_eff,
        a_value,
        i_value,
        g_eff,
        se_unit,
        vif,
        power,
        kept_labels,
        corr_labels,
        corr,
        fds,
        warnings,
        df_resid,
    })
}

/// Maximum scaled prediction variance n x'(X'X)^-1 x over the region: the
/// largest of the sampled points and of the vertices of the numeric
/// coordinates, refined by coordinate ascent over the coordinate values (and
/// the mixture candidate blends) from the best points.
fn max_prediction_variance(
    layout: &ModelLayout,
    ginv: &Mat,
    n: usize,
    coords: &[Vec<Coded>],
    region: &Region,
    mut starts: Vec<(f64, Vec<Option<Coded>>)>,
) -> f64 {
    let score = |pt: &[Option<Coded>]| layout.row(pt).map(|row| n as f64 * ginv.quad_form(&row));
    starts.sort_by(|a, b| b.0.total_cmp(&a.0));
    let Some(template) = starts.first().map(|s| s.1.clone()) else { return f64::NAN };
    let numeric: Vec<usize> = (0..coords.len()).filter(|&f| coords[f].len() > 1 && matches!(coords[f][0], Coded::Num(_))).collect();
    if numeric.len() <= 12 {
        for mask in 0u32..(1u32 << numeric.len()) {
            let mut pt = template.clone();
            for (bit, &f) in numeric.iter().enumerate() {
                let c = &coords[f];
                pt[f] = Some(if (mask >> bit) & 1 == 1 { c[c.len() - 1] } else { c[0] });
            }
            if let Some(v) = score(&pt) {
                starts.push((v, pt));
            }
        }
        starts.sort_by(|a, b| b.0.total_cmp(&a.0));
    }
    let mix_cands = if region.mixture.is_empty() { vec![] } else { region.mixture_candidates() };
    let mut best = starts.first().map(|s| s.0).unwrap_or(f64::NAN);
    for (mut cur, mut pt) in starts.into_iter().take(10) {
        for _ in 0..20 {
            let mut improved = false;
            for (f, cs) in coords.iter().enumerate() {
                for &c in cs {
                    let old = pt[f];
                    pt[f] = Some(c);
                    match score(&pt) {
                        Some(v) if v > cur + 1e-12 => {
                            cur = v;
                            improved = true;
                        }
                        _ => pt[f] = old,
                    }
                }
            }
            for z in &mix_cands {
                let old: Vec<Option<Coded>> = region.mixture.iter().map(|&f| pt[f]).collect();
                for (k, &f) in region.mixture.iter().enumerate() {
                    pt[f] = Some(Coded::Num(z[k]));
                }
                match score(&pt) {
                    Some(v) if v > cur + 1e-12 => {
                        cur = v;
                        improved = true;
                    }
                    _ => {
                        for (k, &f) in region.mixture.iter().enumerate() {
                            pt[f] = old[k];
                        }
                    }
                }
            }
            if !improved {
                break;
            }
        }
        best = best.max(cur);
    }
    best
}
