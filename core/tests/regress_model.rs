mod common;
use common::*;
use serde_json::{Value, json};

const RID: u64 = 100;

fn response() -> Value {
    json!({"id": RID, "name": "Y", "units": "", "goal": "none", "lower": null, "target": null, "upper": null, "weight": 1.0, "importance": 3.0})
}

/// Project with `nf` continuous factors on -1..1 and one run per row of `data` (settings, y, status).
fn build(nf: u64, data: &[(Vec<f64>, f64, &str)]) -> Value {
    let mut p = project((1..=nf).map(|i| cont(i, -1.0, 1.0)).collect());
    p["responses"] = json!([response()]);
    let runs: Vec<Value> = data
        .iter()
        .enumerate()
        .map(|(i, (x, y, st))| {
            let mut settings = serde_json::Map::new();
            for (k, v) in x.iter().enumerate() {
                settings.insert((k as u64 + 1).to_string(), json!(v));
            }
            json!({"id": 200 + i, "std_order": i + 1, "run_order": i + 1, "point_type": "", "status": st,
                   "settings": settings, "responses": {RID.to_string(): y}, "note": ""})
        })
        .collect();
    p["runs"] = json!(runs);
    p["next_id"] = json!(1000);
    p
}

fn full_factorial(nf: usize) -> Vec<Vec<f64>> {
    (0..1usize << nf).map(|r| (0..nf).map(|k| if (r >> k) & 1 == 1 { 1.0 } else { -1.0 }).collect()).collect()
}

fn set_model(e: &mut sdoe::Engine, formula: &str, transform: Option<&str>) {
    let mut p = json!({"response": "100", "formula": formula});
    if let Some(t) = transform {
        p["transform"] = json!(t);
    }
    call(e, "model.set", p);
}

fn rows(table: &Value) -> Vec<Vec<String>> {
    table["rows"].as_array().unwrap().iter().map(|r| r.as_array().unwrap().iter().map(|c| c.as_str().unwrap().to_string()).collect()).collect()
}

fn col(table: &Value, name: &str) -> usize {
    table["columns"].as_array().unwrap().iter().position(|c| c == name).unwrap_or_else(|| panic!("no column {name} in {table}"))
}

/// Finite number in a text cell, or None for NaN, blank, dash and non-numeric text.
fn finite(cell: &str) -> Option<f64> {
    cell.trim().parse::<f64>().ok().filter(|v| v.is_finite())
}

/// True when a p-value cell shows zero (a number equal to 0, or a "<" bound).
fn shows_zero_p(cell: &str) -> bool {
    finite(cell) == Some(0.0) || cell.trim().starts_with('<')
}

fn emphasis_of(table: &Value, row: usize) -> String {
    table["emphasis"].get(row).and_then(|v| v.as_str()).unwrap_or("").to_string()
}

fn stat(fit: &Value, label: &str) -> String {
    fit["stats"].as_array().unwrap().iter().find(|s| s[0] == label).unwrap_or_else(|| panic!("no stat {label}"))[1].as_str().unwrap().to_string()
}

fn has_row(table: &Value, label: &str) -> bool {
    rows(table).iter().any(|r| r[0] == label)
}

fn factorial_with(ys: &[f64], nf: usize) -> Value {
    let d: Vec<_> = full_factorial(nf).into_iter().zip(ys.iter().copied()).map(|(x, y)| (x, y, "done")).collect();
    build(nf as u64, &d)
}

/// The fit groups runs by model-matrix row, so an unreplicated design gets a pure-error row and a lack-of-fit test.
#[test]
fn unreplicated_design_has_no_lack_of_fit_or_pure_error() {
    let mut e = engine_with_project(&factorial_with(&[10., 12., 11., 14., 30., 31., 29., 33.], 3));
    set_model(&mut e, "A+B", None);
    let fit = call(&mut e, "analysis.fit", json!({"response": "100"}));
    let anova = &fit["anova"];
    assert!(!has_row(anova, "Pure error"), "unreplicated design has no true replicates, expected no Pure error row, got {anova}");
    assert!(!has_row(anova, "Lack of fit"), "unreplicated design, expected no Lack of fit row, got {anova}");
}

/// Guard: a design with genuine replicated points keeps a lack-of-fit row with the pure-error SS computed over the replicates.
#[test]
fn replicated_design_reports_lack_of_fit_with_correct_pure_error() {
    // 2^2 run twice; pairs (1,3), (5,5), (2,4), (8,6): pure error SS = 2 + 0 + 2 + 2 = 6 on 4 df.
    let ys = [1., 5., 2., 8., 3., 5., 4., 6.];
    let d: Vec<_> = full_factorial(2).into_iter().cycle().take(8).zip(ys).map(|(x, y)| (x, y, "done")).collect();
    let mut e = engine_with_project(&build(2, &d));
    set_model(&mut e, "A+B", None);
    let fit = call(&mut e, "analysis.fit", json!({"response": "100"}));
    let anova = &fit["anova"];
    let r = rows(anova);
    let pe = r.iter().find(|r| r[0] == "Pure error").unwrap_or_else(|| panic!("expected a Pure error row, got {anova}"));
    let ss: f64 = pe[col(anova, "Sum of squares")].parse().unwrap();
    assert!((ss - 6.0).abs() < 1e-6, "pure error SS: actual {ss}, expected 6");
    assert_eq!(pe[col(anova, "df")], "4", "pure error df");
    assert!(has_row(anova, "Lack of fit"), "expected a Lack of fit row, got {anova}");
}

/// A saturated fit (zero residual df) reports p = 0 for every coefficient and marks the rows significant.
#[test]
fn saturated_fit_does_not_report_zero_p_values() {
    let mut e = engine_with_project(&factorial_with(&[3., 7., 4., 12.], 2));
    set_model(&mut e, "A + B + A*B", None);
    let fit = call(&mut e, "analysis.fit", json!({"response": "100"}));
    let t = &fit["coefficients"];
    let pc = col(t, "p");
    for (i, r) in rows(t).iter().enumerate() {
        assert!(!shows_zero_p(&r[pc]), "term {}: p is {:?}, expected NaN or not reported when residual df is 0", r[0], r[pc]);
        assert_ne!(emphasis_of(t, i), "good", "term {} is marked significant with zero residual df: {t}", r[0]);
    }
}

/// A constant response gives zero coefficients a p-value of 0 and marks them significant.
#[test]
fn constant_response_does_not_report_zero_p_for_zero_coefficients() {
    let mut e = engine_with_project(&factorial_with(&[5., 5., 5., 5.], 2));
    set_model(&mut e, "A + B", None);
    let fit = call(&mut e, "analysis.fit", json!({"response": "100"}));
    let t = &fit["coefficients"];
    let pc = col(t, "p");
    for (i, r) in rows(t).iter().enumerate().filter(|(_, r)| r[0] != "Intercept") {
        assert!(!shows_zero_p(&r[pc]), "term {}: coefficient {} has p {:?}, expected NaN or 1", r[0], r[1], r[pc]);
        assert_ne!(emphasis_of(t, i), "good", "term {} is marked significant for a constant response: {t}", r[0]);
    }
}

/// BIC is clamped through `sse.max(1e-300)`, so a perfect saturated fit reports a large finite negative BIC.
#[test]
fn saturated_fit_does_not_report_finite_bic() {
    let mut e = engine_with_project(&factorial_with(&[3., 7., 4., 12.], 2));
    set_model(&mut e, "A + B + A*B", None);
    let fit = call(&mut e, "analysis.fit", json!({"response": "100"}));
    let bic = stat(&fit, "BIC");
    assert!(finite(&bic).is_none(), "BIC with zero residual df is {bic:?}, expected NaN or not reported");
}

/// Back-transformed intervals under the Inverse transform exclude the point prediction (the PI is negative below the estimate).
#[test]
fn inverse_transform_intervals_contain_the_point_prediction() {
    let d = [(-1., 1.), (-0.5, 2.), (0., 4.), (0.5, 3.), (1., 6.), (0.2, 2.5)].map(|(a, y)| (vec![a], y, "done"));
    let mut e = engine_with_project(&build(1, &d));
    set_model(&mut e, "A", Some("Inverse"));
    for a in [-0.5, 0.0, 0.5] {
        let p = call(&mut e, "analysis.predict", json!({"response": "100", "values": {"1": a}}));
        let y: f64 = p["y"].as_str().unwrap().parse().unwrap();
        for k in ["ci", "pi"] {
            let lo = p[k][0].as_str().unwrap();
            let hi = p[k][1].as_str().unwrap();
            // An unbounded or undefined end (non-finite or not reported) is acceptable.
            let ok = finite(lo).is_none_or(|l| l <= y) && finite(hi).is_none_or(|h| y <= h);
            assert!(ok, "A={a}: {k} ({lo}, {hi}) does not contain the prediction {y}");
        }
    }
}

/// model.auto reads excluded runs, so one Excluded run with y <= 0 removes the Ln transform from the candidates.
#[test]
fn model_auto_ignores_excluded_runs_when_choosing_transform_candidates() {
    let mut d: Vec<_> = [(-1., 1.), (-0.5, 2.), (0., 4.), (0.5, 3.), (1., 6.), (0.2, 2.5)].map(|(a, y)| (vec![a], y, "done")).to_vec();
    d.push((vec![0.8], -5.0, "excluded"));
    let mut e = engine_with_project(&build(1, &d));
    let auto = call(&mut e, "model.auto", json!({"response": "100"}));
    let names: Vec<String> = rows(&auto["table"]).into_iter().map(|r| r[0].clone()).collect();
    assert!(names.iter().any(|n| n == "Ln"), "expected an Ln candidate (all Done runs are positive), candidates: {names:?}");
}

/// The parser consumes the superscript '²' as an identifier character, so "A²" is rejected although the docs define it as a power.
#[test]
fn superscript_two_parses_as_square() {
    let d = [(-1., 1.), (-0.5, 2.), (0., 4.), (0.5, 3.), (1., 6.), (0.2, 2.5)].map(|(a, y)| (vec![a], y, "done"));
    let mut e = engine_with_project(&build(1, &d));
    let r = reply(&mut e, "model.set", json!({"response": "100", "formula": "A + A²"}));
    assert_eq!(r["ok"], json!(true), "model.set with \"A + A²\" failed: {r}");
    let sup = call(&mut e, "model.get", json!({"response": "100"}))["terms"].clone();
    set_model(&mut e, "A + A^2", None);
    let caret = call(&mut e, "model.get", json!({"response": "100"}))["terms"].clone();
    assert_eq!(sup, caret, "terms for A² vs A^2");
}
