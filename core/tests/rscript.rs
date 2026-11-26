//! Runs the R export through R (when `Rscript` is installed) and checks that
//! R reproduces every statistic of every fitted response.

use sdoe::Engine;
use serde_json::{Value, json};
use std::path::Path;
use std::process::Command;

fn reply(e: &mut Engine, m: &str, p: Value) -> Value {
    serde_json::from_str(&e.call(m, &p.to_string())).unwrap()
}

fn call(e: &mut Engine, m: &str, p: Value) -> Value {
    let out = reply(e, m, p);
    assert_eq!(out["ok"], json!(true), "{m} failed: {out}");
    out["result"].clone()
}

fn set(e: &mut Engine, sheet: &str, row: usize, col: usize, text: &str) {
    call(e, "sheet.set", json!({"sheet": sheet, "row": row, "col": col, "text": text}));
}

fn rscript() -> bool {
    let ok = Command::new("Rscript").arg("--version").output().is_ok_and(|o| o.status.success());
    if !ok {
        eprintln!("Rscript not found; R export checks skipped");
    }
    ok
}

/// Exports the project, runs the script and returns its text after checking
/// that every response sdoe can fit agrees with R.
fn check_in_r(e: &mut Engine, name: &str) -> String {
    let ids: Vec<String> =
        call(e, "responses.list", json!({})).as_array().unwrap().iter().map(|r| r["id"].as_str().unwrap().to_string()).collect();
    let fitted = ids.iter().filter(|id| reply(e, "analysis.fit", json!({"response": id}))["ok"] == json!(true)).count();
    let text = call(e, "export.r", json!({}))["text"].as_str().unwrap().to_string();
    let path = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("{name}.R"));
    std::fs::write(&path, &text).unwrap();
    let out = Command::new("Rscript").arg(&path).output().unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "{name}: R failed ({})\n{stderr}", path.display());
    assert!(!stdout.contains("differ from sdoe"), "{name}: {}\n{stdout}", path.display());
    let agree = stdout.lines().filter(|l| l.contains("values agree with sdoe")).count();
    assert_eq!(agree, fitted, "{name}: {}\n{stdout}", path.display());
    text
}

/// Sets every response of every run from `f(row, factor cell texts)`;
/// None leaves the value empty.
fn fill(e: &mut Engine, f: impl Fn(usize, &[String]) -> Vec<Option<f64>>) {
    let nf = call(e, "factors.list", json!({})).as_array().unwrap().len();
    let sheet = call(e, "sheet.get", json!({"sheet": "runs"}));
    let rows = sheet["rows"].as_array().unwrap();
    for (i, row) in rows.iter().enumerate().take(rows.len() - 1) {
        let cells: Vec<String> = (0..nf).map(|j| row[4 + j]["text"].as_str().unwrap().to_string()).collect();
        for (k, v) in f(i, &cells).into_iter().enumerate() {
            if let Some(v) = v {
                set(e, "runs", i, 4 + nf + k, &v.to_string());
            }
        }
    }
}

/// Deterministic noise in [-0.5, 0.5).
fn noise(i: usize, k: usize) -> f64 {
    (((i * 7919 + k * 104729) % 1000) as f64) / 1000.0 - 0.5
}

#[test]
fn examples_agree_with_r() {
    if !rscript() {
        return;
    }
    for name in ["reactor", "blend"] {
        let mut e = Engine::new();
        call(&mut e, "project.example", json!({"name": name}));
        check_in_r(&mut e, name);
    }
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../examples");
    let mut files: Vec<_> = std::fs::read_dir(dir).unwrap().map(|f| f.unwrap().path()).collect();
    files.sort();
    for path in files.iter().filter(|p| p.extension().is_some_and(|x| x == "sdoe")) {
        let mut e = Engine::new();
        call(&mut e, "project.open", json!({"text": std::fs::read_to_string(path).unwrap()}));
        check_in_r(&mut e, path.file_stem().unwrap().to_str().unwrap());
    }
}

/// Chained conditions, a 3-level categorical factor, discrete powers, an
/// environmental factor with a missing value, an excluded run, every
/// transform and non-hierarchical models.
#[test]
fn conditions_and_transforms_agree_with_r() {
    if !rscript() {
        return;
    }
    let mut e = Engine::new();
    let factors = [
        ("T", "Temperature", "Continuous", "-20", "20", "", ""),
        ("M", "Thawing", "Categorical", "", "", "Slow, Fast", "T < 0"),
        ("Z", "Zone", "Categorical", "", "", "Low, Mid, High", "M = Fast"),
        ("N", "Passes", "Discrete", "", "", "1, 2, 3, 4", ""),
        ("E", "Humidity", "Continuous", "0", "10", "", ""),
    ];
    for (i, (sym, name, kind, lo, hi, levels, cond)) in factors.iter().enumerate() {
        set(&mut e, "factors", i, 1, name);
        set(&mut e, "factors", i, 0, sym);
        set(&mut e, "factors", i, 3, kind);
        if !lo.is_empty() {
            set(&mut e, "factors", i, 6, hi);
            set(&mut e, "factors", i, 5, lo);
        }
        if !levels.is_empty() {
            set(&mut e, "factors", i, 7, levels);
        }
        if !cond.is_empty() {
            set(&mut e, "factors", i, 8, cond);
        }
    }
    set(&mut e, "factors", 4, 4, "Environmental");
    let responses = ["Y", "Root", "Inverse", "Power", "Log10", "Ln"];
    for (i, r) in responses.iter().enumerate() {
        set(&mut e, "responses", i, 0, r);
    }
    call(&mut e, "design.preview", json!({"method": "optimal", "options": {"model": "quadratic", "runs": 40}}));
    call(&mut e, "design.apply", json!({"mode": "replace", "randomize": false}));
    fill(&mut e, |i, c| {
        let t: f64 = c[0].parse().unwrap();
        let n: f64 = c[3].parse().unwrap();
        let thaw = match c[1].as_str() {
            "Fast" => 3.0,
            "Slow" => -3.0,
            _ => 0.0,
        };
        let zone = match c[2].as_str() {
            "Low" => -2.0,
            "High" => 1.5,
            _ => 0.0,
        };
        let y = 40.0 + 0.2 * t + thaw + zone + 2.0 * n - 0.4 * n * n + 0.05 * t * n;
        (0..responses.len()).map(|k| Some(y + 1.5 * noise(i, k))).collect()
    });
    // Humidity: measured in every run but one.
    let runs = call(&mut e, "project.state", json!({}))["runs"].as_u64().unwrap() as usize;
    for i in 1..runs {
        set(&mut e, "runs", i, 8, &format!("{}", (i * 37 % 101) as f64 / 10.0));
    }
    call(&mut e, "runs.set_status", json!({"rows": [3], "status": "Excluded"}));
    set(&mut e, "runs", 5, 9, "");

    let ids: Vec<String> =
        call(&mut e, "responses.list", json!({})).as_array().unwrap().iter().map(|r| r["id"].as_str().unwrap().to_string()).collect();
    let models = [
        ("T + M + Z + N + E + T*N + M*N + N^2 + N^3", "none"),
        ("T*N + N^2 + Z + T*Z", "sqrt"),
        ("T + M + Z + N + E + N^2", "inverse"),
        ("T + M + Z + N + T*M + N^2", "Power 0.5"),
        ("T + Z + N + E + M*E", "log10"),
        ("T + M + Z + N + N^2", "Power -1.5"),
    ];
    for (id, (formula, tr)) in ids.iter().zip(models) {
        call(&mut e, "model.set", json!({"response": id, "formula": formula, "transform": tr}));
    }
    let text = check_in_r(&mut e, "conditions");
    assert!(text.contains("a_Z <- ifelse(a_M, is_in(d$M, c(\"Fast\")), FALSE)"), "{text}");
}

/// A 2^(4-1) fraction with a two-factor-interaction model: aliased terms
/// stay out of the R formula.
#[test]
fn aliased_terms_agree_with_r() {
    if !rscript() {
        return;
    }
    let mut e = Engine::new();
    for (i, s) in ["A", "B", "C", "D"].iter().enumerate() {
        set(&mut e, "factors", i, 1, &format!("Factor {s}"));
    }
    set(&mut e, "responses", 0, 0, "Strength (MPa)");
    call(&mut e, "design.preview", json!({"method": "fractional_factorial", "options": {"runs": "8", "center_points": 3}}));
    call(&mut e, "design.apply", json!({"mode": "replace", "randomize": false}));
    fill(&mut e, |i, c| {
        let x: Vec<f64> = c.iter().map(|v| v.parse().unwrap()).collect();
        vec![Some(10.0 + 2.0 * x[0] - x[1] + 0.5 * x[0] * x[1] + noise(i, 0))]
    });
    let id = call(&mut e, "responses.list", json!({}))[0]["id"].as_str().unwrap().to_string();
    call(&mut e, "model.preset", json!({"response": id, "preset": "2fi"}));
    let text = check_in_r(&mut e, "aliased");
    assert!(text.contains("# Left out (aliased with these runs):"), "{text}");
    assert!(text.contains("Strength_MPa ~ "), "{text}");
}

/// Mixture components with a process factor in a model without an
/// intercept.
#[test]
fn mixture_process_agrees_with_r() {
    if !rscript() {
        return;
    }
    let mut e = Engine::new();
    for (i, s) in ["A", "B", "C"].iter().enumerate() {
        set(&mut e, "factors", i, 1, &format!("Component {s}"));
        set(&mut e, "factors", i, 3, "Mixture");
        set(&mut e, "factors", i, 6, "0.8");
        set(&mut e, "factors", i, 5, "0.1");
    }
    set(&mut e, "factors", 3, 1, "Cure");
    set(&mut e, "responses", 0, 0, "Y");
    call(&mut e, "design.preview", json!({"method": "optimal", "options": {"model": "custom", "formula": "A + B + C + A*B + A*D + B*D + C*D", "runs": 16}}));
    call(&mut e, "design.apply", json!({"mode": "replace", "randomize": false}));
    fill(&mut e, |i, c| {
        let x: Vec<f64> = c.iter().map(|v| v.parse().unwrap()).collect();
        vec![Some(5.0 * x[0] + 8.0 * x[1] + 3.0 * x[2] + 6.0 * x[0] * x[1] + x[3] * (x[0] - x[2]) + 0.3 * noise(i, 0))]
    });
    let id = call(&mut e, "responses.list", json!({}))[0]["id"].as_str().unwrap().to_string();
    call(&mut e, "model.set", json!({"response": id, "formula": "A + B + C + A*B + D + A*D + B*D"}));
    check_in_r(&mut e, "mixture_process");
}
