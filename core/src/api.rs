//! Engine: project data, undo history and the last design preview, with
//! JSON command dispatch. Every frontend (the Qt application and the C
//! ABI) goes through `Engine::call`.

use crate::analysis::{self, center_point, display_value, model_for, parse_value, point_from_json, set_model};
use crate::design::{self, Generated};
use crate::format::{self, num};
use crate::io;
use crate::model::equation::{Syntax, equations};
use crate::model::select;
use crate::model::terms::{Preset, Term, format_formula, parse_formula, preset_terms, term_label};
use crate::optimize;
use crate::plot::{Plot, Series, data_axis, nice_axis};
use crate::project::*;
use crate::results;
use crate::rng::Rng;
use crate::sheet;
use crate::table::Table;
use serde_json::{Map, Value as Json, json};

const UNDO_LIMIT: usize = 200;

pub struct Engine {
    pub project: Project,
    undo: Vec<Project>,
    redo: Vec<Project>,
    dirty: bool,
    revision: u64,
    epoch: u64,
    preview: Option<Generated>,
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}

type R = Result<Json, String>;

fn arg_str<'a>(a: &'a Map<String, Json>, k: &str) -> Result<&'a str, String> {
    a.get(k).and_then(|v| v.as_str()).ok_or_else(|| format!("missing '{k}'"))
}

fn arg_u64(a: &Map<String, Json>, k: &str) -> Result<u64, String> {
    match a.get(k) {
        Some(Json::Number(n)) => n.as_u64().or_else(|| n.as_f64().map(|f| f as u64)).ok_or_else(|| format!("bad '{k}'")),
        Some(Json::String(s)) => s.parse().map_err(|_| format!("bad '{k}'")),
        _ => Err(format!("missing '{k}'")),
    }
}

fn arg_usize(a: &Map<String, Json>, k: &str) -> Result<usize, String> {
    arg_u64(a, k).map(|v| v as usize)
}

fn opt_str<'a>(a: &'a Map<String, Json>, k: &str, d: &'a str) -> &'a str {
    a.get(k).and_then(|v| v.as_str()).unwrap_or(d)
}

fn opt_bool(a: &Map<String, Json>, k: &str, d: bool) -> bool {
    a.get(k).and_then(|v| v.as_bool()).unwrap_or(d)
}

fn opt_f64(a: &Map<String, Json>, k: &str, d: f64) -> f64 {
    match a.get(k) {
        Some(Json::Number(n)) => n.as_f64().unwrap_or(d),
        Some(Json::String(s)) => format::parse_num(s).unwrap_or(d),
        _ => d,
    }
}

impl Engine {
    pub fn new() -> Engine {
        Engine { project: Project::default(), undo: Vec::new(), redo: Vec::new(), dirty: false, revision: 0, epoch: 0, preview: None }
    }

    fn changed(&mut self) {
        self.dirty = true;
        self.revision += 1;
    }

    /// Runs a mutation with undo support; the snapshot is dropped on error.
    fn mutate<F: FnOnce(&mut Project) -> R>(&mut self, f: F) -> R {
        let snapshot = self.project.clone();
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(&mut self.project)));
        let outcome = match outcome {
            Ok(r) => r,
            Err(payload) => {
                // a panic leaves the project as it was before the command
                self.project = snapshot;
                std::panic::resume_unwind(payload);
            }
        };
        match outcome {
            Ok(v) => {
                if self.project != snapshot {
                    self.undo.push(snapshot);
                    if self.undo.len() > UNDO_LIMIT {
                        self.undo.remove(0);
                    }
                    self.redo.clear();
                    self.preview = None;
                    self.changed();
                }
                Ok(v)
            }
            Err(e) => {
                self.project = snapshot;
                Err(e)
            }
        }
    }

    /// JSON entry point: returns `{"ok":true,"result":...}` or `{"ok":false,"error":...}`.
    pub fn call(&mut self, method: &str, params: &str) -> String {
        let args: Map<String, Json> = if params.trim().is_empty() {
            Map::new()
        } else {
            match serde_json::from_str::<Json>(params) {
                Ok(Json::Object(m)) => m,
                Ok(_) => return json!({"ok": false, "error": "parameters must be a JSON object"}).to_string(),
                Err(e) => return json!({"ok": false, "error": format!("invalid JSON: {e}")}).to_string(),
            }
        };
        let rev = self.revision;
        let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| self.dispatch(method, &args)));
        let out = match res {
            Ok(Ok(v)) => json!({"ok": true, "result": v, "changed": self.revision != rev, "revision": self.revision}),
            Ok(Err(e)) => json!({"ok": false, "error": e}),
            Err(payload) => {
                let msg = payload
                    .downcast_ref::<&str>()
                    .map(|s| s.to_string())
                    .or_else(|| payload.downcast_ref::<String>().cloned())
                    .unwrap_or_else(|| "unknown panic".into());
                json!({"ok": false, "error": format!("internal error: {msg}")})
            }
        };
        out.to_string()
    }

    pub fn dispatch(&mut self, method: &str, a: &Map<String, Json>) -> R {
        match method {
            "app.version" => Ok(json!(env!("CARGO_PKG_VERSION"))),
            "project.new" => {
                self.project = Project::default();
                self.undo.clear();
                self.redo.clear();
                self.preview = None;
                self.epoch += 1;
                self.changed();
                self.dirty = false;
                Ok(json!(null))
            }
            "project.open" => {
                let p = io::load_project(arg_str(a, "text")?)?;
                self.project = p;
                self.undo.clear();
                self.redo.clear();
                self.preview = None;
                self.epoch += 1;
                self.changed();
                self.dirty = false;
                Ok(json!(null))
            }
            "project.save" => {
                let text = io::save_project(&self.project)?;
                Ok(json!({ "text": text }))
            }
            "project.mark_saved" => {
                self.dirty = false;
                Ok(json!(null))
            }
            "project.state" => Ok(self.state()),
            "project.set" => self.mutate(|p| {
                if let Some(t) = a.get("title").and_then(|v| v.as_str()) {
                    p.title = t.to_string();
                }
                if let Some(t) = a.get("notes").and_then(|v| v.as_str()) {
                    p.notes = t.to_string();
                }
                if a.contains_key("mixture_total") {
                    let v = opt_f64(a, "mixture_total", 1.0);
                    if v <= 0.0 {
                        return Err("the mixture total must be positive".into());
                    }
                    p.mixture_total = v;
                }
                Ok(json!(null))
            }),
            "project.example" => {
                let p = crate::examples::build(opt_str(a, "name", "reactor"))?;
                self.project = p;
                self.undo.clear();
                self.redo.clear();
                self.preview = None;
                self.epoch += 1;
                self.changed();
                Ok(json!(null))
            }
            "edit.undo" => {
                let prev = self.undo.pop().ok_or("nothing to undo")?;
                self.redo.push(std::mem::replace(&mut self.project, prev));
                self.preview = None;
                self.changed();
                Ok(json!(null))
            }
            "edit.redo" => {
                let next = self.redo.pop().ok_or("nothing to redo")?;
                self.undo.push(std::mem::replace(&mut self.project, next));
                self.preview = None;
                self.changed();
                Ok(json!(null))
            }

            // Sheets -----------------------------------------------------
            "sheet.get" => Ok(serde_json::to_value(sheet::get(&self.project, arg_str(a, "sheet")?)?).unwrap()),
            "sheet.set" => {
                let (s, r, c) = (arg_str(a, "sheet")?.to_string(), arg_usize(a, "row")?, arg_usize(a, "col")?);
                let t = opt_str(a, "text", "").to_string();
                self.mutate(|p| sheet::set(p, &s, r, c, &t).map(|_| json!(null)))
            }
            "sheet.paste" => {
                let (s, r, c) = (arg_str(a, "sheet")?.to_string(), arg_usize(a, "row")?, arg_usize(a, "col")?);
                let t = opt_str(a, "text", "").to_string();
                self.mutate(|p| sheet::paste(p, &s, r, c, &t).map(|e| json!({ "errors": e })))
            }
            "sheet.clear" => {
                let s = arg_str(a, "sheet")?.to_string();
                let (t, l, b, r) = (arg_usize(a, "top")?, arg_usize(a, "left")?, arg_usize(a, "bottom")?, arg_usize(a, "right")?);
                self.mutate(|p| sheet::clear(p, &s, t, l, b, r).map(|_| json!(null)))
            }
            "sheet.copy" => {
                let t = sheet::copy(
                    &self.project,
                    arg_str(a, "sheet")?,
                    arg_usize(a, "top")?,
                    arg_usize(a, "left")?,
                    arg_usize(a, "bottom")?,
                    arg_usize(a, "right")?,
                    opt_bool(a, "header", false),
                )?;
                Ok(json!({ "text": t }))
            }
            "sheet.delete_rows" => {
                let s = arg_str(a, "sheet")?.to_string();
                let rows: Vec<usize> = a.get("rows").and_then(|v| v.as_array()).map(|v| v.iter().filter_map(|x| x.as_u64().map(|u| u as usize)).collect()).unwrap_or_default();
                self.mutate(|p| sheet::delete_rows(p, &s, &rows).map(|_| json!(null)))
            }
            "sheet.insert_row" => {
                let s = arg_str(a, "sheet")?.to_string();
                let r = arg_usize(a, "row")?;
                self.mutate(|p| sheet::insert_row(p, &s, r).map(|_| json!(null)))
            }
            "runs.randomize" => {
                let seed = a.get("seed").and_then(|v| v.as_u64()).unwrap_or_else(|| self.revision.wrapping_mul(2654435761) ^ 0x5eed);
                self.mutate(|p| {
                    randomize(p, seed);
                    Ok(json!(null))
                })
            }
            "runs.sort" => {
                let by = opt_str(a, "by", "run").to_string();
                self.mutate(|p| {
                    if by == "std" {
                        p.runs.sort_by_key(|r| r.std_order);
                    } else {
                        p.runs.sort_by_key(|r| r.run_order);
                    }
                    Ok(json!(null))
                })
            }
            "runs.set_status" => {
                let st = RunStatus::from_label(arg_str(a, "status")?).ok_or("bad status")?;
                let rows: Vec<usize> = a.get("rows").and_then(|v| v.as_array()).map(|v| v.iter().filter_map(|x| x.as_u64().map(|u| u as usize)).collect()).unwrap_or_default();
                self.mutate(|p| {
                    for r in rows {
                        if let Some(run) = p.runs.get_mut(r) {
                            run.status = st;
                        }
                    }
                    Ok(json!(null))
                })
            }
            "factors.list" => Ok(self.factor_list()),
            "responses.list" => Ok(json!(self
                .project
                .responses
                .iter()
                .map(|r| json!({
                    "id": r.id.to_string(),
                    "name": r.name,
                    "units": r.units,
                    "goal": r.goal.label(),
                    "count": self.project.runs.iter().filter(|x| x.responses.contains_key(&r.id)).count(),
                }))
                .collect::<Vec<_>>())),

            // Design ------------------------------------------------------
            "design.methods" => Ok(serde_json::to_value(design::methods(&self.project)).unwrap()),
            "design.preview" => {
                let method = arg_str(a, "method")?;
                let opts = a.get("options").and_then(|v| v.as_object()).cloned().unwrap_or_default();
                let g = design::generate(&self.project, method, &opts)?;
                let table = self.preview_table(&g);
                let model = format_formula(&self.project, &g.model);
                let out = json!({
                    "runs": g.runs.len(),
                    "blocks": g.blocks,
                    "notes": g.notes,
                    "table": table,
                    "model": model,
                    "augment": g.keeps_existing,
                });
                self.preview = Some(g);
                Ok(out)
            }
            "design.apply" => {
                let g = self.preview.clone().ok_or("preview a design first")?;
                let replace = opt_str(a, "mode", "replace") == "replace" && !g.keeps_existing;
                let rand = opt_bool(a, "randomize", true);
                let seed = a.get("seed").and_then(|v| v.as_u64()).unwrap_or(self.revision + 17);
                self.mutate(|p| {
                    apply_design(p, &g, replace, rand, seed);
                    Ok(json!({ "runs": p.runs.len() }))
                })
            }
            "design.next_run" => self.next_run(a),
            "design.add_run" => {
                let vals = a.get("values").cloned();
                let pt = opt_str(a, "type", "added").to_string();
                self.mutate(|p| {
                    let ri = sheet::new_run(p, &pt);
                    if let Some(Json::Object(m)) = vals {
                        for (k, v) in m {
                            let Ok(id) = k.parse::<u64>() else { continue };
                            let Some(fi) = p.factor_index(id) else { continue };
                            let Some(t) = v.as_str() else { continue };
                            if t.trim().is_empty() {
                                continue;
                            }
                            let val = parse_value(p, fi, t)?;
                            p.runs[ri].settings.insert(id, val);
                        }
                    }
                    Ok(json!({ "row": ri }))
                })
            }

            // Models ------------------------------------------------------
            "model.get" => {
                let rid = arg_u64(a, "response")?;
                let (terms, tr) = model_for(&self.project, rid);
                Ok(json!({
                    "formula": format_formula(&self.project, &terms),
                    "transform": tr.label(),
                    "terms": terms.iter().map(|t| term_label(&self.project, t)).collect::<Vec<_>>(),
                    "factors": results::model_factor_ids(&self.project, &terms).iter().map(|i| i.to_string()).collect::<Vec<_>>(),
                    "presets": Preset::all().iter().map(|(k, l)| json!([k, l])).collect::<Vec<_>>(),
                    "syntaxes": Syntax::all().iter().map(|(k, l)| json!([k, l])).collect::<Vec<_>>(),
                    "transforms": ["None", "Ln", "Log10", "Square root", "Inverse"],
                }))
            }
            "model.set" => {
                let rid = arg_u64(a, "response")?;
                let formula = a.get("formula").and_then(|v| v.as_str()).map(String::from);
                let tr = a.get("transform").and_then(|v| v.as_str()).map(String::from);
                let hier = opt_bool(a, "hierarchy", false);
                self.mutate(|p| {
                    p.response_index(rid).ok_or("unknown response")?;
                    let (mut terms, mut transform) = model_for(p, rid);
                    if let Some(f) = formula {
                        terms = parse_formula(p, &f)?;
                        if hier {
                            terms = crate::model::terms::complete_hierarchy(&terms);
                        }
                        if terms.is_empty() {
                            return Err("the model needs at least one term".into());
                        }
                    }
                    if let Some(t) = tr {
                        transform = Transform::parse(&t).ok_or("unknown transform")?;
                    }
                    set_model(p, rid, &terms, transform);
                    Ok(json!(null))
                })
            }
            "model.preset" => {
                let rid = arg_u64(a, "response")?;
                let preset = Preset::parse(arg_str(a, "preset")?).ok_or("unknown preset")?;
                let env = opt_bool(a, "env_interactions", false);
                self.mutate(|p| {
                    let (_, tr) = model_for(p, rid);
                    let terms = preset_terms(p, preset, env);
                    set_model(p, rid, &terms, tr);
                    Ok(json!(null))
                })
            }
            "model.remove_term" => {
                let rid = arg_u64(a, "response")?;
                let label = arg_str(a, "term")?.to_string();
                self.mutate(|p| {
                    let (mut terms, tr) = model_for(p, rid);
                    let before = terms.len();
                    let target = terms.iter().find(|t| term_label(p, t) == label).cloned().ok_or("term not in model")?;
                    if terms.iter().any(|t| t.contains(&target)) {
                        return Err(format!("{label} is part of a higher-order term in the model; remove that term first"));
                    }
                    terms.retain(|t| *t != target);
                    if terms.len() == before || terms.is_empty() {
                        return Err("cannot remove the last term".into());
                    }
                    set_model(p, rid, &terms, tr);
                    Ok(json!(null))
                })
            }
            "model.select" => {
                let rid = arg_u64(a, "response")?;
                let method = opt_str(a, "method", "aicc").to_string();
                let alpha = opt_f64(a, "alpha", 0.10);
                let pool_name = opt_str(a, "pool", "current").to_string();
                let env = opt_bool(a, "env_interactions", false);
                self.mutate(|p| {
                    let (cur, tr) = model_for(p, rid);
                    let pool: Vec<Term> = match Preset::parse(&pool_name) {
                        Some(pr) => {
                            let mut v = preset_terms(p, pr, env);
                            for t in &cur {
                                if !v.contains(t) {
                                    v.push(t.clone());
                                }
                            }
                            v
                        }
                        None => cur.clone(),
                    };
                    let (terms, log) = if method == "backward" {
                        select::backward_p(p, rid, &pool, tr, alpha)?
                    } else {
                        let start = preset_terms(p, Preset::Linear, false);
                        let start: Vec<Term> = start.into_iter().filter(|t| pool.contains(t)).collect();
                        select::stepwise_aicc(p, rid, &pool, tr, &start)?
                    };
                    if terms.is_empty() {
                        return Err("selection removed every term".into());
                    }
                    set_model(p, rid, &terms, tr);
                    let steps: Vec<String> = log
                        .iter()
                        .map(|s| {
                            if s.term.is_empty() {
                                format!("{} (criterion {})", s.action, num(s.criterion, 5))
                            } else {
                                format!("{} {} ({})", s.action, s.term, num(s.criterion, 5))
                            }
                        })
                        .collect();
                    Ok(json!({ "steps": steps, "formula": format_formula(p, &terms) }))
                })
            }
            "model.auto" => {
                let rid = arg_u64(a, "response")?;
                let env = opt_bool(a, "env_interactions", true);
                self.mutate(|p| {
                    let cands = select::auto_model(p, rid, env)?;
                    let best = cands[0].clone();
                    set_model(p, rid, &best.terms, best.transform);
                    let mut t = Table::new(&["Transform", "Terms", "Predicted R² (original scale)", "Adjusted R²", "AICc"], &[false, true, true, true, true]);
                    for (i, c) in cands.iter().enumerate() {
                        t.push(
                            vec![c.transform.label(), c.terms.len().to_string(), format::fixed(c.pred_r2, 4), format::fixed(c.adj_r2, 4), format::fixed(c.aicc, 2)],
                            "",
                            if i == 0 { "good" } else { "" },
                        );
                    }
                    Ok(json!({ "table": t, "formula": format_formula(p, &best.terms), "transform": best.transform.label() }))
                })
            }

            // Analysis ----------------------------------------------------
            "analysis.fit" => results::fit_summary(&self.project, arg_u64(a, "response")?),
            "analysis.equation" => {
                let p = &self.project;
                let rid = arg_u64(a, "response")?;
                let f = results::fit_for(p, rid)?;
                let ri = p.response_index(rid).ok_or("unknown response")?;
                let actual = opt_str(a, "units", "actual") == "actual";
                let syntax = Syntax::parse(opt_str(a, "syntax", "plain"));
                let eqs = equations(p, &f, &p.responses[ri].name, actual, syntax, 6);
                let mut text = String::new();
                let comment = match syntax {
                    Syntax::Plain => Some(""),
                    Syntax::Python => Some("# "),
                    Syntax::C => Some("// "),
                    Syntax::Latex => Some("% "),
                    Syntax::Excel => None,
                };
                if let (false, Some(c), Some(note)) = (actual, comment, crate::model::equation::coded_note(p, &f)) {
                    text.push_str(&format!("{c}{note}\n"));
                }
                for e in &eqs {
                    if !e.condition.is_empty() {
                        text.push_str(&format!("{}:\n", e.condition));
                    }
                    text.push_str(&e.text);
                    text.push('\n');
                    if let Some(x) = &e.explicit {
                        text.push_str(x);
                        text.push('\n');
                    }
                    if eqs.len() > 1 {
                        text.push('\n');
                    }
                }
                Ok(json!({ "text": text.trim_end(), "count": eqs.len() }))
            }
            "analysis.predict" => {
                let p = &self.project;
                let rid = arg_u64(a, "response")?;
                let f = results::fit_for(p, rid)?;
                let pt = point_from_json(p, a.get("values"))?;
                let pr = f.predict_point(&pt, 0.05).ok_or("missing factor values")?;
                Ok(json!({ "y": num(pr.y, 6), "ci": [num(pr.ci.0, 5), num(pr.ci.1, 5)], "pi": [num(pr.pi.0, 5), num(pr.pi.1, 5)] }))
            }
            "analysis.plot" => self.analysis_plot(a),
            "analysis.robustness" => results::robustness(&self.project, arg_u64(a, "response")?, opt_str(a, "sn", "nominal")),

            // Evaluation ---------------------------------------------------
            "evaluate.run" => self.evaluate(a),

            // Optimisation --------------------------------------------------
            "optimize.get" => Ok(self.optimize_get()),
            "optimize.set" => {
                let s = parse_opt_settings(&self.project, a)?;
                self.mutate(|p| {
                    p.optimize = s;
                    Ok(json!(null))
                })
            }
            "optimize.run" => self.optimize_run(a),
            "optimize.profile" => self.optimize_profile(a),
            "optimize.control_law" => self.control_law(a),

            // Import / export ----------------------------------------------
            "import.preview" => Ok(serde_json::to_value(io::import_preview(&self.project, arg_str(a, "text")?)?).unwrap()),
            "import.apply" => {
                let text = arg_str(a, "text")?.to_string();
                let mapping: Vec<String> = a.get("mapping").and_then(|v| v.as_array()).map(|v| v.iter().map(|x| x.as_str().unwrap_or("ignore").to_string()).collect()).unwrap_or_default();
                let replace = opt_str(a, "mode", "append") == "replace";
                self.mutate(|p| io::import_apply(p, &text, &mapping, replace).map(|r| serde_json::to_value(r).unwrap()))
            }
            "export.csv" => Ok(json!({ "text": io::export_runs_csv(&self.project)? })),
            "export.report" => Ok(json!({ "html": crate::report::html(&self.project) })),
            "export.r" => Ok(json!({ "text": crate::rscript::script(&self.project) })),
            _ => Err(format!("unknown method '{method}'")),
        }
    }

    fn state(&self) -> Json {
        let p = &self.project;
        let done = p.runs.iter().filter(|r| r.status == RunStatus::Done).count();
        json!({
            "title": p.title,
            "notes": p.notes,
            "dirty": self.dirty,
            "can_undo": !self.undo.is_empty(),
            "can_redo": !self.redo.is_empty(),
            "revision": self.revision,
            "epoch": self.epoch,
            "factors": p.factors.len(),
            "responses": p.responses.len(),
            "runs": p.runs.len(),
            "done": done,
            "mixture_total": p.mixture_total,
            "has_mixture": p.factors.iter().any(|f| f.is_mixture()),
            "has_env": p.factors.iter().any(|f| f.role == Role::Environmental),
            "design": p.design.as_ref().map(|d| json!({ "method": d.method, "notes": d.notes })),
        })
    }

    fn factor_list(&self) -> Json {
        let p = &self.project;
        let center = center_point(p);
        json!(p
            .factors
            .iter()
            .enumerate()
            .map(|(i, f)| {
                let (lo, hi) = f.range();
                let levels: Vec<String> = match &f.kind {
                    FactorKind::Categorical { levels } => levels.clone(),
                    FactorKind::Discrete { levels } => levels.iter().map(|v| num(*v, 10)).collect(),
                    _ => vec![],
                };
                json!({
                    "id": f.id.to_string(),
                    "symbol": f.symbol,
                    "name": f.name,
                    "units": f.units,
                    "kind": format!("{:?}", f.kind).split_whitespace().next().unwrap_or("").to_string(),
                    "role": f.role.label(),
                    "numeric": f.is_numeric(),
                    "categorical": f.is_categorical(),
                    "low": lo,
                    "high": hi,
                    "levels": levels,
                    "center": display_value(p, i, center[i]),
                    "condition": crate::condition::describe(p, i),
                })
            })
            .collect::<Vec<_>>())
    }

    fn preview_table(&self, g: &Generated) -> Table {
        let p = &self.project;
        let shown: Vec<usize> = (0..p.factors.len())
            .filter(|&f| g.runs.iter().any(|r| r.coded[f].is_some() || r.env[f].is_some()))
            .collect();
        let mut cols: Vec<String> = vec!["#".into(), "Type".into()];
        if g.blocks > 1 {
            cols.push("Block".into());
        }
        cols.extend(shown.iter().map(|&f| format!("{}: {}", p.factors[f].symbol, p.factors[f].name)));
        let colrefs: Vec<&str> = cols.iter().map(String::as_str).collect();
        let mut numeric = vec![true, false];
        if g.blocks > 1 {
            numeric.push(true);
        }
        numeric.extend(shown.iter().map(|&f| p.factors[f].is_numeric()));
        let mut t = Table::new(&colrefs, &numeric);
        for (i, r) in g.runs.iter().enumerate() {
            let mut row = vec![(i + 1).to_string(), r.point_type.clone()];
            if g.blocks > 1 {
                row.push((r.block + 1).to_string());
            }
            for &f in &shown {
                row.push(match (&r.coded[f], &r.env[f]) {
                    (Some(c), _) => display_value(p, f, Some(*c)),
                    (None, Some(Value::Num(x))) => num(*x, 6),
                    (None, Some(Value::Text(s))) => s.clone(),
                    _ => String::new(),
                });
            }
            t.push(row, "", "");
        }
        t
    }

    fn analysis_plot(&self, a: &Map<String, Json>) -> R {
        let p = &self.project;
        let rid = arg_u64(a, "response")?;
        let kind = arg_str(a, "kind")?;
        if kind == "data" {
            let fid = arg_u64(a, "x")?;
            let fi = p.factor_index(fid).ok_or("unknown factor")?;
            return Ok(serde_json::to_value(results::data_scatter(p, rid, fi)?).unwrap());
        }
        let f = results::fit_for(p, rid)?;
        let current = point_from_json(p, a.get("values"))?;
        let factor = |k: &str| -> Result<usize, String> { p.factor_index(arg_u64(a, k)?).ok_or_else(|| "unknown factor".to_string()) };
        let plot: Plot = match kind {
            "pred_actual" => results::plot_pred_actual(p, &f, rid),
            "resid_fitted" => results::plot_resid_fitted(p, &f),
            "resid_normal" => results::plot_resid_normal(p, &f),
            "resid_order" => results::plot_resid_order(p, &f),
            "cooks" => results::plot_cooks(p, &f),
            "boxcox" => results::plot_boxcox(&f)?,
            "pareto" => results::plot_pareto(&f),
            "halfnormal" => results::plot_half_normal(&f),
            "interaction" => results::interaction(p, &f, rid, factor("x")?, factor("y")?, &current),
            "contour" => results::contour(p, &f, rid, factor("x")?, factor("y")?, &current)?,
            "surface" => results::surface(p, &f, rid, factor("x")?, factor("y")?, &current, opt_f64(a, "azimuth", 35.0), opt_f64(a, "elevation", 30.0))?,
            "profiler" => {
                let (plots, info) = results::profiler(p, &f, rid, &current);
                return Ok(json!({ "plots": plots, "prediction": info }));
            }
            _ => return Err(format!("unknown plot '{kind}'")),
        };
        Ok(serde_json::to_value(plot).unwrap())
    }

    fn evaluate(&self, a: &Map<String, Json>) -> R {
        let p = &self.project;
        let points: Vec<Vec<Option<Coded>>> = if opt_str(a, "source", "runs") == "preview" {
            let g = self.preview.as_ref().ok_or("no design preview")?;
            let mut pts: Vec<Vec<Option<Coded>>> = if g.keeps_existing {
                p.runs.iter().filter(|r| r.status != RunStatus::Excluded).map(|r| p.coded_run(r)).collect()
            } else {
                vec![]
            };
            for r in &g.runs {
                let mut pt = r.coded.clone();
                for (f, v) in r.env.iter().enumerate() {
                    if let Some(v) = v {
                        pt[f] = p.code(f, v);
                    }
                }
                pts.push(pt);
            }
            pts
        } else {
            p.runs.iter().filter(|r| r.status != RunStatus::Excluded).map(|r| p.coded_run(r)).collect()
        };
        let formula = opt_str(a, "formula", "");
        let terms = if formula.trim().is_empty() {
            match Preset::parse(opt_str(a, "preset", "")) {
                Some(pr) => preset_terms(p, pr, opt_bool(a, "env_interactions", false)),
                None => match &self.preview {
                    Some(g) if opt_str(a, "source", "runs") == "preview" => g.model.clone(),
                    _ => analysis::default_terms(p),
                },
            }
        } else {
            parse_formula(p, formula)?
        };
        let snr = opt_f64(a, "snr", 2.0);
        let e = design::evaluate::evaluate(p, &points, &terms, snr, 0.05)?;
        let mut t = Table::new(&["Term", "Std. error / σ", "VIF", "Power"], &[false, true, true, true]);
        for i in 0..e.kept_labels.len() {
            t.push(
                vec![
                    e.kept_labels[i].clone(),
                    format::fixed(e.se_unit[i], 4),
                    if e.vif[i].is_finite() { format::fixed(e.vif[i], 2) } else { String::new() },
                    if e.power[i].is_finite() { format!("{:.1} %", 100.0 * e.power[i]) } else { String::new() },
                ],
                "",
                if e.power[i] < 0.8 { "muted" } else { "" },
            );
        }
        let stats = vec![
            ("Runs".to_string(), e.n.to_string()),
            ("Model coefficients".into(), e.p.to_string()),
            ("Residual df".into(), num(e.df_resid, 4)),
            ("D-efficiency".into(), format!("{} %", format::fixed(e.d_eff, 2))),
            ("G-efficiency".into(), if e.g_eff.is_finite() { format!("{} %", format::fixed(e.g_eff, 2)) } else { String::new() }),
            ("A (trace of (X'X)⁻¹)".into(), num(e.a_value, 5)),
            ("Average prediction variance".into(), num(e.i_value, 5)),
        ];
        // Correlation heat map.
        let k = e.corr_labels.len();
        let mut values = Vec::new();
        for i in 0..k {
            for j in 0..k {
                values.push(e.corr[(i, j)].abs());
            }
        }
        let corr_plot = Plot {
            title: "Correlation of model terms (|r|)".into(),
            x: crate::plot::category_axis(&e.corr_labels),
            y: crate::plot::Axis { min: k as f64 - 0.5, max: -0.5, ticks: crate::plot::category_axis(&e.corr_labels).ticks, hidden: false },
            heatmap: Some(crate::plot::Heatmap {
                nx: k,
                ny: k,
                x0: 0.0,
                x1: k as f64 - 1.0,
                y0: 0.0,
                y1: k as f64 - 1.0,
                values,
                vmin: 0.0,
                vmax: 1.0,
                diverging: false,
                row_labels: e.corr_labels.clone(),
                col_labels: e.corr_labels.clone(),
            }),
            kind: "matrix",
            ..Default::default()
        };
        let n = e.fds.len();
        let fx: Vec<f64> = (0..n).map(|i| (i as f64 + 0.5) / n as f64).collect();
        let fds_plot = Plot {
            title: "Fraction of design space".into(),
            x_label: "Fraction of design space".into(),
            y_label: "Scaled prediction variance".into(),
            x: nice_axis(0.0, 1.0, 6, true),
            y: data_axis(&e.fds, 6),
            series: vec![Series::new("SPV", "line", fx, e.fds.clone(), 0)],
            kind: "xy",
            ..Default::default()
        };
        Ok(json!({
            "formula": format_formula(p, &terms),
            "stats": stats,
            "table": t,
            "warnings": e.warnings,
            "correlation": corr_plot,
            "fds": fds_plot,
        }))
    }

    fn next_run(&self, a: &Map<String, Json>) -> R {
        let p = &self.project;
        let env_now = point_from_json(p, a.get("env"))?;
        let mode = opt_str(a, "mode", "info");
        let values: Vec<Option<Coded>> = if mode == "best" {
            let mut s = p.optimize.clone();
            for (fi, f) in p.factors.iter().enumerate() {
                if f.role == Role::Environmental {
                    if let Some(c) = env_now[fi] {
                        s.env.insert(f.id, EnvSetting { mode: "fixed".into(), value: Some(p.decode(fi, c)), mean: None, sd: None });
                    }
                }
            }
            let (models, _) = optimize::response_models(p);
            let prob = optimize::Problem::new(p, &s, models)?;
            let sol = prob.solve(1, None);
            let (pt, _) = sol.into_iter().next().ok_or("no solution")?;
            let mut pt = pt;
            for (fi, f) in p.factors.iter().enumerate() {
                if f.role == Role::Environmental {
                    pt[fi] = env_now[fi];
                }
            }
            pt
        } else {
            let formula = opt_str(a, "formula", "");
            let terms = if formula.trim().is_empty() {
                match p.responses.first() {
                    Some(r) => model_for(p, r.id).0,
                    None => analysis::default_terms(p),
                }
            } else {
                parse_formula(p, formula)?
            };
            let mut base = vec![None; p.factors.len()];
            for (fi, f) in p.factors.iter().enumerate() {
                if !f.role.is_designed() {
                    base[fi] = env_now[fi].or(if f.role == Role::Block { Some(Coded::Level(0)) } else { None });
                }
            }
            let nr = design::optimal::next_run(p, &terms, &base)?;
            nr.point
        };
        let mut values = values;
        let conds = crate::condition::coded_conditions(p);
        let act = crate::condition::activity(&conds, &values);
        crate::condition::blank_inactive(&conds, &mut values);
        let mut rows = Table::new(&["Factor", "Value"], &[false, true]);
        let mut map = Map::new();
        for (fi, f) in p.factors.iter().enumerate() {
            if f.role == Role::Block {
                continue;
            }
            let txt = display_value(p, fi, values[fi]);
            let shown = if act[fi] == Some(false) { "—".to_string() } else { txt.clone() };
            let style = if f.role == Role::Environmental || act[fi] == Some(false) { "muted" } else { "" };
            rows.push(vec![format!("{}: {}", f.symbol, f.name), shown], &f.id.to_string(), style);
            map.insert(f.id.to_string(), json!(txt));
        }
        // Predictions at the suggested point.
        let mut preds = Vec::new();
        for r in &p.responses {
            if let Ok(fit) = results::fit_for(p, r.id) {
                if let Some(pr) = fit.predict_point(&values, 0.05) {
                    preds.push(format!("{}: {} (95% PI {} to {})", r.name, num(pr.y, 5), num(pr.pi.0, 4), num(pr.pi.1, 4)));
                }
            }
        }
        Ok(json!({ "table": rows, "values": map, "predictions": preds }))
    }

    fn optimize_get(&self) -> Json {
        let p = &self.project;
        let mut env = Vec::new();
        let mut fixed = Vec::new();
        for (fi, f) in p.factors.iter().enumerate() {
            let s = p.optimize.env.get(&f.id);
            match f.role {
                Role::Environmental | Role::Noise => {
                    let obs = analysis::factor_stats(p, fi);
                    let (c, h) = f.center_half();
                    let mean = s.and_then(|s| s.mean).or(obs.map(|o| o.0)).unwrap_or(c);
                    let sd = s.and_then(|s| s.sd).or(obs.map(|o| o.1)).unwrap_or(h / 2.0);
                    let value = s.and_then(|s| s.value.clone()).map(|v| match v {
                        Value::Num(x) => num(x, 6),
                        Value::Text(t) => t,
                    });
                    let default_value = display_value(p, fi, center_point(p)[fi]);
                    env.push(json!({
                        "id": f.id.to_string(),
                        "name": f.name,
                        "symbol": f.symbol,
                        "role": f.role.label(),
                        "categorical": f.is_categorical(),
                        "levels": f.cat_levels(),
                        "mode": s.map(|s| s.mode.clone()).unwrap_or(if f.role == Role::Noise { "random".into() } else { "fixed".into() }),
                        "value": value.unwrap_or(default_value),
                        "mean": num(mean, 6),
                        "sd": num(sd, 6),
                    }));
                }
                Role::Controlled => {
                    fixed.push(json!({
                        "id": f.id.to_string(),
                        "name": f.name,
                        "symbol": f.symbol,
                        "categorical": f.is_categorical(),
                        "levels": f.cat_levels(),
                        "value": p.optimize.fixed.get(&f.id).map(|v| match v {
                            Value::Num(x) => num(*x, 6),
                            Value::Text(t) => t.clone(),
                        }).unwrap_or_default(),
                    }));
                }
                Role::Block => {}
            }
        }
        let goals: Vec<Json> = p
            .responses
            .iter()
            .map(|r| json!({ "name": r.name, "goal": r.goal.label(), "lower": r.lower, "target": r.target, "upper": r.upper }))
            .collect();
        json!({ "env": env, "fixed": fixed, "goals": goals })
    }

    fn optimize_run(&mut self, a: &Map<String, Json>) -> R {
        if a.contains_key("env") || a.contains_key("fixed") {
            let s = parse_opt_settings(&self.project, a)?;
            if s != self.project.optimize {
                self.mutate(|p| {
                    p.optimize = s;
                    Ok(json!(null))
                })?;
            }
        }
        let p = &self.project;
        let (models, problems) = optimize::response_models(p);
        let prob = optimize::Problem::new(p, &p.optimize, models)?;
        let nsol = a.get("solutions").and_then(|v| v.as_u64()).unwrap_or(8) as usize;
        let sols = prob.solve(nsol, None);
        let robust = prob.scenarios.len() > 1;
        let dec: Vec<usize> = prob.cont.iter().chain(&prob.discrete).chain(&prob.mixture).chain(&prob.cats).cloned().collect();
        let mut dec_sorted = dec.clone();
        dec_sorted.sort_unstable();
        let mut cols: Vec<String> = vec!["#".into(), "Desirability".into()];
        cols.extend(dec_sorted.iter().map(|&f| p.factors[f].name.clone()));
        for m in &prob.models {
            let r = &p.responses[m.index];
            cols.push(r.name.clone());
            if robust {
                cols.push(format!("{} SD", r.name));
            }
        }
        let refs: Vec<&str> = cols.iter().map(String::as_str).collect();
        let mut numeric = vec![true, true];
        numeric.extend(dec_sorted.iter().map(|&f| p.factors[f].is_numeric()));
        numeric.extend(std::iter::repeat_n(true, cols.len() - numeric.len()));
        let mut t = Table::new(&refs, &numeric);
        let mut out_sols = Vec::new();
        let conds = crate::condition::coded_conditions(p);
        for (i, (pt, e)) in sols.iter().enumerate() {
            let mut row = vec![(i + 1).to_string(), format::fixed(e.d, 4)];
            let mut values = Map::new();
            let mut pt = pt.clone();
            crate::condition::blank_inactive(&conds, &mut pt);
            for &f in &dec_sorted {
                let txt = display_value(p, f, pt[f]);
                values.insert(p.factors[f].id.to_string(), json!(txt));
                row.push(if txt.is_empty() { "—".to_string() } else { txt });
            }
            for (fi, fac) in p.factors.iter().enumerate() {
                if !dec_sorted.contains(&fi) && fac.role != Role::Block {
                    values.insert(fac.id.to_string(), json!(display_value(p, fi, pt[fi])));
                }
            }
            for (mean, sd, _) in &e.responses {
                row.push(num(*mean, 5));
                if robust {
                    row.push(num(*sd, 4));
                }
            }
            t.push(row, &i.to_string(), if i == 0 { "good" } else { "" });
            out_sols.push(json!({ "values": values, "d": e.d }));
        }
        let mut notes = problems;
        if robust {
            notes.push(format!(
                "Robust optimisation: desirability averaged over {} combinations of the random environmental and noise factors.",
                prob.scenarios.len()
            ));
        }
        if sols.first().is_some_and(|s| s.1.d == 0.0) {
            notes.push("No setting satisfies all goals; the table lists the closest settings. Widen the limits on the Responses page.".into());
        }
        Ok(json!({ "table": t, "solutions": out_sols, "notes": notes, "robust": robust }))
    }

    /// Desirability profiler: response curves for each decision factor at a point.
    fn optimize_profile(&self, a: &Map<String, Json>) -> R {
        let p = &self.project;
        let pt = point_from_json(p, a.get("values"))?;
        let (models, _) = optimize::response_models(p);
        let prob = optimize::Problem::new(p, &p.optimize, models)?;
        let dec: Vec<usize> = {
            let mut v: Vec<usize> = prob.cont.iter().chain(&prob.discrete).chain(&prob.cats).chain(&prob.mixture).cloned().collect();
            v.sort_unstable();
            v
        };
        let base = prob.evaluate(&pt);
        let mut rows: Vec<Vec<Plot>> = Vec::new();
        let nr = prob.models.len();
        let mut curves: Vec<Vec<(Vec<f64>, Vec<f64>)>> = vec![Vec::new(); nr + 1];
        for &fi in &dec {
            let mix_pos = prob.mixture.iter().position(|&f| f == fi);
            let sweep: Vec<Coded> = match &p.factors[fi].kind {
                FactorKind::Categorical { levels } => (0..levels.len()).map(Coded::Level).collect(),
                FactorKind::Mixture { .. } => {
                    // Feasible range of the component; the other components
                    // rescale so that the blend keeps the mixture total.
                    let (lo, hi) = mix_pos.map(|i| prob.mixture_range(i)).unwrap_or((0.0, 1.0));
                    (0..21).map(|i| Coded::Num(lo + (hi - lo) * i as f64 / 20.0)).collect()
                }
                FactorKind::Discrete { .. } => design::level_grid(p, fi, 0),
                _ => (0..31).map(|i| Coded::Num(-1.0 + 2.0 * i as f64 / 30.0)).collect(),
            };
            let mut xs = Vec::new();
            let mut ys = vec![Vec::new(); nr + 1];
            for c in sweep {
                let mut q = pt.clone();
                match mix_pos {
                    Some(i) => prob.set_mixture(&mut q, i, c.num()),
                    None => q[fi] = Some(c),
                }
                let e = prob.evaluate(&q);
                xs.push(if p.factors[fi].is_categorical() {
                    c.num()
                } else {
                    p.decode(fi, c).as_num().unwrap_or(c.num())
                });
                for (k, r) in e.responses.iter().enumerate() {
                    ys[k].push(r.0);
                }
                ys[nr].push(e.d);
            }
            for k in 0..=nr {
                curves[k].push((xs.clone(), ys[k].clone()));
            }
        }
        for k in 0..=nr {
            let all: Vec<f64> = curves[k].iter().flat_map(|c| c.1.clone()).collect();
            let yax = if k == nr { nice_axis(0.0, 1.0, 3, true) } else { data_axis(&all, 4) };
            let label = if k == nr { "Desirability".to_string() } else { p.responses[prob.models[k].index].name.clone() };
            let mut row = Vec::new();
            for (j, &fi) in dec.iter().enumerate() {
                let (xs, ys) = &curves[k][j];
                let cur_x = pt[fi].map(|c| if p.factors[fi].is_categorical() { c.num() } else { p.decode(fi, c).as_num().unwrap_or(0.0) }).unwrap_or(0.0);
                let mut v = Series::new("", "vline", vec![cur_x], vec![], -3);
                v.dashed = true;
                let kind = if matches!(p.factors[fi].kind, FactorKind::Categorical { .. } | FactorKind::Discrete { .. }) { "linepoints" } else { "line" };
                let x = match &p.factors[fi].kind {
                    FactorKind::Categorical { levels } => crate::plot::category_axis(levels),
                    _ => {
                        let (lo, hi) = p.factors[fi].range();
                        nice_axis(lo, hi, 4, false)
                    }
                };
                row.push(Plot {
                    x_label: if k == nr { p.factors[fi].name.clone() } else { String::new() },
                    y_label: if j == 0 { label.clone() } else { String::new() },
                    x,
                    y: yax.clone(),
                    series: vec![Series::new("", kind, xs.clone(), ys.clone(), if k == nr { 2 } else { 0 }), v],
                    kind: "xy",
                    note: p.factors[fi].id.to_string(),
                    ..Default::default()
                });
            }
            rows.push(row);
        }
        let values: Vec<String> = base.responses.iter().map(|r| num(r.0, 5)).collect();
        Ok(json!({ "rows": rows, "desirability": base.d, "values": values }))
    }

    fn control_law(&self, a: &Map<String, Json>) -> R {
        let p = &self.project;
        let fid = arg_u64(a, "factor")?;
        let fi = p.factor_index(fid).ok_or("unknown factor")?;
        let law = optimize::control_law(p, &p.optimize, fi, 13)?;
        if law.env_values.is_empty() {
            return Err("no solutions found".into());
        }
        let mut plots = Vec::new();
        let mut formulas = Vec::new();
        let ename = crate::model::equation::sanitize_ident(&p.factors[fi].name);
        for (d, coef, r2) in &law.fits {
            let ys: Vec<f64> = law.settings.iter().map(|pt| p.decode(*d, pt[*d].unwrap()).as_num().unwrap_or(f64::NAN)).collect();
            let fx: Vec<f64> = (0..50).map(|i| law.env_values[0] + (law.env_values[law.env_values.len() - 1] - law.env_values[0]) * i as f64 / 49.0).collect();
            let fy: Vec<f64> = fx.iter().map(|x| coef.iter().enumerate().map(|(k, c)| c * x.powi(k as i32)).sum()).collect();
            let (lo, hi) = p.factors[*d].range();
            let mut yv = ys.clone();
            yv.extend([lo, hi]);
            let mut fit_line = Series::new("Fitted law", "line", fx, fy, -2);
            fit_line.dashed = true;
            plots.push(Plot {
                title: format!("Best {} vs {}", p.factors[*d].name, p.factors[fi].name),
                x_label: p.factors[fi].name.clone(),
                y_label: p.factors[*d].name.clone(),
                x: data_axis(&law.env_values, 6),
                y: data_axis(&yv, 6),
                series: vec![Series::new("Optimum", "linepoints", law.env_values.clone(), ys, 0), fit_line],
                kind: "xy",
                legend: true,
                ..Default::default()
            });
            let dname = crate::model::equation::sanitize_ident(&p.factors[*d].name);
            let (ymin, ymax) = law.settings.iter().filter_map(|pt| p.decode(*d, pt[*d]?).as_num()).fold((f64::INFINITY, f64::NEG_INFINITY), |a, v| (a.0.min(v), a.1.max(v)));
            if ymax - ymin <= 1e-9 * ymax.abs().max(1.0) {
                formulas.push(format!("{dname} = {}    (independent of {ename})", num(ymin, 6)));
                plots.last_mut().map(|pl: &mut Plot| pl.series.truncate(1));
                continue;
            }
            // Drops terms whose contribution over the range is negligible.
            let (elo, ehi) = (law.env_values[0], law.env_values[law.env_values.len() - 1]);
            let span = (ymax - ymin).abs();
            let mut terms = vec![num(coef[0], 6)];
            for (k, c) in coef.iter().enumerate().skip(1) {
                let contrib = (c * ehi.powi(k as i32)).abs().max((c * elo.powi(k as i32)).abs());
                if contrib < 1e-6 * span {
                    continue;
                }
                let pw = if k == 1 { ename.clone() } else { format!("{ename}^{k}") };
                terms.push(format!("{} {}*{}", if *c < 0.0 { "-" } else { "+" }, num(c.abs(), 6), pw));
            }
            formulas.push(format!("{dname} = {}    (R² = {})", terms.join(" "), format::fixed(*r2, 4)));
        }
        let mut cols = vec![p.factors[fi].name.clone(), "Desirability".into()];
        let dec: Vec<usize> = (0..p.factors.len()).filter(|&i| p.factors[i].role == Role::Controlled && !p.optimize.fixed.contains_key(&p.factors[i].id)).collect();
        cols.extend(dec.iter().map(|&i| p.factors[i].name.clone()));
        let refs: Vec<&str> = cols.iter().map(String::as_str).collect();
        let mut numeric = vec![true, true];
        numeric.extend(dec.iter().map(|&i| p.factors[i].is_numeric()));
        let mut t = Table::new(&refs, &numeric);
        for (k, x) in law.env_values.iter().enumerate() {
            let mut row = vec![num(*x, 5), format::fixed(law.desirability[k], 4)];
            row.extend(dec.iter().map(|&i| display_value(p, i, law.settings[k][i])));
            t.push(row, "", "");
        }
        Ok(json!({ "plots": plots, "formulas": formulas, "table": t }))
    }
}

fn parse_opt_settings(p: &Project, a: &Map<String, Json>) -> Result<OptimizeSettings, String> {
    let mut s = OptimizeSettings::default();
    if let Some(Json::Object(env)) = a.get("env") {
        for (k, v) in env {
            let Ok(id) = k.parse::<u64>() else { continue };
            let Some(fi) = p.factor_index(id) else { continue };
            let Some(o) = v.as_object() else { continue };
            let mode = o.get("mode").and_then(|m| m.as_str()).unwrap_or("fixed").to_string();
            let value = match o.get("value").and_then(|x| x.as_str()) {
                Some(t) if !t.trim().is_empty() => Some(parse_value(p, fi, t)?),
                _ => None,
            };
            let num_of = |key: &str| o.get(key).and_then(|x| match x {
                Json::String(s) => format::parse_num(s),
                Json::Number(n) => n.as_f64(),
                _ => None,
            });
            s.env.insert(id, EnvSetting { mode, value, mean: num_of("mean"), sd: num_of("sd").map(|v| v.abs()) });
        }
    }
    if let Some(Json::Object(fx)) = a.get("fixed") {
        for (k, v) in fx {
            let Ok(id) = k.parse::<u64>() else { continue };
            let Some(fi) = p.factor_index(id) else { continue };
            if let Some(t) = v.as_str() {
                if !t.trim().is_empty() {
                    s.fixed.insert(id, parse_value(p, fi, t)?);
                }
            }
        }
    }
    Ok(s)
}

/// Random run order, randomised within blocks.
pub fn randomize(p: &mut Project, seed: u64) {
    let mut rng = Rng::new(seed);
    let block_id = p.factors.iter().find(|f| f.role == Role::Block).map(|f| f.id);
    let block_of = |r: &Run| -> String {
        block_id
            .and_then(|id| r.settings.get(&id))
            .map(|v| match v {
                Value::Num(x) => format!("{:020.6}", x),
                Value::Text(t) => t.clone(),
            })
            .unwrap_or_default()
    };
    let mut idx: Vec<usize> = (0..p.runs.len()).collect();
    rng.shuffle(&mut idx);
    idx.sort_by_key(|&i| block_of(&p.runs[i]));
    for (k, &i) in idx.iter().enumerate() {
        p.runs[i].run_order = k as u32 + 1;
    }
    p.runs.sort_by_key(|r| r.run_order);
}

pub fn apply_design(p: &mut Project, g: &Generated, replace: bool, randomize_order: bool, seed: u64) {
    if replace {
        p.runs.clear();
        p.models.clear();
    }
    // Block factor for blocked designs.
    let block_fi = if g.blocks > 1 {
        let levels: Vec<String> = (1..=g.blocks).map(|b| b.to_string()).collect();
        match p.factors.iter().position(|f| f.role == Role::Block) {
            Some(i) => {
                p.factors[i].kind = FactorKind::Categorical { levels };
                Some(i)
            }
            None => {
                let id = p.new_id();
                let symbol = p.next_symbol();
                p.factors.push(Factor { id, symbol, name: "Block".into(), units: String::new(), kind: FactorKind::Categorical { levels }, role: Role::Block, condition: None });
                Some(p.factors.len() - 1)
            }
        }
    } else {
        None
    };
    let std0 = p.runs.iter().map(|r| r.std_order).max().unwrap_or(0);
    let ord0 = p.runs.iter().map(|r| r.run_order).max().unwrap_or(0);
    let first_new = p.runs.len();
    for (i, gr) in g.runs.iter().enumerate() {
        let id = p.new_id();
        let mut run = Run {
            id,
            std_order: std0 + i as u32 + 1,
            run_order: ord0 + i as u32 + 1,
            point_type: gr.point_type.clone(),
            status: RunStatus::Planned,
            settings: Default::default(),
            responses: Default::default(),
            note: String::new(),
        };
        for (fi, c) in gr.coded.iter().enumerate() {
            if let Some(c) = c {
                if fi < p.factors.len() {
                    run.settings.insert(p.factors[fi].id, p.decode(fi, *c));
                }
            }
            if let Some(v) = &gr.env[fi] {
                run.settings.insert(p.factors[fi].id, v.clone());
            }
        }
        if let Some(bi) = block_fi {
            run.settings.insert(p.factors[bi].id, Value::Text((gr.block + 1).to_string()));
        }
        p.runs.push(run);
    }
    if randomize_order {
        let mut rng = Rng::new(seed);
        let n = g.runs.len();
        let mut idx: Vec<usize> = (0..n).collect();
        rng.shuffle(&mut idx);
        idx.sort_by_key(|&i| g.runs[i].block);
        for (k, &i) in idx.iter().enumerate() {
            p.runs[first_new + i].run_order = ord0 + k as u32 + 1;
        }
        p.runs[first_new..].sort_by_key(|r| r.run_order);
    }
    let mut info = DesignInfo { method: g.method.clone(), notes: g.notes.clone(), model: crate::model::terms::to_specs(p, &g.model) };
    if !replace {
        if let Some(old) = &p.design {
            info.notes.insert(0, format!("Added to a previous {} design.", old.method));
        }
    }
    p.design = Some(info);
}
