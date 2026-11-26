//! Delimited text import/export and project serialisation. All functions
//! work on strings; reading and writing files belongs to the frontend.

use crate::format;
use crate::project::*;
use crate::sheet;
use serde::Serialize;
use std::collections::BTreeSet;

pub fn save_project(p: &Project) -> Result<String, String> {
    serde_json::to_string_pretty(p).map_err(|e| e.to_string())
}

pub fn load_project(text: &str) -> Result<Project, String> {
    let p: Project = serde_json::from_str(text).map_err(|e| format!("not a valid project file: {e}"))?;
    if p.format > FORMAT_VERSION {
        return Err("the project was saved by a newer version".into());
    }
    Ok(p)
}

pub fn detect_delimiter(text: &str) -> char {
    let first: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).take(5).collect();
    let mut best = (',', 0usize);
    for d in ['\t', ';', ','] {
        let counts: Vec<usize> = first.iter().map(|l| l.matches(d).count()).collect();
        if counts.is_empty() || counts[0] == 0 {
            continue;
        }
        let consistent = counts.iter().all(|&c| c == counts[0]);
        let score = counts[0] * if consistent { 2 } else { 1 };
        if score > best.1 {
            best = (d, score);
        }
    }
    best.0
}

/// Parses delimited text with double-quote escaping.
pub fn parse_delimited(text: &str, delim: char) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut field = String::new();
    let mut in_q = false;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if in_q {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    field.push('"');
                    chars.next();
                } else {
                    in_q = false;
                }
            } else {
                field.push(c);
            }
        } else if c == '"' && field.is_empty() {
            in_q = true;
        } else if c == delim {
            row.push(std::mem::take(&mut field));
        } else if c == '\n' || c == '\r' {
            if c == '\r' && chars.peek() == Some(&'\n') {
                chars.next();
            }
            row.push(std::mem::take(&mut field));
            if row.iter().any(|f| !f.trim().is_empty()) {
                rows.push(std::mem::take(&mut row));
            } else {
                row.clear();
            }
        } else {
            field.push(c);
        }
    }
    if !field.is_empty() || !row.is_empty() {
        row.push(field);
        if row.iter().any(|f| !f.trim().is_empty()) {
            rows.push(row);
        }
    }
    rows
}

fn quote(s: &str, delim: char) -> String {
    if s.contains(delim) || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// Run table as CSV text.
pub fn export_runs_csv(p: &Project) -> Result<String, String> {
    let s = sheet::get(p, "runs")?;
    let mut out = String::new();
    out.push_str(&s.columns.iter().map(|c| quote(&c.title, ',')).collect::<Vec<_>>().join(","));
    out.push('\n');
    let n = s.rows.len() - if s.placeholder { 1 } else { 0 };
    for row in &s.rows[..n] {
        out.push_str(&row.iter().map(|c| quote(&c.text, ',')).collect::<Vec<_>>().join(","));
        out.push('\n');
    }
    Ok(out)
}

#[derive(Serialize, Clone, Debug)]
pub struct ImportColumn {
    pub name: String,
    pub numeric: bool,
    pub distinct: usize,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub suggestion: String,
    pub sample: Vec<String>,
}

#[derive(Serialize, Clone, Debug)]
pub struct ImportPreview {
    pub delimiter: String,
    pub rows: usize,
    pub columns: Vec<ImportColumn>,
    pub targets: Vec<(String, String)>,
}

/// Strips the "S: " symbol prefix and " [units]" suffix of exported headers.
fn header_name(h: &str) -> (Option<String>, String) {
    let mut name = h.trim().to_string();
    let mut sym = None;
    if let Some((a, b)) = name.split_once(": ") {
        if !a.contains(' ') && a.len() <= 4 {
            sym = Some(a.to_string());
            name = b.to_string();
        }
    }
    if let Some(i) = name.rfind(" [") {
        if name.ends_with(']') {
            name.truncate(i);
        }
    }
    (sym, name)
}

pub fn import_targets(p: &Project) -> Vec<(String, String)> {
    let mut t = vec![
        ("ignore".to_string(), "Ignore".to_string()),
        ("new_factor".into(), "New controlled factor".into()),
        ("new_env".into(), "New environmental factor".into()),
        ("new_noise".into(), "New noise factor".into()),
        ("new_response".into(), "New response".into()),
        ("meta:run".into(), "Run order".into()),
        ("meta:std".into(), "Standard order".into()),
        ("meta:status".into(), "Status".into()),
        ("meta:note".into(), "Note".into()),
    ];
    for f in &p.factors {
        t.push((format!("factor:{}", f.id), format!("Factor {}: {}", f.symbol, f.name)));
    }
    for r in &p.responses {
        t.push((format!("response:{}", r.id), format!("Response {}", r.name)));
    }
    t
}

pub fn import_preview(p: &Project, text: &str) -> Result<ImportPreview, String> {
    let delim = detect_delimiter(text);
    let rows = parse_delimited(text, delim);
    if rows.len() < 2 {
        return Err("the text needs a header line and at least one data line".into());
    }
    let header = &rows[0];
    let data = &rows[1..];
    let mut cols = Vec::new();
    let mut last_numeric = None;
    for (j, h) in header.iter().enumerate() {
        let vals: Vec<&str> = data.iter().filter_map(|r| r.get(j)).map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
        let nums: Vec<f64> = vals.iter().filter_map(|v| format::parse_num(v)).collect();
        let numeric = !vals.is_empty() && nums.len() == vals.len();
        let distinct: BTreeSet<&str> = vals.iter().cloned().collect();
        if numeric {
            last_numeric = Some(j);
        }
        cols.push(ImportColumn {
            name: h.trim().to_string(),
            numeric,
            distinct: distinct.len(),
            min: nums.iter().cloned().reduce(f64::min),
            max: nums.iter().cloned().reduce(f64::max),
            suggestion: String::new(),
            sample: vals.iter().take(3).map(|s| s.to_string()).collect(),
        });
    }
    let any_existing_response = cols.iter().any(|c| {
        let (_, n) = header_name(&c.name);
        p.responses.iter().any(|r| r.name.eq_ignore_ascii_case(&n))
    });
    for (j, c) in cols.iter_mut().enumerate() {
        let (sym, name) = header_name(&c.name);
        let lname = name.to_ascii_lowercase();
        c.suggestion = if let Some(f) = p.factors.iter().find(|f| {
            f.name.eq_ignore_ascii_case(&name) || sym.as_deref().is_some_and(|s| s.eq_ignore_ascii_case(&f.symbol) && f.name.eq_ignore_ascii_case(&name))
        }) {
            format!("factor:{}", f.id)
        } else if let Some(r) = p.responses.iter().find(|r| r.name.eq_ignore_ascii_case(&name)) {
            format!("response:{}", r.id)
        } else {
            match lname.as_str() {
                "run" | "run order" | "runorder" => "meta:run".into(),
                "std" | "std order" | "standard order" | "stdorder" => "meta:std".into(),
                "status" => "meta:status".into(),
                "note" | "notes" | "comment" => "meta:note".into(),
                "type" | "point type" => "ignore".into(),
                _ if c.distinct == 0 => "ignore".into(),
                _ if Some(j) == last_numeric && !any_existing_response && p.responses.is_empty() => "new_response".into(),
                _ => "new_factor".into(),
            }
        };
    }
    Ok(ImportPreview {
        delimiter: match delim {
            '\t' => "tab".into(),
            d => d.to_string(),
        },
        rows: data.len(),
        columns: cols,
        targets: import_targets(p),
    })
}

#[derive(Serialize, Clone, Debug, Default)]
pub struct ImportReport {
    pub runs: usize,
    pub new_factors: usize,
    pub new_responses: usize,
    pub skipped_cells: usize,
    pub messages: Vec<String>,
}

/// Imports rows as runs according to the column mapping (one target per column).
pub fn import_apply(p: &mut Project, text: &str, mapping: &[String], replace: bool) -> Result<ImportReport, String> {
    let delim = detect_delimiter(text);
    let rows = parse_delimited(text, delim);
    if rows.len() < 2 {
        return Err("no data rows".into());
    }
    let header = rows[0].clone();
    let data = &rows[1..];
    let mut rep = ImportReport::default();
    // Resolves targets, creating factors and responses.
    #[derive(Clone)]
    enum Target {
        Ignore,
        Factor(u64),
        Response(u64),
        Run,
        Std,
        Status,
        Note,
    }
    let mut targets = Vec::new();
    for (j, h) in header.iter().enumerate() {
        let m = mapping.get(j).map(String::as_str).unwrap_or("ignore");
        let (_, name) = header_name(h);
        let vals: Vec<&str> = data.iter().filter_map(|r| r.get(j)).map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
        let nums: Vec<f64> = vals.iter().filter_map(|v| format::parse_num(v)).collect();
        let numeric = !vals.is_empty() && nums.len() == vals.len();
        let t = match m {
            "new_factor" | "new_env" | "new_noise" => {
                let id = p.new_id();
                let kind = if numeric {
                    let lo = nums.iter().cloned().fold(f64::INFINITY, f64::min);
                    let hi = nums.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
                    let hi = if hi > lo { hi } else { lo + 1.0 };
                    FactorKind::Continuous { low: lo, high: hi }
                } else {
                    let mut lv: Vec<String> = Vec::new();
                    for v in &vals {
                        if !lv.iter().any(|x| x == v) {
                            lv.push(v.to_string());
                        }
                    }
                    if lv.len() < 2 {
                        lv.push("Other".into());
                    }
                    FactorKind::Categorical { levels: lv }
                };
                let symbol = p.next_symbol();
                p.factors.push(Factor {
                    id,
                    symbol,
                    name: if name.is_empty() { format!("Factor {}", p.factors.len() + 1) } else { name.clone() },
                    units: String::new(),
                    kind,
                    role: match m {
                        "new_env" => Role::Environmental,
                        "new_noise" => Role::Noise,
                        _ => Role::Controlled,
                    },
                    condition: None,
                });
                rep.new_factors += 1;
                Target::Factor(id)
            }
            "new_response" => {
                let i = sheet::new_response(p, &name);
                rep.new_responses += 1;
                Target::Response(p.responses[i].id)
            }
            "meta:run" => Target::Run,
            "meta:std" => Target::Std,
            "meta:status" => Target::Status,
            "meta:note" => Target::Note,
            s if s.starts_with("factor:") => Target::Factor(s[7..].parse().map_err(|_| "bad mapping")?),
            s if s.starts_with("response:") => Target::Response(s[9..].parse().map_err(|_| "bad mapping")?),
            _ => Target::Ignore,
        };
        targets.push(t);
    }
    if replace {
        p.runs.clear();
    }
    for row in data {
        let ri = sheet::new_run(p, "imported");
        let mut has_resp = false;
        let mut status: Option<RunStatus> = None;
        for (j, t) in targets.iter().enumerate() {
            let Some(v) = row.get(j).map(|s| s.trim()) else { continue };
            if v.is_empty() {
                continue;
            }
            match t {
                Target::Ignore => {}
                Target::Factor(id) => {
                    let Some(fi) = p.factor_index(*id) else { continue };
                    let f = p.factors[fi].clone();
                    let val = match &f.kind {
                        FactorKind::Categorical { levels } => match f.level_index(v) {
                            Some(i) => Some(Value::Text(levels[i].clone())),
                            None if f.role == Role::Environmental => {
                                let mut lv = levels.clone();
                                lv.push(v.to_string());
                                p.factors[fi].kind = FactorKind::Categorical { levels: lv };
                                Some(Value::Text(v.to_string()))
                            }
                            None => None,
                        },
                        _ => format::parse_num(v).map(Value::Num),
                    };
                    match val {
                        Some(val) => {
                            p.runs[ri].settings.insert(*id, val);
                        }
                        // Marks such as "n/a" in a conditional factor's column.
                        None if f.condition.is_some() => {}
                        None => rep.skipped_cells += 1,
                    }
                }
                Target::Response(id) => match format::parse_num(v) {
                    Some(x) => {
                        p.runs[ri].responses.insert(*id, x);
                        has_resp = true;
                    }
                    None => rep.skipped_cells += 1,
                },
                Target::Run => {
                    if let Some(x) = format::parse_num(v) {
                        p.runs[ri].run_order = x as u32;
                    }
                }
                Target::Std => {
                    if let Some(x) = format::parse_num(v) {
                        p.runs[ri].std_order = x as u32;
                    }
                }
                Target::Status => status = RunStatus::from_label(v),
                Target::Note => p.runs[ri].note = v.to_string(),
            }
        }
        p.runs[ri].status = status.unwrap_or(if has_resp { RunStatus::Done } else { RunStatus::Planned });
        rep.runs += 1;
    }
    // Widens continuous ranges to cover imported values.
    for fi in 0..p.factors.len() {
        let id = p.factors[fi].id;
        if let FactorKind::Continuous { low, high } = p.factors[fi].kind {
            let vals: Vec<f64> = p.runs.iter().filter_map(|r| r.settings.get(&id).and_then(|v| v.as_num())).collect();
            let lo = vals.iter().cloned().fold(low, f64::min);
            let hi = vals.iter().cloned().fold(high, f64::max);
            if lo < low || hi > high {
                if p.factors[fi].role == Role::Environmental {
                    p.factors[fi].kind = FactorKind::Continuous { low: lo, high: hi };
                } else {
                    rep.messages.push(format!("Some values of {} lie outside its range.", p.factors[fi].name));
                }
            }
        }
    }
    if rep.skipped_cells > 0 {
        rep.messages.push(format!("{} cells could not be read and were left empty.", rep.skipped_cells));
    }
    Ok(rep)
}
