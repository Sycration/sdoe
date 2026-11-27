//! Optimal designs by coordinate exchange (Meyer & Nachtsheim) for D, I and A
//! criteria, with fixed runs (augmentation), fixed environmental covariate
//! values and mixed continuous / discrete / categorical / mixture factors.
//! Also provides the one-run sequential ("next run") search.

use super::{GenRun, Generated, Opts, designed, env_factors, level_grid};
use crate::linalg::{Mat, chol_logdet, cholesky, dot, inverse_spd};
use crate::model::terms::{ModelLayout, Preset, Term, parse_formula, preset_terms};
use crate::project::{Coded, FactorKind, Project, RunStatus, Value};
use crate::rng::Rng;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Criterion {
    D,
    I,
    A,
}

impl Criterion {
    pub fn parse(s: &str) -> Criterion {
        match s.trim().to_ascii_uppercase().as_str() {
            "I" => Criterion::I,
            "A" => Criterion::A,
            _ => Criterion::D,
        }
    }
}

const RIDGE: f64 = 1e-7;

/// Region description for every factor of the project: the candidate
/// coordinate values for designed factors and sampling rules for the
/// moment matrix.
pub struct Region<'a> {
    pub p: &'a Project,
    /// Candidate coordinates per factor index (empty for non-designed).
    pub grid: Vec<Vec<Coded>>,
    pub mixture: Vec<usize>,
}

impl<'a> Region<'a> {
    pub fn new(p: &'a Project, factors: &[usize], continuous_levels: usize) -> Region<'a> {
        let mut grid = vec![Vec::new(); p.factors.len()];
        let mut mixture = Vec::new();
        for &f in factors {
            if p.factors[f].is_mixture() {
                mixture.push(f);
            } else {
                grid[f] = level_grid(p, f, continuous_levels);
            }
        }
        Region { p, grid, mixture }
    }

    /// Mixture pseudo-component upper bounds (coded).
    fn mix_upper(&self, f: usize) -> f64 {
        match self.p.factors[f].kind {
            FactorKind::Mixture { low, high } => ((high - low) / self.p.mixture_coding().span()).clamp(0.0, 1.0),
            _ => 1.0,
        }
    }

    /// Projects pseudo-component coordinates onto {0 <= z_i <= u_i, sum z = 1}
    /// by clamping and spreading the remainder over the components that can
    /// take it.
    fn project_blend(&self, z: &mut [f64]) {
        let up: Vec<f64> = self.mixture.iter().map(|&f| self.mix_upper(f)).collect();
        for _ in 0..100 {
            for (v, u) in z.iter_mut().zip(&up) {
                *v = v.clamp(0.0, *u);
            }
            let diff = 1.0 - z.iter().sum::<f64>();
            if diff.abs() < 1e-13 {
                break;
            }
            let free: Vec<usize> = (0..z.len()).filter(|&i| if diff > 0.0 { z[i] < up[i] - 1e-13 } else { z[i] > 1e-13 }).collect();
            if free.is_empty() {
                break;
            }
            for &i in &free {
                z[i] += diff / free.len() as f64;
            }
        }
    }

    /// Random mixture composition in pseudo components respecting upper bounds.
    fn random_mixture(&self, rng: &mut Rng) -> Vec<f64> {
        let q = self.mixture.len();
        let e: Vec<f64> = (0..q).map(|_| -rng.uniform().max(1e-12).ln()).collect();
        let s: f64 = e.iter().sum();
        let mut z: Vec<f64> = e.iter().map(|v| v / s).collect();
        self.project_blend(&mut z);
        z
    }

    /// Extreme vertices of the constrained simplex {0 <= z_i <= u_i, sum z = 1}
    /// with the set of bounds active at each (bit i: component i at a bound).
    /// Each vertex has all components but one at a bound (XVERT).
    fn extreme_vertices(&self) -> Vec<(Vec<f64>, u64)> {
        let q = self.mixture.len();
        let up: Vec<f64> = self.mixture.iter().map(|&f| self.mix_upper(f)).collect();
        let mut out: Vec<(Vec<f64>, u64)> = Vec::new();
        for j in 0..q {
            let others: Vec<usize> = (0..q).filter(|&i| i != j).collect();
            for mask in 0u64..(1u64 << others.len()) {
                let mut z = vec![0.0; q];
                for (b, &i) in others.iter().enumerate() {
                    if (mask >> b) & 1 == 1 {
                        z[i] = up[i];
                    }
                }
                let rest = 1.0 - z.iter().sum::<f64>();
                if rest < -1e-12 || rest > up[j] + 1e-12 {
                    continue;
                }
                z[j] = rest.clamp(0.0, up[j]);
                let mut active = 0u64;
                for i in 0..q {
                    if z[i] <= 1e-12 || z[i] >= up[i] - 1e-12 {
                        active |= 1 << i;
                    }
                }
                if !out.iter().any(|(v, _)| v.iter().zip(&z).all(|(a, b)| (a - b).abs() < 1e-12)) {
                    out.push((z, active));
                }
            }
        }
        out
    }

    /// Mixture candidate points: a simplex lattice, centroids and axial blends
    /// of the pseudo-component simplex (filtered by the upper bounds), plus
    /// the extreme vertices of the constrained region, the midpoints of its
    /// edges and the centroid of its vertices. The lattice degree and the
    /// subset centroids shrink with the number of components to keep the
    /// candidate set small.
    pub fn mixture_candidates(&self) -> Vec<Vec<f64>> {
        let q = self.mixture.len();
        let mut out: Vec<Vec<f64>> = Vec::new();
        let m = if q <= 6 {
            4
        } else if q <= 10 {
            3
        } else {
            2
        };
        let mut cur = vec![0usize; q];
        fn rec(i: usize, left: usize, q: usize, m: usize, cur: &mut Vec<usize>, out: &mut Vec<Vec<f64>>) {
            if i == q - 1 {
                cur[i] = left;
                out.push(cur.iter().map(|&c| c as f64 / m as f64).collect());
                return;
            }
            for v in 0..=left {
                cur[i] = v;
                rec(i + 1, left - v, q, m, cur, out);
            }
        }
        rec(0, m, q, m, &mut cur, &mut out);
        if q <= 10 {
            for s in 1u64..(1u64 << q) {
                let c = s.count_ones() as f64;
                out.push((0..q).map(|i| if (s >> i) & 1 == 1 { 1.0 / c } else { 0.0 }).collect());
            }
        } else {
            out.push(vec![1.0 / q as f64; q]);
        }
        for i in 0..q {
            out.push((0..q).map(|j| if i == j { (1.0 + q as f64) / (2.0 * q as f64) } else { 1.0 / (2.0 * q as f64) }).collect());
        }
        out.retain(|z| z.iter().zip(&self.mixture).all(|(v, &f)| *v <= self.mix_upper(f) + 1e-9));
        if q <= 12 && self.mixture.iter().any(|&f| self.mix_upper(f) < 1.0 - 1e-12) {
            let verts = self.extreme_vertices();
            // Two vertices span an edge when they share q - 2 active bounds.
            for a in 0..verts.len() {
                for b in a + 1..verts.len() {
                    if (verts[a].1 & verts[b].1).count_ones() as usize + 2 >= q {
                        out.push(verts[a].0.iter().zip(&verts[b].0).map(|(x, y)| 0.5 * (x + y)).collect());
                    }
                }
            }
            if !verts.is_empty() {
                let k = verts.len() as f64;
                out.push((0..q).map(|i| verts.iter().map(|v| v.0[i]).sum::<f64>() / k).collect());
            }
            out.extend(verts.into_iter().map(|v| v.0));
        }
        out.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        out.dedup_by(|a, b| a.iter().zip(b.iter()).all(|(x, y)| (x - y).abs() < 1e-12));
        out
    }

    /// A random point; factors outside the region take their `base` values.
    pub fn random_point(&self, rng: &mut Rng, base: &[Option<Coded>]) -> Vec<Option<Coded>> {
        let mut pt = base.to_vec();
        for (f, g) in self.grid.iter().enumerate() {
            if !g.is_empty() {
                pt[f] = Some(g[rng.below(g.len())]);
            }
        }
        if !self.mixture.is_empty() {
            let z = self.random_mixture(rng);
            for (i, &f) in self.mixture.iter().enumerate() {
                pt[f] = Some(Coded::Num(z[i]));
            }
        }
        pt
    }

    /// A uniformly random point for moment-matrix integration (continuous
    /// factors are sampled continuously).
    fn sample_point(&self, rng: &mut Rng, sample_env: &[usize]) -> Vec<Option<Coded>> {
        let mut pt = vec![None; self.p.factors.len()];
        for (f, g) in self.grid.iter().enumerate() {
            if g.is_empty() {
                continue;
            }
            pt[f] = Some(match self.p.factors[f].kind {
                FactorKind::Continuous { .. } => Coded::Num(rng.uniform() * 2.0 - 1.0),
                _ => g[rng.below(g.len())],
            });
        }
        for &e in sample_env {
            let f = &self.p.factors[e];
            pt[e] = Some(if f.is_categorical() {
                Coded::Level(rng.below(f.level_count().max(1)))
            } else if let FactorKind::Discrete { .. } = f.kind {
                let g = level_grid(self.p, e, 0);
                g[rng.below(g.len())]
            } else {
                Coded::Num(rng.uniform() * 2.0 - 1.0)
            });
        }
        if !self.mixture.is_empty() {
            let z = self.random_mixture(rng);
            for (i, &f) in self.mixture.iter().enumerate() {
                pt[f] = Some(Coded::Num(z[i]));
            }
        }
        pt
    }

    /// Moment matrix W = E[f(x) f(x)'] over the region (Monte Carlo).
    pub fn moment_matrix(&self, layout: &ModelLayout, env: &[usize], samples: usize) -> Mat {
        let mut rng = Rng::new(0xA11CE);
        let p = layout.ncols();
        let mut w = Mat::zeros(p, p);
        let mut n = 0;
        for _ in 0..samples {
            let pt = self.sample_point(&mut rng, env);
            if let Some(row) = layout.row(&pt) {
                for i in 0..p {
                    for j in 0..p {
                        w[(i, j)] += row[i] * row[j];
                    }
                }
                n += 1;
            }
        }
        for v in w.data.iter_mut() {
            *v /= n.max(1) as f64;
        }
        w
    }
}

/// Information matrix inverse state with rank-2 updates.
struct InfoState {
    minv: Mat,
    logdet: f64,
}

impl InfoState {
    fn from_rows(rows: &[Vec<f64>], p: usize) -> Option<InfoState> {
        let mut m = Mat::zeros(p, p);
        for r in rows {
            for i in 0..p {
                for j in 0..p {
                    m[(i, j)] += r[i] * r[j];
                }
            }
        }
        for i in 0..p {
            m[(i, i)] += RIDGE * rows.len().max(1) as f64;
        }
        let l = cholesky(&m)?;
        let logdet = chol_logdet(&l);
        let minv = inverse_spd(&m)?;
        Some(InfoState { minv, logdet })
    }

    /// Criterion change for replacing row `v` by row `u`:
    /// returns (log det ratio, trace(W M'^-1) - trace(W M^-1)).
    fn delta(&self, u: &[f64], v: &[f64], w: Option<&Mat>) -> Option<(f64, f64, Vec<f64>, f64, Vec<f64>, f64)> {
        let a = self.minv.mul_vec(u);
        let alpha = 1.0 + dot(u, &a);
        let mv = self.minv.mul_vec(v);
        let av = dot(&a, v);
        let b: Vec<f64> = mv.iter().zip(&a).map(|(m, ai)| m - ai * av / alpha).collect();
        let beta = 1.0 - dot(v, &b);
        if alpha <= 1e-12 || beta <= 1e-12 {
            return None;
        }
        let ld = alpha.ln() + beta.ln();
        let tr = match w {
            Some(w) => -w.quad_form(&a) / alpha + w.quad_form(&b) / beta,
            None => 0.0,
        };
        Some((ld, tr, a, alpha, b, beta))
    }

    fn apply(&mut self, a: &[f64], alpha: f64, b: &[f64], beta: f64, ld: f64) {
        let p = a.len();
        for i in 0..p {
            for j in 0..p {
                self.minv[(i, j)] += -a[i] * a[j] / alpha + b[i] * b[j] / beta;
            }
        }
        self.logdet += ld;
    }
}

/// Exact criterion values of a model matrix.
pub struct DesignValues {
    pub logdet: f64,
    pub d_eff: f64,
    pub a_value: f64,
    pub i_value: f64,
}

pub fn design_values(rows: &[Vec<f64>], p: usize, w: Option<&Mat>) -> Option<DesignValues> {
    let mut m = Mat::zeros(p, p);
    for r in rows {
        for i in 0..p {
            for j in 0..p {
                m[(i, j)] += r[i] * r[j];
            }
        }
    }
    let l = cholesky(&m)?;
    let logdet = chol_logdet(&l);
    let minv = inverse_spd(&m)?;
    let n = rows.len() as f64;
    let d_eff = 100.0 * (logdet / p as f64).exp() / n;
    let a_value = minv.trace();
    let i_value = w.map(|w| w.matmul(&minv).trace()).unwrap_or(f64::NAN);
    Some(DesignValues { logdet, d_eff, a_value, i_value })
}

fn crit_value(c: Criterion, st: &InfoState, w: Option<&Mat>) -> f64 {
    match c {
        Criterion::D => -st.logdet,
        Criterion::A => st.minv.trace(),
        Criterion::I => w.map(|w| w.matmul(&st.minv).trace()).unwrap_or(0.0),
    }
}

/// Parses a list of numbers or level names.
fn parse_values(text: &str) -> Vec<String> {
    text.split(|c: char| c == ',' || c == ';' || c.is_whitespace()).map(str::trim).filter(|s| !s.is_empty()).map(String::from).collect()
}

pub struct ExchangeResult {
    pub points: Vec<Vec<Option<Coded>>>,
    pub criterion: f64,
}

/// Core coordinate exchange.
///
/// `fixed` rows stay unchanged; `free` rows start from their given values
/// (factors outside the region, e.g. known environmental values, are fixed
/// coordinates).
pub fn coordinate_exchange(
    region: &Region,
    layout: &ModelLayout,
    fixed: &[Vec<Option<Coded>>],
    free_base: &[Vec<Option<Coded>>],
    crit: Criterion,
    w: Option<&Mat>,
    starts: usize,
    seed: u64,
) -> Option<ExchangeResult> {
    let p = layout.ncols();
    let fixed_rows: Vec<Vec<f64>> = fixed.iter().filter_map(|pt| layout.row(pt)).collect();
    let mut rng = Rng::new(seed);
    let mut best: Option<ExchangeResult> = None;
    let mix_cands = if region.mixture.is_empty() { vec![] } else { region.mixture_candidates() };
    let coords: Vec<usize> = (0..region.grid.len()).filter(|&f| !region.grid[f].is_empty()).collect();
    for _start in 0..starts.max(1) {
        let mut pts: Vec<Vec<Option<Coded>>> = free_base.iter().map(|b| region.random_point(&mut rng, b)).collect();
        let mut rows: Vec<Vec<f64>> = match pts.iter().map(|pt| layout.row(pt)).collect::<Option<Vec<_>>>() {
            Some(r) => r,
            None => return None,
        };
        let mut all: Vec<Vec<f64>> = fixed_rows.clone();
        all.extend(rows.iter().cloned());
        let Some(mut st) = InfoState::from_rows(&all, p) else { continue };
        let mut cur = crit_value(crit, &st, w);
        for _pass in 0..60 {
            let mut improved = false;
            for i in 0..pts.len() {
                // Ordinary coordinates.
                for &f in &coords {
                    let mut best_move: Option<(Coded, Vec<f64>, f64, (f64, f64, Vec<f64>, f64, Vec<f64>, f64))> = None;
                    let orig = pts[i][f];
                    for &c in &region.grid[f] {
                        if Some(c) == orig {
                            continue;
                        }
                        pts[i][f] = Some(c);
                        let Some(newrow) = layout.row(&pts[i]) else { continue };
                        if let Some(d) = st.delta(&newrow, &rows[i], w) {
                            let gain = match crit {
                                Criterion::D => d.0,
                                Criterion::I | Criterion::A => -d.1,
                            };
                            if gain > 1e-10 && best_move.as_ref().is_none_or(|b| gain > b.2) {
                                best_move = Some((c, newrow, gain, d));
                            }
                        }
                    }
                    pts[i][f] = orig;
                    if let Some((c, newrow, _, d)) = best_move {
                        pts[i][f] = Some(c);
                        st.apply(&d.2, d.3, &d.4, d.5, d.0);
                        rows[i] = newrow;
                        improved = true;
                    }
                }
                // Mixture components move together over candidate blends.
                if !region.mixture.is_empty() {
                    let orig: Vec<Option<Coded>> = region.mixture.iter().map(|&f| pts[i][f]).collect();
                    let mut best_move: Option<(usize, Vec<f64>, f64, (f64, f64, Vec<f64>, f64, Vec<f64>, f64))> = None;
                    for (ci, z) in mix_cands.iter().enumerate() {
                        for (k, &f) in region.mixture.iter().enumerate() {
                            pts[i][f] = Some(Coded::Num(z[k]));
                        }
                        let Some(newrow) = layout.row(&pts[i]) else { continue };
                        if let Some(d) = st.delta(&newrow, &rows[i], w) {
                            let gain = match crit {
                                Criterion::D => d.0,
                                _ => -d.1,
                            };
                            if gain > 1e-10 && best_move.as_ref().is_none_or(|b| gain > b.2) {
                                best_move = Some((ci, newrow, gain, d));
                            }
                        }
                    }
                    for (k, &f) in region.mixture.iter().enumerate() {
                        pts[i][f] = orig[k];
                    }
                    if let Some((ci, newrow, _, d)) = best_move {
                        for (k, &f) in region.mixture.iter().enumerate() {
                            pts[i][f] = Some(Coded::Num(mix_cands[ci][k]));
                        }
                        st.apply(&d.2, d.3, &d.4, d.5, d.0);
                        rows[i] = newrow;
                        improved = true;
                    }
                }
            }
            // Refreshes the inverse to limit rounding drift.
            let mut all: Vec<Vec<f64>> = fixed_rows.clone();
            all.extend(rows.iter().cloned());
            if let Some(s) = InfoState::from_rows(&all, p) {
                st = s;
            }
            let v = crit_value(crit, &st, w);
            if !improved || (cur - v).abs() < 1e-10 * cur.abs().max(1.0) {
                cur = v;
                break;
            }
            cur = v;
        }
        if best.as_ref().is_none_or(|b| cur < b.criterion) {
            best = Some(ExchangeResult { points: pts, criterion: cur });
        }
    }
    best
}

/// Model terms for the optimal design options.
pub fn model_from_opts(p: &Project, o: &Opts) -> Result<Vec<Term>, String> {
    let env_int = o.bool("env_interactions", false);
    let m = o.str("model", "quadratic");
    if m == "custom" {
        let f = o.str("formula", "");
        if f.trim().is_empty() {
            return Err("enter a custom formula".into());
        }
        return parse_formula(p, &f);
    }
    let preset = Preset::parse(&m).unwrap_or(Preset::Quadratic);
    Ok(preset_terms(p, preset, env_int))
}

pub fn generate(p: &Project, o: &Opts) -> Result<Generated, String> {
    let fs = designed(p);
    if fs.is_empty() {
        return Err("no controlled or noise factors".into());
    }
    let crit = Criterion::parse(&o.str("criterion", "D"));
    let mut terms = model_from_opts(p, o)?;
    let env = env_factors(p);
    let nf = p.factors.len();

    // Environmental factors: known values become fixed coordinates; without
    // values their terms leave the design model (they enter the analysis
    // as measured covariates).
    let mut env_vals: Vec<(usize, Vec<Coded>, Vec<Value>)> = Vec::new();
    for &e in &env {
        let text = o.str(&format!("env_{}", p.factors[e].id), "");
        let items = parse_values(&text);
        if items.is_empty() {
            continue;
        }
        let mut coded = Vec::new();
        let mut actual = Vec::new();
        for it in items {
            let v = if p.factors[e].is_categorical() {
                Value::Text(it.clone())
            } else {
                Value::Num(crate::format::parse_num(&it).ok_or_else(|| format!("'{it}' is not a number ({})", p.factors[e].name))?)
            };
            let c = p.code(e, &v).ok_or_else(|| format!("'{it}' is not a level of {}", p.factors[e].name))?;
            coded.push(c);
            actual.push(v);
        }
        env_vals.push((e, coded, actual));
    }
    let known_env: Vec<usize> = env_vals.iter().map(|x| x.0).collect();
    let dropped_env: Vec<usize> = env.iter().cloned().filter(|e| !known_env.contains(e)).collect();
    terms.retain(|t| !t.factors().any(|f| dropped_env.contains(&f)));
    let layout = ModelLayout::new(p, &terms);
    let pcols = layout.ncols();

    // Existing runs for augmentation.
    let augment = o.bool("augment", false);
    let mut fixed: Vec<Vec<Option<Coded>>> = Vec::new();
    let mut skipped = 0;
    if augment {
        for r in &p.runs {
            if r.status == RunStatus::Excluded {
                continue;
            }
            let pt = p.coded_run(r);
            if layout.row(&pt).is_some() {
                fixed.push(pt);
            } else {
                skipped += 1;
            }
        }
    }
    let mut n = o.int("runs", 0).max(0) as usize;
    let max_known = env_vals.iter().map(|x| x.1.len()).max().unwrap_or(0);
    if n == 0 {
        n = if max_known > 0 { max_known } else { (pcols + 6).saturating_sub(fixed.len()).max(1) };
    }
    if fixed.len() + n < pcols && max_known > 0 && o.int("runs", 0) <= 0 {
        return Err(format!(
            "{} known environmental values give {} runs, but the model has {} coefficients; add values, set more runs (values then repeat in order) or simplify the model",
            max_known, n, pcols
        ));
    }
    if fixed.len() + n < pcols {
        return Err(format!(
            "the model has {pcols} coefficients, so the design needs at least {} runs{}",
            pcols,
            if augment { format!(" in total ({} exist)", fixed.len()) } else { String::new() }
        ));
    }
    let mut free_base = vec![vec![None; nf]; n];
    for (e, coded, _) in &env_vals {
        for (i, base) in free_base.iter_mut().enumerate() {
            base[*e] = Some(coded[i % coded.len()]);
        }
    }
    if fs.iter().any(|&f| p.factors[f].is_mixture()) {
        super::mixture::check_bounds(p)?;
    }
    let region = Region::new(p, &fs, 11);
    // I: average prediction variance trace(W M^-1) with the moment matrix W;
    // A: trace(M^-1), the same form with W = identity.
    let w = (crit == Criterion::I).then(|| region.moment_matrix(&layout, &known_env, 4000));
    let w_exchange = match crit {
        Criterion::A => Some(Mat::identity(pcols)),
        _ => w.clone(),
    };
    let starts = o.int("starts", 20).clamp(1, 500) as usize;
    let seed = o.int("seed", 1).max(0) as u64;
    let res = coordinate_exchange(&region, &layout, &fixed, &free_base, crit, w_exchange.as_ref(), starts, seed)
        .ok_or("no non-singular design was found; add runs or simplify the model")?;

    let mut runs = Vec::new();
    for (i, pt) in res.points.iter().enumerate() {
        let mut r = GenRun::new(nf, if augment { "augment" } else { "optimal" });
        for &f in &fs {
            r.coded[f] = pt[f];
        }
        for (e, _, actual) in &env_vals {
            r.env[*e] = Some(actual[i % actual.len()].clone());
        }
        runs.push(r);
    }
    let mut all_rows: Vec<Vec<f64>> = fixed.iter().filter_map(|pt| layout.row(pt)).collect();
    all_rows.extend(res.points.iter().filter_map(|pt| layout.row(pt)));
    let mut notes = vec![format!(
        "{}-optimal design: {} runs for a model with {} coefficients.",
        match crit {
            Criterion::D => "D",
            Criterion::I => "I",
            Criterion::A => "A",
        },
        n,
        pcols
    )];
    if augment {
        notes.push(format!("Augments {} existing runs.", fixed.len()));
        if skipped > 0 {
            notes.push(format!("{skipped} existing runs lack values for model factors and were not counted."));
        }
    }
    if let Some(v) = design_values(&all_rows, pcols, w.as_ref()) {
        notes.push(format!("D-efficiency {}%, A = {}{}.", crate::format::num(v.d_eff, 4), crate::format::num(v.a_value, 4), if v.i_value.is_finite() {
            format!(", average prediction variance {}", crate::format::num(v.i_value, 4))
        } else {
            String::new()
        }));
    }
    if !known_env.is_empty() {
        let names: Vec<String> = known_env.iter().map(|&e| p.factors[e].name.clone()).collect();
        notes.push(format!("Planned around the known values of {}; the runs use them in the listed order.", names.join(", ")));
    }
    if !dropped_env.is_empty() {
        let names: Vec<String> = dropped_env.iter().map(|&e| p.factors[e].name.clone()).collect();
        notes.push(format!(
            "No values were given for {}; their terms are left out of the design model and enter the analysis once measured.",
            names.join(", ")
        ));
    }
    Ok(Generated { runs, notes, blocks: 1, model: terms, keeps_existing: augment, ..Default::default() })
}

/// Result of the sequential next-run search.
pub struct NextRun {
    pub point: Vec<Option<Coded>>,
    /// Relative prediction variance x'(X'X)^-1 x at the chosen point.
    pub variance: f64,
    pub d_gain: f64,
}

/// Finds the settings of the designed factors that add the most
/// information (largest prediction variance) to the existing runs, with
/// environmental factors fixed at their current values.
pub fn next_run(p: &Project, terms: &[Term], env_now: &[Option<Coded>]) -> Result<NextRun, String> {
    let fs = designed(p);
    let layout = ModelLayout::new(p, terms);
    let pc = layout.ncols();
    let mut m = Mat::zeros(pc, pc);
    let mut count = 0;
    for r in &p.runs {
        if r.status == RunStatus::Excluded {
            continue;
        }
        if let Some(row) = layout.row(&p.coded_run(r)) {
            for i in 0..pc {
                for j in 0..pc {
                    m[(i, j)] += row[i] * row[j];
                }
            }
            count += 1;
        }
    }
    // With a weak prior term the information matrix is invertible even with few runs.
    for i in 0..pc {
        m[(i, i)] += 1e-3;
    }
    let minv = inverse_spd(&m).ok_or("information matrix is singular")?;
    let region = Region::new(p, &fs, 11);
    let mut base = env_now.to_vec();
    base.resize(p.factors.len(), None);
    let score = |pt: &Vec<Option<Coded>>| layout.row(pt).map(|r| minv.quad_form(&r));
    let mut rng = Rng::new(7 + count as u64);
    let mut best: Option<(Vec<Option<Coded>>, f64)> = None;
    let mix_cands = if region.mixture.is_empty() { vec![] } else { region.mixture_candidates() };
    for _ in 0..40 {
        let mut pt = region.random_point(&mut rng, &base);
        let Some(mut cur) = score(&pt) else {
            return Err("current values are missing for a factor in the model".into());
        };
        for _ in 0..30 {
            let mut improved = false;
            for f in 0..region.grid.len() {
                for &c in &region.grid[f] {
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
        if best.as_ref().is_none_or(|b| cur > b.1) {
            best = Some((pt, cur));
        }
    }
    let (point, variance) = best.ok_or("no candidate point")?;
    Ok(NextRun { point, variance, d_gain: (1.0 + variance).ln() })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::*;

    fn project(k: usize) -> Project {
        let mut p = Project::default();
        for i in 0..k {
            let id = p.new_id();
            p.factors.push(Factor {
                id,
                symbol: symbol_for(i),
                name: format!("X{i}"),
                units: String::new(),
                kind: FactorKind::Continuous { low: 0.0, high: 10.0 },
                role: Role::Controlled,
                condition: None,
            });
        }
        p
    }

    #[test]
    fn d_optimal_main_effects_is_orthogonal() {
        let p = project(3);
        let terms = preset_terms(&p, Preset::Linear, false);
        let layout = ModelLayout::new(&p, &terms);
        let region = Region::new(&p, &[0, 1, 2], 11);
        let base = vec![vec![None; 3]; 8];
        let r = coordinate_exchange(&region, &layout, &[], &base, Criterion::D, None, 10, 3).unwrap();
        let rows: Vec<Vec<f64>> = r.points.iter().map(|pt| layout.row(pt).unwrap()).collect();
        let v = design_values(&rows, 4, None).unwrap();
        assert!((v.d_eff - 100.0).abs() < 1e-6, "D-eff {}", v.d_eff);
    }

    #[test]
    fn quadratic_design_is_nonsingular() {
        let p = project(3);
        let terms = preset_terms(&p, Preset::Quadratic, false);
        let layout = ModelLayout::new(&p, &terms);
        let region = Region::new(&p, &[0, 1, 2], 11);
        let base = vec![vec![None; 3]; 16];
        let r = coordinate_exchange(&region, &layout, &[], &base, Criterion::D, None, 5, 3).unwrap();
        let rows: Vec<Vec<f64>> = r.points.iter().map(|pt| layout.row(pt).unwrap()).collect();
        let v = design_values(&rows, 10, None).unwrap();
        assert!(v.d_eff > 40.0, "D-eff {}", v.d_eff);
    }
}
