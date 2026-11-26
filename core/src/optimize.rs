//! Multi-response optimisation with desirability functions, conditional on
//! fixed environmental values or robust over random environmental and noise
//! factors, plus environment-to-setting adjustment laws.

use crate::analysis::{fit_response, response_range};
use crate::model::fit::Fit;
use crate::project::{Coded, FactorKind, Goal, OptimizeSettings, Project, Role, Value};
use crate::rng::Rng;
use crate::stats;

#[derive(Clone, Debug)]
pub struct DesirSpec {
    pub goal: Goal,
    pub lower: f64,
    pub target: f64,
    pub upper: f64,
    pub weight: f64,
    pub importance: f64,
}

impl DesirSpec {
    pub fn d(&self, y: f64) -> f64 {
        let (l, t, u, w) = (self.lower, self.target, self.upper, self.weight.max(0.01));
        if !y.is_finite() {
            return 0.0;
        }
        match self.goal {
            Goal::None => 1.0,
            Goal::Maximize => {
                if y <= l {
                    0.0
                } else if y >= u {
                    1.0
                } else {
                    ((y - l) / (u - l)).powf(w)
                }
            }
            Goal::Minimize => {
                if y >= u {
                    0.0
                } else if y <= l {
                    1.0
                } else {
                    ((u - y) / (u - l)).powf(w)
                }
            }
            Goal::Target => {
                if y < l || y > u {
                    0.0
                } else if y <= t {
                    if t > l { ((y - l) / (t - l)).powf(w) } else { 1.0 }
                } else if u > t {
                    ((u - y) / (u - t)).powf(w)
                } else {
                    1.0
                }
            }
            Goal::InRange => {
                if y >= l && y <= u {
                    1.0
                } else {
                    0.0
                }
            }
        }
    }

    /// Unclamped closeness score in roughly [0, 1] with a nonzero slope where
    /// the desirability is flat at zero.
    pub fn soft(&self, y: f64) -> f64 {
        let span = (self.upper - self.lower).abs().max(1e-12);
        match self.goal {
            Goal::None => 1.0,
            Goal::Maximize => (y - self.lower) / span,
            Goal::Minimize => (self.upper - y) / span,
            Goal::Target | Goal::InRange => 1.0 - (y - self.target).abs() / span,
        }
    }
}

pub struct RespModel {
    pub index: usize,
    pub fit: Fit,
    pub spec: DesirSpec,
}

/// Collects fitted models for all responses (and the goals of those with goals).
pub fn response_models(p: &Project) -> (Vec<RespModel>, Vec<String>) {
    let mut out = Vec::new();
    let mut problems = Vec::new();
    for (i, r) in p.responses.iter().enumerate() {
        let Ok(fit) = fit_response(p, r.id) else {
            problems.push(format!("{} has no fitted model", r.name));
            continue;
        };
        let (dlo, dhi) = response_range(p, r.id).unwrap_or((0.0, 1.0));
        let lower = r.lower.unwrap_or(dlo);
        let upper = r.upper.unwrap_or(dhi);
        let target = r.target.unwrap_or((lower + upper) / 2.0);
        out.push(RespModel {
            index: i,
            fit,
            spec: DesirSpec { goal: r.goal, lower, target, upper, weight: r.weight, importance: r.importance.clamp(0.1, 10.0) },
        });
    }
    (out, problems)
}

/// One environment/noise scenario: weight and coded values.
type Scenario = (f64, Vec<(usize, Coded)>);

pub struct Problem<'a> {
    pub p: &'a Project,
    pub models: Vec<RespModel>,
    /// Decision factors (controlled, not fixed).
    pub cont: Vec<usize>,
    pub discrete: Vec<usize>,
    pub cats: Vec<usize>,
    pub mixture: Vec<usize>,
    pub base: Vec<Option<Coded>>,
    pub scenarios: Vec<Scenario>,
}

#[derive(Clone, Debug)]
pub struct Evaluation {
    pub d: f64,
    pub soft: f64,
    /// Per model: (mean, sd over scenarios, desirability of mean).
    pub responses: Vec<(f64, f64, f64)>,
}

impl<'a> Problem<'a> {
    pub fn new(p: &'a Project, settings: &OptimizeSettings, models: Vec<RespModel>) -> Result<Problem<'a>, String> {
        let mut base = crate::analysis::center_point(p);
        let mut cont = Vec::new();
        let mut discrete = Vec::new();
        let mut cats = Vec::new();
        let mut mixture = Vec::new();
        let mut random: Vec<(usize, Vec<(f64, Coded)>)> = Vec::new();
        for (fi, f) in p.factors.iter().enumerate() {
            let fixed = settings.fixed.get(&f.id).and_then(|v| p.code(fi, v));
            match f.role {
                Role::Controlled => {
                    if let Some(c) = fixed {
                        base[fi] = Some(c);
                        continue;
                    }
                    match f.kind {
                        FactorKind::Continuous { .. } => cont.push(fi),
                        FactorKind::Discrete { .. } => discrete.push(fi),
                        FactorKind::Categorical { .. } => cats.push(fi),
                        FactorKind::Mixture { .. } => mixture.push(fi),
                    }
                }
                Role::Block => base[fi] = Some(Coded::Level(0)),
                Role::Noise | Role::Environmental => {
                    let s = settings.env.get(&f.id);
                    let random_mode = s.map(|s| s.mode == "random").unwrap_or(f.role == Role::Noise);
                    if !random_mode {
                        if let Some(c) = s.and_then(|s| s.value.as_ref()).and_then(|v| p.code(fi, v)) {
                            base[fi] = Some(c);
                        }
                        continue;
                    }
                    if f.is_categorical() {
                        let k = f.level_count().max(1);
                        random.push((fi, (0..k).map(|l| (1.0 / k as f64, Coded::Level(l))).collect()));
                    } else {
                        let (c, h) = f.center_half();
                        let obs = crate::analysis::factor_stats(p, fi);
                        let mean = s.and_then(|s| s.mean).or(obs.map(|o| o.0)).unwrap_or(c);
                        let sd = s.and_then(|s| s.sd).or(obs.map(|o| o.1)).unwrap_or(h / 2.0);
                        let (mc, sc) = ((mean - c) / h, sd / h);
                        let r3 = 3f64.sqrt();
                        random.push((
                            fi,
                            vec![
                                (1.0 / 6.0, Coded::Num(mc - r3 * sc)),
                                (2.0 / 3.0, Coded::Num(mc)),
                                (1.0 / 6.0, Coded::Num(mc + r3 * sc)),
                            ],
                        ));
                    }
                }
            }
        }
        let mut scenarios: Vec<Scenario> = vec![(1.0, vec![])];
        let total: f64 = random.iter().map(|(_, v)| v.len() as f64).product();
        if total <= 243.0 {
            for (fi, nodes) in &random {
                let mut next = Vec::new();
                for (w, s) in &scenarios {
                    for (wn, c) in nodes {
                        let mut s2 = s.clone();
                        s2.push((*fi, *c));
                        next.push((w * wn, s2));
                    }
                }
                scenarios = next;
            }
        } else {
            let mut rng = Rng::new(99);
            scenarios.clear();
            for _ in 0..200 {
                let mut s = Vec::new();
                for (fi, nodes) in &random {
                    // Samples nodes by weight.
                    let u = rng.uniform();
                    let mut acc = 0.0;
                    let mut pick = nodes[0].1;
                    for (w, c) in nodes {
                        acc += w;
                        if u <= acc {
                            pick = *c;
                            break;
                        }
                    }
                    s.push((*fi, pick));
                }
                scenarios.push((1.0 / 200.0, s));
            }
        }
        if !models.iter().any(|m| m.spec.goal != Goal::None) {
            return Err("set a goal for at least one response (Responses page)".into());
        }
        Ok(Problem { p, models, cont, discrete, cats, mixture, base, scenarios })
    }

    pub fn evaluate(&self, pt: &[Option<Coded>]) -> Evaluation {
        let mut sums = vec![(0.0, 0.0); self.models.len()];
        let mut dsum = 0.0;
        let mut softsum = 0.0;
        let mut wsum = 0.0;
        let mut q = pt.to_vec();
        let total_imp: f64 = self.models.iter().filter(|m| m.spec.goal != Goal::None).map(|m| m.spec.importance).sum();
        for (w, sc) in &self.scenarios {
            for &(fi, c) in sc {
                q[fi] = Some(c);
            }
            let mut logd = 0.0;
            let mut zero = false;
            let mut soft = 0.0;
            for (mi, m) in self.models.iter().enumerate() {
                let y = m
                    .fit
                    .layout()
                    .row(&q)
                    .map(|row| m.fit.input.transform.inverse(m.fit.predict_row(&row).0))
                    .unwrap_or(f64::NAN);
                sums[mi].0 += w * y;
                sums[mi].1 += w * y * y;
                if m.spec.goal != Goal::None {
                    let d = m.spec.d(y);
                    if d <= 0.0 {
                        zero = true;
                    } else {
                        logd += m.spec.importance / total_imp * d.ln();
                    }
                    soft += m.spec.importance / total_imp * m.spec.soft(y).min(1.0);
                }
            }
            dsum += w * if zero { 0.0 } else { logd.exp() };
            softsum += w * soft;
            wsum += w;
        }
        let responses = self
            .models
            .iter()
            .zip(&sums)
            .map(|(m, (s, s2))| {
                let mean = s / wsum;
                let var = (s2 / wsum - mean * mean).max(0.0);
                (mean, var.sqrt(), m.spec.d(mean))
            })
            .collect();
        Evaluation { d: dsum / wsum, soft: softsum / wsum, responses }
    }

    fn score(&self, pt: &[Option<Coded>]) -> f64 {
        let e = self.evaluate(pt);
        if e.d > 0.0 { e.d } else { 1e-4 * (e.soft - 1.0).min(0.0).max(-1e3) + 1e-6 * e.soft }
    }

    fn mix_upper(&self, f: usize) -> f64 {
        match self.p.factors[f].kind {
            FactorKind::Mixture { low, high } => ((high - low) / self.p.mixture_coding().span()).clamp(0.0, 1.0),
            _ => 1.0,
        }
    }

    /// Projects mixture coordinates onto the bounded simplex.
    fn project_mixture(&self, z: &mut [f64]) {
        for _ in 0..50 {
            for (i, v) in z.iter_mut().enumerate() {
                *v = v.clamp(0.0, self.mix_upper(self.mixture[i]));
            }
            let s: f64 = z.iter().sum();
            let diff = 1.0 - s;
            if diff.abs() < 1e-12 {
                break;
            }
            let free: Vec<usize> = (0..z.len())
                .filter(|&i| if diff > 0.0 { z[i] < self.mix_upper(self.mixture[i]) - 1e-12 } else { z[i] > 1e-12 })
                .collect();
            if free.is_empty() {
                break;
            }
            for &i in &free {
                z[i] += diff / free.len() as f64;
            }
        }
    }

    /// Builds the point from a continuous vector [cont..., discrete..., mixture...].
    fn point_from(&self, x: &[f64], catvals: &[usize]) -> Vec<Option<Coded>> {
        let mut pt = self.base.clone();
        let mut k = 0;
        for &f in self.cont.iter().chain(&self.discrete) {
            pt[f] = Some(Coded::Num(x[k].clamp(-1.0, 1.0)));
            k += 1;
        }
        if !self.mixture.is_empty() {
            let mut z = x[k..k + self.mixture.len()].to_vec();
            self.project_mixture(&mut z);
            for (i, &f) in self.mixture.iter().enumerate() {
                pt[f] = Some(Coded::Num(z[i]));
            }
        }
        for (i, &f) in self.cats.iter().enumerate() {
            pt[f] = Some(Coded::Level(catvals[i]));
        }
        pt
    }

    fn dims(&self) -> usize {
        self.cont.len() + self.discrete.len() + self.mixture.len()
    }

    fn snap(&self, pt: &mut [Option<Coded>]) {
        for &f in &self.discrete {
            let g: Vec<f64> = crate::design::level_grid(self.p, f, 0).iter().map(|c| c.num()).collect();
            if let Some(Coded::Num(x)) = pt[f] {
                pt[f] = Some(Coded::Num(crate::project::nearest(&g, x)));
            }
        }
    }

    /// Coordinate search over discrete and categorical levels.
    fn polish(&self, pt: &mut Vec<Option<Coded>>) -> f64 {
        let mut cur = self.score(pt);
        for _ in 0..20 {
            let mut improved = false;
            for &f in self.discrete.iter().chain(&self.cats) {
                for c in crate::design::level_grid(self.p, f, 0) {
                    let old = pt[f];
                    pt[f] = Some(c);
                    let s = self.score(pt);
                    if s > cur + 1e-12 {
                        cur = s;
                        improved = true;
                    } else {
                        pt[f] = old;
                    }
                }
            }
            if !improved {
                break;
            }
        }
        cur
    }

    fn x_from_point(&self, pt: &[Option<Coded>]) -> Vec<f64> {
        self.cont
            .iter()
            .chain(&self.discrete)
            .chain(&self.mixture)
            .map(|&f| pt[f].map(|c| c.num()).unwrap_or(0.0))
            .collect()
    }

    /// Multi-start Nelder-Mead over continuous coordinates for each
    /// categorical combination; returns distinct solutions sorted by D.
    pub fn solve(&self, nsol: usize, warm: Option<&[Option<Coded>]>) -> Vec<(Vec<Option<Coded>>, Evaluation)> {
        let mut rng = Rng::new(2024);
        let combos: Vec<Vec<usize>> = {
            let mut c: Vec<Vec<usize>> = vec![vec![]];
            for &f in &self.cats {
                let k = self.p.factors[f].level_count();
                c = c.into_iter().flat_map(|v| (0..k).map(move |l| {
                    let mut w = v.clone();
                    w.push(l);
                    w
                })).collect();
            }
            if c.len() > 48 {
                rng.shuffle(&mut c);
                c.truncate(48);
            }
            c
        };
        let dim = self.dims();
        let mut found: Vec<(Vec<Option<Coded>>, f64)> = Vec::new();
        for combo in &combos {
            let mut starts: Vec<Vec<f64>> = Vec::new();
            if let Some(w) = warm {
                starts.push(self.x_from_point(w));
            }
            for _ in 0..30 {
                let mut x: Vec<f64> = (0..self.cont.len() + self.discrete.len()).map(|_| rng.uniform() * 2.0 - 1.0).collect();
                if !self.mixture.is_empty() {
                    let e: Vec<f64> = self.mixture.iter().map(|_| -rng.uniform().max(1e-12).ln()).collect();
                    let s: f64 = e.iter().sum();
                    x.extend(e.iter().map(|v| v / s));
                }
                starts.push(x);
            }
            let mut scored: Vec<(f64, Vec<f64>)> = starts.into_iter().map(|x| (self.score(&self.point_from(&x, combo)), x)).collect();
            scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
            for (_, x0) in scored.into_iter().take(if dim == 0 { 1 } else { 4 }) {
                let x = if dim == 0 {
                    x0
                } else {
                    nelder_mead(|x| -self.score(&self.point_from(x, combo)), &x0, 0.25, 300 * dim.max(1))
                };
                let mut pt = self.point_from(&x, combo);
                self.snap(&mut pt);
                let s = self.polish(&mut pt);
                found.push((pt, s));
            }
        }
        found.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        let mut out: Vec<(Vec<Option<Coded>>, Evaluation)> = Vec::new();
        for (pt, _) in found {
            let dup = out.iter().any(|(q, _)| {
                q.iter().zip(&pt).all(|(a, b)| match (a, b) {
                    (Some(Coded::Num(x)), Some(Coded::Num(y))) => (x - y).abs() < 0.04,
                    (a, b) => a == b,
                })
            });
            if !dup {
                let e = self.evaluate(&pt);
                out.push((pt, e));
            }
            if out.len() >= nsol {
                break;
            }
        }
        out
    }
}

/// Nelder-Mead minimisation.
pub fn nelder_mead<F: Fn(&[f64]) -> f64>(f: F, x0: &[f64], step: f64, max_iter: usize) -> Vec<f64> {
    let n = x0.len();
    let mut simplex: Vec<Vec<f64>> = vec![x0.to_vec()];
    for i in 0..n {
        let mut x = x0.to_vec();
        x[i] += if x[i] + step <= 1.0 { step } else { -step };
        simplex.push(x);
    }
    let mut vals: Vec<f64> = simplex.iter().map(|x| f(x)).collect();
    for _ in 0..max_iter {
        let mut idx: Vec<usize> = (0..=n).collect();
        idx.sort_by(|&a, &b| vals[a].partial_cmp(&vals[b]).unwrap_or(std::cmp::Ordering::Equal));
        simplex = idx.iter().map(|&i| simplex[i].clone()).collect();
        vals = idx.iter().map(|&i| vals[i]).collect();
        if (vals[n] - vals[0]).abs() < 1e-10 && {
            let spread: f64 = (0..n).map(|j| (simplex[n][j] - simplex[0][j]).abs()).fold(0.0, f64::max);
            spread < 1e-6
        } {
            break;
        }
        let centroid: Vec<f64> = (0..n).map(|j| simplex[..n].iter().map(|x| x[j]).sum::<f64>() / n as f64).collect();
        let lerp = |t: f64| -> Vec<f64> { (0..n).map(|j| centroid[j] + t * (simplex[n][j] - centroid[j])).collect() };
        let xr = lerp(-1.0);
        let fr = f(&xr);
        if fr < vals[0] {
            let xe = lerp(-2.0);
            let fe = f(&xe);
            if fe < fr {
                simplex[n] = xe;
                vals[n] = fe;
            } else {
                simplex[n] = xr;
                vals[n] = fr;
            }
        } else if fr < vals[n - 1] {
            simplex[n] = xr;
            vals[n] = fr;
        } else {
            let xc = if fr < vals[n] { lerp(-0.5) } else { lerp(0.5) };
            let fc = f(&xc);
            if fc < vals[n].min(fr) {
                simplex[n] = xc;
                vals[n] = fc;
            } else {
                for i in 1..=n {
                    for j in 0..n {
                        simplex[i][j] = simplex[0][j] + 0.5 * (simplex[i][j] - simplex[0][j]);
                    }
                    vals[i] = f(&simplex[i]);
                }
            }
        }
    }
    let best = (0..=n).min_by(|&a, &b| vals[a].partial_cmp(&vals[b]).unwrap_or(std::cmp::Ordering::Equal)).unwrap();
    simplex[best].clone()
}

/// Result of an adjustment-law computation for one environmental factor.
pub struct ControlLaw {
    pub env_values: Vec<f64>,
    pub settings: Vec<Vec<Option<Coded>>>,
    pub desirability: Vec<f64>,
    /// For each continuous decision factor: polynomial coefficients (actual
    /// units, ascending powers) and R^2.
    pub fits: Vec<(usize, Vec<f64>, f64)>,
}

/// Optimal decision settings as a function of one environmental factor.
pub fn control_law(p: &Project, settings: &OptimizeSettings, env_fi: usize, steps: usize) -> Result<ControlLaw, String> {
    let f = &p.factors[env_fi];
    if !f.is_numeric() {
        return Err("choose a numeric environmental factor".into());
    }
    let (lo, hi) = f.range();
    let mut env_values = Vec::new();
    let mut sols = Vec::new();
    let mut ds = Vec::new();
    let mut warm: Option<Vec<Option<Coded>>> = None;
    for i in 0..steps {
        let x = lo + (hi - lo) * i as f64 / (steps - 1) as f64;
        let mut s = settings.clone();
        s.env.insert(f.id, crate::project::EnvSetting { mode: "fixed".into(), value: Some(Value::Num(x)), mean: None, sd: None });
        let (models, _) = response_models(p);
        let prob = Problem::new(p, &s, models)?;
        let best = prob.solve(1, warm.as_deref());
        let Some((pt, e)) = best.into_iter().next() else { continue };
        warm = Some(pt.clone());
        env_values.push(x);
        sols.push(pt);
        ds.push(e.d);
    }
    // Polynomial fits of each continuous controlled setting vs the factor.
    let mut fits = Vec::new();
    let decision: Vec<usize> = p
        .factors
        .iter()
        .enumerate()
        .filter(|(_, g)| g.role == Role::Controlled && matches!(g.kind, FactorKind::Continuous { .. } | FactorKind::Discrete { .. } | FactorKind::Mixture { .. }))
        .map(|(i, _)| i)
        .collect();
    for &d in &decision {
        let ys: Vec<f64> = sols.iter().filter_map(|pt| p.decode(d, pt[d]?).as_num()).collect();
        if ys.len() != env_values.len() || ys.len() < 3 {
            continue;
        }
        let deg = if ys.len() >= 6 { 2 } else { 1 };
        let rows: Vec<Vec<f64>> = env_values.iter().map(|x| (0..=deg).map(|k| x.powi(k as i32)).collect()).collect();
        let x = crate::linalg::Mat::from_rows(&rows);
        let dec = crate::linalg::LsqDecomp::new(&x, 1e-10);
        let b = dec.solve(&ys);
        let mut coef = vec![0.0; deg + 1];
        for (k, &c) in dec.kept.iter().enumerate() {
            coef[c] = b[k];
        }
        let fitted: Vec<f64> = env_values.iter().map(|x| coef.iter().enumerate().map(|(k, c)| c * x.powi(k as i32)).sum()).collect();
        let m = stats::mean(&ys);
        let sst: f64 = ys.iter().map(|y| (y - m).powi(2)).sum();
        let sse: f64 = ys.iter().zip(&fitted).map(|(a, b)| (a - b).powi(2)).sum();
        let r2 = if sst > 1e-14 { 1.0 - sse / sst } else { 1.0 };
        fits.push((d, coef, r2));
    }
    Ok(ControlLaw { env_values, settings: sols, desirability: ds, fits })
}
