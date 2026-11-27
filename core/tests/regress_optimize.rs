mod common;
use common::*;
use sdoe::Engine;
use serde_json::{Value, json};

fn num(v: &Value) -> f64 {
    match v {
        Value::String(s) => s.parse().unwrap_or_else(|_| panic!("not a number: {s}")),
        other => other.as_f64().unwrap_or_else(|| panic!("not a number: {other}")),
    }
}

fn factor_by_id(e: &mut Engine, id: &str) -> Value {
    let list = call(e, "factors.list", json!({}));
    list.as_array().unwrap().iter().find(|f| f["id"] == json!(id)).unwrap_or_else(|| panic!("no factor {id}")).clone()
}

/// Mixture components of a solution's `values` map must add up to the total and respect bounds.
fn check_mixture(e: &mut Engine, solutions: &Value, what: &str) {
    let total = num(&call(e, "project.state", json!({}))["mixture_total"]);
    let comps: Vec<Value> = call(e, "factors.list", json!({})).as_array().unwrap().iter().filter(|f| f["kind"] == json!("Mixture")).cloned().collect();
    let sols = solutions.as_array().unwrap();
    assert!(!sols.is_empty(), "{what}: no solutions");
    for s in sols {
        let mut sum = 0.0;
        for c in &comps {
            let v = num(&s["values"][c["id"].as_str().unwrap()]);
            let (lo, hi) = (c["low"].as_f64().unwrap(), c["high"].as_f64().unwrap());
            assert!(v >= lo - 1e-6 && v <= hi + 1e-6, "{what}: component {} = {v} outside [{lo}, {hi}]", c["name"]);
            sum += v;
        }
        // Values arrive as text with 5 significant digits, so the sum can be off by display rounding.
        assert!((sum - total).abs() <= 1e-4 * total, "{what}: components sum to {sum}, mixture total is {total}: {s}");
    }
}

/// A fixed concrete component is left out of the projection onto the mixture total, so the blend adds up to 420 instead of 400.
#[test]
fn fixed_mixture_component_keeps_concrete_total() {
    let mut e = engine_with_example("mfg-concrete-binder-mixture.sdoe");
    let r = call(&mut e, "optimize.run", json!({"fixed": {"4": "20"}}));
    check_mixture(&mut e, &r["solutions"], "concrete with silica fume fixed at 20");
}

/// A fixed tablet component is left out of the projection onto the mixture total, so the blend adds up to 88 instead of 85.
#[test]
fn fixed_mixture_component_keeps_tablet_total() {
    let mut e = engine_with_example("bio-tablet-formulation.sdoe");
    let r = call(&mut e, "optimize.run", json!({"fixed": {"1": "75"}}));
    check_mixture(&mut e, &r["solutions"], "tablet with cellulose fixed at 75");
}

/// Discrete factors are snapped after the simplex search without re-optimising the continuous factors, so the free optimum is worse than a run with one level fixed.
#[test]
fn free_optimum_is_not_worse_than_any_fixed_discrete_level() {
    let mut e = engine_with_example("field-ml-hyperparameter-lhs.sdoe");
    let free = num(&call(&mut e, "optimize.run", json!({}))["solutions"][0]["d"]);
    let levels: Vec<String> = factor_by_id(&mut e, "2")["levels"].as_array().unwrap().iter().map(|l| l.as_str().unwrap().to_string()).collect();
    assert!(!levels.is_empty());
    for l in levels {
        let mut e2 = engine_with_example("field-ml-hyperparameter-lhs.sdoe");
        let r = call(&mut e2, "optimize.run", json!({"fixed": {"2": l}}));
        let d = num(&r["solutions"][0]["d"]);
        assert!(free >= d - 1e-6, "free optimum D = {free} is below D = {d} with batch size fixed at {l}");
    }
}

/// The profiler predicts at block level 0 only, so its response differs from the average over the blocks.
#[test]
fn profile_prediction_averages_over_block_levels() {
    let mut e = engine_with_example("bio-bread-ccd.sdoe");
    let pt = json!({"1": "62", "2": "9", "3": "55", "4": "Lot 24-117"});
    let prof = call(&mut e, "optimize.profile", json!({"values": pt}));
    let got = num(&prof["values"][0]);
    let blocks = factor_by_id(&mut e, "8")["levels"].as_array().unwrap().clone();
    let mut preds = vec![];
    for b in &blocks {
        let mut v = pt.clone();
        v["8"] = b.clone();
        preds.push(num(&call(&mut e, "analysis.predict", json!({"response": "5", "values": v}))["y"]));
    }
    assert!((preds[0] - preds[1]).abs() > 1.0, "test premise: block predictions differ: {preds:?}");
    let mean = preds.iter().sum::<f64>() / preds.len() as f64;
    // The profile value arrives as text with 5 significant digits.
    assert!((got - mean).abs() <= 1e-4 * mean.abs(), "profile predicts {got}, average over blocks is {mean} (per block {preds:?})");
}

fn check_plot_within_bounds(e: &mut Engine, plot: &Value, what: &str) {
    if let Some(id) = plot["note"].as_str() {
        let f = factor_by_id_opt(e, id);
        if let Some(f) = f {
            let (lo, hi) = (f["low"].as_f64().unwrap(), f["high"].as_f64().unwrap());
            for s in plot["series"].as_array().unwrap() {
                for x in s["x"].as_array().unwrap() {
                    let x = x.as_f64().unwrap();
                    assert!(x >= lo - 1e-9 && x <= hi + 1e-9, "{what}: {} sweep x = {x} outside [{lo}, {hi}]", f["name"]);
                }
            }
        }
    }
}

fn factor_by_id_opt(e: &mut Engine, id: &str) -> Option<Value> {
    call(e, "factors.list", json!({})).as_array().unwrap().iter().find(|f| f["id"] == json!(id)).cloned()
}

/// The mixture contour grid runs past the component's upper bound, giving y1 = 12 for a factor limited to 8.
#[test]
fn mixture_contour_stays_within_factor_bounds() {
    let mut e = engine_with_example("bio-tablet-formulation.sdoe");
    let rid = call(&mut e, "responses.list", json!({}))[0]["id"].as_str().unwrap().to_string();
    for (x, y) in [("1", "2"), ("2", "3"), ("3", "2")] {
        let p = call(&mut e, "analysis.plot", json!({"response": rid, "kind": "contour", "x": x, "y": y, "values": {}}));
        let h = &p["heatmap"];
        for (axis, id, lo_k, hi_k) in [("x", x, "x0", "x1"), ("y", y, "y0", "y1")] {
            let f = factor_by_id(&mut e, id);
            let (lo, hi) = (f["low"].as_f64().unwrap(), f["high"].as_f64().unwrap());
            let (a, b) = (num(&h[lo_k]), num(&h[hi_k]));
            assert!(a >= lo - 1e-9 && b <= hi + 1e-9, "{axis} axis {} (x={x}, y={y}): heatmap spans [{a}, {b}], factor range is [{lo}, {hi}]", f["name"]);
        }
    }
}

/// The mixture profiler sweeps each component beyond its upper bound.
#[test]
fn mixture_profiler_sweep_stays_within_factor_bounds() {
    let mut e = engine_with_example("bio-tablet-formulation.sdoe");
    let rid = call(&mut e, "responses.list", json!({}))[0]["id"].as_str().unwrap().to_string();
    let p = call(&mut e, "analysis.plot", json!({"response": rid, "kind": "profiler", "values": {}}));
    for plot in p["plots"].as_array().unwrap() {
        check_plot_within_bounds(&mut e, plot, "analysis profiler");
    }
    let prof = call(&mut e, "optimize.profile", json!({"values": {"1": "75", "2": "5", "3": "5", "4": "14"}}));
    for row in prof["rows"].as_array().unwrap() {
        for plot in row.as_array().unwrap() {
            check_plot_within_bounds(&mut e, plot, "optimize profiler");
        }
    }
}

fn zero_response_project(second_group: f64) -> Value {
    let mut p = project(vec![cont(1, 0.0, 10.0)]);
    p["responses"] = json!([{"id": 5, "name": "Y", "units": "", "goal": "maximize", "lower": null, "target": null, "upper": null, "weight": 1.0, "importance": 3.0}]);
    let mut runs = vec![];
    for (i, (x, y)) in [(0.0, 0.0), (0.0, 0.0), (10.0, second_group), (10.0, second_group)].into_iter().enumerate() {
        runs.push(json!({"id": 10 + i, "std_order": i + 1, "run_order": i + 1, "point_type": "factorial", "status": "done",
            "settings": {"1": x}, "responses": {"5": y}, "note": ""}));
    }
    p["runs"] = json!(runs);
    p
}

/// The nominal S/N analysis panics on NaN when control groups have all-zero responses (partial_cmp unwrap).
#[test]
fn robustness_nominal_with_all_zero_groups_does_not_panic() {
    let mut e = engine_with_project(&zero_response_project(0.0));
    let out = reply(&mut e, "analysis.robustness", json!({"response": "5", "sn": "nominal"}));
    assert!(out["ok"].is_boolean(), "reply is neither a result nor an error: {out}");
}

/// A maximise goal with lower limit 10 above upper limit 0 is accepted silently and acts as a step function.
#[test]
fn desirability_limits_with_lower_above_upper_are_not_accepted_silently() {
    let mut e = engine_with_example("bio-bread-ccd.sdoe");
    let a = reply(&mut e, "sheet.set", json!({"sheet": "responses", "row": 0, "col": 3, "text": "10"}));
    let b = reply(&mut e, "sheet.set", json!({"sheet": "responses", "row": 0, "col": 5, "text": "0"}));
    if a["ok"] == json!(false) || b["ok"] == json!(false) {
        return;
    }
    let goals = call(&mut e, "optimize.get", json!({}))["goals"].clone();
    let g = &goals[0];
    if num(&g["lower"]) <= num(&g["upper"]) {
        return;
    }
    let run = reply(&mut e, "optimize.run", json!({}));
    let noted = run["result"]["notes"].as_array().is_some_and(|n| n.iter().any(|t| {
        let t = t.as_str().unwrap_or("").to_lowercase();
        t.contains("limit") || t.contains("lower") || t.contains("upper")
    }));
    assert!(run["ok"] == json!(false) || noted, "limits lower = {}, upper = {} accepted without error or note: {run}", g["lower"], g["upper"]);
}

/// A fixed value outside the factor's range is neither rejected nor clamped.
#[test]
fn fixed_value_outside_factor_range_is_rejected_or_clamped() {
    let mut e = engine_with_example("bio-bread-ccd.sdoe");
    let f = factor_by_id(&mut e, "1");
    let (lo, hi) = (f["low"].as_f64().unwrap(), f["high"].as_f64().unwrap());
    let out = reply(&mut e, "optimize.run", json!({"fixed": {"1": "100"}}));
    if out["ok"] == json!(false) {
        return;
    }
    let sols = out["result"]["solutions"].as_array().unwrap();
    assert!(!sols.is_empty(), "no solutions: {out}");
    for s in sols {
        let v = num(&s["values"]["1"]);
        assert!(v >= lo && v <= hi, "fixed value 100 returned as {v}, factor range is [{lo}, {hi}]");
    }
}

/// The adjustment law listed formulas for held factors, which its table leaves out.
#[test]
fn adjustment_law_leaves_out_held_factors() {
    let mut e = engine_with_example("field-heat-pump-heating-curve.sdoe");
    call(&mut e, "optimize.set", json!({"fixed": {"2": "70"}, "env": {}}));
    let law = call(&mut e, "optimize.control_law", json!({"factor": "4"}));
    let formulas: Vec<&str> = law["formulas"].as_array().unwrap().iter().map(|f| f.as_str().unwrap()).collect();
    assert!(!formulas.iter().any(|f| f.starts_with("Fan_speed")), "formulas for held Fan speed: {formulas:?}");
    let cols = law["table"]["columns"].as_array().unwrap();
    assert!(!cols.iter().any(|c| c == "Fan speed"), "table columns {cols:?}");
    assert_eq!(formulas.len(), law["plots"].as_array().unwrap().len());
}
