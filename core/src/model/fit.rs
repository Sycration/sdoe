//! Least-squares model fitting, ANOVA, diagnostics and prediction.

use super::terms::{ModelLayout, Term};
use crate::linalg::{LsqDecomp, Mat, dot, inverse_spd};
use crate::project::{Coded, Project, RunStatus, Transform};
use crate::stats;
use std::collections::HashMap;

pub const ALIAS_TOL: f64 = 1e-8;

/// Data assembled for a fit.
#[derive(Clone, Debug)]
pub struct FitInput {
    pub response_id: u64,
    pub layout: ModelLayout,
    pub x: Mat,
    /// Response on the transformed scale.
    pub y: Vec<f64>,
    /// Response on the original scale.
    pub y_raw: Vec<f64>,
    /// Project run indices of the rows.
    pub rows: Vec<usize>,
    pub transform: Transform,
    pub skipped_missing_factor: usize,
    pub skipped_missing_response: usize,
    pub skipped_transform: usize,
    pub excluded: usize,
}

pub fn gather(p: &Project, response_id: u64, terms: &[Term], transform: Transform) -> FitInput {
    let layout = ModelLayout::new(p, terms);
    let mut x = Mat::zeros(0, layout.ncols());
    let mut y = Vec::new();
    let mut y_raw = Vec::new();
    let mut rows = Vec::new();
    let (mut smf, mut smr, mut st, mut ex) = (0, 0, 0, 0);
    for (ri, run) in p.runs.iter().enumerate() {
        if run.status == RunStatus::Excluded {
            ex += 1;
            continue;
        }
        let Some(&yv) = run.responses.get(&response_id) else {
            smr += 1;
            continue;
        };
        if !yv.is_finite() {
            smr += 1;
            continue;
        }
        let point = p.coded_run(run);
        let Some(row) = layout.row(&point) else {
            smf += 1;
            continue;
        };
        let yt = transform.apply(yv);
        if !yt.is_finite() {
            st += 1;
            continue;
        }
        x.push_row(&row);
        y.push(yt);
        y_raw.push(yv);
        rows.push(ri);
    }
    if x.rows == 0 {
        x.cols = layout.ncols();
    }
    FitInput {
        response_id,
        layout,
        x,
        y,
        y_raw,
        rows,
        transform,
        skipped_missing_factor: smf,
        skipped_missing_response: smr,
        skipped_transform: st,
        excluded: ex,
    }
}

#[derive(Clone, Debug)]
pub struct AnovaRow {
    pub source: String,
    pub ss: f64,
    pub df: f64,
    pub ms: f64,
    pub f: f64,
    pub p: f64,
    /// Term index for term rows.
    pub term: Option<usize>,
}

#[derive(Clone, Debug)]
pub struct Fit {
    pub input: FitInput,
    pub decomp: LsqDecomp,
    pub n: usize,
    pub rank: usize,
    /// Coefficient per layout column (NaN for aliased columns).
    pub coef: Vec<f64>,
    pub aliased: Vec<bool>,
    pub se: Vec<f64>,
    pub t: Vec<f64>,
    pub p: Vec<f64>,
    pub vif: Vec<f64>,
    /// `(X'X)^-1` over the kept columns.
    pub xtx_inv: Mat,
    pub fitted: Vec<f64>,
    pub resid: Vec<f64>,
    pub leverage: Vec<f64>,
    pub sse: f64,
    pub sst: f64,
    pub df_resid: f64,
    pub mse: f64,
    pub r2: f64,
    pub adj_r2: f64,
    pub pred_r2: f64,
    pub press: f64,
    pub mean_y: f64,
    pub cv: f64,
    pub adeq_precision: f64,
    pub aicc: f64,
    pub bic: f64,
    pub anova: Vec<AnovaRow>,
    pub model_row: AnovaRow,
    pub resid_row: AnovaRow,
    pub lack_of_fit: Option<(AnovaRow, AnovaRow)>,
}

pub struct Prediction {
    pub y: f64,
    pub y_t: f64,
    pub se_fit: f64,
    pub ci: (f64, f64),
    pub pi: (f64, f64),
}

fn sse_of(x: &Mat, y: &[f64]) -> (f64, usize) {
    if x.cols == 0 {
        return (y.iter().map(|v| v * v).sum(), 0);
    }
    let d = LsqDecomp::new(x, ALIAS_TOL);
    let b = d.solve(y);
    let xs = x.select_cols(&d.kept);
    let f = xs.mul_vec(&b);
    (y.iter().zip(&f).map(|(a, b)| (a - b).powi(2)).sum(), d.rank())
}

pub fn fit(input: FitInput) -> Result<Fit, String> {
    let n = input.y.len();
    let pcols = input.layout.ncols();
    if n == 0 {
        return Err("no runs with complete data for this response".into());
    }
    if pcols == 0 {
        return Err("the model has no terms".into());
    }
    let x = &input.x;
    let y = &input.y;
    let decomp = LsqDecomp::new(x, ALIAS_TOL);
    let rank = decomp.rank();
    let b_kept = decomp.solve(y);
    let mut coef = vec![f64::NAN; pcols];
    let mut aliased = vec![true; pcols];
    for (k, &c) in decomp.kept.iter().enumerate() {
        coef[c] = b_kept[k];
        aliased[c] = false;
    }
    let xs = x.select_cols(&decomp.kept);
    let fitted = xs.mul_vec(&b_kept);
    let resid: Vec<f64> = y.iter().zip(&fitted).map(|(a, b)| a - b).collect();
    let sse: f64 = resid.iter().map(|e| e * e).sum();
    let mean_y = stats::mean(y);
    let intercept = input.layout.intercept;
    let mixture_like = !intercept;
    let sst: f64 = y.iter().map(|v| (v - mean_y).powi(2)).sum();
    let df_resid = n as f64 - rank as f64;
    let mse = if df_resid > 0.0 { sse / df_resid } else { f64::NAN };
    let xtx_inv = decomp.xtx_inverse();
    let leverage = decomp.leverage();
    let mut se = vec![f64::NAN; pcols];
    let mut t = vec![f64::NAN; pcols];
    let mut p = vec![f64::NAN; pcols];
    for (k, &c) in decomp.kept.iter().enumerate() {
        se[c] = (mse * xtx_inv[(k, k)]).sqrt();
        t[c] = coef[c] / se[c];
        p[c] = stats::t_two_sided_p(t[c], df_resid);
    }

    // Variance inflation factors from the centred predictor columns.
    let mut vif = vec![f64::NAN; pcols];
    {
        let pred_cols: Vec<usize> =
            decomp.kept.iter().cloned().filter(|&c| input.layout.columns[c].term.is_some()).collect();
        if !pred_cols.is_empty() && n > 1 {
            let mut xc = x.select_cols(&pred_cols);
            if intercept {
                for j in 0..xc.cols {
                    let m = stats::mean(&xc.col(j));
                    for i in 0..xc.rows {
                        xc[(i, j)] -= m;
                    }
                }
            }
            let g = xc.gram();
            if let Some(gi) = inverse_spd(&g) {
                for (j, &c) in pred_cols.iter().enumerate() {
                    vif[c] = gi[(j, j)] * g[(j, j)];
                }
            }
        }
    }

    // Model and per-term ANOVA (partial sums of squares).
    let model_df = rank as f64 - 1.0;
    let ss_model = (sst - sse).max(0.0);
    let ms_model = if model_df > 0.0 { ss_model / model_df } else { f64::NAN };
    let f_model = ms_model / mse;
    let model_row = AnovaRow {
        source: "Model".into(),
        ss: ss_model,
        df: model_df,
        ms: ms_model,
        f: f_model,
        p: stats::f_sf(f_model, model_df, df_resid),
        term: None,
    };
    let mut anova = Vec::new();
    let layout = &input.layout;
    let mix_linear: Vec<usize> = if mixture_like { layout.mixture_linear.clone() } else { vec![] };
    if !mix_linear.is_empty() {
        // Linear blending group: compared against an intercept-only replacement.
        let mut cols: Vec<usize> = decomp
            .kept
            .iter()
            .cloned()
            .filter(|&c| !mix_linear.contains(&layout.columns[c].term.unwrap_or(usize::MAX)))
            .collect();
        let mut xr = x.select_cols(&cols);
        let ones = vec![1.0; n];
        let mut m2 = Mat::zeros(n, xr.cols + 1);
        for i in 0..n {
            m2[(i, 0)] = ones[i];
            for j in 0..xr.cols {
                m2[(i, j + 1)] = xr[(i, j)];
            }
        }
        xr = m2;
        cols.clear();
        let (sse_r, rank_r) = sse_of(&xr, y);
        let df = rank as f64 - rank_r as f64;
        if df > 0.0 {
            let ss = (sse_r - sse).max(0.0);
            let ms = ss / df;
            let f = ms / mse;
            anova.push(AnovaRow {
                source: "Linear mixture".into(),
                ss,
                df,
                ms,
                f,
                p: stats::f_sf(f, df, df_resid),
                term: None,
            });
        }
    }
    for ti in 0..layout.terms.len() {
        if mix_linear.contains(&ti) {
            continue;
        }
        let tcols = layout.term_columns(ti);
        let reduced: Vec<usize> = decomp.kept.iter().cloned().filter(|c| !tcols.contains(c)).collect();
        let xr = x.select_cols(&reduced);
        let (sse_r, rank_r) = sse_of(&xr, y);
        let df = rank as f64 - rank_r as f64;
        let ss = if df > 0.0 { (sse_r - sse).max(0.0) } else { 0.0 };
        let ms = if df > 0.0 { ss / df } else { f64::NAN };
        let f = ms / mse;
        anova.push(AnovaRow {
            source: crate::model::terms::term_label_from_layout(layout, ti),
            ss,
            df,
            ms,
            f,
            p: if df > 0.0 { stats::f_sf(f, df, df_resid) } else { f64::NAN },
            term: Some(ti),
        });
    }
    let resid_row = AnovaRow {
        source: "Residual".into(),
        ss: sse,
        df: df_resid,
        ms: mse,
        f: f64::NAN,
        p: f64::NAN,
        term: None,
    };

    // Lack of fit from replicated design points.
    let mut groups: HashMap<Vec<i64>, Vec<usize>> = HashMap::new();
    for i in 0..n {
        let key: Vec<i64> = x.row(i).iter().map(|v| (v * 1e7).round() as i64).collect();
        groups.entry(key).or_default().push(i);
    }
    let mut ss_pe = 0.0;
    for g in groups.values() {
        if g.len() > 1 {
            let m = g.iter().map(|&i| y[i]).sum::<f64>() / g.len() as f64;
            ss_pe += g.iter().map(|&i| (y[i] - m).powi(2)).sum::<f64>();
        }
    }
    let df_pe = n as f64 - groups.len() as f64;
    let df_lof = df_resid - df_pe;
    let lack_of_fit = if df_pe > 0.0 && df_lof > 0.0 {
        let ss_lof = (sse - ss_pe).max(0.0);
        let ms_lof = ss_lof / df_lof;
        let ms_pe = ss_pe / df_pe;
        let f = ms_lof / ms_pe;
        Some((
            AnovaRow {
                source: "Lack of fit".into(),
                ss: ss_lof,
                df: df_lof,
                ms: ms_lof,
                f,
                p: stats::f_sf(f, df_lof, df_pe),
                term: None,
            },
            AnovaRow { source: "Pure error".into(), ss: ss_pe, df: df_pe, ms: ms_pe, f: f64::NAN, p: f64::NAN, term: None },
        ))
    } else {
        None
    };

    let r2 = if sst > 0.0 { 1.0 - sse / sst } else { f64::NAN };
    let adj_r2 = if sst > 0.0 && df_resid > 0.0 {
        1.0 - (sse / df_resid) / (sst / (n as f64 - 1.0))
    } else {
        f64::NAN
    };
    let mut press = 0.0;
    for i in 0..n {
        let d = 1.0 - leverage[i];
        if d < 1e-10 {
            press = f64::NAN;
            break;
        }
        press += (resid[i] / d).powi(2);
    }
    let pred_r2 = if sst > 0.0 { 1.0 - press / sst } else { f64::NAN };
    let rmse = mse.sqrt();
    let cv = 100.0 * rmse / mean_y.abs();
    let fmin = fitted.iter().cloned().fold(f64::INFINITY, f64::min);
    let fmax = fitted.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let adeq_precision = (fmax - fmin) / (rank as f64 * mse / n as f64).sqrt();
    let k = rank as f64 + 1.0;
    let nf = n as f64;
    let ll = nf * (sse.max(1e-300) / nf).ln();
    let aicc = if nf - k - 1.0 > 0.0 { ll + 2.0 * k + 2.0 * k * (k + 1.0) / (nf - k - 1.0) } else { f64::NAN };
    let bic = ll + k * nf.ln();

    Ok(Fit {
        n,
        rank,
        coef,
        aliased,
        se,
        t,
        p,
        vif,
        xtx_inv,
        fitted,
        resid,
        leverage,
        sse,
        sst,
        df_resid,
        mse,
        r2,
        adj_r2,
        pred_r2,
        press,
        mean_y,
        cv,
        adeq_precision,
        aicc,
        bic,
        anova,
        model_row,
        resid_row,
        lack_of_fit,
        decomp,
        input,
    })
}

impl Fit {
    pub fn layout(&self) -> &ModelLayout {
        &self.input.layout
    }

    pub fn rmse(&self) -> f64 {
        self.mse.sqrt()
    }

    pub fn kept_coefs(&self) -> Vec<f64> {
        self.decomp.kept.iter().map(|&c| self.coef[c]).collect()
    }

    /// Prediction on the transformed scale with its standard error.
    pub fn predict_row(&self, row: &[f64]) -> (f64, f64) {
        let xk: Vec<f64> = self.decomp.kept.iter().map(|&c| row[c]).collect();
        let yhat = dot(&xk, &self.kept_coefs());
        let v = self.xtx_inv.quad_form(&xk);
        (yhat, (v.max(0.0) * self.mse).sqrt())
    }

    pub fn predict_point(&self, point: &[Option<Coded>], alpha: f64) -> Option<Prediction> {
        let row = self.layout().row(point)?;
        let (yt, se) = self.predict_row(&row);
        let tq = if self.df_resid > 0.0 { stats::t_quantile(1.0 - alpha / 2.0, self.df_resid) } else { f64::NAN };
        let s = self.rmse();
        let tr = self.input.transform;
        let back = |a: f64, b: f64| {
            let (x, y) = (tr.inverse(a), tr.inverse(b));
            if x <= y { (x, y) } else { (y, x) }
        };
        let ci = back(yt - tq * se, yt + tq * se);
        let spi = (se * se + s * s).sqrt();
        let pi = back(yt - tq * spi, yt + tq * spi);
        Some(Prediction { y: tr.inverse(yt), y_t: yt, se_fit: se, ci, pi })
    }

    /// Internally studentised residuals.
    pub fn studentized(&self) -> Vec<f64> {
        let s = self.rmse();
        self.resid
            .iter()
            .zip(&self.leverage)
            .map(|(e, h)| if *h < 1.0 - 1e-10 { e / (s * (1.0 - h).sqrt()) } else { f64::NAN })
            .collect()
    }

    /// Externally studentised residuals.
    pub fn ext_studentized(&self) -> Vec<f64> {
        let df = self.df_resid;
        self.studentized()
            .into_iter()
            .map(|r| {
                let d = df - r * r;
                if df > 1.0 && d > 0.0 { r * ((df - 1.0) / d).sqrt() } else { f64::NAN }
            })
            .collect()
    }

    pub fn cooks_distance(&self) -> Vec<f64> {
        let p = self.rank as f64;
        self.studentized()
            .into_iter()
            .zip(&self.leverage)
            .map(|(r, h)| r * r * h / (p * (1.0 - h)))
            .collect()
    }

    /// Leave-one-out predictions on the original scale.
    pub fn loo_predictions(&self) -> Vec<f64> {
        let tr = self.input.transform;
        self.input
            .y
            .iter()
            .zip(&self.resid)
            .zip(&self.leverage)
            .map(|((y, e), h)| if *h < 1.0 - 1e-10 { tr.inverse(y - e / (1.0 - h)) } else { f64::NAN })
            .collect()
    }

    /// PRESS-based predicted R^2 on the original (untransformed) scale.
    pub fn pred_r2_original(&self) -> f64 {
        let loo = self.loo_predictions();
        let yr = &self.input.y_raw;
        let m = stats::mean(yr);
        let sst: f64 = yr.iter().map(|v| (v - m).powi(2)).sum();
        let press: f64 = yr.iter().zip(&loo).map(|(a, b)| (a - b).powi(2)).sum();
        if sst > 0.0 && press.is_finite() { 1.0 - press / sst } else { f64::NAN }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::terms::{Preset, preset_terms};
    use crate::project::*;

    pub fn two_factor_project() -> Project {
        let mut p = Project::default();
        for (s, n) in [("A", "Temp"), ("B", "Time")] {
            let id = p.new_id();
            p.factors.push(Factor {
                id,
                symbol: s.into(),
                name: n.into(),
                units: String::new(),
                kind: FactorKind::Continuous { low: -1.0, high: 1.0 },
                role: Role::Controlled,
                condition: None,
            });
        }
        let rid = p.new_id();
        p.responses.push(Response {
            id: rid,
            name: "Y".into(),
            units: String::new(),
            goal: Goal::None,
            lower: None,
            target: None,
            upper: None,
            weight: 1.0,
            importance: 3.0,
        });
        let pts = [
            (-1.0, -1.0, 28.0),
            (1.0, -1.0, 36.0),
            (-1.0, 1.0, 18.0),
            (1.0, 1.0, 31.0),
            (-1.0, -1.0, 25.0),
            (1.0, -1.0, 32.0),
            (-1.0, 1.0, 19.0),
            (1.0, 1.0, 30.0),
            (-1.0, -1.0, 27.0),
            (1.0, -1.0, 32.0),
            (-1.0, 1.0, 23.0),
            (1.0, 1.0, 29.0),
        ];
        for (i, (a, b, y)) in pts.iter().enumerate() {
            let id = p.new_id();
            let mut run = Run {
                id,
                std_order: i as u32 + 1,
                run_order: i as u32 + 1,
                point_type: String::new(),
                status: RunStatus::Done,
                settings: Default::default(),
                responses: Default::default(),
                note: String::new(),
            };
            run.settings.insert(p.factors[0].id, Value::Num(*a));
            run.settings.insert(p.factors[1].id, Value::Num(*b));
            run.responses.insert(rid, *y);
            p.runs.push(run);
        }
        p
    }

    #[test]
    fn montgomery_two_squared() {
        // Montgomery, Design and Analysis of Experiments, Example 6.1.
        let p = two_factor_project();
        let terms = preset_terms(&p, Preset::Interactions, false);
        let inp = gather(&p, p.responses[0].id, &terms, Transform::None);
        let f = fit(inp).unwrap();
        assert!((f.coef[0] - 27.5).abs() < 1e-9);
        assert!((f.coef[1] - 25.0 / 6.0).abs() < 1e-9);
        assert!((f.coef[2] - (-15.0 / 6.0)).abs() < 1e-9);
        assert!((f.coef[3] - 5.0 / 6.0).abs() < 1e-9);
        assert!((f.anova[0].ss - 208.333333333).abs() < 1e-6);
        assert!((f.anova[1].ss - 75.0).abs() < 1e-6);
        assert!((f.anova[2].ss - 8.333333333).abs() < 1e-6);
        assert!((f.sse - 31.333333333).abs() < 1e-6);
        assert!((f.anova[0].f - 53.1914893617).abs() < 1e-6);
    }
}
