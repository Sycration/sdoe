mod common;
use common::*;
use sdoe::Engine;
use serde_json::{Value, json};
use std::time::{Duration, Instant};

fn first_response(e: &mut sdoe::Engine) -> String {
    call(e, "responses.list", json!({}))[0]["id"].as_str().unwrap().to_string()
}

fn factor_id(e: &mut sdoe::Engine, i: usize) -> String {
    call(e, "factors.list", json!({}))[i]["id"].as_str().unwrap().to_string()
}

fn reactor() -> sdoe::Engine {
    let mut e = sdoe::Engine::new();
    call(&mut e, "project.example", json!({"name": "reactor"}));
    e
}

/// Value of the face milling example project file.
fn milling() -> Value {
    serde_json::from_str(&example_text("mfg-face-milling-ccd.sdoe")).unwrap()
}

/// Engine after `project.open` of `p`, or `None` when the engine rejects the project.
fn try_open(p: &Value) -> Option<sdoe::Engine> {
    let mut e = sdoe::Engine::new();
    let out = reply(&mut e, "project.open", json!({"text": p.to_string()}));
    (out["ok"] == json!(true)).then_some(e)
}

fn assert_replied(out: &Value, what: &str) {
    assert!(out["ok"].is_boolean(), "{what}: no reply with an ok field: {out}");
}

fn nested_parens(n: usize) -> String {
    format!("{}A{}", "(".repeat(n), ")".repeat(n))
}

// ---- 1. recursion depth of the formula parser --------------------------------

isolated_test!(model_set_formula_with_200000_nested_parentheses_replies, 20, 2048, {
    // Guards against the formula parser recursing once per parenthesis until the stack overflows.
    let mut e = reactor();
    let rid = first_response(&mut e);
    let out = reply(&mut e, "model.set", json!({"response": rid, "formula": nested_parens(200_000)}));
    assert_replied(&out, "model.set with 200000 parentheses");
});

isolated_test!(model_set_formula_with_5000_nested_parentheses_replies_on_2mb_stack, 20, 2048, {
    // Guards against the formula parser using stack space in proportion to the nesting depth on a 2 MB stack.
    let handle = std::thread::Builder::new()
        .stack_size(2 * 1024 * 1024)
        .spawn(|| {
            let mut e = reactor();
            let rid = first_response(&mut e);
            let out = reply(&mut e, "model.set", json!({"response": rid, "formula": nested_parens(5000)}));
            assert_replied(&out, "model.set with 5000 parentheses");
        })
        .unwrap();
    handle.join().expect("the thread with a 2 MB stack panicked");
});

// ---- 2. analysis.plot data with an unknown response ----------------------------

#[test]
fn analysis_plot_data_with_unknown_response_returns_an_error() {
    // Guards against `analysis.plot` kind "data" unwrapping a missing response id.
    let mut e = reactor();
    let x = factor_id(&mut e, 0);
    let out = reply(&mut e, "analysis.plot", json!({"kind": "data", "response": "99", "x": x}));
    assert_eq!(out["ok"], json!(false), "unknown response 99 must give ok:false, got {out}");
}

// ---- 3. evaluate.run with huge values ------------------------------------------

#[test]
fn evaluate_run_with_huge_run_values_replies() {
    // Guards against `evaluate.run` unwrapping a failed float comparison when run values are near the f64 limit.
    let mut e = reactor();
    let sheet = call(&mut e, "sheet.get", json!({"sheet": "runs"}));
    let title = sheet["columns"][4]["title"].as_str().unwrap().to_string();
    let kind = sheet["columns"][4]["kind"].as_str().unwrap().to_string();
    assert_eq!(kind, "factor", "column 4 must be a factor column, got {title} of kind {kind}");
    let o = reply(&mut e, "sheet.set", json!({"sheet": "runs", "row": 0, "col": 4, "text": "1e308"}));
    assert_eq!(o["ok"], json!(true), "{o}");
    let o = reply(&mut e, "sheet.set", json!({"sheet": "runs", "row": 1, "col": 4, "text": "-1e308"}));
    assert_eq!(o["ok"], json!(true), "{o}");
    for formula in ["A*B", "A^2", "A+B+A*B"] {
        let out = reply(&mut e, "evaluate.run", json!({"formula": formula, "source": "runs"}));
        assert_replied(&out, &format!("evaluate.run {formula} with 1e308 values"));
    }
}

// ---- 4. empty level set in a loaded project ------------------------------------

fn empty_levels_project(kind: Value) -> Value {
    let mut p = milling();
    p["factors"][0]["kind"] = kind;
    p
}

fn check_empty_levels(kind: Value) {
    let p = empty_levels_project(kind);
    let Some(mut e) = try_open(&p) else { return };
    let out = reply(&mut e, "design.preview", json!({"method": "lhs", "options": {}}));
    assert_eq!(out["ok"], json!(false), "project.open accepted a factor without levels and design.preview lhs then succeeded: {out}");
}

#[test]
fn empty_categorical_levels_are_rejected_on_open_or_preview() {
    // Guards against `design.preview` lhs panicking for a loaded categorical factor with no levels.
    check_empty_levels(json!({"type": "categorical", "levels": []}));
}

#[test]
fn empty_discrete_levels_are_rejected_on_open_or_preview() {
    // Guards against `design.preview` lhs panicking for a loaded discrete factor with no levels.
    check_empty_levels(json!({"type": "discrete", "levels": []}));
}

// ---- 5. unbounded design options -----------------------------------------------

isolated_test!(design_preview_full_factorial_with_huge_replicates_replies, 20, 2048, {
    // Guards against `replicates` of 100000000 being accepted although the range is 1 to 20.
    let mut e = reactor();
    let out = reply(&mut e, "design.preview", json!({"method": "full_factorial", "options": {"replicates": 100000000}}));
    assert_replied(&out, "full_factorial replicates 100000000");
});

isolated_test!(design_preview_ccd_with_huge_center_points_replies, 20, 2048, {
    // Guards against `center_points` of 4000000000 being accepted although the range is 0 to 50.
    let mut e = reactor();
    let out = reply(&mut e, "design.preview", json!({"method": "ccd", "options": {"center_points": 4000000000u64}}));
    assert_replied(&out, "ccd center_points 4000000000");
});

isolated_test!(design_preview_optimal_with_huge_runs_replies, 20, 2048, {
    // Guards against `runs` of 1000000000 being accepted although the range is 0 to 1000.
    let mut e = reactor();
    let out = reply(&mut e, "design.preview", json!({"method": "optimal", "options": {"runs": 1000000000}}));
    assert_replied(&out, "optimal runs 1000000000");
});

// ---- 6. unbounded term powers and expansion -------------------------------------

isolated_test!(huge_term_power_from_model_set_does_not_hang_equation_or_report, 20, 2048, {
    // Guards against a power of 4294967295 being accepted by `model.set` and then expanded term by term.
    let mut e = reactor();
    let rid = first_response(&mut e);
    let set = reply(&mut e, "model.set", json!({"response": rid, "formula": "A^4294967295"}));
    assert_replied(&set, "model.set A^4294967295");
    if set["ok"] == json!(true) {
        let eq = reply(&mut e, "analysis.equation", json!({"response": rid}));
        assert_replied(&eq, "analysis.equation");
        let rep = reply(&mut e, "export.report", json!({}));
        assert_replied(&rep, "export.report");
    }
});

isolated_test!(huge_term_power_from_project_file_does_not_hang_equation, 20, 2048, {
    // Guards against `analysis.equation` expanding a loaded term with the power 4000000000.
    let mut p = milling();
    p["models"]["6"]["terms"] = json!([[[1, 4000000000u64]]]);
    let Some(mut e) = try_open(&p) else { return };
    let out = reply(&mut e, "analysis.equation", json!({"response": "6"}));
    assert_replied(&out, "analysis.equation with power 4000000000");
});

isolated_test!(repeated_binomial_product_formula_fits_within_time_and_memory, 20, 1024, {
    // Guards against a product of 30 sums being expanded into every term before it is fitted.
    let mut e = reactor();
    let rid = first_response(&mut e);
    let formula = vec!["(A+B)"; 30].join("*");
    let start = Instant::now();
    let set = reply(&mut e, "model.set", json!({"response": rid, "formula": formula}));
    assert_replied(&set, "model.set product of sums");
    if set["ok"] == json!(true) {
        let fit = reply(&mut e, "analysis.fit", json!({"response": rid}));
        assert_replied(&fit, "analysis.fit");
    }
    assert!(start.elapsed() < Duration::from_secs(5), "took {:?}, expected under 5 s", start.elapsed());
});

// ---- 7. project load validation --------------------------------------------------

fn unique_factor_ids(e: &mut sdoe::Engine) -> (Vec<String>, bool) {
    let ids: Vec<String> = call(e, "factors.list", json!({})).as_array().unwrap().iter().map(|f| f["id"].as_str().unwrap().to_string()).collect();
    let mut sorted = ids.clone();
    sorted.sort();
    sorted.dedup();
    let unique = sorted.len() == ids.len();
    (ids, unique)
}

#[test]
fn duplicate_factor_ids_are_rejected_or_repaired_on_open() {
    // Guards against `project.open` keeping two factors with the same id.
    let mut p = project(vec![cont(1, 0.0, 1.0), cont(2, 0.0, 1.0)]);
    p["factors"][1]["id"] = json!(1);
    let Some(mut e) = try_open(&p) else { return };
    let (ids, unique) = unique_factor_ids(&mut e);
    assert!(unique, "factor ids after open are {ids:?}, expected all distinct");
}

#[test]
fn next_id_below_existing_ids_does_not_produce_a_duplicate_factor_id() {
    // Guards against `project.open` keeping a next_id that an existing factor id already uses.
    let mut p = project(vec![cont(1, 0.0, 1.0), cont(2, 0.0, 1.0)]);
    p["next_id"] = json!(1);
    let Some(mut e) = try_open(&p) else { return };
    let out = reply(&mut e, "sheet.insert_row", json!({"sheet": "factors", "row": 2}));
    assert_replied(&out, "sheet.insert_row");
    let (ids, unique) = unique_factor_ids(&mut e);
    assert!(unique, "factor ids after inserting a row are {ids:?}, expected all distinct");
}

#[test]
fn model_for_a_missing_response_is_rejected_or_dropped_on_open() {
    // Guards against `project.open` keeping a model keyed to a response id that does not exist.
    let mut p = milling();
    p["models"]["99"] = json!({"terms": [[[1, 1]]], "transform": {"type": "none"}});
    let Some(mut e) = try_open(&p) else { return };
    let saved: Value = serde_json::from_str(call(&mut e, "project.save", json!({}))["text"].as_str().unwrap()).unwrap();
    let rids: Vec<String> = saved["responses"].as_array().unwrap().iter().map(|r| r["id"].to_string()).collect();
    let keys: Vec<&String> = saved["models"].as_object().unwrap().keys().collect();
    assert!(keys.iter().all(|k| rids.contains(k)), "model keys {keys:?} not all in response ids {rids:?}");
}

fn check_mixture_total(total: f64) {
    let mut p = project(vec![cont(1, 0.0, 1.0)]);
    p["mixture_total"] = json!(total);
    let Some(mut e) = try_open(&p) else { return };
    let st = call(&mut e, "project.state", json!({}));
    let t = st["mixture_total"].as_f64().unwrap();
    assert!(t > 0.0, "mixture_total {total} was loaded as {t}, expected a value above 0");
}

#[test]
fn zero_mixture_total_is_rejected_or_repaired_on_open() {
    // Guards against `project.open` keeping a mixture_total of 0, which `project.set` forbids.
    check_mixture_total(0.0);
}

#[test]
fn negative_mixture_total_is_rejected_or_repaired_on_open() {
    // Guards against `project.open` keeping a negative mixture_total, which `project.set` forbids.
    check_mixture_total(-1.0);
}

#[test]
fn continuous_low_above_high_is_rejected_or_repaired_on_open() {
    // Guards against `project.open` keeping a continuous factor whose low is above its high.
    let p = project(vec![cont(1, 5.0, 1.0), cont(2, 0.0, 1.0)]);
    let Some(mut e) = try_open(&p) else { return };
    let f = &call(&mut e, "factors.list", json!({}))[0];
    let num = |v: &Value| v.as_f64().or_else(|| v.as_str().and_then(|t| t.parse().ok())).unwrap_or_else(|| panic!("low/high not a number: {f}"));
    let (low, high) = (num(&f["low"]), num(&f["high"]));
    assert!(low < high, "loaded factor has low {low} and high {high}, expected low below high");
}

// ---- 8. float round trip ----------------------------------------------------------

/// Numbers with a fraction or exponent in JSON text, outside strings.
fn fractional_tokens(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut in_str = false;
    let mut esc = false;
    let mut cur = String::new();
    for c in text.chars().chain(std::iter::once(' ')) {
        if in_str {
            if esc {
                esc = false;
            } else if c == '\\' {
                esc = true;
            } else if c == '"' {
                in_str = false;
            }
            continue;
        }
        if c.is_ascii_digit() || matches!(c, '+' | '-' | '.' | 'e' | 'E') {
            cur.push(c);
            continue;
        }
        if cur.contains(['.', 'e', 'E']) {
            out.push(std::mem::take(&mut cur));
        }
        cur.clear();
        if c == '"' {
            in_str = true;
        }
    }
    out
}

#[test]
fn project_save_keeps_every_float_of_the_loaded_file_bit_for_bit() {
    // Guards against `project.open` parsing floats with an inexact parser, which changes the last digit.
    let text = example_text("field-ml-hyperparameter-lhs.sdoe");
    let mut e = engine_with_text(&text);
    let saved = call(&mut e, "project.save", json!({}))["text"].as_str().unwrap().to_string();
    let saved_bits: std::collections::HashSet<u64> = fractional_tokens(&saved).iter().map(|t| t.parse::<f64>().unwrap().to_bits()).collect();
    let missing: Vec<String> = fractional_tokens(&text).into_iter().filter(|t| !saved_bits.contains(&t.parse::<f64>().unwrap().to_bits())).collect();
    assert!(missing.is_empty(), "numbers of the original file that differ after load and save: {missing:?}");
}

// ---- 9. CSV precision --------------------------------------------------------------

#[test]
fn csv_export_then_import_replace_keeps_run_values_exactly() {
    // Guards against `export.csv` writing only 10 significant digits.
    let value = 0.123456789012345678_f64;
    let mut p = project(vec![cont(1, 0.0, 1.0), cont(2, 0.0, 1.0)]);
    p["responses"] = json!([{"id": 3, "name": "Y", "units": "", "goal": "none", "lower": null, "target": null, "upper": null, "weight": 1.0, "importance": 3.0}]);
    p["runs"] = json!([
        {"id": 4, "std_order": 1, "run_order": 1, "point_type": "factorial", "status": "done", "settings": {"1": value, "2": 0.5}, "responses": {"3": value * 3.0}, "note": ""},
        {"id": 5, "std_order": 2, "run_order": 2, "point_type": "factorial", "status": "done", "settings": {"1": 1.0 / 3.0, "2": 0.25}, "responses": {"3": 2.0 / 3.0}, "note": ""}
    ]);
    let mut e = engine_with_project(&p);
    let csv = call(&mut e, "export.csv", json!({}))["text"].as_str().unwrap().to_string();
    let prev = call(&mut e, "import.preview", json!({"text": csv}));
    let mapping: Vec<String> = prev["columns"].as_array().unwrap().iter().map(|c| c["suggestion"].as_str().unwrap().to_string()).collect();
    call(&mut e, "import.apply", json!({"text": csv, "mapping": mapping, "mode": "replace"}));
    let saved: Value = serde_json::from_str(call(&mut e, "project.save", json!({}))["text"].as_str().unwrap()).unwrap();
    let want = [value, 0.5, value * 3.0, 1.0 / 3.0, 0.25, 2.0 / 3.0];
    let runs = saved["runs"].as_array().unwrap();
    assert_eq!(runs.len(), 2, "run count after import: {runs:?}");
    let got: Vec<f64> = runs
        .iter()
        .flat_map(|r| {
            let s = &r["settings"];
            let y = &r["responses"];
            vec![s["1"].as_f64(), s["2"].as_f64(), y["3"].as_f64()]
        })
        .map(|v| v.unwrap_or(f64::NAN))
        .collect();
    let mut got_sorted = got.clone();
    got_sorted.sort_by(f64::total_cmp);
    let mut want_sorted = want.to_vec();
    want_sorted.sort_by(f64::total_cmp);
    for (g, w) in got_sorted.iter().zip(&want_sorted) {
        assert!(((g - w) / w).abs() <= 1e-15, "value after round trip {g:?}, expected {w:?}");
    }
}

// ---- 10. BOM ------------------------------------------------------------------------

#[test]
fn import_preview_strips_utf8_bom_from_first_column_name() {
    // Guards against the importer keeping a leading U+FEFF in the first header.
    let mut e = sdoe::Engine::new();
    let prev = call(&mut e, "import.preview", json!({"text": "\u{feff}T,Y\n1,2\n3,4\n"}));
    let name = prev["columns"][0]["name"].as_str().unwrap().to_string();
    assert!(!name.contains('\u{feff}'), "first column name is {name:?}, expected \"T\"");
}

// ---- 11. carriage return in a header --------------------------------------------------

#[test]
fn factor_name_with_carriage_return_survives_csv_export_and_import() {
    // Guards against `export.csv` leaving a bare carriage return unquoted although the importer reads it as a record break.
    let mut p = project(vec![cont(1, 0.0, 1.0), cont(2, 0.0, 1.0)]);
    p["factors"][0]["name"] = json!("a\rb");
    p["runs"] = json!([
        {"id": 4, "std_order": 1, "run_order": 1, "point_type": "factorial", "status": "done", "settings": {"1": 0.0, "2": 0.5}, "responses": {}, "note": ""},
        {"id": 5, "std_order": 2, "run_order": 2, "point_type": "factorial", "status": "done", "settings": {"1": 1.0, "2": 0.25}, "responses": {}, "note": ""}
    ]);
    let mut e = engine_with_project(&p);
    let columns = call(&mut e, "sheet.get", json!({"sheet": "runs"}))["columns"].as_array().unwrap().len();
    let csv = call(&mut e, "export.csv", json!({}))["text"].as_str().unwrap().to_string();
    let prev = call(&mut e, "import.preview", json!({"text": csv}));
    let n = prev["columns"].as_array().unwrap().len();
    assert_eq!(n, columns, "imported column count {n}, expected {columns}; rows {}, expected 2", prev["rows"]);
    assert_eq!(prev["rows"], json!(2), "imported rows {}, expected 2", prev["rows"]);
}

/// Clearing a one-column, multi-row selection empties every cell in one undo step (clearing through sheet.paste dropped the empty lines and cleared nothing).
#[test]
fn sheet_clear_empties_a_single_column_block() {
    let mut e = Engine::new();
    call(&mut e, "project.example", json!({"name": "reactor"}));
    let sheet = call(&mut e, "sheet.get", json!({"sheet": "runs"}));
    let cols = sheet["columns"].as_array().unwrap();
    let col = cols.iter().position(|c| c["kind"] == json!("response")).expect("a response column");
    let text = |e: &mut Engine, r: usize| call(e, "sheet.get", json!({"sheet": "runs"}))["rows"][r][col]["text"].as_str().unwrap().to_string();
    assert!((0..3).all(|r| !text(&mut e, r).is_empty()), "test premise: rows 0-2 have values");
    call(&mut e, "sheet.clear", json!({"sheet": "runs", "top": 0, "left": col, "bottom": 2, "right": col}));
    let after: Vec<String> = (0..4).map(|r| text(&mut e, r)).collect();
    assert!(after[..3].iter().all(|t| t.is_empty()) && !after[3].is_empty(), "after clearing rows 0-2: {after:?}");
    call(&mut e, "edit.undo", json!({}));
    assert!((0..3).all(|r| !text(&mut e, r).is_empty()), "one undo step restores all three cells");
}

/// Text that is not a number in the mixture total reset the total to 1 without an error.
#[test]
fn non_numeric_mixture_total_is_rejected() {
    let mut e = engine_with_project(&project(vec![mix(1, 0.0, 1.0), mix(2, 0.0, 1.0)]));
    call(&mut e, "project.set", json!({"mixture_total": "100"}));
    let err = call_err(&mut e, "project.set", json!({"mixture_total": "abc"}));
    assert!(err.contains("abc"), "{err}");
    assert_eq!(call(&mut e, "project.state", json!({}))["mixture_total"], json!(100.0));
}

/// Low and High were checked one at a time, so entering Low 30 and then High 70 on a factor
/// with the range -1 to 1 failed with "low must be below high".
#[test]
fn factor_range_can_be_entered_in_either_order() {
    let mut e = engine_with_project(&project(vec![cont(1, -1.0, 1.0)]));
    let range = |e: &mut Engine| {
        let f = &call(e, "factors.list", json!({}))[0];
        (f["low"].as_f64().unwrap(), f["high"].as_f64().unwrap())
    };
    call(&mut e, "sheet.set", json!({"sheet": "factors", "row": 0, "col": 5, "text": "30"}));
    call(&mut e, "sheet.set", json!({"sheet": "factors", "row": 0, "col": 6, "text": "70"}));
    assert_eq!(range(&mut e), (30.0, 70.0));
    call(&mut e, "sheet.set", json!({"sheet": "factors", "row": 0, "col": 6, "text": "-20"}));
    call(&mut e, "sheet.set", json!({"sheet": "factors", "row": 0, "col": 5, "text": "-60"}));
    assert_eq!(range(&mut e), (-60.0, -20.0));
}

/// Import accepted any number for a discrete factor, while the Runs sheet accepts only levels.
#[test]
fn import_applies_the_runs_sheet_rules_to_discrete_factors() {
    let mut f2 = disc(2, &[10.0, 20.0]);
    f2["role"] = json!("environmental");
    let mut p = project(vec![disc(1, &[1.0, 2.0, 3.0]), f2]);
    p["responses"] = json!([{"id": 3, "name": "Y", "units": "", "goal": "none", "lower": null, "target": null, "upper": null, "weight": 1.0, "importance": 3.0}]);
    let mut e = engine_with_project(&p);
    let csv = "F1,F2,Y\n2,10,5\n2.5,15,6\n";
    let prev = call(&mut e, "import.preview", json!({"text": csv}));
    let mapping: Vec<String> = prev["columns"].as_array().unwrap().iter().map(|c| c["suggestion"].as_str().unwrap().to_string()).collect();
    call(&mut e, "import.apply", json!({"text": csv, "mapping": mapping, "mode": "replace"}));
    let saved: Value = serde_json::from_str(call(&mut e, "project.save", json!({}))["text"].as_str().unwrap()).unwrap();
    let runs = saved["runs"].as_array().unwrap();
    assert_eq!(runs[0]["settings"]["1"], json!(2.0));
    assert!(runs[1]["settings"].get("1").is_none(), "2.5 is not a level of F1 but was imported: {}", runs[1]);
    assert_eq!(runs[1]["settings"]["2"], json!(15.0));
    assert_eq!(saved["factors"][1]["kind"]["levels"], json!([10.0, 15.0, 20.0]), "environmental discrete factor levels");
}
