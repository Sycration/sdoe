use sdoe::Engine;
use serde_json::{Value, json};

fn call(e: &mut Engine, m: &str, p: Value) -> Value {
    let out: Value = serde_json::from_str(&e.call(m, &p.to_string())).unwrap();
    assert_eq!(out["ok"], json!(true), "{m} failed: {out}");
    out["result"].clone()
}

fn call_err(e: &mut Engine, m: &str, p: Value) -> String {
    let out: Value = serde_json::from_str(&e.call(m, &p.to_string())).unwrap();
    assert_eq!(out["ok"], json!(false), "{m} unexpectedly succeeded");
    out["error"].as_str().unwrap().to_string()
}

fn first_response(e: &mut Engine) -> String {
    call(e, "responses.list", json!({}))[0]["id"].as_str().unwrap().to_string()
}

#[test]
fn reactor_workflow() {
    let mut e = Engine::new();
    call(&mut e, "project.example", json!({"name": "reactor"}));
    let st = call(&mut e, "project.state", json!({}));
    assert_eq!(st["runs"], json!(26));
    let rid = first_response(&mut e);
    let fit = call(&mut e, "analysis.fit", json!({"response": rid}));
    assert!(fit["formula"].as_str().unwrap().contains("E"), "humidity joins the model: {}", fit["formula"]);
    let r2: f64 = fit["stats"][4][1].as_str().unwrap().parse().unwrap();
    assert!(r2 > 0.8, "R2 {r2}");
    // Equations in all syntaxes and units.
    for syn in ["plain", "latex", "excel", "python", "c"] {
        for units in ["actual", "coded"] {
            let eq = call(&mut e, "analysis.equation", json!({"response": rid, "syntax": syn, "units": units}));
            assert!(!eq["text"].as_str().unwrap().is_empty());
        }
    }
    // Plots.
    let factors = call(&mut e, "factors.list", json!({}));
    let fa = factors[0]["id"].as_str().unwrap();
    let fb = factors[1]["id"].as_str().unwrap();
    let fc = factors[2]["id"].as_str().unwrap();
    for kind in ["pred_actual", "resid_fitted", "resid_normal", "resid_order", "cooks", "boxcox", "pareto", "halfnormal", "profiler"] {
        call(&mut e, "analysis.plot", json!({"response": rid, "kind": kind}));
    }
    call(&mut e, "analysis.plot", json!({"response": rid, "kind": "contour", "x": fa, "y": fb}));
    call(&mut e, "analysis.plot", json!({"response": rid, "kind": "surface", "x": fa, "y": fb}));
    call(&mut e, "analysis.plot", json!({"response": rid, "kind": "interaction", "x": fa, "y": fc}));
    call(&mut e, "analysis.plot", json!({"response": rid, "kind": "data", "x": fa}));
    // Model editing: the example model already contains the humidity interaction.
    assert!(fit["formula"].as_str().unwrap().contains("A*E"), "{}", fit["formula"]);
    call(&mut e, "model.preset", json!({"response": rid, "preset": "quadratic", "env_interactions": false}));
    let plain = call(&mut e, "analysis.fit", json!({"response": rid}));
    let hint = plain["warnings"].as_array().unwrap().iter().any(|w| w.as_str().unwrap().contains("Environment × factor interactions are significant"));
    assert!(hint, "hint expected: {}", plain["warnings"]);
    call(&mut e, "model.preset", json!({"response": rid, "preset": "quadratic", "env_interactions": true}));
    let m = call(&mut e, "model.get", json!({"response": rid}));
    assert!(m["formula"].as_str().unwrap().contains("A*E"), "{}", m["formula"]);
    let fit2 = call(&mut e, "analysis.fit", json!({"response": rid}));
    let adj2: f64 = fit2["stats"][5][1].as_str().unwrap().parse().unwrap();
    let adj1: f64 = plain["stats"][5][1].as_str().unwrap().parse().unwrap();
    assert!(adj2 > adj1, "environment interaction improves the fit: {adj1} -> {adj2}");
    call(&mut e, "model.select", json!({"response": rid, "method": "aicc"}));
    call(&mut e, "model.select", json!({"response": rid, "method": "backward", "alpha": 0.1}));
    let auto = call(&mut e, "model.auto", json!({"response": rid}));
    assert!(auto["table"]["rows"].as_array().unwrap().len() >= 2);
    call(&mut e, "model.set", json!({"response": rid, "formula": "A + B + A*B + A^2 + E + A*E", "transform": "None"}));
    let err = call_err(&mut e, "model.remove_term", json!({"response": rid, "term": "A"}));
    assert!(err.contains("higher-order"));
    call(&mut e, "model.remove_term", json!({"response": rid, "term": "A*B"}));
    // Optimisation: conditional and robust; control law.
    let hum = factors[4]["id"].as_str().unwrap();
    let res = call(&mut e, "optimize.run", json!({"env": {hum: {"mode": "fixed", "value": "35"}}}));
    let d_dry = res["solutions"][0]["d"].as_f64().unwrap();
    let t_dry: f64 = res["solutions"][0]["values"][fa].as_str().unwrap().parse().unwrap();
    let res = call(&mut e, "optimize.run", json!({"env": {hum: {"mode": "fixed", "value": "68"}}}));
    let t_wet: f64 = res["solutions"][0]["values"][fa].as_str().unwrap().parse().unwrap();
    assert!(d_dry > 0.0);
    assert!(t_wet < t_dry, "humidity shifts the best temperature down: {t_dry} -> {t_wet}");
    let rob = call(&mut e, "optimize.run", json!({"env": {hum: {"mode": "random", "mean": "50", "sd": "10"}}}));
    assert_eq!(rob["robust"], json!(true));
    call(&mut e, "model.preset", json!({"response": rid, "preset": "quadratic", "env_interactions": true}));
    let law = call(&mut e, "optimize.control_law", json!({"factor": hum}));
    let formulas: Vec<String> = law["formulas"].as_array().unwrap().iter().map(|f| f.as_str().unwrap().to_string()).collect();
    assert!(formulas.iter().any(|f| f.starts_with("Temperature =") && f.contains("Humidity")), "{formulas:?}");
    let prof = call(&mut e, "optimize.profile", json!({"values": res["solutions"][0]["values"]}));
    assert!(prof["rows"].as_array().unwrap().len() == 3);
    // Next run given today's humidity.
    let nr = call(&mut e, "design.next_run", json!({"env": {hum: "55"}, "mode": "info"}));
    assert_eq!(nr["values"][hum], json!("55"));
    let nb = call(&mut e, "design.next_run", json!({"env": {hum: "55"}, "mode": "best"}));
    call(&mut e, "design.add_run", json!({"values": nb["values"]}));
    // Evaluation.
    let ev = call(&mut e, "evaluate.run", json!({"preset": "quadratic"}));
    assert!(ev["table"]["rows"].as_array().unwrap().len() > 5);
    // Export, report, round trip.
    let csv = call(&mut e, "export.csv", json!({}))["text"].as_str().unwrap().to_string();
    assert!(csv.lines().count() >= 27);
    let html = call(&mut e, "export.report", json!({}))["html"].as_str().unwrap().to_string();
    assert!(html.contains("<svg"));
    let saved = call(&mut e, "project.save", json!({}))["text"].as_str().unwrap().to_string();
    let mut e2 = Engine::new();
    call(&mut e2, "project.open", json!({"text": saved}));
    assert_eq!(call(&mut e2, "project.state", json!({}))["runs"], call(&mut e, "project.state", json!({}))["runs"]);
    // Import the CSV into a fresh project (observational data).
    let mut e3 = Engine::new();
    let pv = call(&mut e3, "import.preview", json!({"text": csv}));
    let mapping: Vec<String> = pv["columns"].as_array().unwrap().iter().map(|c| c["suggestion"].as_str().unwrap().to_string()).collect();
    let rep = call(&mut e3, "import.apply", json!({"text": csv, "mapping": mapping, "mode": "replace"}));
    assert_eq!(rep["runs"], json!(27));
    // Undo / redo.
    call(&mut e, "edit.undo", json!({}));
    call(&mut e, "edit.redo", json!({}));
}

#[test]
fn every_design_method() {
    let mut e = Engine::new();
    let s = "factors";
    for (r, name) in ["Temp", "Pressure", "Speed", "Feed"].iter().enumerate() {
        call(&mut e, "sheet.set", json!({"sheet": s, "row": r, "col": 1, "text": name}));
    }
    call(&mut e, "sheet.set", json!({"sheet": "responses", "row": 0, "col": 0, "text": "Y"}));
    let methods = call(&mut e, "design.methods", json!({}));
    let mut tried = 0;
    for m in methods.as_array().unwrap() {
        if !m["available"].as_bool().unwrap() {
            continue;
        }
        let id = m["id"].as_str().unwrap();
        let pv = call(&mut e, "design.preview", json!({"method": id, "options": {}}));
        assert!(pv["runs"].as_u64().unwrap() > 0, "{id}");
        call(&mut e, "evaluate.run", json!({"source": "preview"}));
        tried += 1;
    }
    assert!(tried >= 9, "tried {tried}");
    // CCD then apply and randomize.
    let pv = call(&mut e, "design.preview", json!({"method": "ccd", "options": {"alpha": "rotatable", "center_points": 6}}));
    assert_eq!(pv["runs"], json!(16 + 8 + 6));
    call(&mut e, "design.apply", json!({"mode": "replace", "randomize": true}));
    assert_eq!(call(&mut e, "project.state", json!({}))["runs"], json!(30));
    // Fractional factorial with explicit generator.
    let pv = call(&mut e, "design.preview", json!({"method": "fractional_factorial", "options": {"generators": "D=ABC"}}));
    assert!(pv["notes"].as_array().unwrap().iter().any(|n| n.as_str().unwrap().contains("resolution IV")), "{}", pv["notes"]);
    // Mixed factors: categorical + discrete + environmental with known values.
    call(&mut e, "sheet.set", json!({"sheet": s, "row": 2, "col": 3, "text": "Categorical"}));
    call(&mut e, "sheet.set", json!({"sheet": s, "row": 2, "col": 7, "text": "Low, Mid, High"}));
    call(&mut e, "sheet.set", json!({"sheet": s, "row": 3, "col": 3, "text": "Discrete"}));
    call(&mut e, "sheet.set", json!({"sheet": s, "row": 3, "col": 7, "text": "1, 2, 4, 8"}));
    call(&mut e, "sheet.set", json!({"sheet": s, "row": 4, "col": 1, "text": "Ambient"}));
    call(&mut e, "sheet.set", json!({"sheet": s, "row": 4, "col": 4, "text": "Environmental"}));
    let f = call(&mut e, "factors.list", json!({}));
    let amb = f[4]["id"].as_str().unwrap();
    let pv = call(&mut e, "design.preview", json!({"method": "optimal", "options": {
        "model": "quadratic", "env_interactions": true, format!("env_{amb}"): "-0.8 -0.5 0 0.4 0.9 1 -1 0.2 0.6 -0.3 0.1 0.7 -0.6 0.3 0.8 -0.9 0.5 -0.2 0 1 -1 0.45 0.15 -0.15 0.35 -0.35 0.55 -0.55 0.75 -0.75"
    }}));
    assert_eq!(pv["runs"], json!(30));
    call(&mut e, "design.apply", json!({"mode": "replace"}));
    let runs = call(&mut e, "sheet.get", json!({"sheet": "runs"}));
    assert_eq!(runs["rows"].as_array().unwrap().len(), 31);
    // Fill responses by pasting a column.
    let col = 4 + 5;
    let vals: Vec<String> = (0..30).map(|i| format!("{}", 10.0 + (i as f64 * 0.37).sin())).collect();
    let r = call(&mut e, "sheet.paste", json!({"sheet": "runs", "row": 0, "col": col, "text": vals.join("\n")}));
    assert!(r["errors"].as_array().unwrap().is_empty(), "{r}");
    let rid = first_response(&mut e);
    call(&mut e, "analysis.fit", json!({"response": rid}));
    // Taguchi with a noise factor.
    call(&mut e, "sheet.set", json!({"sheet": s, "row": 1, "col": 4, "text": "Noise"}));
    call(&mut e, "sheet.set", json!({"sheet": s, "row": 3, "col": 7, "text": "1, 2, 4"}));
    let pv = call(&mut e, "design.preview", json!({"method": "taguchi", "options": {}}));
    assert!(pv["runs"].as_u64().unwrap() >= 8);
}

#[test]
fn mixture_example() {
    let mut e = Engine::new();
    call(&mut e, "project.example", json!({"name": "blend"}));
    let rid = first_response(&mut e);
    let fit = call(&mut e, "analysis.fit", json!({"response": rid}));
    assert!(fit["anova"]["rows"].as_array().unwrap().iter().any(|r| r[0] == json!("Linear mixture")));
    let r2: f64 = fit["stats"][4][1].as_str().unwrap().parse().unwrap();
    assert!(r2 > 0.9, "{r2}");
    call(&mut e, "analysis.equation", json!({"response": rid, "units": "actual"}));
    let f = call(&mut e, "factors.list", json!({}));
    call(&mut e, "analysis.plot", json!({"response": rid, "kind": "contour", "x": f[0]["id"], "y": f[1]["id"]}));
    call(&mut e, "analysis.plot", json!({"response": rid, "kind": "profiler"}));
    let res = call(&mut e, "optimize.run", json!({}));
    assert!(res["solutions"][0]["d"].as_f64().unwrap() > 0.0);
    let pv = call(&mut e, "design.preview", json!({"method": "optimal", "options": {"model": "2fi", "runs": 10}}));
    assert_eq!(pv["runs"], json!(10));
    call(&mut e, "design.preview", json!({"method": "simplex_centroid", "options": {}}));
}

#[test]
fn loading_a_project_drops_transient_state() {
    let mut e = Engine::new();
    let epoch0 = call(&mut e, "project.state", json!({}))["epoch"].as_u64().unwrap();
    call(&mut e, "project.example", json!({"name": "reactor"}));
    let state = call(&mut e, "project.state", json!({}));
    assert!(state["epoch"].as_u64().unwrap() > epoch0);
    call(&mut e, "design.preview", json!({"method": "ccd", "options": {}}));
    call(&mut e, "sheet.set", json!({"sheet": "factors", "row": 0, "col": 1, "text": "Changed"}));
    let err = call_err(&mut e, "design.apply", json!({}));
    assert!(err.contains("preview a design first"), "{err}");
    call(&mut e, "project.example", json!({"name": "reactor"}));
    let state = call(&mut e, "project.state", json!({}));
    assert_eq!(state["can_undo"], json!(false));
    assert_eq!(state["can_redo"], json!(false));
}

fn set(e: &mut Engine, sheet: &str, row: usize, col: usize, text: &str) {
    call(e, "sheet.set", json!({"sheet": sheet, "row": row, "col": col, "text": text}));
}

fn set_err(e: &mut Engine, sheet: &str, row: usize, col: usize, text: &str) -> String {
    call_err(e, "sheet.set", json!({"sheet": sheet, "row": row, "col": col, "text": text}))
}

/// Temperature T (-20..20), thawing method M (only below 0) and time X.
fn frozen_project(e: &mut Engine) {
    call(e, "project.new", json!({}));
    set(e, "factors", 0, 1, "Temperature");
    set(e, "factors", 0, 0, "T");
    set(e, "factors", 0, 5, "-20");
    set(e, "factors", 0, 6, "20");
    set(e, "factors", 1, 1, "Thawing");
    set(e, "factors", 1, 0, "M");
    set(e, "factors", 1, 3, "Categorical");
    set(e, "factors", 1, 7, "Slow, Fast");
    set(e, "factors", 2, 1, "Time");
    set(e, "factors", 2, 0, "X");
    set(e, "responses", 0, 0, "Y");
}

#[test]
fn conditional_factors() {
    let mut e = Engine::new();
    frozen_project(&mut e);
    // Invalid conditions.
    assert!(set_err(&mut e, "factors", 1, 8, "T < 50").contains("range"));
    assert!(set_err(&mut e, "factors", 1, 8, "Q < 0").contains("unknown factor"));
    assert!(set_err(&mut e, "factors", 1, 8, "M = Slow").contains("itself"));
    assert!(set_err(&mut e, "factors", 1, 8, "T = 3").contains("continuous"));
    set(&mut e, "factors", 1, 8, "T < 0");
    assert!(set_err(&mut e, "factors", 0, 8, "M = Fast").contains("depends on"));
    let fs = call(&mut e, "sheet.get", json!({"sheet": "factors"}));
    assert_eq!(fs["rows"][1][8]["text"], json!("T < 0"));

    // Full factorial: the 2 runs at T = 20 that differ only in M merge.
    let pv = call(&mut e, "design.preview", json!({"method": "full_factorial", "options": {"levels": 2}}));
    assert_eq!(pv["runs"], json!(6), "{pv}");
    assert!(pv["notes"].to_string().contains("set in 4 of 6 runs"), "{}", pv["notes"]);
    call(&mut e, "design.apply", json!({"mode": "replace", "randomize": false}));

    // Runs sheet: M is inactive (blank, not editable) where T >= 0.
    let rs = call(&mut e, "sheet.get", json!({"sheet": "runs"}));
    let rows = rs["rows"].as_array().unwrap();
    let mut inactive = 0;
    for (i, row) in rows.iter().enumerate().take(6) {
        let t: f64 = row[4]["text"].as_str().unwrap().parse().unwrap();
        let m = row[5]["text"].as_str().unwrap();
        let x: f64 = row[6]["text"].as_str().unwrap().parse().unwrap();
        if t >= 0.0 {
            assert_eq!(row[5]["state"], json!("inactive"));
            assert_eq!(row[5]["editable"], json!(false));
            inactive += 1;
        } else {
            assert!(m == "Slow" || m == "Fast");
        }
        // Y = 10 + 2 T/20 + (T < 0: ±3 by method) + X.
        let thaw = if t < 0.0 { if m == "Fast" { 3.0 } else { -3.0 } } else { 0.0 };
        let y = 10.0 + 2.0 * t / 20.0 + thaw + x;
        set(&mut e, "runs", i, 7, &y.to_string());
    }
    assert_eq!(inactive, 2);
    // A value typed for an inactive factor is accepted without error.
    set(&mut e, "runs", rows.iter().position(|r| r[5]["state"] == json!("inactive")).unwrap(), 5, "n/a");

    // The fit is exact, and M has no effect at T = 20.
    let rid = first_response(&mut e);
    call(&mut e, "model.set", json!({"response": rid, "formula": "T + M + X"}));
    let fit = call(&mut e, "analysis.fit", json!({"response": rid}));
    let r2: f64 = fit["stats"][4][1].as_str().unwrap().parse().unwrap();
    assert!(r2 > 0.999999, "R2 {r2}: {fit}");
    let ids: Vec<String> = call(&mut e, "factors.list", json!({}))
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["id"].as_str().unwrap().to_string())
        .collect();
    let pred = |e: &mut Engine, t: &str, m: &str| -> f64 {
        let r = call(e, "analysis.predict", json!({"response": rid, "values": {ids[0].clone(): t, ids[1].clone(): m, ids[2].clone(): "0"}}));
        r["y"].as_str().unwrap().parse().unwrap()
    };
    let (a, b) = (pred(&mut e, "20", "Slow"), pred(&mut e, "20", "Fast"));
    assert!((a - b).abs() < 1e-9 && (a - 12.0).abs() < 1e-6, "{a} {b}");
    let (a, b) = (pred(&mut e, "-20", "Slow"), pred(&mut e, "-20", "Fast"));
    assert!((a - 5.0).abs() < 1e-6 && (b - 11.0).abs() < 1e-6, "{a} {b}");

    // Actual-unit equations split by method below 0 and one equation at or above 0.
    let eq = call(&mut e, "analysis.equation", json!({"response": rid, "units": "actual"}));
    let text = eq["text"].as_str().unwrap();
    assert_eq!(eq["count"], json!(3), "{text}");
    assert!(text.contains("Temperature >= 0"), "{text}");
    assert!(text.contains("Thawing = Fast, Temperature < 0"), "{text}");
    let eq = call(&mut e, "analysis.equation", json!({"response": rid, "units": "coded"}));
    assert!(eq["text"].as_str().unwrap().contains("Terms with M are 0"), "{}", eq["text"]);

    // The optimum (T = 20) shows M as not applicable.
    set(&mut e, "responses", 0, 2, "Maximize");
    let opt = call(&mut e, "optimize.run", json!({}));
    let best = &opt["table"]["rows"][0];
    assert_eq!(best[3], json!("—"), "{opt}");
    assert_eq!(opt["solutions"][0]["values"][ids[1].clone()], json!(""), "{opt}");
    let t: f64 = best[2].as_str().unwrap().parse().unwrap();
    assert!(t > 19.0, "{opt}");

    // Optimal designs leave M blank where T >= 0.
    let pv = call(&mut e, "design.preview", json!({"method": "optimal", "options": {"model": "linear", "runs": 10}}));
    for row in pv["table"]["rows"].as_array().unwrap() {
        let cells = row.as_array().unwrap();
        let t: f64 = cells[2].as_str().unwrap().parse().unwrap();
        let m = cells[3].as_str().unwrap();
        assert_eq!(t >= 0.0, m.is_empty(), "{row}");
    }

    // The condition survives saving, and goes with its parent factor.
    let saved = call(&mut e, "project.save", json!({}));
    call(&mut e, "project.open", json!({"text": saved["text"]}));
    let fs = call(&mut e, "sheet.get", json!({"sheet": "factors"}));
    assert_eq!(fs["rows"][1][8]["text"], json!("T < 0"));
    call(&mut e, "sheet.delete_rows", json!({"sheet": "factors", "rows": [0]}));
    let fs = call(&mut e, "sheet.get", json!({"sheet": "factors"}));
    assert_eq!(fs["rows"][0][8]["text"], json!(""));
}

#[test]
fn conditional_on_categorical_parent() {
    let mut e = Engine::new();
    frozen_project(&mut e);
    // P (Fresh, Frozen) decides whether M applies; T stays unconditional.
    set(&mut e, "factors", 3, 1, "Product");
    set(&mut e, "factors", 3, 0, "P");
    set(&mut e, "factors", 3, 3, "Categorical");
    set(&mut e, "factors", 3, 7, "Fresh, Frozen");
    assert!(set_err(&mut e, "factors", 1, 8, "P = Fresh, Frozen").contains("every value"));
    assert!(set_err(&mut e, "factors", 1, 8, "P < 1").contains("categorical"));
    set(&mut e, "factors", 1, 8, "P = frozen");
    let fs = call(&mut e, "sheet.get", json!({"sheet": "factors"}));
    assert_eq!(fs["rows"][1][8]["text"], json!("P = Frozen"));
    // Renaming the levels renames them in the condition.
    set(&mut e, "factors", 3, 7, "Chilled, Deep-frozen");
    let fs = call(&mut e, "sheet.get", json!({"sheet": "factors"}));
    assert_eq!(fs["rows"][1][8]["text"], json!("P = Deep-frozen"));
    // The interaction M*P duplicates M and is not part of the presets.
    call(&mut e, "design.preview", json!({"method": "full_factorial", "options": {"levels": 2}}));
    call(&mut e, "design.apply", json!({"mode": "replace", "randomize": false}));
    let rid = first_response(&mut e);
    call(&mut e, "model.preset", json!({"response": rid, "preset": "2fi"}));
    let m = call(&mut e, "model.get", json!({"response": rid}));
    let f = m["formula"].as_str().unwrap();
    assert!(f.contains("T*M") && !f.contains("M*P"), "{f}");
    // Changing P to continuous drops the condition that no longer fits.
    set(&mut e, "factors", 3, 3, "Continuous");
    let fs = call(&mut e, "sheet.get", json!({"sheet": "factors"}));
    assert_eq!(fs["rows"][1][8]["text"], json!(""));
}
