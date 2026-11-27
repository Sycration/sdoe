//! Model selection: stepwise AICc, backward elimination and automatic
//! model search across response transformations.

use super::fit::{ALIAS_TOL, gather};
use super::terms::{ModelLayout, Preset, Term, preset_terms, sort_terms};
use crate::linalg::{LsqDecomp, Mat};
use crate::project::{Project, Role, Transform};
use crate::stats;

/// Fast evaluator of term subsets on a fixed full model matrix.
struct SubsetEval {
    x: Mat,
    y: Vec<f64>,
    intercept_col: Option<usize>,
    term_cols: Vec<Vec<usize>>,
}

struct SubsetFit {
    sse: f64,
    rank: usize,
}

impl SubsetEval {
    fn new(p: &Project, response_id: u64, terms: &[Term], transform: Transform) -> Option<(SubsetEval, ModelLayout)> {
        let inp = gather(p, response_id, terms, transform);
        if inp.y.is_empty() {
            return None;
        }
        let layout = inp.layout.clone();
        let term_cols = (0..layout.terms.len()).map(|t| layout.term_columns(t)).collect();
        let intercept_col = layout.intercept.then_some(0);
        Some((SubsetEval { x: inp.x, y: inp.y, intercept_col, term_cols }, layout))
    }

    fn eval(&self, active: &[bool]) -> SubsetFit {
        let mut cols: Vec<usize> = self.intercept_col.into_iter().collect();
        for (t, &a) in active.iter().enumerate() {
            if a {
                cols.extend(&self.term_cols[t]);
            }
        }
        if cols.is_empty() {
            return SubsetFit { sse: self.y.iter().map(|v| v * v).sum(), rank: 0 };
        }
        let xs = self.x.select_cols(&cols);
        let d = LsqDecomp::new(&xs, ALIAS_TOL);
        let b = d.solve(&self.y);
        let fitted = xs.select_cols(&d.kept).mul_vec(&b);
        let sse = self.y.iter().zip(&fitted).map(|(a, b)| (a - b).powi(2)).sum();
        SubsetFit { sse, rank: d.rank() }
    }

    fn aicc(&self, f: &SubsetFit) -> f64 {
        let n = self.y.len() as f64;
        let k = f.rank as f64 + 1.0;
        if n - k - 1.0 <= 0.0 {
            return f64::INFINITY;
        }
        n * (crate::model::fit::sse_floor(f.sse, &self.y) / n).ln() + 2.0 * k + 2.0 * k * (k + 1.0) / (n - k - 1.0)
    }
}

/// Terms that a selection procedure never removes: block effects and
/// linear mixture blending terms.
fn always_keep(p: &Project, t: &Term) -> bool {
    t.0.len() == 1 && {
        let f = &p.factors[t.0[0].0];
        f.role == Role::Block || (f.is_mixture() && t.0[0].1 == 1)
    }
}

fn parents_present(terms: &[Term], active: &[bool], t: usize) -> bool {
    terms[t].parents().iter().all(|par| terms.iter().zip(active).any(|(u, &a)| a && u == par))
}

fn is_parent_of_active(terms: &[Term], active: &[bool], t: usize) -> bool {
    terms.iter().zip(active).any(|(u, &a)| a && u.contains(&terms[t]))
}

#[derive(Clone, Debug)]
pub struct StepLog {
    pub action: String,
    pub term: String,
    pub criterion: f64,
}

/// Bidirectional stepwise selection by AICc within the candidate `pool`,
/// respecting model hierarchy.
pub fn stepwise_aicc(
    p: &Project,
    response_id: u64,
    pool: &[Term],
    transform: Transform,
    start: &[Term],
) -> Result<(Vec<Term>, Vec<StepLog>), String> {
    let (ev, layout) = SubsetEval::new(p, response_id, pool, transform).ok_or("no complete data")?;
    let terms = &layout.terms;
    let mut active: Vec<bool> = terms.iter().map(|t| start.contains(t) || always_keep(p, t)).collect();
    // Hierarchy repair of the start set.
    for _ in 0..4 {
        for i in 0..terms.len() {
            if active[i] {
                for par in terms[i].parents() {
                    if let Some(j) = terms.iter().position(|u| *u == par) {
                        active[j] = true;
                    }
                }
            }
        }
    }
    let mut log = Vec::new();
    let mut cur = ev.aicc(&ev.eval(&active));
    log.push(StepLog { action: "Start".into(), term: String::new(), criterion: cur });
    for _ in 0..500 {
        // A move adds one term together with its missing lower-order terms,
        // or removes one term. Adding the lower-order terms in the same move
        // lets a term such as A^2 enter after A has been removed.
        let mut best: Option<(usize, bool, f64, Vec<usize>)> = None;
        for t in 0..terms.len() {
            let adding = !active[t];
            let mut set = vec![t];
            if adding {
                set.extend((0..terms.len()).filter(|&u| !active[u] && terms[t].contains(&terms[u])));
            } else if always_keep(p, &terms[t]) || is_parent_of_active(terms, &active, t) {
                continue;
            }
            for &u in &set {
                active[u] = adding;
            }
            let ok = !adding || set.iter().all(|&u| parents_present(terms, &active, u));
            let c = if ok { ev.aicc(&ev.eval(&active)) } else { f64::INFINITY };
            for &u in &set {
                active[u] = !adding;
            }
            if c < cur - 1e-9 && best.as_ref().is_none_or(|b| c < b.2) {
                best = Some((t, adding, c, set));
            }
        }
        match best {
            Some((t, adding, c, set)) => {
                for &u in &set {
                    active[u] = adding;
                }
                cur = c;
                let mut term = layout.term_labels[t].clone();
                if set.len() > 1 {
                    let with: Vec<&str> = set[1..].iter().map(|&u| layout.term_labels[u].as_str()).collect();
                    term = format!("{term} with {}", with.join(", "));
                }
                log.push(StepLog {
                    action: if adding { "Add".into() } else { "Remove".into() },
                    term,
                    criterion: c,
                });
            }
            None => break,
        }
    }
    let mut out: Vec<Term> = terms.iter().zip(&active).filter(|(_, a)| **a).map(|(t, _)| t.clone()).collect();
    sort_terms(&mut out);
    Ok((out, log))
}

/// Backward elimination by partial F-test p-value, respecting hierarchy.
pub fn backward_p(
    p: &Project,
    response_id: u64,
    pool: &[Term],
    transform: Transform,
    alpha: f64,
) -> Result<(Vec<Term>, Vec<StepLog>), String> {
    let (ev, layout) = SubsetEval::new(p, response_id, pool, transform).ok_or("no complete data")?;
    let terms = &layout.terms;
    let mut active = vec![true; terms.len()];
    let mut log = Vec::new();
    let n = ev.y.len() as f64;
    loop {
        let full = ev.eval(&active);
        let df_res = n - full.rank as f64;
        if df_res <= 0.0 {
            // Saturated: drops the smallest contribution until a residual exists.
            let mut best: Option<(usize, f64)> = None;
            for t in 0..terms.len() {
                if !active[t] || always_keep(p, &terms[t]) || is_parent_of_active(terms, &active, t) {
                    continue;
                }
                active[t] = false;
                let r = ev.eval(&active);
                active[t] = true;
                let inc = r.sse - full.sse;
                if best.is_none_or(|b| inc < b.1) {
                    best = Some((t, inc));
                }
            }
            match best {
                Some((t, _)) => {
                    active[t] = false;
                    log.push(StepLog { action: "Remove".into(), term: layout.term_labels[t].clone(), criterion: f64::NAN });
                    continue;
                }
                None => break,
            }
        }
        let mse = full.sse / df_res;
        let mut worst: Option<(usize, f64)> = None;
        for t in 0..terms.len() {
            if !active[t] || always_keep(p, &terms[t]) || is_parent_of_active(terms, &active, t) {
                continue;
            }
            active[t] = false;
            let r = ev.eval(&active);
            active[t] = true;
            let df = (full.rank - r.rank) as f64;
            let pv = if df > 0.0 {
                stats::f_sf(((r.sse - full.sse).max(0.0) / df) / mse, df, df_res)
            } else {
                1.0
            };
            if worst.is_none_or(|w| pv > w.1) {
                worst = Some((t, pv));
            }
        }
        match worst {
            Some((t, pv)) if pv > alpha => {
                active[t] = false;
                log.push(StepLog { action: "Remove".into(), term: layout.term_labels[t].clone(), criterion: pv });
            }
            _ => break,
        }
    }
    let mut out: Vec<Term> = terms.iter().zip(&active).filter(|(_, a)| **a).map(|(t, _)| t.clone()).collect();
    sort_terms(&mut out);
    Ok((out, log))
}

#[derive(Clone, Debug)]
pub struct AutoCandidate {
    pub transform: Transform,
    pub terms: Vec<Term>,
    pub pred_r2: f64,
    pub adj_r2: f64,
    pub aicc: f64,
}

/// Searches transforms x hierarchical stepwise models and ranks the results
/// by predicted R^2 on the original response scale.
pub fn auto_model(p: &Project, response_id: u64, env_interactions: bool) -> Result<Vec<AutoCandidate>, String> {
    // the runs the fit uses: not excluded, finite response, all factors present
    let start = preset_terms(p, Preset::Linear, false);
    let ys: Vec<f64> = gather(p, response_id, &start, Transform::None).y_raw;
    if ys.is_empty() {
        return Err("no data for this response".into());
    }
    let ymin = ys.iter().cloned().fold(f64::INFINITY, f64::min);
    let mut transforms = vec![Transform::None];
    if ymin > 0.0 {
        transforms.extend([Transform::Ln, Transform::Sqrt, Transform::Inverse]);
    } else if ymin >= 0.0 {
        transforms.push(Transform::Sqrt);
    }
    let n = ys.len();
    let mut pool = preset_terms(p, Preset::Quadratic, env_interactions);
    let cubic = preset_terms(p, Preset::Cubic, env_interactions);
    if n >= 2 * cubic.len() + 4 {
        pool = cubic;
    }
    let mut out = Vec::new();
    for tr in transforms {
        let Ok((terms, _)) = stepwise_aicc(p, response_id, &pool, tr, &start) else { continue };
        let inp = gather(p, response_id, &terms, tr);
        let Ok(f) = super::fit::fit(inp) else { continue };
        out.push(AutoCandidate { transform: tr, terms, pred_r2: f.pred_r2_original(), adj_r2: f.adj_r2, aicc: f.aicc });
    }
    out.sort_by(|a, b| {
        let ka = if a.pred_r2.is_nan() { f64::NEG_INFINITY } else { a.pred_r2 };
        let kb = if b.pred_r2.is_nan() { f64::NEG_INFINITY } else { b.pred_r2 };
        kb.partial_cmp(&ka).unwrap_or(std::cmp::Ordering::Equal)
    });
    if out.is_empty() {
        return Err("no model could be fitted".into());
    }
    Ok(out)
}
