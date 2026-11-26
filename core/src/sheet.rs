//! Spreadsheet views of project data (factors, responses, runs) with cell
//! editing, row operations and clipboard text exchange. Frontends render
//! the cells and send edits back as text.

use crate::analysis::parse_value;
use crate::format;
use crate::model::terms::from_specs;
use crate::project::*;
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct Cell {
    pub text: String,
    /// normal | readonly | needed | warn | excluded | placeholder | error | inactive
    pub state: &'static str,
    pub editable: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct Column {
    pub title: String,
    /// meta | factor | noise | env | block | response | text
    pub kind: &'static str,
    pub tip: String,
    /// text | choice | combo | none
    pub editor: &'static str,
    pub choices: Vec<String>,
    pub numeric: bool,
}

#[derive(Clone, Debug, Serialize, Default)]
pub struct Sheet {
    pub columns: Vec<Column>,
    pub rows: Vec<Vec<Cell>>,
    /// True when the last row is an empty row for adding entries.
    pub placeholder: bool,
}

fn col(title: &str, kind: &'static str, editor: &'static str, choices: Vec<String>, numeric: bool, tip: &str) -> Column {
    Column { title: title.into(), kind, tip: tip.into(), editor, choices, numeric }
}

fn cell(text: impl Into<String>, state: &'static str, editable: bool) -> Cell {
    Cell { text: text.into(), state, editable }
}

fn num_text(v: f64) -> String {
    format::num(v, 10)
}

fn opt_num(v: Option<f64>) -> String {
    v.map(num_text).unwrap_or_default()
}

pub const KIND_LABELS: [&str; 4] = ["Continuous", "Discrete", "Categorical", "Mixture"];

fn kind_label(k: &FactorKind) -> &'static str {
    match k {
        FactorKind::Continuous { .. } => "Continuous",
        FactorKind::Discrete { .. } => "Discrete",
        FactorKind::Categorical { .. } => "Categorical",
        FactorKind::Mixture { .. } => "Mixture",
    }
}

fn role_kind(r: Role) -> &'static str {
    match r {
        Role::Controlled => "factor",
        Role::Noise => "noise",
        Role::Environmental => "env",
        Role::Block => "block",
    }
}

pub fn get(p: &Project, name: &str) -> Result<Sheet, String> {
    match name {
        "factors" => Ok(factors_sheet(p)),
        "responses" => Ok(responses_sheet(p)),
        "runs" => Ok(runs_sheet(p)),
        _ => Err(format!("unknown sheet '{name}'")),
    }
}

fn factors_sheet(p: &Project) -> Sheet {
    let roles: Vec<String> = Role::all().iter().map(|r| r.label().to_string()).collect();
    let columns = vec![
        col("Symbol", "meta", "text", vec![], false, "Short name used in model formulas"),
        col("Name", "text", "text", vec![], false, ""),
        col("Units", "text", "text", vec![], false, ""),
        col("Type", "text", "choice", KIND_LABELS.iter().map(|s| s.to_string()).collect(), false, ""),
        col(
            "Role",
            "text",
            "choice",
            roles,
            false,
            "Controlled: set in experiment and production. Noise: set in the experiment, random in production. Environmental: cannot be set, measured at each run. Block: grouping of runs.",
        ),
        col("Low", "text", "text", vec![], true, "For environmental factors: the expected range"),
        col("High", "text", "text", vec![], true, ""),
        col("Levels", "text", "text", vec![], false, "Comma-separated levels for discrete and categorical factors"),
        col(
            "Condition",
            "text",
            "text",
            vec![],
            false,
            "Applies the factor only when another factor meets a condition, e.g. T < 0 or M = Freeze, Flash. Leave empty for a factor that always applies.",
        ),
    ];
    let mut rows = Vec::new();
    for (fi, f) in p.factors.iter().enumerate() {
        let (low, high, lev_text, lh_edit, lev_edit) = match &f.kind {
            FactorKind::Continuous { low, high } | FactorKind::Mixture { low, high } => (num_text(*low), num_text(*high), String::new(), true, false),
            FactorKind::Discrete { levels } => {
                let (lo, hi) = f.range();
                (num_text(lo), num_text(hi), levels.iter().map(|v| num_text(*v)).collect::<Vec<_>>().join(", "), false, true)
            }
            FactorKind::Categorical { levels } => (String::new(), String::new(), levels.join(", "), false, true),
        };
        let lh_state = if lh_edit { "normal" } else { "readonly" };
        rows.push(vec![
            cell(&f.symbol, "normal", true),
            cell(&f.name, "normal", true),
            cell(&f.units, "normal", true),
            cell(kind_label(&f.kind), "normal", true),
            cell(f.role.label(), "normal", true),
            cell(low, lh_state, lh_edit),
            cell(high, lh_state, lh_edit),
            cell(lev_text, if lev_edit { "normal" } else { "readonly" }, lev_edit),
            {
                let can = f.role != Role::Block && !f.is_mixture();
                cell(crate::condition::describe(p, fi), if can { "normal" } else { "readonly" }, can)
            },
        ]);
    }
    rows.push((0..9).map(|_| cell("", "placeholder", true)).collect());
    Sheet { columns, rows, placeholder: true }
}

fn responses_sheet(p: &Project) -> Sheet {
    let columns = vec![
        col("Name", "text", "text", vec![], false, ""),
        col("Units", "text", "text", vec![], false, ""),
        col("Goal", "text", "choice", Goal::all().iter().map(|g| g.label().to_string()).collect(), false, "Used by the optimizer"),
        col("Lower", "text", "text", vec![], true, "Lower limit of acceptable values (defaults to the observed minimum)"),
        col("Target", "text", "text", vec![], true, ""),
        col("Upper", "text", "text", vec![], true, "Upper limit of acceptable values (defaults to the observed maximum)"),
        col("Weight", "text", "text", vec![], true, "Desirability shape: below 1 lenient, above 1 strict"),
        col("Importance", "text", "text", vec![], true, "Relative importance 1-5"),
    ];
    let mut rows = Vec::new();
    for r in &p.responses {
        let target_state = if r.goal == Goal::Target { "normal" } else { "readonly" };
        rows.push(vec![
            cell(&r.name, "normal", true),
            cell(&r.units, "normal", true),
            cell(r.goal.label(), "normal", true),
            cell(opt_num(r.lower), "normal", true),
            cell(opt_num(r.target), target_state, true),
            cell(opt_num(r.upper), "normal", true),
            cell(num_text(r.weight), "normal", true),
            cell(num_text(r.importance), "normal", true),
        ]);
    }
    rows.push((0..8).map(|_| cell("", "placeholder", true)).collect());
    Sheet { columns, rows, placeholder: true }
}

const RUN_META: usize = 4;

fn runs_sheet(p: &Project) -> Sheet {
    let mut columns = vec![
        col("Run", "meta", "none", vec![], true, "Run order"),
        col("Std", "meta", "none", vec![], true, "Standard order"),
        col("Type", "meta", "none", vec![], false, "Point type"),
        col("Status", "meta", "choice", vec!["Planned".into(), "Done".into(), "Excluded".into()], false, "Excluded runs are left out of all analyses"),
    ];
    for f in &p.factors {
        let (editor, choices) = match &f.kind {
            FactorKind::Categorical { levels } => (if f.role == Role::Environmental { "combo" } else { "choice" }, levels.clone()),
            FactorKind::Discrete { levels } => ("combo", levels.iter().map(|v| num_text(*v)).collect()),
            _ => ("text", vec![]),
        };
        let title = if f.units.is_empty() { format!("{}: {}", f.symbol, f.name) } else { format!("{}: {} [{}]", f.symbol, f.name, f.units) };
        let tip = match f.role {
            Role::Environmental => "Environmental: enter the measured value for each run",
            Role::Noise => "Noise factor: set as planned during the experiment",
            Role::Block => "Block",
            Role::Controlled => "Controlled factor",
        };
        let cond = f.condition.as_ref().map(|c| crate::condition::format(p, c)).unwrap_or_default();
        let tip = if cond.is_empty() { tip.to_string() } else { format!("{tip}. Applies when {cond}.") };
        columns.push(col(&title, role_kind(f.role), editor, choices, f.is_numeric(), &tip));
    }
    for r in &p.responses {
        let title = if r.units.is_empty() { r.name.clone() } else { format!("{} [{}]", r.name, r.units) };
        columns.push(col(&title, "response", "text", vec![], true, "Response: enter the measured result"));
    }
    columns.push(col("Note", "text", "text", vec![], false, ""));
    let conds = crate::condition::coded_conditions(p);
    let conditional = crate::condition::any(&conds);
    let mut rows = Vec::new();
    for (ri, run) in p.runs.iter().enumerate() {
        let excluded = run.status == RunStatus::Excluded;
        let act = if conditional { crate::condition::run_activity(p, &conds, ri) } else { Vec::new() };
        let st = |s: &'static str| if excluded { "excluded" } else { s };
        let mut row = vec![
            cell(run.run_order.to_string(), st("readonly"), false),
            cell(run.std_order.to_string(), st("readonly"), false),
            cell(&run.point_type, st("readonly"), false),
            cell(run.status.label(), if excluded { "excluded" } else { "normal" }, true),
        ];
        for (fi, f) in p.factors.iter().enumerate() {
            if act.get(fi).copied().flatten() == Some(false) {
                row.push(cell("", st("inactive"), false));
                continue;
            }
            match run.settings.get(&f.id) {
                None => row.push(cell("", st("needed"), true)),
                Some(v) => {
                    let text = match v {
                        Value::Num(x) => num_text(*x),
                        Value::Text(t) => t.clone(),
                    };
                    let state = match (&f.kind, v) {
                        (FactorKind::Continuous { .. } | FactorKind::Mixture { .. }, Value::Num(x)) => {
                            let (lo, hi) = f.range();
                            let tol = (hi - lo).abs() * 1e-9;
                            if *x < lo - tol || *x > hi + tol { "warn" } else { "normal" }
                        }
                        (FactorKind::Categorical { .. }, _) => {
                            if p.code(fi, v).is_some() { "normal" } else { "error" }
                        }
                        _ => "normal",
                    };
                    row.push(cell(text, st(state), true));
                }
            }
        }
        for r in &p.responses {
            match run.responses.get(&r.id) {
                Some(v) => row.push(cell(num_text(*v), st("normal"), true)),
                None => row.push(cell("", st("needed"), true)),
            }
        }
        row.push(cell(&run.note, st("normal"), true));
        rows.push(row);
    }
    let ncols = columns.len();
    rows.push((0..ncols).map(|c| cell("", "placeholder", c >= RUN_META - 1)).collect());
    Sheet { columns, rows, placeholder: true }
}

/// Sets one cell from user text.
pub fn set(p: &mut Project, name: &str, row: usize, c: usize, text: &str) -> Result<(), String> {
    match name {
        "factors" => set_factor(p, row, c, text),
        "responses" => set_response(p, row, c, text),
        "runs" => set_run(p, row, c, text),
        _ => Err(format!("unknown sheet '{name}'")),
    }
}

fn valid_symbol(s: &str) -> bool {
    let mut ch = s.chars();
    matches!(ch.next(), Some(c) if c.is_alphabetic()) && s.chars().all(|c| c.is_alphanumeric() || c == '_')
}

fn parse_levels_num(text: &str) -> Result<Vec<f64>, String> {
    let mut v = Vec::new();
    for t in text.split([',', ';', ' ', '\t']).map(str::trim).filter(|s| !s.is_empty()) {
        v.push(format::parse_num(t).ok_or_else(|| format!("'{t}' is not a number"))?);
    }
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v.dedup();
    if v.len() < 2 {
        return Err("enter at least two levels".into());
    }
    Ok(v)
}

fn parse_levels_text(text: &str) -> Result<Vec<String>, String> {
    let mut v: Vec<String> = Vec::new();
    for t in text.split([',', ';', '\t']).map(str::trim).filter(|s| !s.is_empty()) {
        if !v.iter().any(|x| x == t) {
            v.push(t.to_string());
        }
    }
    if v.len() < 2 {
        return Err("enter at least two levels separated by commas".into());
    }
    Ok(v)
}

pub fn new_factor(p: &mut Project, name: &str) -> usize {
    let id = p.new_id();
    let symbol = p.next_symbol();
    let n = p.factors.len() + 1;
    p.factors.push(Factor {
        id,
        symbol,
        name: if name.trim().is_empty() { format!("Factor {n}") } else { name.trim().to_string() },
        units: String::new(),
        kind: FactorKind::Continuous { low: -1.0, high: 1.0 },
        role: Role::Controlled,
        condition: None,
    });
    p.factors.len() - 1
}

/// Converts run values after a factor kind change.
fn convert_runs(p: &mut Project, fi: usize) {
    let f = p.factors[fi].clone();
    for run in p.runs.iter_mut() {
        if let Some(v) = run.settings.get(&f.id).cloned() {
            let nv = match (&f.kind, v) {
                (FactorKind::Categorical { levels }, Value::Num(x)) => {
                    let t = num_text(x);
                    levels.iter().find(|l| **l == t).map(|l| Value::Text(l.clone()))
                }
                (FactorKind::Categorical { .. }, Value::Text(t)) => Some(Value::Text(t)),
                (_, Value::Text(t)) => format::parse_num(&t).map(Value::Num),
                (_, v) => Some(v),
            };
            match nv {
                Some(v) => {
                    run.settings.insert(f.id, v);
                }
                None => {
                    run.settings.remove(&f.id);
                }
            }
        }
    }
    // Drops model terms that became invalid.
    let specs: Vec<(u64, ModelSpec)> = p.models.iter().map(|(k, v)| (*k, v.clone())).collect();
    for (rid, spec) in specs {
        let terms = from_specs(p, &spec.terms);
        let ok: Vec<_> = terms.into_iter().filter(|t| crate::model::terms::validate_term(p, t).is_ok()).collect();
        let s = crate::model::terms::to_specs(p, &ok);
        p.models.insert(rid, ModelSpec { terms: s, transform: spec.transform });
    }
}

fn set_factor(p: &mut Project, row: usize, c: usize, text: &str) -> Result<(), String> {
    let t = text.trim();
    let fi = if row >= p.factors.len() {
        if t.is_empty() {
            return Ok(());
        }
        let fi = new_factor(p, if c == 1 { t } else { "" });
        if c == 1 {
            return Ok(());
        }
        fi
    } else {
        row
    };
    match c {
        0 => {
            if !valid_symbol(t) {
                return Err("symbols start with a letter and contain letters, digits or _".into());
            }
            if p.factors.iter().enumerate().any(|(i, f)| i != fi && f.symbol.eq_ignore_ascii_case(t)) {
                return Err(format!("symbol '{t}' is already used"));
            }
            p.factors[fi].symbol = t.to_string();
        }
        1 => {
            if t.is_empty() {
                return Err("the name cannot be empty".into());
            }
            p.factors[fi].name = t.to_string();
        }
        2 => p.factors[fi].units = t.to_string(),
        3 => {
            let k = KIND_LABELS
                .iter()
                .find(|k| !t.is_empty() && k.to_ascii_lowercase().starts_with(&t.to_ascii_lowercase()))
                .ok_or_else(|| format!("type must be one of {}", KIND_LABELS.join(", ")))?;
            let (lo, hi) = p.factors[fi].range();
            let old = p.factors[fi].kind.clone();
            let new_kind = match *k {
                "Continuous" => FactorKind::Continuous { low: lo, high: hi },
                "Mixture" => FactorKind::Mixture { low: 0.0, high: p.mixture_total },
                "Discrete" => match &old {
                    FactorKind::Discrete { .. } => old.clone(),
                    _ => FactorKind::Discrete { levels: vec![lo, (lo + hi) / 2.0, hi] },
                },
                _ => match &old {
                    FactorKind::Categorical { .. } => old.clone(),
                    FactorKind::Discrete { levels } => FactorKind::Categorical { levels: levels.iter().map(|v| num_text(*v)).collect() },
                    _ => FactorKind::Categorical { levels: vec!["L1".into(), "L2".into()] },
                },
            };
            if new_kind != old {
                p.factors[fi].kind = new_kind;
                convert_runs(p, fi);
            }
        }
        4 => {
            p.factors[fi].role = Role::from_label(t).ok_or("role must be Controlled, Noise, Environmental or Block")?;
        }
        5 | 6 => {
            let v = format::parse_num(t).ok_or_else(|| format!("'{t}' is not a number"))?;
            match &mut p.factors[fi].kind {
                FactorKind::Continuous { low, high } | FactorKind::Mixture { low, high } => {
                    if c == 5 {
                        if v >= *high {
                            return Err("low must be below high".into());
                        }
                        *low = v;
                    } else {
                        if v <= *low {
                            return Err("high must be above low".into());
                        }
                        *high = v;
                    }
                }
                _ => return Err("edit the levels of discrete and categorical factors".into()),
            }
        }
        7 => {
            let f = &p.factors[fi];
            match &f.kind {
                FactorKind::Discrete { .. } => {
                    let lv = parse_levels_num(t)?;
                    p.factors[fi].kind = FactorKind::Discrete { levels: lv };
                }
                FactorKind::Categorical { levels: old } => {
                    let lv = parse_levels_text(t)?;
                    let old = old.clone();
                    let id = f.id;
                    // Same count: positional rename; otherwise keep matching values.
                    if lv.len() == old.len() {
                        crate::condition::rename_levels(p, id, &old, &lv);
                        for run in p.runs.iter_mut() {
                            if let Some(Value::Text(v)) = run.settings.get(&id).cloned() {
                                if let Some(i) = old.iter().position(|o| *o == v) {
                                    run.settings.insert(id, Value::Text(lv[i].clone()));
                                }
                            }
                        }
                    }
                    p.factors[fi].kind = FactorKind::Categorical { levels: lv };
                    convert_runs(p, fi);
                }
                _ => return Err("continuous factors use Low and High".into()),
            }
        }
        8 => {
            if t.is_empty() {
                p.factors[fi].condition = None;
            } else {
                p.factors[fi].condition = Some(crate::condition::parse(p, fi, t)?);
            }
        }
        _ => return Err("invalid column".into()),
    }
    crate::condition::prune(p);
    Ok(())
}

pub fn new_response(p: &mut Project, name: &str) -> usize {
    let id = p.new_id();
    let n = p.responses.len() + 1;
    p.responses.push(Response {
        id,
        name: if name.trim().is_empty() { format!("R{n}") } else { name.trim().to_string() },
        units: String::new(),
        goal: Goal::None,
        lower: None,
        target: None,
        upper: None,
        weight: 1.0,
        importance: 3.0,
    });
    p.responses.len() - 1
}

fn set_response(p: &mut Project, row: usize, c: usize, text: &str) -> Result<(), String> {
    let t = text.trim();
    let ri = if row >= p.responses.len() {
        if t.is_empty() {
            return Ok(());
        }
        let ri = new_response(p, if c == 0 { t } else { "" });
        if c == 0 {
            return Ok(());
        }
        ri
    } else {
        row
    };
    let opt = |t: &str| -> Result<Option<f64>, String> {
        if t.is_empty() { Ok(None) } else { format::parse_num(t).map(Some).ok_or_else(|| format!("'{t}' is not a number")) }
    };
    let r = &mut p.responses[ri];
    match c {
        0 => {
            if t.is_empty() {
                return Err("the name cannot be empty".into());
            }
            r.name = t.into();
        }
        1 => r.units = t.into(),
        2 => r.goal = Goal::from_label(t).ok_or("goal must be None, Maximize, Minimize, Target or In range")?,
        3 => r.lower = opt(t)?,
        4 => r.target = opt(t)?,
        5 => r.upper = opt(t)?,
        6 => r.weight = opt(t)?.unwrap_or(1.0).clamp(0.1, 10.0),
        7 => r.importance = opt(t)?.unwrap_or(3.0).clamp(1.0, 5.0),
        _ => return Err("invalid column".into()),
    }
    Ok(())
}

pub fn new_run(p: &mut Project, point_type: &str) -> usize {
    let id = p.new_id();
    let ord = p.runs.iter().map(|r| r.run_order).max().unwrap_or(0) + 1;
    let std = p.runs.iter().map(|r| r.std_order).max().unwrap_or(0) + 1;
    p.runs.push(Run {
        id,
        std_order: std,
        run_order: ord,
        point_type: point_type.into(),
        status: RunStatus::Planned,
        settings: Default::default(),
        responses: Default::default(),
        note: String::new(),
    });
    p.runs.len() - 1
}

fn set_run(p: &mut Project, row: usize, c: usize, text: &str) -> Result<(), String> {
    let t = text.trim();
    let nf = p.factors.len();
    let nr = p.responses.len();
    if c < RUN_META - 1 {
        return Err("this column is read-only".into());
    }
    let ri = if row >= p.runs.len() {
        if t.is_empty() {
            return Ok(());
        }
        new_run(p, "added")
    } else {
        row
    };
    if c == RUN_META - 1 {
        p.runs[ri].status = RunStatus::from_label(t).ok_or("status must be Planned, Done or Excluded")?;
        return Ok(());
    }
    let k = c - RUN_META;
    if k < nf {
        let fid = p.factors[k].id;
        if t.is_empty() {
            p.runs[ri].settings.remove(&fid);
            return Ok(());
        }
        if p.factors[k].condition.is_some() {
            let conds = crate::condition::coded_conditions(p);
            if crate::condition::run_activity(p, &conds, ri)[k] == Some(false) {
                // The factor does not apply in this run: a value that fits
                // is kept for when it applies again, anything else (such
                // as "n/a") leaves the cell empty.
                match parse_value(p, k, t) {
                    Ok(v) if p.code(k, &v).is_some() => {
                        p.runs[ri].settings.insert(fid, v);
                    }
                    _ => {
                        p.runs[ri].settings.remove(&fid);
                    }
                }
                return Ok(());
            }
        }
        let f = p.factors[k].clone();
        let v = match &f.kind {
            FactorKind::Categorical { levels } => match f.level_index(t) {
                Some(i) => Value::Text(levels[i].clone()),
                None if f.role == Role::Environmental => {
                    // Observed environmental categories extend the level list.
                    let mut lv = levels.clone();
                    lv.push(t.to_string());
                    p.factors[k].kind = FactorKind::Categorical { levels: lv };
                    Value::Text(t.to_string())
                }
                None => return Err(format!("'{t}' is not a level of {} ({})", f.name, levels.join(", "))),
            },
            FactorKind::Discrete { levels } => {
                let x = format::parse_num(t).ok_or_else(|| format!("'{t}' is not a number"))?;
                if !levels.iter().any(|l| (l - x).abs() <= 1e-9 * l.abs().max(1.0)) {
                    if f.role == Role::Environmental {
                        let mut lv = levels.clone();
                        lv.push(x);
                        lv.sort_by(|a, b| a.partial_cmp(b).unwrap());
                        p.factors[k].kind = FactorKind::Discrete { levels: lv };
                    } else {
                        return Err(format!(
                            "{} is not a level of {} ({})",
                            num_text(x),
                            f.name,
                            levels.iter().map(|v| num_text(*v)).collect::<Vec<_>>().join(", ")
                        ));
                    }
                }
                Value::Num(x)
            }
            _ => parse_value(p, k, t)?,
        };
        p.runs[ri].settings.insert(fid, v);
        return Ok(());
    }
    let k = k - nf;
    if k < nr {
        let rid = p.responses[k].id;
        if t.is_empty() {
            p.runs[ri].responses.remove(&rid);
            return Ok(());
        }
        let v = format::parse_num(t).ok_or_else(|| format!("'{t}' is not a number"))?;
        p.runs[ri].responses.insert(rid, v);
        if p.runs[ri].status == RunStatus::Planned {
            p.runs[ri].status = RunStatus::Done;
        }
        return Ok(());
    }
    if k == nr {
        p.runs[ri].note = t.to_string();
        return Ok(());
    }
    Err("invalid column".into())
}

pub fn delete_rows(p: &mut Project, name: &str, rows: &[usize]) -> Result<(), String> {
    let mut rows: Vec<usize> = rows.to_vec();
    rows.sort_unstable();
    rows.dedup();
    for &r in rows.iter().rev() {
        match name {
            "factors" if r < p.factors.len() => {
                let id = p.factors[r].id;
                p.factors.remove(r);
                for run in p.runs.iter_mut() {
                    run.settings.remove(&id);
                }
                for spec in p.models.values_mut() {
                    spec.terms.retain(|t| !t.0.iter().any(|(f, _)| *f == id));
                }
                if let Some(d) = &mut p.design {
                    d.model.retain(|t| !t.0.iter().any(|(f, _)| *f == id));
                }
                p.optimize.env.remove(&id);
                p.optimize.fixed.remove(&id);
                crate::condition::prune(p);
            }
            "responses" if r < p.responses.len() => {
                let id = p.responses[r].id;
                p.responses.remove(r);
                for run in p.runs.iter_mut() {
                    run.responses.remove(&id);
                }
                p.models.remove(&id);
            }
            "runs" if r < p.runs.len() => {
                p.runs.remove(r);
            }
            "factors" | "responses" | "runs" => {}
            _ => return Err(format!("unknown sheet '{name}'")),
        }
    }
    Ok(())
}

pub fn insert_row(p: &mut Project, name: &str, at: usize) -> Result<(), String> {
    match name {
        "factors" => {
            let i = new_factor(p, "");
            let f = p.factors.remove(i);
            p.factors.insert(at.min(p.factors.len()), f);
        }
        "responses" => {
            let i = new_response(p, "");
            let r = p.responses.remove(i);
            p.responses.insert(at.min(p.responses.len()), r);
        }
        "runs" => {
            let i = new_run(p, "added");
            let r = p.runs.remove(i);
            p.runs.insert(at.min(p.runs.len()), r);
        }
        _ => return Err(format!("unknown sheet '{name}'")),
    }
    Ok(())
}

/// Tab-separated text of a cell range (with a header line when the range
/// starts at row 0 and `header` is set).
pub fn copy(p: &Project, name: &str, top: usize, left: usize, bottom: usize, right: usize, header: bool) -> Result<String, String> {
    let s = get(p, name)?;
    let last_row = s.rows.len().saturating_sub(if s.placeholder { 1 } else { 0 });
    let bottom = bottom.min(last_row.saturating_sub(1));
    let right = right.min(s.columns.len().saturating_sub(1));
    let mut out = String::new();
    if header {
        let h: Vec<String> = (left..=right).map(|c| s.columns[c].title.clone()).collect();
        out.push_str(&h.join("\t"));
        out.push('\n');
    }
    for r in top..=bottom {
        let line: Vec<String> = (left..=right).map(|c| s.rows[r][c].text.clone()).collect();
        out.push_str(&line.join("\t"));
        out.push('\n');
    }
    Ok(out)
}

/// Pastes tab/semicolon/comma-separated text starting at a cell. Returns
/// the messages of cells that could not be set.
pub fn paste(p: &mut Project, name: &str, row: usize, c: usize, text: &str) -> Result<Vec<String>, String> {
    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty() || text.lines().count() == 1).collect();
    let delim = if text.contains('\t') { '\t' } else if text.contains(';') { ';' } else if lines.len() > 1 && text.contains(',') && !text.contains(". ") { ',' } else { '\t' };
    let mut errors = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        let cells: Vec<&str> = line.split(delim).collect();
        for (j, v) in cells.iter().enumerate() {
            let r = row + i;
            // Rows beyond the end extend the sheet through its placeholder.
            let target_row = match name {
                "runs" => r.min(p.runs.len()),
                "factors" => r.min(p.factors.len()),
                "responses" => r.min(p.responses.len()),
                _ => r,
            };
            if let Err(e) = set(p, name, target_row, c + j, v) {
                if !(v.trim().is_empty()) {
                    errors.push(format!("row {}, column {}: {e}", r + 1, c + j + 1));
                }
            }
        }
    }
    Ok(errors)
}
