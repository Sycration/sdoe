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

/// Effects of the fit of `p` for response `RID`, by label.
fn effects_of(p: &Value) -> Vec<(String, f64)> {
    let proj: sdoe::project::Project = serde_json::from_value(p.clone()).unwrap();
    let fit = sdoe::results::fit_for(&proj, RID).unwrap();
    sdoe::model::effects::screen(&fit, 0.05).effects.into_iter().map(|e| (e.label, e.effect)).collect()
}

fn effect(effects: &[(String, f64)], label: &str) -> f64 {
    effects.iter().find(|e| e.0 == label).unwrap_or_else(|| panic!("no effect {label} in {effects:?}")).1
}

/// The screening plots showed a two-level categorical effect at its coefficient (half the change
/// between the levels) and a square at 2 x its coefficient (twice the change from 0 to 1).
#[test]
fn effects_are_the_change_between_the_extremes_of_each_column() {
    let mut p = project(vec![cont(1, -1.0, 1.0), cat(2, &["lo", "hi"])]);
    p["responses"] = json!([response()]);
    // y = 10 + 3 A + 4 [B = hi], replicated with small deviations.
    let data = [(-1., "lo", 7.0), (1., "lo", 13.1), (-1., "hi", 11.0), (1., "hi", 16.9), (-1., "lo", 7.1), (1., "lo", 12.9), (-1., "hi", 10.9), (1., "hi", 17.0)];
    let runs: Vec<Value> = data
        .iter()
        .enumerate()
        .map(|(i, (a, b, y))| {
            json!({"id": 200 + i, "std_order": i + 1, "run_order": i + 1, "point_type": "", "status": "done",
                   "settings": {"1": a, "2": b}, "responses": {RID.to_string(): y}, "note": ""})
        })
        .collect();
    p["runs"] = json!(runs);
    let mut e = engine_with_project(&p);
    set_model(&mut e, "A + B", None);
    let saved: Value = serde_json::from_str(call(&mut e, "project.save", json!({}))["text"].as_str().unwrap()).unwrap();
    let eff = effects_of(&saved);
    assert!((effect(&eff, "A") - 6.0).abs() < 0.1, "effect of A: {eff:?}");
    assert!((effect(&eff, "B[lo]").abs() - 4.0).abs() < 0.1, "effect of B: {eff:?}");

    // y = 5 + 2 A^2: the square goes from 0 to 1, so its effect is 2.
    let d = [(-1., 7.0), (0., 5.0), (1., 7.0), (-1., 7.05), (0., 4.95), (1., 7.0)].map(|(a, y)| (vec![a], y, "done"));
    let mut e = engine_with_project(&build(1, &d));
    set_model(&mut e, "A + A^2", None);
    let saved: Value = serde_json::from_str(call(&mut e, "project.save", json!({}))["text"].as_str().unwrap()).unwrap();
    let eff = effects_of(&saved);
    assert!((effect(&eff, "A^2") - 2.0).abs() < 0.1, "effect of A^2: {eff:?}");
}

/// Positive responses with curvature, for the transform tests.
fn curved_positive() -> Value {
    let pts: Vec<(Vec<f64>, f64, &str)> = full_factorial(2)
        .into_iter()
        .chain([vec![0.0, 0.0], vec![0.0, 0.0], vec![1.0, 0.0], vec![-1.0, 0.0], vec![0.0, 1.0], vec![0.0, -1.0]])
        .enumerate()
        .map(|(i, x)| {
            let y = (1.0 + 0.8 * x[0] + 0.3 * x[1] + 0.2 * x[0] * x[1]).exp() * (1.0 + 0.03 * ((i * 7 % 5) as f64 - 2.0));
            (x, y, "done")
        })
        .collect();
    build(2, &pts)
}

/// With a transform, the Analysis page reported predicted R² only on the transformed scale,
/// while the automatic search ranked the same model by predicted R² on the original scale.
#[test]
fn transformed_fit_reports_predicted_r2_on_the_original_scale_as_the_search_does() {
    let mut e = engine_with_project(&curved_positive());
    let auto = call(&mut e, "model.auto", json!({"response": "100"}));
    // model.auto sets the best candidate (first row) as the model.
    let table = &auto["table"];
    let best = &rows(table)[0];
    assert_ne!(best[0], "None", "the test data call for a transform: {table}");
    let ranked = &best[col(table, "Predicted R² (original scale)")];
    let fit = call(&mut e, "analysis.fit", json!({"response": "100"}));
    let shown = stat(&fit, "Predicted R² (original scale)");
    assert_eq!(shown, *ranked, "Analysis page vs search");
    set_model(&mut e, auto["formula"].as_str().unwrap(), Some("None"));
    let fit = call(&mut e, "analysis.fit", json!({"response": "100"}));
    assert!(!fit["stats"].as_array().unwrap().iter().any(|s| s[0] == "Predicted R² (original scale)"), "untransformed fit lists an original-scale value");
}

/// Stepwise selection starts from the main effects. With y = A²·B, neither A², A·B, A nor B
/// alone lowers AICc, so the main effects were removed and A²·B, whose lower-order terms were
/// then missing, could never enter: the selection removed every term.
#[test]
fn stepwise_can_add_a_term_together_with_its_missing_lower_order_terms() {
    let noise = [0.12, -0.08, 0.05, -0.11, 0.09, -0.04, 0.07, -0.1, 0.02, 0.03, -0.06, 0.08, -0.02, 0.04, -0.09, 0.1, -0.05, 0.01];
    let d: Vec<(Vec<f64>, f64, &str)> = (0..18)
        .map(|i| {
            let (a, b) = ((i % 3) as f64 - 1.0, ((i / 3) % 3) as f64 - 1.0);
            (vec![a, b], 10.0 + 4.0 * a * a * b + noise[i], "done")
        })
        .collect();
    let mut e = engine_with_project(&build(2, &d));
    set_model(&mut e, "A + B + A*B + A^2 + A^2*B", None);
    let r = reply(&mut e, "model.select", json!({"response": "100", "method": "aicc", "pool": "current"}));
    assert_eq!(r["ok"], json!(true), "stepwise failed: {r}");
    let terms = call(&mut e, "model.get", json!({"response": "100"}))["terms"].clone();
    assert!(terms.as_array().unwrap().iter().any(|t| t == "A^2*B"), "stepwise result {terms} lacks A^2*B; steps {}", r["result"]["steps"]);
}

/// The transform list offered no power transforms, although the Box-Cox plot recommends them.
#[test]
fn transform_list_offers_every_box_cox_recommendation() {
    let mut e = engine_with_project(&curved_positive());
    let list: Vec<String> = call(&mut e, "model.get", json!({"response": "100"}))["transforms"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_string()).collect();
    for l in [1.0, 0.5, 0.0, -0.5, -1.0, 2.0, -2.0, 3.0, -3.0] {
        let label = sdoe::model::effects::lambda_transform(l).label();
        assert!(list.contains(&label), "{label} missing from {list:?}");
    }
    for t in &list {
        set_model(&mut e, "A + B", Some(t));
        assert_eq!(call(&mut e, "model.get", json!({"response": "100"}))["transform"], json!(t));
    }
}

/// The outlier warning used |ext. studentized residual| > 3.5, the residual plots drew lines at ±3.
#[test]
fn residual_plot_limits_match_the_outlier_warning() {
    let mut d: Vec<(Vec<f64>, f64, &str)> = (0..12).map(|i| (vec![-1.0 + 2.0 * i as f64 / 11.0], 3.0 + (i * 5 % 7) as f64 * 0.1, "done")).collect();
    d[5].1 += 40.0;
    let p = build(1, &d);
    let mut e = engine_with_project(&p);
    set_model(&mut e, "A", None);
    let fit = call(&mut e, "analysis.fit", json!({"response": "100"}));
    let warning = fit["warnings"].as_array().unwrap().iter().map(|w| w.as_str().unwrap()).find(|w| w.starts_with("Possible outliers")).expect("outlier warning").to_string();
    let limit: f64 = warning.split("> ").nth(1).unwrap().split(')').next().unwrap().parse().unwrap();
    let proj: sdoe::project::Project = serde_json::from_value(p).unwrap();
    let f = sdoe::results::fit_for(&proj, RID).unwrap();
    for plot in [sdoe::results::plot_resid_order(&proj, &f), sdoe::results::plot_resid_fitted(&proj, &f)] {
        let lines: Vec<f64> = plot.series.iter().filter(|s| s.kind == "hline" && s.dashed).flat_map(|s| s.y.clone()).collect();
        assert_eq!(lines, vec![-limit, limit], "{}: dashed lines vs warning limit {limit}", plot.title);
    }
}

/// Python equations of Ln models called math.exp without importing math.
#[test]
fn python_equation_imports_math_and_comments_conditions() {
    let mut e = engine_with_project(&curved_positive());
    set_model(&mut e, "A + B", Some("Ln"));
    let text = call(&mut e, "analysis.equation", json!({"response": "100", "syntax": "python"}))["text"].as_str().unwrap().to_string();
    assert!(text.contains("math."), "{text}");
    assert!(text.lines().next() == Some("import math"), "{text}");

    let mut p = project(vec![cont(1, -1.0, 1.0), cat(2, &["lo", "hi"])]);
    p["responses"] = json!([response()]);
    let runs: Vec<Value> = [(-1., "lo", 7.0), (1., "lo", 13.1), (-1., "hi", 11.0), (1., "hi", 16.9), (0., "lo", 10.2)]
        .iter()
        .enumerate()
        .map(|(i, (a, b, y))| json!({"id": 200 + i, "std_order": i + 1, "run_order": i + 1, "point_type": "", "status": "done", "settings": {"1": a, "2": b}, "responses": {RID.to_string(): y}, "note": ""}))
        .collect();
    p["runs"] = json!(runs);
    let mut e = engine_with_project(&p);
    set_model(&mut e, "A + B", None);
    let text = call(&mut e, "analysis.equation", json!({"response": "100", "syntax": "python", "units": "actual"}))["text"].as_str().unwrap().to_string();
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        assert!(line.starts_with('#') || line.contains(" = "), "line {line:?} is not Python in:\n{text}");
    }
}

/// Backward elimination of a saturated model logged "Remove X ()".
#[test]
fn saturated_backward_elimination_log_has_no_empty_criterion() {
    let ys: Vec<f64> = full_factorial(3).iter().enumerate().map(|(i, x)| 10.0 + 5.0 * x[0] + 0.01 * (i * 3 % 4) as f64).collect();
    let mut e = engine_with_project(&factorial_with(&ys, 3));
    set_model(&mut e, "A + B + C + A*B + A*C + B*C + A*B*C", None);
    let r = call(&mut e, "model.select", json!({"response": "100", "method": "backward", "alpha": 0.1, "pool": "current"}));
    for s in r["steps"].as_array().unwrap() {
        assert!(!s.as_str().unwrap().contains("()"), "step {s}");
    }
}
