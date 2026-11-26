//! Analysis outputs: fit summaries, tables, canonical analysis and plots.

use crate::analysis::{center_point, display_value, model_for};
use crate::format::{self, num, pval};
use crate::model::effects;
use crate::model::fit::{Fit, fit, gather};
use crate::model::terms::{ColPart, Term, format_formula};
use crate::plot::*;
use crate::project::{Coded, FactorKind, Project, Role, RunStatus};
use crate::stats;
use crate::table::Table;
use serde_json::{Value as Json, json};

pub fn fit_for(p: &Project, rid: u64) -> Result<Fit, String> {
    let (terms, tr) = model_for(p, rid);
    fit(gather(p, rid, &terms, tr))
}

pub fn coefficient_table(fit: &Fit) -> Table {
    let mut t = Table::new(
        &["Term", "Coefficient", "Std. error", "t", "p", "95% CI low", "95% CI high", "VIF"],
        &[false, true, true, true, true, true, true, true],
    );
    let tq = if fit.df_resid > 0.0 { stats::t_quantile(0.975, fit.df_resid) } else { f64::NAN };
    let layout = fit.layout();
    let mut seen_terms = std::collections::HashSet::new();
    for (ci, col) in layout.columns.iter().enumerate() {
        let tag = match col.term {
            Some(ti) if seen_terms.insert(ti) => layout.term_labels[ti].clone(),
            _ => String::new(),
        };
        if fit.aliased[ci] {
            t.push(vec![col.label.clone(), "aliased".into(), String::new(), String::new(), String::new(), String::new(), String::new(), String::new()], &tag, "muted");
            continue;
        }
        let c = fit.coef[ci];
        let se = fit.se[ci];
        let p = fit.p[ci];
        let em = if col.term.is_none() {
            ""
        } else if p < 0.05 {
            "good"
        } else if p > 0.10 {
            "muted"
        } else {
            ""
        };
        t.push(
            vec![
                col.label.clone(),
                num(c, 6),
                num(se, 4),
                format::fixed(fit.t[ci], 3),
                pval(p),
                num(c - tq * se, 5),
                num(c + tq * se, 5),
                if col.term.is_some() { format::fixed(fit.vif[ci], 2) } else { String::new() },
            ],
            &tag,
            em,
        );
    }
    t
}

pub fn anova_table(fit: &Fit) -> Table {
    let mut t = Table::new(&["Source", "Sum of squares", "df", "Mean square", "F", "p"], &[false, true, true, true, true, true]);
    let row = |r: &crate::model::fit::AnovaRow| {
        vec![
            r.source.clone(),
            num(r.ss, 6),
            num(r.df, 4),
            num(r.ms, 6),
            if r.f.is_finite() { format::fixed(r.f, 3) } else { String::new() },
            pval(r.p),
        ]
    };
    let m = &fit.model_row;
    t.push(row(m), "", "strong");
    let layout = fit.layout();
    for r in &fit.anova {
        let em = if r.p < 0.05 { "good" } else if r.p > 0.10 { "muted" } else { "" };
        let tag = r.term.map(|ti| layout.term_labels[ti].clone()).unwrap_or_default();
        t.push(row(r), &tag, em);
    }
    t.push(row(&fit.resid_row), "", "");
    if let Some((lof, pe)) = &fit.lack_of_fit {
        let em = if lof.p < 0.05 { "bad" } else { "" };
        t.push(row(lof), "", em);
        t.push(row(pe), "", "");
    }
    t.push(
        vec!["Total (corrected)".into(), num(fit.sst, 6), num(fit.n as f64 - 1.0, 4), String::new(), String::new(), String::new()],
        "",
        "strong",
    );
    t
}

pub fn stats_pairs(fit: &Fit) -> Vec<(String, String)> {
    vec![
        ("Runs used".into(), fit.n.to_string()),
        ("Std. deviation".into(), num(fit.rmse(), 5)),
        ("Mean".into(), num(fit.mean_y, 5)),
        ("C.V. %".into(), format::fixed(fit.cv, 2)),
        ("R²".into(), format::fixed(fit.r2, 4)),
        ("Adjusted R²".into(), format::fixed(fit.adj_r2, 4)),
        ("Predicted R²".into(), format::fixed(fit.pred_r2, 4)),
        ("Adeq. precision".into(), format::fixed(fit.adeq_precision, 3)),
        ("PRESS".into(), num(fit.press, 5)),
        ("AICc".into(), format::fixed(fit.aicc, 2)),
        ("BIC".into(), format::fixed(fit.bic, 2)),
    ]
}

/// Tests whether interactions between the environmental factors in the
/// model and the designed main effects would add significant information;
/// returns a hint naming the environmental factors when they do.
pub fn env_interaction_hint(p: &Project, fit: &Fit) -> Option<String> {
    let layout = fit.layout();
    let mains: Vec<usize> = layout.terms.iter().filter(|t| t.0.len() == 1 && t.0[0].1 == 1).map(|t| t.0[0].0).collect();
    let env: Vec<usize> = mains.iter().cloned().filter(|&f| p.factors[f].role == Role::Environmental).collect();
    let designed: Vec<usize> = mains.iter().cloned().filter(|&f| p.factors[f].role.is_designed() && !p.factors[f].is_mixture()).collect();
    if env.is_empty() || designed.is_empty() {
        return None;
    }
    let mut extra = Vec::new();
    for &e in &env {
        for &d in &designed {
            let t = Term::single(e, 1).mul(&Term::single(d, 1));
            if !layout.terms.contains(&t) {
                extra.push(t);
            }
        }
    }
    if extra.is_empty() {
        return None;
    }
    let mut terms = layout.terms.clone();
    terms.extend(extra);
    crate::model::terms::sort_terms(&mut terms);
    let big = fit_terms(p, fit, &terms)?;
    let df1 = big.rank as f64 - fit.rank as f64;
    if df1 <= 0.0 || big.df_resid <= 0.0 {
        return None;
    }
    let f = ((fit.sse - big.sse).max(0.0) / df1) / big.mse;
    let pv = stats::f_sf(f, df1, big.df_resid);
    if pv >= 0.05 {
        return None;
    }
    let names: Vec<String> = env.iter().map(|&e| p.factors[e].name.clone()).collect();
    Some(format!(
        "Environment × factor interactions are significant (p = {}): the best settings depend on {}. Add them with Model ▸ Include environment × factor interactions, then choose a preset or run stepwise selection.",
        pval(pv).trim_start_matches("< "),
        names.join(", ")
    ))
}

fn fit_terms(p: &Project, base: &Fit, terms: &[Term]) -> Option<Fit> {
    fit(gather(p, base.input.response_id, terms, base.input.transform)).ok()
}

pub fn warnings(p: &Project, fit: &Fit) -> Vec<String> {
    let mut w = Vec::new();
    let inp = &fit.input;
    if inp.excluded > 0 {
        w.push(format!("{} excluded runs are left out.", inp.excluded));
    }
    if inp.skipped_missing_response > 0 {
        w.push(format!("{} runs without a value for this response are left out.", inp.skipped_missing_response));
    }
    if inp.skipped_missing_factor > 0 {
        let env_missing = p.factors.iter().any(|f| {
            f.role == Role::Environmental
                && inp.layout.factors_used().contains(&p.factor_index(f.id).unwrap_or(usize::MAX))
                && p.runs.iter().any(|r| r.status != RunStatus::Excluded && !r.settings.contains_key(&f.id))
        });
        w.push(format!(
            "{} runs lack values for model factors and are left out{}.",
            inp.skipped_missing_factor,
            if env_missing { " (enter the measured environmental values)" } else { "" }
        ));
    }
    if inp.skipped_transform > 0 {
        w.push(format!("{} runs have values the transform cannot handle (e.g. ln of 0).", inp.skipped_transform));
    }
    let aliased: Vec<String> = fit
        .layout()
        .columns
        .iter()
        .zip(&fit.aliased)
        .filter(|(_, a)| **a)
        .map(|(c, _)| c.label.clone())
        .collect();
    if !aliased.is_empty() {
        w.push(format!("Aliased (not estimable with these runs): {}.", aliased.join(", ")));
    }
    if fit.df_resid <= 0.0 {
        w.push("No residual degrees of freedom: p-values need replicates or fewer terms (see the half-normal plot).".into());
    }
    if fit.adj_r2.is_finite() && fit.pred_r2.is_finite() && fit.adj_r2 - fit.pred_r2 > 0.2 {
        w.push("Predicted R² is much lower than adjusted R²: consider removing insignificant terms or a transform.".into());
    }
    if let Some((lof, _)) = &fit.lack_of_fit {
        if lof.p < 0.05 {
            w.push("Significant lack of fit: the model misses curvature or interactions, or there are outliers.".into());
        }
    }
    if fit.adeq_precision.is_finite() && fit.adeq_precision < 4.0 && fit.df_resid > 0.0 {
        w.push("Adequate precision below 4: the signal is weak relative to noise.".into());
    }
    for (ci, col) in fit.layout().columns.iter().enumerate() {
        if col.term.is_some() && fit.vif[ci] > 10.0 {
            w.push(format!("{} has VIF {:.1}: its estimate is correlated with other terms.", col.label, fit.vif[ci]));
        }
    }
    let ext = fit.ext_studentized();
    let outliers: Vec<String> = ext
        .iter()
        .enumerate()
        .filter(|(_, r)| r.abs() > 3.5)
        .map(|(i, _)| p.runs[inp.rows[i]].run_order.to_string())
        .collect();
    if let Some(h) = env_interaction_hint(p, fit) {
        w.push(h);
    }
    if !outliers.is_empty() {
        w.push(format!("Possible outliers (|studentized residual| > 3.5) at run {}.", outliers.join(", ")));
    }
    w
}

/// Stationary point and eigen analysis of the quadratic part in the
/// numeric factors (other factors at their center / first level).
pub fn canonical(p: &Project, fit: &Fit) -> Vec<String> {
    let layout = fit.layout();
    let numeric: Vec<usize> = layout
        .factors_used()
        .into_iter()
        .filter(|&f| matches!(p.factors[f].kind, FactorKind::Continuous { .. } | FactorKind::Discrete { .. }))
        .collect();
    let has_sq = |f: usize| layout.terms.iter().any(|t| t.0 == vec![(f, 2)]);
    let qf: Vec<usize> = numeric.into_iter().filter(|&f| has_sq(f)).collect();
    if qf.is_empty() {
        return vec![];
    }
    let k = qf.len();
    let mut b = vec![0.0; k];
    let mut bm = crate::linalg::Mat::zeros(k, k);
    for (ci, col) in layout.columns.iter().enumerate() {
        if fit.aliased[ci] || col.term.is_none() {
            continue;
        }
        let c = fit.coef[ci];
        let parts: Vec<(usize, u32)> = col
            .parts
            .iter()
            .filter_map(|p| match p {
                ColPart::Num { factor, power } => Some((*factor, *power)),
                _ => None,
            })
            .collect();
        if parts.len() != col.parts.len() || !parts.iter().all(|(f, _)| qf.contains(f)) {
            continue;
        }
        let idx = |f: usize| qf.iter().position(|&g| g == f).unwrap();
        match parts.as_slice() {
            [(f, 1)] => b[idx(*f)] += c,
            [(f, 2)] => bm[(idx(*f), idx(*f))] += c,
            [(f, 1), (g, 1)] => {
                bm[(idx(*f), idx(*g))] += c / 2.0;
                bm[(idx(*g), idx(*f))] += c / 2.0;
            }
            _ => {}
        }
    }
    let Some(binv) = crate::linalg::inverse(&bm) else {
        return vec!["The quadratic part is singular: the surface has a ridge.".into()];
    };
    let xs: Vec<f64> = binv.mul_vec(&b).iter().map(|v| -0.5 * v).collect();
    let (vals, _) = crate::linalg::sym_eigen(&bm);
    let kind = if vals.iter().all(|v| *v < 0.0) {
        "maximum"
    } else if vals.iter().all(|v| *v > 0.0) {
        "minimum"
    } else {
        "saddle point"
    };
    let inside = xs.iter().all(|x| x.abs() <= 1.0 + 1e-9);
    let mut pt = center_point(p);
    for (i, &f) in qf.iter().enumerate() {
        pt[f] = Some(Coded::Num(xs[i]));
    }
    let loc: Vec<String> = qf
        .iter()
        .enumerate()
        .map(|(i, &f)| format!("{} = {}", p.factors[f].name, display_value(p, f, Some(Coded::Num(xs[i])))))
        .collect();
    let mut out = vec![format!(
        "Stationary point ({kind}{}): {}",
        if inside { ", inside the factor ranges" } else { ", outside the factor ranges" },
        loc.join(", ")
    )];
    if let Some(pr) = fit.predict_point(&pt, 0.05) {
        out.push(format!("Predicted response there: {}", num(pr.y, 5)));
    }
    out.push(format!("Eigenvalues: {}", vals.iter().map(|v| num(*v, 4)).collect::<Vec<_>>().join(", ")));
    out
}

pub fn fit_summary(p: &Project, rid: u64) -> Result<Json, String> {
    let ri = p.response_index(rid).ok_or("unknown response")?;
    let (terms, tr) = model_for(p, rid);
    let f = fit(gather(p, rid, &terms, tr))?;
    let screen = effects::screen(&f, 0.05);
    Ok(json!({
        "response": p.responses[ri].name,
        "formula": format_formula(p, &terms),
        "transform": tr.label(),
        "stats": stats_pairs(&f),
        "anova": anova_table(&f),
        "coefficients": coefficient_table(&f),
        "warnings": warnings(p, &f),
        "canonical": canonical(p, &f),
        "lenth": { "pse": screen.pse, "me": screen.me, "sme": screen.sme, "used": screen.uses_lenth },
        "df_resid": f.df_resid,
    }))
}

// ---------------------------------------------------------------------------
// Plots
// ---------------------------------------------------------------------------

fn run_ids(p: &Project, fit: &Fit) -> Vec<i64> {
    fit.input.rows.iter().map(|&r| p.runs[r].run_order as i64).collect()
}

fn resp_label(p: &Project, rid: u64) -> String {
    p.response_index(rid).map(|i| {
        let r = &p.responses[i];
        if r.units.is_empty() { r.name.clone() } else { format!("{} [{}]", r.name, r.units) }
    }).unwrap_or_default()
}

fn factor_label(p: &Project, fi: usize) -> String {
    let f = &p.factors[fi];
    if f.units.is_empty() { format!("{}: {}", f.symbol, f.name) } else { format!("{}: {} [{}]", f.symbol, f.name, f.units) }
}

pub fn plot_pred_actual(p: &Project, fit: &Fit, rid: u64) -> Plot {
    let tr = fit.input.transform;
    let pred: Vec<f64> = fit.fitted.iter().map(|v| tr.inverse(*v)).collect();
    let act = fit.input.y_raw.clone();
    let mut all = pred.clone();
    all.extend(&act);
    let ax = data_axis(&all, 6);
    let mut pts = Series::new("Runs", "points", act.clone(), pred, 0);
    pts.ids = run_ids(p, fit);
    Plot {
        title: "Predicted vs actual".into(),
        x_label: format!("Actual {}", resp_label(p, rid)),
        y_label: "Predicted".into(),
        x: ax.clone(),
        y: ax.clone(),
        series: vec![Series::new("", "line", vec![ax.min, ax.max], vec![ax.min, ax.max], -2), pts],
        kind: "xy",
        ..Default::default()
    }
}

pub fn plot_resid_fitted(p: &Project, fit: &Fit) -> Plot {
    let r = fit.ext_studentized();
    let xs = fit.fitted.clone();
    let lim = r.iter().cloned().filter(|v| v.is_finite()).fold(3.5f64, |a, b| a.max(b.abs() + 0.3));
    let mut pts = Series::new("Runs", "points", xs.clone(), r, 0);
    pts.ids = run_ids(p, fit);
    let mut s = vec![Series::new("", "hline", vec![], vec![0.0], -1)];
    let mut h = Series::new("", "hline", vec![], vec![-3.0, 3.0], -3);
    h.dashed = true;
    s.push(h);
    s.push(pts);
    Plot {
        title: "Residuals vs predicted".into(),
        x_label: if fit.input.transform == crate::project::Transform::None { "Predicted".into() } else { "Predicted (transformed)".into() },
        y_label: "Ext. studentized residual".into(),
        x: data_axis(&xs, 6),
        y: nice_axis(-lim, lim, 7, true),
        series: s,
        kind: "xy",
        ..Default::default()
    }
}

pub fn plot_resid_normal(p: &Project, fit: &Fit) -> Plot {
    let r = fit.ext_studentized();
    let ids = run_ids(p, fit);
    let mut idx: Vec<usize> = (0..r.len()).filter(|&i| r[i].is_finite()).collect();
    idx.sort_by(|&a, &b| r[a].partial_cmp(&r[b]).unwrap());
    let q = effects::normal_quantiles(idx.len());
    let xs: Vec<f64> = idx.iter().map(|&i| r[i]).collect();
    let mut pts = Series::new("Runs", "points", xs.clone(), q.clone(), 0);
    pts.ids = idx.iter().map(|&i| ids[i]).collect();
    let ax = data_axis(&xs, 6);
    let line = Series::new("", "line", vec![ax.min, ax.max], vec![ax.min, ax.max], -2);
    Plot {
        title: "Normal plot of residuals".into(),
        x_label: "Ext. studentized residual".into(),
        y_label: "Normal quantile".into(),
        x: ax,
        y: data_axis(&q, 6),
        series: vec![line, pts],
        kind: "xy",
        ..Default::default()
    }
}

pub fn plot_resid_order(p: &Project, fit: &Fit) -> Plot {
    let r = fit.ext_studentized();
    let ids = run_ids(p, fit);
    let xs: Vec<f64> = ids.iter().map(|&i| i as f64).collect();
    let mut order: Vec<usize> = (0..xs.len()).collect();
    order.sort_by(|&a, &b| xs[a].partial_cmp(&xs[b]).unwrap());
    let ox: Vec<f64> = order.iter().map(|&i| xs[i]).collect();
    let oy: Vec<f64> = order.iter().map(|&i| r[i]).collect();
    let lim = oy.iter().cloned().filter(|v| v.is_finite()).fold(3.5f64, |a, b| a.max(b.abs() + 0.3));
    let mut pts = Series::new("Runs", "points", ox.clone(), oy.clone(), 0);
    pts.ids = order.iter().map(|&i| ids[i]).collect();
    let mut h = Series::new("", "hline", vec![], vec![-3.0, 3.0], -3);
    h.dashed = true;
    Plot {
        title: "Residuals vs run order".into(),
        x_label: "Run".into(),
        y_label: "Ext. studentized residual".into(),
        x: data_axis(&ox, 8),
        y: nice_axis(-lim, lim, 7, true),
        series: vec![Series::new("", "line", ox, oy, -1), Series::new("", "hline", vec![], vec![0.0], -1), h, pts],
        kind: "xy",
        ..Default::default()
    }
}

pub fn plot_cooks(p: &Project, fit: &Fit) -> Plot {
    let c = fit.cooks_distance();
    let ids = run_ids(p, fit);
    let xs: Vec<f64> = ids.iter().map(|&i| i as f64).collect();
    let mut bars = Series::new("Cook's distance", "bars", xs.clone(), c.clone(), 0);
    bars.ids = ids;
    let mut lim = Series::new("", "hline", vec![], vec![4.0 / fit.n as f64], -3);
    lim.dashed = true;
    let mut ys = c.clone();
    ys.push(0.0);
    ys.push(4.0 / fit.n as f64);
    Plot {
        title: "Cook's distance".into(),
        x_label: "Run".into(),
        y_label: "Cook's distance".into(),
        x: data_axis(&xs, 8),
        y: data_axis(&ys, 6),
        series: vec![bars, lim],
        kind: "xy",
        ..Default::default()
    }
}

pub fn plot_boxcox(fit: &Fit) -> Result<Plot, String> {
    let bc = effects::box_cox(fit, 0.05)?;
    let mut s = vec![Series::new("ln(SSE)", "line", bc.lambdas.clone(), bc.ln_sse.clone(), 0)];
    let mut thr = Series::new("", "hline", vec![], vec![bc.threshold], -2);
    thr.dashed = true;
    s.push(thr);
    if bc.ci.0.is_finite() {
        let mut v = Series::new("95% CI", "vline", vec![bc.ci.0, bc.ci.1], vec![], -2);
        v.dashed = true;
        s.push(v);
    }
    let current = match fit.input.transform {
        crate::project::Transform::None => 1.0,
        crate::project::Transform::Ln => 0.0,
        crate::project::Transform::Sqrt => 0.5,
        crate::project::Transform::Inverse => -1.0,
        crate::project::Transform::Power(l) => l,
        crate::project::Transform::Log10 => 0.0,
    };
    s.push(Series::new("Current", "vline", vec![current], vec![], 1));
    s.push(Series::new("Best", "vline", vec![bc.best], vec![], -3));
    Ok(Plot {
        title: "Box-Cox".into(),
        x_label: "λ".into(),
        y_label: "ln(residual SS)".into(),
        x: nice_axis(-3.0, 3.0, 7, true),
        y: data_axis(&bc.ln_sse, 6),
        series: s,
        kind: "xy",
        legend: true,
        note: format!(
            "Best λ = {}, 95% CI {} to {}. Recommended: {}.",
            num(bc.best, 3),
            num(bc.ci.0, 3),
            num(bc.ci.1, 3),
            crate::model::effects::lambda_transform(bc.recommended).label()
        ),
        ..Default::default()
    })
}

pub fn plot_pareto(fit: &Fit) -> Plot {
    let sc = effects::screen(fit, 0.05);
    let mut e = sc.effects.clone();
    e.sort_by(|a, b| b.t.abs().partial_cmp(&a.t.abs()).unwrap_or(std::cmp::Ordering::Equal));
    let n = e.len();
    let ys: Vec<f64> = (0..n).map(|i| i as f64).collect();
    let xs: Vec<f64> = e.iter().map(|x| x.t.abs()).collect();
    let labels: Vec<String> = e.iter().map(|x| x.label.clone()).collect();
    let mut bars = Series::new("|t|", "hbars", xs.clone(), ys, 0);
    bars.labels = labels.clone();
    bars.values = e.iter().map(|x| if x.effect >= 0.0 { 1.0 } else { -1.0 }).collect();
    let mut lim = Series::new(if sc.uses_lenth { "Lenth ME" } else { "t (α = 0.05)" }, "vline", vec![sc.t_crit], vec![], -3);
    lim.dashed = true;
    let mut xr = xs.clone();
    xr.push(0.0);
    xr.push(sc.t_crit);
    let mut ax = data_axis(&xr, 6);
    ax.min = 0.0;
    let mut yax = category_axis(&labels);
    yax.ticks.reverse();
    Plot {
        title: "Pareto of standardized effects".into(),
        x_label: if sc.uses_lenth { "|effect| / PSE".into() } else { "|t|".into() },
        y_label: String::new(),
        x: ax,
        y: Axis { min: n as f64 - 0.5, max: -0.5, ticks: yax.ticks, hidden: false },
        series: vec![bars, lim],
        kind: "xy",
        legend: true,
        ..Default::default()
    }
}

pub fn plot_half_normal(fit: &Fit) -> Plot {
    let sc = effects::screen(fit, 0.05);
    let mut e = sc.effects.clone();
    e.sort_by(|a, b| a.effect.abs().partial_cmp(&b.effect.abs()).unwrap_or(std::cmp::Ordering::Equal));
    let q = effects::half_normal_quantiles(e.len());
    let xs: Vec<f64> = e.iter().map(|x| x.effect.abs()).collect();
    let mut pts = Series::new("Effects", "points", xs.clone(), q.clone(), 0);
    let mut lab = Series::new("", "labels", vec![], vec![], 0);
    for (i, x) in e.iter().enumerate() {
        if x.effect.abs() > sc.me || i + 3 >= e.len() {
            lab.x.push(xs[i]);
            lab.y.push(q[i]);
            lab.labels.push(x.label.clone());
        }
    }
    pts.labels = e.iter().map(|x| x.label.clone()).collect();
    let slope_x = if sc.pse > 0.0 { sc.pse } else { 1.0 };
    let maxq = q.last().cloned().unwrap_or(1.0);
    let line = Series::new("", "line", vec![0.0, slope_x * maxq * 1.2], vec![0.0, maxq * 1.2], -2);
    let mut me = Series::new("ME", "vline", vec![sc.me], vec![], -3);
    me.dashed = true;
    let mut xr = xs.clone();
    xr.push(0.0);
    xr.push(sc.me);
    let mut ax = data_axis(&xr, 6);
    ax.min = 0.0;
    Plot {
        title: "Half-normal plot of effects".into(),
        x_label: "|Effect|".into(),
        y_label: "Half-normal quantile".into(),
        x: ax,
        y: data_axis(&q, 6),
        series: vec![line, me, pts, lab],
        kind: "xy",
        ..Default::default()
    }
}

/// Coded positions along a factor for curves.
fn factor_sweep(p: &Project, fi: usize, n: usize) -> Vec<Coded> {
    match &p.factors[fi].kind {
        FactorKind::Categorical { levels } => (0..levels.len()).map(Coded::Level).collect(),
        FactorKind::Mixture { .. } => (0..n).map(|i| Coded::Num(i as f64 / (n - 1) as f64)).collect(),
        FactorKind::Discrete { .. } => crate::design::level_grid(p, fi, 0),
        _ => (0..n).map(|i| Coded::Num(-1.0 + 2.0 * i as f64 / (n - 1) as f64)).collect(),
    }
}

fn is_discrete(p: &Project, fi: usize) -> bool {
    matches!(p.factors[fi].kind, FactorKind::Discrete { .. })
}

/// Sets a mixture component and rescales the others proportionally (Cox direction).
fn set_component(p: &Project, pt: &mut [Option<Coded>], fi: usize, z: f64) {
    let mix: Vec<usize> = (0..p.factors.len()).filter(|&i| p.factors[i].is_mixture()).collect();
    let others: f64 = mix.iter().filter(|&&i| i != fi).map(|&i| pt[i].map(|c| c.num()).unwrap_or(0.0)).sum();
    for &i in &mix {
        if i == fi {
            pt[i] = Some(Coded::Num(z));
        } else {
            let v = pt[i].map(|c| c.num()).unwrap_or(0.0);
            let nv = if others > 1e-12 { v / others * (1.0 - z) } else { (1.0 - z) / (mix.len() - 1).max(1) as f64 };
            pt[i] = Some(Coded::Num(nv));
        }
    }
}

fn actual_x(p: &Project, fi: usize, c: Coded) -> f64 {
    match p.decode(fi, c) {
        crate::project::Value::Num(x) if !p.factors[fi].is_categorical() => {
            if let FactorKind::Discrete { .. } = p.factors[fi].kind {
                let (ce, h) = p.factors[fi].center_half();
                ce + c.num() * h
            } else {
                x
            }
        }
        _ => c.num(),
    }
}

fn x_axis_for(p: &Project, fi: usize) -> Axis {
    let f = &p.factors[fi];
    match &f.kind {
        FactorKind::Categorical { levels } => category_axis(levels),
        _ => {
            let (lo, hi) = f.range();
            nice_axis(lo, hi, 5, false)
        }
    }
}

/// Profiler: one plot per model factor with prediction and 95% CI, other
/// factors at `current`. All plots share a y range.
pub fn profiler(p: &Project, fit: &Fit, rid: u64, current: &[Option<Coded>]) -> (Vec<Plot>, Json) {
    let factors = fit.layout().factors_used();
    let mut plots = Vec::new();
    let mut all_y = Vec::new();
    for &fi in &factors {
        let mut xs = Vec::new();
        let mut ys = Vec::new();
        let mut lo = Vec::new();
        let mut hi = Vec::new();
        for c in factor_sweep(p, fi, 41) {
            let mut pt = current.to_vec();
            if p.factors[fi].is_mixture() {
                set_component(p, &mut pt, fi, c.num());
            } else {
                pt[fi] = Some(c);
            }
            if let Some(pr) = fit.predict_point(&pt, 0.05) {
                xs.push(actual_x(p, fi, c));
                ys.push(pr.y);
                lo.push(pr.ci.0);
                hi.push(pr.ci.1);
            }
        }
        all_y.extend(ys.iter().cloned());
        all_y.extend(lo.iter().cloned().filter(|v| v.is_finite()));
        all_y.extend(hi.iter().cloned().filter(|v| v.is_finite()));
        let cat = p.factors[fi].is_categorical();
        let disc = is_discrete(p, fi);
        let mut band = Series::new("95% CI", if cat || disc { "errorbars" } else { "band" }, xs.clone(), lo, 0);
        band.y2 = hi;
        let line = Series::new("Prediction", if cat { "points" } else if disc { "linepoints" } else { "line" }, xs.clone(), ys, 0);
        let cur_x = current[fi].map(|c| actual_x(p, fi, c)).unwrap_or(f64::NAN);
        let mut v = Series::new("Current", "vline", vec![cur_x], vec![], -3);
        v.dashed = true;
        plots.push(Plot {
            title: String::new(),
            x_label: factor_label(p, fi),
            y_label: resp_label(p, rid),
            x: x_axis_for(p, fi),
            y: Axis::default(),
            series: vec![band, line, v],
            kind: "xy",
            note: p.factors[fi].id.to_string(),
            ..Default::default()
        });
    }
    let yax = data_axis(&all_y, 5);
    for pl in plots.iter_mut() {
        pl.y = yax.clone();
    }
    let pred = fit.predict_point(current, 0.05);
    let info = match pred {
        Some(pr) => json!({
            "y": num(pr.y, 6),
            "ci": [num(pr.ci.0, 5), num(pr.ci.1, 5)],
            "pi": [num(pr.pi.0, 5), num(pr.pi.1, 5)],
            "se": num(pr.se_fit, 4),
        }),
        None => json!(null),
    };
    (plots, info)
}

pub fn interaction(p: &Project, fit: &Fit, rid: u64, a: usize, b: usize, current: &[Option<Coded>]) -> Plot {
    let traces: Vec<Coded> = match &p.factors[b].kind {
        FactorKind::Categorical { levels } => (0..levels.len()).map(Coded::Level).collect(),
        FactorKind::Mixture { .. } => vec![Coded::Num(0.0), Coded::Num(1.0)],
        _ => vec![Coded::Num(-1.0), Coded::Num(1.0)],
    };
    let mut series = Vec::new();
    let mut all = Vec::new();
    for (k, &tb) in traces.iter().enumerate() {
        let mut xs = Vec::new();
        let mut ys = Vec::new();
        for c in factor_sweep(p, a, 25) {
            let mut pt = current.to_vec();
            pt[a] = Some(c);
            pt[b] = Some(tb);
            if let Some(pr) = fit.predict_point(&pt, 0.05) {
                xs.push(actual_x(p, a, c));
                ys.push(pr.y);
            }
        }
        all.extend(ys.iter().cloned());
        let name = format!("{} = {}", p.factors[b].name, display_value(p, b, Some(tb)));
        let kind = if p.factors[a].is_categorical() || is_discrete(p, a) { "linepoints" } else { "line" };
        series.push(Series::new(&name, kind, xs, ys, k as i32));
    }
    Plot {
        title: format!("Interaction {} × {}", p.factors[a].symbol, p.factors[b].symbol),
        x_label: factor_label(p, a),
        y_label: resp_label(p, rid),
        x: x_axis_for(p, a),
        y: data_axis(&all, 6),
        series,
        kind: "xy",
        legend: true,
        ..Default::default()
    }
}

fn grid_values(p: &Project, fit: &Fit, a: usize, b: usize, current: &[Option<Coded>], n: usize) -> (Vec<f64>, Vec<f64>, Vec<Vec<f64>>) {
    let sa = factor_sweep(p, a, n);
    let sb = factor_sweep(p, b, n);
    let xs: Vec<f64> = sa.iter().map(|&c| actual_x(p, a, c)).collect();
    let ys: Vec<f64> = sb.iter().map(|&c| actual_x(p, b, c)).collect();
    let both_mix = p.factors[a].is_mixture() && p.factors[b].is_mixture();
    let mut grid = vec![vec![f64::NAN; sa.len()]; sb.len()];
    for (j, &cb) in sb.iter().enumerate() {
        for (i, &ca) in sa.iter().enumerate() {
            let mut pt = current.to_vec();
            if both_mix {
                let (za, zb) = (ca.num(), cb.num());
                if za + zb > 1.0 + 1e-9 {
                    continue;
                }
                let mix: Vec<usize> = (0..p.factors.len()).filter(|&k| p.factors[k].is_mixture() && k != a && k != b).collect();
                let rest: f64 = mix.iter().map(|&k| current[k].map(|c| c.num()).unwrap_or(0.0)).sum();
                for &k in &mix {
                    let v = current[k].map(|c| c.num()).unwrap_or(0.0);
                    pt[k] = Some(Coded::Num(if rest > 1e-12 { v / rest * (1.0 - za - zb) } else { (1.0 - za - zb) / mix.len().max(1) as f64 }));
                }
                if mix.is_empty() && (za + zb - 1.0).abs() > 1e-9 {
                    continue;
                }
                pt[a] = Some(ca);
                pt[b] = Some(cb);
            } else {
                pt[a] = Some(ca);
                pt[b] = Some(cb);
            }
            if let Some(pr) = fit.predict_point(&pt, 0.05) {
                grid[j][i] = pr.y;
            }
        }
    }
    (xs, ys, grid)
}

pub fn contour(p: &Project, fit: &Fit, rid: u64, a: usize, b: usize, current: &[Option<Coded>]) -> Result<Plot, String> {
    if p.factors[a].is_categorical() || p.factors[b].is_categorical() {
        return Err("contour plots need two numeric factors".into());
    }
    let (xs, ys, grid) = grid_values(p, fit, a, b, current, 61);
    let flat: Vec<f64> = grid.iter().flatten().cloned().filter(|v| v.is_finite()).collect();
    let vmin = flat.iter().cloned().fold(f64::INFINITY, f64::min);
    let vmax = flat.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let levels = contour_levels(vmin, vmax, 9);
    let (mut series, labels) = contours(&xs, &ys, &grid, &levels);
    series.push(labels);
    // Design points in the plane (other factors ignored).
    let (ida, idb) = (p.factors[a].id, p.factors[b].id);
    let mut px = Vec::new();
    let mut py = Vec::new();
    let mut ids = Vec::new();
    for r in &p.runs {
        if r.status == RunStatus::Excluded {
            continue;
        }
        if let (Some(va), Some(vb)) = (r.settings.get(&ida).and_then(|v| v.as_num()), r.settings.get(&idb).and_then(|v| v.as_num())) {
            px.push(va);
            py.push(vb);
            ids.push(r.run_order as i64);
        }
    }
    let mut pts = Series::new("Runs", "points", px, py, -1);
    pts.ids = ids;
    series.push(pts);
    let mut values = Vec::new();
    for row in &grid {
        values.extend(row.iter().cloned());
    }
    let (xlo, xhi) = p.factors[a].range();
    let (ylo, yhi) = p.factors[b].range();
    Ok(Plot {
        title: format!("{} contours", p.responses[p.response_index(rid).unwrap()].name),
        x_label: factor_label(p, a),
        y_label: factor_label(p, b),
        x: nice_axis(xlo, xhi, 6, false),
        y: nice_axis(ylo, yhi, 6, false),
        series,
        heatmap: Some(Heatmap {
            nx: xs.len(),
            ny: ys.len(),
            x0: xs[0],
            x1: xs[xs.len() - 1],
            y0: ys[0],
            y1: ys[ys.len() - 1],
            values,
            vmin,
            vmax,
            diverging: false,
            ..Default::default()
        }),
        kind: "xy",
        value_range: Some((vmin, vmax)),
        ..Default::default()
    })
}

pub fn surface(p: &Project, fit: &Fit, rid: u64, a: usize, b: usize, current: &[Option<Coded>], az: f64, el: f64) -> Result<Plot, String> {
    if p.factors[a].is_categorical() || p.factors[b].is_categorical() {
        return Err("surface plots need two numeric factors".into());
    }
    let (xs, ys, grid) = grid_values(p, fit, a, b, current, 31);
    let rname = p.responses[p.response_index(rid).unwrap()].name.clone();
    let mut pl = surface_3d(&xs, &ys, &grid, az, el, (&p.factors[a].name, &p.factors[b].name, &rname));
    pl.title = format!("{rname} surface");
    Ok(pl)
}

/// Raw data: response against one factor.
pub fn data_scatter(p: &Project, rid: u64, fi: usize) -> Plot {
    let fid = p.factors[fi].id;
    let mut xs = Vec::new();
    let mut ys = Vec::new();
    let mut ids = Vec::new();
    for r in &p.runs {
        if r.status == RunStatus::Excluded {
            continue;
        }
        if let (Some(v), Some(y)) = (r.settings.get(&fid), r.responses.get(&rid)) {
            if let Some(c) = p.code(fi, v) {
                xs.push(actual_x(p, fi, c));
                ys.push(*y);
                ids.push(r.run_order as i64);
            }
        }
    }
    let mut s = Series::new("Runs", "points", xs.clone(), ys.clone(), 0);
    s.ids = ids;
    let mut series = vec![s];
    // Least-squares line for numeric factors.
    if !p.factors[fi].is_categorical() && xs.len() >= 2 {
        let mx = stats::mean(&xs);
        let my = stats::mean(&ys);
        let sxx: f64 = xs.iter().map(|x| (x - mx).powi(2)).sum();
        if sxx > 0.0 {
            let b = xs.iter().zip(&ys).map(|(x, y)| (x - mx) * (y - my)).sum::<f64>() / sxx;
            let (lo, hi) = p.factors[fi].range();
            let lo = lo.min(xs.iter().cloned().fold(f64::INFINITY, f64::min));
            let hi = hi.max(xs.iter().cloned().fold(f64::NEG_INFINITY, f64::max));
            let mut l = Series::new(&format!("r = {}", num(stats::correlation(&xs, &ys), 3)), "line", vec![lo, hi], vec![my + b * (lo - mx), my + b * (hi - mx)], -2);
            l.dashed = true;
            series.push(l);
        }
    }
    let x = if p.factors[fi].is_categorical() {
        x_axis_for(p, fi)
    } else {
        let (lo, hi) = p.factors[fi].range();
        let mut v = xs.clone();
        v.push(lo);
        v.push(hi);
        data_axis(&v, 6)
    };
    Plot {
        title: format!("{} vs {}", p.responses[p.response_index(rid).unwrap()].name, p.factors[fi].name),
        x_label: factor_label(p, fi),
        y_label: resp_label(p, rid),
        x,
        y: data_axis(&ys, 6),
        series,
        kind: "xy",
        legend: true,
        ..Default::default()
    }
}

/// Signal-to-noise (Taguchi) analysis grouped by controlled-factor settings.
pub fn robustness(p: &Project, rid: u64, sn: &str) -> Result<Json, String> {
    let control: Vec<usize> = (0..p.factors.len()).filter(|&i| p.factors[i].role == Role::Controlled).collect();
    if control.is_empty() {
        return Err("no controlled factors".into());
    }
    let mut groups: Vec<(Vec<String>, Vec<f64>)> = Vec::new();
    for r in &p.runs {
        if r.status == RunStatus::Excluded {
            continue;
        }
        let Some(&y) = r.responses.get(&rid) else { continue };
        let key: Option<Vec<String>> = control
            .iter()
            .map(|&f| r.settings.get(&p.factors[f].id).map(|v| match v {
                crate::project::Value::Num(x) => num(*x, 8),
                crate::project::Value::Text(t) => t.clone(),
            }))
            .collect();
        let Some(key) = key else { continue };
        match groups.iter_mut().find(|g| g.0 == key) {
            Some(g) => g.1.push(y),
            None => groups.push((key, vec![y])),
        }
    }
    let sn_of = |v: &[f64]| -> f64 {
        match sn {
            "larger" => -10.0 * (v.iter().map(|y| 1.0 / (y * y)).sum::<f64>() / v.len() as f64).log10(),
            "smaller" => -10.0 * (v.iter().map(|y| y * y).sum::<f64>() / v.len() as f64).log10(),
            _ => {
                let m = stats::mean(v);
                let s2 = stats::variance(v);
                10.0 * (m * m / s2).log10()
            }
        }
    };
    let usable: Vec<&(Vec<String>, Vec<f64>)> = groups.iter().filter(|g| g.1.len() >= if sn == "nominal" { 2 } else { 1 }).collect();
    if usable.len() < 2 {
        return Err("signal-to-noise analysis needs repeated observations at the same controlled settings (crossed noise conditions or replicates)".into());
    }
    let mut cols: Vec<&str> = control.iter().map(|&f| p.factors[f].symbol.as_str()).collect();
    cols.extend(["n", "Mean", "Std. dev.", "S/N"]);
    let mut numeric = vec![false; control.len()];
    numeric.extend([true, true, true, true]);
    let mut table = Table::new(&cols, &numeric);
    let mut sns = Vec::new();
    for g in &usable {
        let s = sn_of(&g.1);
        sns.push(s);
        let mut row = g.0.clone();
        row.extend([g.1.len().to_string(), num(stats::mean(&g.1), 5), num(stats::std_dev(&g.1), 4), format::fixed(s, 3)]);
        table.push(row, "", "");
    }
    // Main effects of S/N and mean per factor level.
    let mut plots = Vec::new();
    let mut best = Vec::new();
    for (k, &f) in control.iter().enumerate() {
        let mut levels: Vec<String> = usable.iter().map(|g| g.0[k].clone()).collect();
        levels.sort_by(|a, b| match (format::parse_num(a), format::parse_num(b)) {
            (Some(x), Some(y)) => x.partial_cmp(&y).unwrap(),
            _ => a.cmp(b),
        });
        levels.dedup();
        if levels.len() < 2 || levels.len() > 9 {
            continue;
        }
        let mut msn = Vec::new();
        let mut mmean = Vec::new();
        for l in &levels {
            let idx: Vec<usize> = (0..usable.len()).filter(|&i| usable[i].0[k] == *l).collect();
            msn.push(idx.iter().map(|&i| sns[i]).sum::<f64>() / idx.len() as f64);
            mmean.push(idx.iter().map(|&i| stats::mean(&usable[i].1)).sum::<f64>() / idx.len() as f64);
        }
        let bi = (0..levels.len()).max_by(|&a, &b| msn[a].partial_cmp(&msn[b]).unwrap()).unwrap();
        best.push(format!("{} = {}", p.factors[f].name, levels[bi]));
        let xs: Vec<f64> = (0..levels.len()).map(|i| i as f64).collect();
        plots.push(Plot {
            title: String::new(),
            x_label: factor_label(p, f),
            y_label: "Mean S/N (dB)".into(),
            x: category_axis(&levels),
            y: data_axis(&msn, 5),
            series: vec![Series::new("S/N", "linepoints", xs.clone(), msn, 0)],
            kind: "xy",
            ..Default::default()
        });
        plots.push(Plot {
            title: String::new(),
            x_label: factor_label(p, f),
            y_label: "Mean response".into(),
            x: category_axis(&levels),
            y: data_axis(&mmean, 5),
            series: vec![Series::new("Mean", "linepoints", xs, mmean, 1)],
            kind: "xy",
            ..Default::default()
        });
    }
    // Shared y scale across the S/N plots.
    let all_sn: Vec<f64> = plots.iter().step_by(2).flat_map(|pl| pl.series[0].y.clone()).collect();
    let ax = data_axis(&all_sn, 5);
    for pl in plots.iter_mut().step_by(2) {
        pl.y = ax.clone();
    }
    Ok(json!({
        "table": table,
        "plots": plots,
        "best": best,
    }))
}

/// Terms in the model that a contour or interaction plot can use.
pub fn model_factor_ids(p: &Project, terms: &[Term]) -> Vec<u64> {
    let mut v: Vec<usize> = terms.iter().flat_map(|t| t.factors().collect::<Vec<_>>()).collect();
    v.sort_unstable();
    v.dedup();
    v.into_iter().map(|f| p.factors[f].id).collect()
}
