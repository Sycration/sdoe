//! Generates manufacturing and materials example projects for sdoe.
//!
//! Run with `cargo run --release -p sdoe-core --example gen_manufacturing`.
//! The program writes `examples/mfg-*.sdoe` at the workspace root, reloads
//! every file in a fresh engine and prints a verification summary.
//!
//! All data are simulated from fixed seeds, so repeated runs produce
//! identical files.

use sdoe::Engine;
use sdoe::project::{RunStatus, Value};
use sdoe::rng::Rng;
use serde_json::{Value as Json, json};
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Engine helpers
// ---------------------------------------------------------------------------

fn call(e: &mut Engine, m: &str, p: Json) -> Json {
    let out: Json = serde_json::from_str(&e.call(m, &p.to_string())).expect("engine output is JSON");
    if out["ok"] != json!(true) {
        panic!("{m} {p} failed: {}", out["error"]);
    }
    out["result"].clone()
}

fn set(e: &mut Engine, sheet: &str, row: usize, col: usize, text: &str) {
    call(e, "sheet.set", json!({"sheet": sheet, "row": row, "col": col, "text": text}));
}

/// Adds a factor through the factor sheet and returns its id.
/// `kind` is Continuous, Discrete, Categorical or Mixture; `levels` is used
/// by discrete and categorical factors, `lo`/`hi` by the others.
fn factor(e: &mut Engine, name: &str, units: &str, kind: &str, role: &str, lo: f64, hi: f64, levels: &str) -> u64 {
    let row = e.project.factors.len();
    set(e, "factors", row, 1, name);
    set(e, "factors", row, 2, units);
    set(e, "factors", row, 3, kind);
    set(e, "factors", row, 4, role);
    match kind {
        "Continuous" | "Mixture" => {
            // High first: the default range is [-1, 1] (or [0, total] for mixtures).
            set(e, "factors", row, 6, &lo_hi_text(hi));
            set(e, "factors", row, 5, &lo_hi_text(lo));
        }
        _ => set(e, "factors", row, 7, levels),
    }
    e.project.factors[row].id
}

fn lo_hi_text(v: f64) -> String {
    format!("{v}")
}

/// Adds a response through the response sheet and returns its id.
/// Limits are given as text so that empty cells stay empty.
fn response(e: &mut Engine, name: &str, units: &str, goal: &str, lower: &str, target: &str, upper: &str, weight: f64, importance: f64) -> u64 {
    let row = e.project.responses.len();
    set(e, "responses", row, 0, name);
    set(e, "responses", row, 1, units);
    set(e, "responses", row, 2, goal);
    set(e, "responses", row, 3, lower);
    set(e, "responses", row, 4, target);
    set(e, "responses", row, 5, upper);
    set(e, "responses", row, 6, &format!("{weight}"));
    set(e, "responses", row, 7, &format!("{importance}"));
    e.project.responses[row].id
}

fn apply_design(e: &mut Engine, method: &str, options: Json, seed: u64) -> usize {
    let pv = call(e, "design.preview", json!({"method": method, "options": options}));
    for n in pv["notes"].as_array().into_iter().flatten() {
        println!("    design note: {}", n.as_str().unwrap_or(""));
    }
    call(e, "design.apply", json!({"mode": "replace", "randomize": true, "seed": seed}));
    // Settings are rounded to 6 decimals (optimal mixture blends otherwise carry long fractions).
    for run in e.project.runs.iter_mut() {
        for v in run.settings.values_mut() {
            if let Value::Num(x) = v {
                *x = round(*x, 6);
            }
        }
    }
    e.project.runs.len()
}

fn save(e: &mut Engine, dir: &Path, file: &str) -> PathBuf {
    let text = call(e, "project.save", json!({}))["text"].as_str().unwrap().to_string();
    let path = dir.join(file);
    std::fs::write(&path, text).expect("write project file");
    path
}

// ---------------------------------------------------------------------------
// Numeric helpers
// ---------------------------------------------------------------------------

fn round(x: f64, decimals: i32) -> f64 {
    let s = 10f64.powi(decimals);
    let r = (x * s).round() / s;
    if r == 0.0 { 0.0 } else { r }
}

/// Rounds to the nearest multiple of `step` (e.g. 5 mm for slump readings).
fn round_step(x: f64, step: f64) -> f64 {
    (x / step).round() * step
}

/// Linear coding of an actual value to [-1, 1].
fn coded(v: f64, lo: f64, hi: f64) -> f64 {
    (v - (lo + hi) / 2.0) / ((hi - lo) / 2.0)
}

fn num(run: &sdoe::project::Run, id: u64) -> f64 {
    run.settings[&id].as_num().expect("numeric setting")
}

fn text(run: &sdoe::project::Run, id: u64) -> String {
    match &run.settings[&id] {
        Value::Text(t) => t.clone(),
        Value::Num(x) => format!("{x}"),
    }
}

/// Indices of the runs sorted by run order.
fn by_run_order(e: &Engine) -> Vec<usize> {
    let mut idx: Vec<usize> = (0..e.project.runs.len()).collect();
    idx.sort_by_key(|&i| e.project.runs[i].run_order);
    idx
}

// ---------------------------------------------------------------------------
// 1. Injection moulding: Taguchi crossed array
// ---------------------------------------------------------------------------

fn injection_moulding(dir: &Path) -> PathBuf {
    println!("Building injection moulding (Taguchi) project");
    let mut e = Engine::new();
    call(&mut e, "project.set", json!({
        "title": "Injection moulding of a PA6 connector housing: robust parameter design",
        "notes": "Robust parameter study for an unfilled PA6 connector housing on a 110 t machine with a 2-cavity hot-runner mould. \
Four machine settings are varied on three levels in an L9 inner array. Each setting is run with two resin lots and at two \
conditioning humidities (outer array), because both vary in production and cannot be held constant. Five parts per condition \
are collected after the process has stabilised; the recorded values are the averages. Shrinkage is measured on the 40 mm \
length after 24 h conditioning (target 1.20 %), weight on a 0.01 g balance and warpage as flatness deviation on the CMM. \
Use Analysis > Robustness with the nominal-the-best signal-to-noise ratio for shrinkage and smaller-the-better for warpage."
    }));
    let melt = factor(&mut e, "Melt temperature", "°C", "Continuous", "Controlled", 230.0, 270.0, "");
    let hold = factor(&mut e, "Hold pressure", "bar", "Continuous", "Controlled", 400.0, 700.0, "");
    let cool = factor(&mut e, "Cooling time", "s", "Continuous", "Controlled", 8.0, 16.0, "");
    let mould = factor(&mut e, "Mould temperature", "°C", "Continuous", "Controlled", 50.0, 90.0, "");
    let lot = factor(&mut e, "Resin lot", "", "Categorical", "Noise", 0.0, 0.0, "Lot 4471, Lot 4518");
    let hum = factor(&mut e, "Conditioning humidity", "%RH", "Discrete", "Noise", 0.0, 0.0, "35, 65");
    let shrink = response(&mut e, "Shrinkage", "%", "Target", "1.1", "1.2", "1.3", 1.0, 5.0);
    let weight = response(&mut e, "Part weight", "g", "In range", "18.4", "", "18.9", 1.0, 2.0);
    let warp = response(&mut e, "Warpage", "mm", "Minimize", "0.05", "", "0.25", 1.0, 3.0);
    let n = apply_design(&mut e, "taguchi", json!({"levels": "3", "inner": "L9", "outer": "full"}), 1207);
    println!("    {n} runs");

    let mut rng = Rng::new(4471);
    let order = by_run_order(&e);
    for (k, &i) in order.iter().enumerate() {
        let run = &mut e.project.runs[i];
        let a = coded(num(run, melt), 230.0, 270.0);
        let b = coded(num(run, hold), 400.0, 700.0);
        let c = coded(num(run, cool), 8.0, 16.0);
        let d = coded(num(run, mould), 50.0, 90.0);
        let l = if text(run, lot) == "Lot 4471" { -1.0 } else { 1.0 };
        let h = coded(num(run, hum), 35.0, 65.0);
        // The lot-to-lot difference is smaller at higher hold pressure, and the
        // humidity sensitivity is smaller with a warmer mould.
        let lot_eff = 0.085 * l * (1.0 - 0.85 * (b + 1.0) / 2.0);
        let hum_eff = 0.050 * h * (1.0 - 0.6 * d);
        let s = 1.24 + 0.04 * a - 0.11 * b + 0.035 * b * b - 0.045 * c + 0.07 * d + 0.02 * a * d + lot_eff + hum_eff + 0.010 * rng.normal();
        let w = 18.62 + 0.05 * a + 0.13 * b - 0.04 * b * b + 0.03 * c - 0.02 * d + 0.035 * l + 0.025 * h + 0.012 * rng.normal();
        let wp = 0.13 - 0.045 * c + 0.02 * c * c + 0.03 * d + 0.012 * a - 0.015 * b + 0.012 * l * (1.0 - 0.5 * c) + 0.006 * h + 0.006 * rng.normal();
        run.status = RunStatus::Done;
        // Short shot: lowest melt temperature and hold pressure combined with the
        // higher-viscosity resin lot at the higher humidity.
        if a < -0.5 && b < -0.5 && l > 0.0 && h > 0.0 {
            run.status = RunStatus::Excluded;
            run.note = "Short shot: latch tip not filled on cavity 2; no parts measured".into();
            continue;
        }
        run.responses.insert(shrink, round(s, 3));
        run.responses.insert(warp, round(wp.max(0.02), 3));
        if k == 22 {
            run.note = "Balance out of calibration during this condition; weight not recorded".into();
        } else {
            run.responses.insert(weight, round(w, 2));
        }
    }
    let path = save(&mut e, dir, "mfg-injection-molding-taguchi.sdoe");
    println!("    wrote {}", path.display());
    path
}

// ---------------------------------------------------------------------------
// 2. FDM 3D printing: I-optimal design with mixed factor types
// ---------------------------------------------------------------------------

fn fdm_printing(dir: &Path) -> PathBuf {
    println!("Building FDM printing (optimal design) project");
    let mut e = Engine::new();
    call(&mut e, "project.set", json!({
        "title": "FDM tensile bars: layer height, material, nozzle temperature and print speed",
        "notes": "ISO 527-2 type 1A tensile bars printed flat (XY, ±45° raster, 3 perimeters) on an enclosed FDM printer with a 0.4 mm \
nozzle. Nozzle temperature is set as an offset from each material's datasheet value (PLA 210 °C, PETG 240 °C, ABS 250 °C). \
Spools stay on the printer between prints, so the filament moisture content rises over the campaign; it is measured with a \
moisture analyser before each print and recorded as an environmental factor. The PETG spool was dried once during the \
campaign (see run notes). Tensile strength is the mean of three bars per run; dimensional error is the mean absolute deviation \
of three 20.00 mm reference features printed on the same plate."
    }));
    let layer = factor(&mut e, "Layer height", "mm", "Discrete", "Controlled", 0.0, 0.0, "0.1, 0.15, 0.2, 0.3");
    let mat = factor(&mut e, "Material", "", "Categorical", "Controlled", 0.0, 0.0, "PLA, PETG, ABS");
    let temp = factor(&mut e, "Nozzle temperature offset", "°C", "Continuous", "Controlled", -15.0, 15.0, "");
    let speed = factor(&mut e, "Print speed", "mm/s", "Continuous", "Controlled", 30.0, 90.0, "");
    let moist = factor(&mut e, "Filament moisture", "%", "Continuous", "Environmental", 0.05, 0.35, "");
    let ts = response(&mut e, "Tensile strength", "MPa", "Maximize", "30", "", "55", 1.0, 4.0);
    let de = response(&mut e, "Dimensional error", "mm", "Minimize", "0.03", "", "0.20", 1.0, 3.0);
    let n = apply_design(&mut e, "optimal", json!({"criterion": "I", "model": "quadratic", "runs": 34, "starts": 30, "seed": 17}), 3301);
    println!("    {n} runs");

    let mut rng = Rng::new(3302);
    let order = by_run_order(&e);
    // Moisture of each spool (% w/w): starting value and uptake per print (about 1.5 h).
    let mut spool: std::collections::BTreeMap<&str, f64> = [("PLA", 0.09), ("PETG", 0.11), ("ABS", 0.13)].into_iter().collect();
    let uptake: std::collections::BTreeMap<&str, f64> = [("PLA", 0.006), ("PETG", 0.016), ("ABS", 0.009)].into_iter().collect();
    let mut petg_dried = false;
    for (k, &i) in order.iter().enumerate() {
        let run = &mut e.project.runs[i];
        let m_name = text(run, mat);
        let key: &str = match m_name.as_str() {
            "PLA" => "PLA",
            "PETG" => "PETG",
            _ => "ABS",
        };
        if key == "PETG" && !petg_dried && k >= 18 {
            spool.insert("PETG", 0.05);
            petg_dried = true;
            run.note = "PETG spool dried 6 h at 65 °C before this print".into();
        }
        let m_true = spool[key];
        let m_meas = round((m_true + 0.008 * rng.normal()).max(0.02), 2);
        spool.insert(key, m_true + uptake[key] * (0.8 + 0.4 * rng.uniform()));
        run.settings.insert(moist, Value::Num(m_meas));

        let lh = (num(run, layer) - 0.2) / 0.1;
        let tc = coded(num(run, temp), -15.0, 15.0);
        let sp = coded(num(run, speed), 30.0, 90.0);
        let mc = m_true - 0.25;
        let base = match key {
            "PLA" => 52.0,
            "PETG" => 45.0,
            _ => 37.0,
        };
        // Strength decreases with moisture, more so at higher nozzle temperature (hydrolysis, steam voids).
        let strength = base - 3.2 * lh - 1.0 * lh * lh + 2.4 * tc - 1.6 * tc * tc - 2.1 * sp + 0.9 * tc * sp - 11.0 * mc - 13.0 * mc * tc + 0.9 * rng.normal();
        let mat_err = match key {
            "PLA" => 0.0,
            "PETG" => 0.015,
            _ => 0.045,
        };
        let err = 0.085 + mat_err + 0.030 * lh + 0.022 * sp + 0.014 * sp * sp + 0.012 * tc + 0.06 * mc + 0.006 * rng.normal();
        run.status = RunStatus::Done;
        if key == "ABS" && sp > 0.5 && lh < -0.4 && run.note.is_empty() {
            run.status = RunStatus::Excluded;
            run.note = "Bar corner lifted from the bed (warping); print aborted at 60 %".into();
            continue;
        }
        if k == 9 {
            run.note = "One bar slipped in the grips; strength of this run not recorded".into();
        } else {
            run.responses.insert(ts, round(strength, 1));
        }
        run.responses.insert(de, round(err.max(0.015), 3));
    }
    // Strength model: quadratic in the printed factors with moisture and its
    // interactions, reduced by stepwise AICc.
    call(&mut e, "model.select", json!({"response": ts.to_string(), "method": "aicc", "pool": "quadratic", "env_interactions": true}));
    call(&mut e, "model.select", json!({"response": de.to_string(), "method": "aicc", "pool": "quadratic", "env_interactions": false}));
    let path = save(&mut e, dir, "mfg-fdm-tensile-optimal.sdoe");
    println!("    wrote {}", path.display());
    path
}

// ---------------------------------------------------------------------------
// 3. Face milling: face-centred CCD with tool wear as environmental factor
// ---------------------------------------------------------------------------

fn face_milling(dir: &Path) -> PathBuf {
    println!("Building face milling (CCD) project");
    let mut e = Engine::new();
    call(&mut e, "project.set", json!({
        "title": "Face milling of 42CrMo4: surface roughness and spindle power",
        "notes": "Face milling of quenched and tempered 42CrMo4 (approx. 300 HB) blocks on a 5-axis machining centre with a Ø50 mm \
face mill, 4 coated carbide inserts, emulsion coolant. Each run machines one 200 mm long pass. Flank wear VB of the leading \
insert is measured with a tool microscope before each run and recorded as an environmental factor; the insert set is replaced \
when VB exceeds 0.25 mm (see run notes). Ra is the mean of three profilometer traces across the feed marks (λc = 0.8 mm); \
spindle power is the mean cutting power from the spindle drive minus the idle power at the same speed."
    }));
    let vc = factor(&mut e, "Cutting speed", "m/min", "Continuous", "Controlled", 150.0, 250.0, "");
    let fz = factor(&mut e, "Feed per tooth", "mm", "Continuous", "Controlled", 0.06, 0.14, "");
    let ap = factor(&mut e, "Axial depth of cut", "mm", "Continuous", "Controlled", 0.5, 2.0, "");
    let ae = factor(&mut e, "Radial engagement", "%", "Continuous", "Controlled", 30.0, 70.0, "");
    let vb = factor(&mut e, "Flank wear VB", "mm", "Continuous", "Environmental", 0.0, 0.3, "");
    let ra = response(&mut e, "Surface roughness Ra", "µm", "Minimize", "0.4", "", "1.6", 1.0, 4.0);
    let pw = response(&mut e, "Spindle power", "kW", "Maximize", "0.5", "", "2.5", 1.0, 2.0);
    let n = apply_design(&mut e, "ccd", json!({"alpha": "face", "fraction": "full", "center_points": 6}), 5150);
    println!("    {n} runs");

    let mut rng = Rng::new(5151);
    let order = by_run_order(&e);
    let mut wear = 0.03;
    let mut insert_set = 1;
    for (k, &i) in order.iter().enumerate() {
        let run = &mut e.project.runs[i];
        let v = num(run, vc);
        let f = num(run, fz);
        let d = num(run, ap);
        let w = num(run, ae);
        if wear > 0.25 {
            wear = 0.02 + 0.01 * rng.uniform();
            insert_set += 1;
            run.note = format!("New insert set (set {insert_set}) mounted before this run");
        }
        let vb_meas = round((wear + 0.006 * rng.normal()).max(0.0), 2);
        run.settings.insert(vb, Value::Num(vb_meas));

        let a = coded(v, 150.0, 250.0);
        let b = coded(f, 0.06, 0.14);
        let c = coded(d, 0.5, 2.0);
        let dd = coded(w, 30.0, 70.0);
        let wc = wear - 0.13;
        // Feed marks are larger with a worn edge: wear × feed interaction.
        let r = 0.62 + 0.30 * b + 0.07 * b * b - 0.10 * a + 0.06 * a * a + 0.04 * c + 0.03 * dd - 0.04 * a * b + 1.5 * wc + 1.6 * wc * b;
        let r = r * (1.0 + 0.04 * rng.normal()) + 0.015 * rng.normal();

        // Cutting power from the specific cutting force (kc1.1 = 2100 N/mm², mc = 0.25).
        let dia = 50.0;
        let z = 4.0;
        let n_rpm = v * 1000.0 / (std::f64::consts::PI * dia);
        let vf = f * z * n_rpm;
        let ae_mm = w / 100.0 * dia;
        let hm = f * (ae_mm / dia).sqrt();
        let kc = 2100.0 * hm.powf(-0.25);
        let p = d * ae_mm * vf * kc / 6.0e7 * (1.0 + 0.5 * wear);
        let p = p * (1.0 + 0.025 * rng.normal());

        wear += 0.0045 * (v / 200.0).powi(3) * (0.6 + 0.4 * d / 1.25) * (0.7 + 0.3 * w / 50.0) * (0.9 + 0.2 * rng.uniform()) + 0.004;

        run.status = RunStatus::Done;
        run.responses.insert(pw, round(p, 2));
        if c > 0.5 && dd > 0.5 && a > 0.5 && b < -0.5 {
            run.status = RunStatus::Excluded;
            run.note = "Chatter marks over the whole pass; values recorded but not representative".into();
            run.responses.insert(ra, round(r * 2.4, 2));
            continue;
        }
        if k == 14 {
            if run.note.is_empty() {
                run.note = "Profilometer stylus damaged; Ra not measured".into();
            }
        } else {
            run.responses.insert(ra, round(r.max(0.15), 2));
        }
    }
    call(&mut e, "model.select", json!({"response": ra.to_string(), "method": "aicc", "pool": "quadratic", "env_interactions": true}));
    call(&mut e, "model.set", json!({"response": pw.to_string(), "formula": "(A+B+C+D)^2 + A^2 + B^2 + C^2 + D^2 + E", "transform": "Ln"}));
    call(&mut e, "model.select", json!({"response": pw.to_string(), "method": "backward", "alpha": 0.05}));
    let path = save(&mut e, dir, "mfg-face-milling-ccd.sdoe");
    println!("    wrote {}", path.display());
    path
}

// ---------------------------------------------------------------------------
// 4. Concrete binder: constrained mixture, D-optimal
// ---------------------------------------------------------------------------

fn concrete_mixture(dir: &Path) -> PathBuf {
    println!("Building concrete binder (mixture) project");
    let mut e = Engine::new();
    call(&mut e, "project.set", json!({
        "title": "Blended-cement concrete: binder composition for C40/50",
        "mixture_total": 400.0,
        "notes": "Binder composition study for a C40/50 structural concrete with 400 kg/m³ total binder, water/binder ratio 0.42, \
1780 kg/m³ aggregate (0/4 sand and 4/16 crushed limestone) and a fixed PCE superplasticiser dosage of 0.8 % of binder. \
The four binder components are dosed in kg/m³ and always add up to 400 kg/m³; the component limits are based on EN 206 practice \
and plant silo capacity; the design blends are rounded to whole kilograms for batching. Mixes are cast in a 60 L laboratory pan mixer, four per day, slump is read to the nearest 5 mm, \
and strength is the mean of three 150 mm cubes cured in water at 20 °C. The fresh concrete temperature is measured after \
mixing and recorded as an environmental factor."
    }));
    let cem = factor(&mut e, "Portland cement CEM I 52.5 R", "kg/m³", "Mixture", "Controlled", 200.0, 400.0, "");
    let fa = factor(&mut e, "Fly ash", "kg/m³", "Mixture", "Controlled", 0.0, 120.0, "");
    let sl = factor(&mut e, "Ground granulated slag", "kg/m³", "Mixture", "Controlled", 0.0, 180.0, "");
    let sf = factor(&mut e, "Silica fume", "kg/m³", "Mixture", "Controlled", 0.0, 40.0, "");
    let tf = factor(&mut e, "Fresh concrete temperature", "°C", "Continuous", "Environmental", 15.0, 28.0, "");
    let s7 = response(&mut e, "7-day compressive strength", "MPa", "Maximize", "30", "", "45", 1.0, 2.0);
    let s28 = response(&mut e, "28-day compressive strength", "MPa", "Maximize", "50", "", "65", 1.0, 5.0);
    let slump = response(&mut e, "Slump", "mm", "In range", "80", "", "140", 1.0, 3.0);
    let n = apply_design(&mut e, "optimal", json!({"criterion": "D", "model": "quadratic", "runs": 24, "starts": 40, "seed": 23}), 4020);
    println!("    {n} runs");

    // Batching precision is 1 kg: the supplementary materials are rounded and
    // cement is set to the remainder, so every blend still adds up to 400 kg/m³.
    for run in e.project.runs.iter_mut() {
        let mut rest = 400.0;
        for id in [fa, sl, sf] {
            let v = num(run, id).round();
            rest -= v;
            run.settings.insert(id, Value::Num(v));
        }
        run.settings.insert(cem, Value::Num(rest));
    }
    let mut rng = Rng::new(4021);
    let order = by_run_order(&e);
    for (k, &i) in order.iter().enumerate() {
        let run = &mut e.project.runs[i];
        let x1 = num(run, cem) / 400.0;
        let x2 = num(run, fa) / 400.0;
        let x3 = num(run, sl) / 400.0;
        let x4 = num(run, sf) / 400.0;
        // Mixing day (4 mixes per day): mornings cooler, slow warming over the week.
        let day = (k / 4) as f64;
        let slot = (k % 4) as f64;
        let t_true = 17.0 + 0.9 * day + 1.3 * slot + 0.4 * rng.normal();
        let t_meas = round(t_true, 1);
        run.settings.insert(tf, Value::Num(t_meas));
        let dt = t_true - 20.0;
        let s28v = 49.0 * x1 + 24.0 * x2 + 41.0 * x3 + 98.0 * x4 + 34.0 * x1 * x3 + 150.0 * x1 * x4 + 22.0 * x1 * x2 + 0.18 * dt + 1.3 * rng.normal();
        let s7v = 38.0 * x1 + 9.0 * x2 + 17.0 * x3 + 75.0 * x4 + 12.0 * x1 * x3 + 70.0 * x1 * x4 + 0.45 * dt + 1.1 * rng.normal();
        let sl_v = 92.0 * x1 + 185.0 * x2 + 128.0 * x3 - 260.0 * x4 + 120.0 * x2 * x3 - 1.6 * dt + 7.0 * rng.normal();
        run.status = RunStatus::Done;
        run.responses.insert(s28, round(s28v, 1));
        run.responses.insert(slump, round_step(sl_v.max(10.0), 5.0));
        if k == 6 {
            run.note = "One 7-day cube damaged during demoulding and the remaining two differ by 18 %; 7-day result discarded".into();
        } else {
            run.responses.insert(s7, round(s7v, 1));
        }
    }
    // Reduced Scheffé models from stepwise AICc selection.
    for rid in [s7, s28, slump] {
        let r = call(&mut e, "model.select", json!({"response": rid.to_string(), "method": "aicc"}));
        println!("    reduced model: {}", r["formula"].as_str().unwrap_or(""));
    }
    let path = save(&mut e, dir, "mfg-concrete-binder-mixture.sdoe");
    println!("    wrote {}", path.display());
    path
}

// ---------------------------------------------------------------------------
// 5. Laser welding: definitive screening, planned (no results yet)
// ---------------------------------------------------------------------------

fn laser_welding_planned(dir: &Path) -> PathBuf {
    println!("Building laser welding (DSD, planned) project");
    let mut e = Engine::new();
    call(&mut e, "project.set", json!({
        "title": "Fibre laser welding of 6082-T6 butt joints: screening plan",
        "notes": "Screening plan for autogenous fibre laser welding of 3 mm 6082-T6 aluminium butt joints (6 kW fibre laser, \
Ø0.3 mm fibre, 200 mm welds, coupons cleaned and degreased within 4 h before welding). In the definitive screening design, main \
effects are unaliased with two-factor interactions and curvature is detectable. Joint gap is measured with feeler gauges at \
three positions per coupon and the ambient humidity in the cell is read from the logger at the start of each weld; enter \
both before analysing. Penetration and porosity come from one metallographic cross-section at mid-length, tensile strength \
from two transverse specimens per weld (ISO 4136)."
    }));
    factor(&mut e, "Laser power", "kW", "Continuous", "Controlled", 2.5, 4.5, "");
    factor(&mut e, "Welding speed", "m/min", "Continuous", "Controlled", 2.0, 6.0, "");
    factor(&mut e, "Focal position", "mm", "Continuous", "Controlled", -2.0, 1.0, "");
    factor(&mut e, "Shielding gas flow", "L/min", "Continuous", "Controlled", 10.0, 25.0, "");
    factor(&mut e, "Shielding gas", "", "Categorical", "Controlled", 0.0, 0.0, "Argon, Helium");
    factor(&mut e, "Beam oscillation amplitude", "mm", "Continuous", "Controlled", 0.0, 1.0, "");
    factor(&mut e, "Joint gap", "mm", "Continuous", "Environmental", 0.0, 0.3, "");
    factor(&mut e, "Ambient humidity", "%RH", "Continuous", "Environmental", 25.0, 75.0, "");
    response(&mut e, "Penetration depth", "mm", "In range", "3.0", "", "3.6", 1.0, 5.0);
    response(&mut e, "Porosity", "% area", "Minimize", "0.2", "", "1.5", 1.0, 4.0);
    response(&mut e, "Tensile strength", "MPa", "Maximize", "180", "", "240", 1.0, 3.0);
    response(&mut e, "Top bead width", "mm", "None", "", "", "", 1.0, 1.0);
    let n = apply_design(&mut e, "dsd", json!({"extra_center": 2}), 6082);
    println!("    {n} runs");
    let path = save(&mut e, dir, "mfg-laser-welding-dsd-planned.sdoe");
    println!("    wrote {}", path.display());
    path
}

// ---------------------------------------------------------------------------
// 6. Heat treatment: empty template
// ---------------------------------------------------------------------------

fn heat_treatment_template(dir: &Path) -> PathBuf {
    println!("Building heat treatment template");
    let mut e = Engine::new();
    call(&mut e, "project.set", json!({
        "title": "Quench and temper of 42CrMo4 shafts (template)",
        "notes": "Template for a hardening and tempering study on Ø40 mm 42CrMo4 shafts in a sealed-quench furnace. Factors and \
responses are defined; no design is applied yet. The quench bath temperature drifts with the load sequence and is recorded \
as an environmental factor. Suggested start: an optimal (I) design for a quadratic model with about 24 runs, or a \
response-surface design after a screening round. Hardness is measured at mid-radius (HRC, mean of five indents), impact \
energy with Charpy V specimens at -20 °C (mean of three), distortion as total runout between centres."
    }));
    factor(&mut e, "Austenitising temperature", "°C", "Continuous", "Controlled", 830.0, 880.0, "");
    factor(&mut e, "Soak time", "min", "Continuous", "Controlled", 30.0, 90.0, "");
    factor(&mut e, "Quench medium", "", "Categorical", "Controlled", 0.0, 0.0, "Oil, Polymer 10 %, Polymer 20 %");
    factor(&mut e, "Tempering temperature", "°C", "Continuous", "Controlled", 450.0, 650.0, "");
    factor(&mut e, "Tempering time", "h", "Discrete", "Controlled", 0.0, 0.0, "1, 2, 4");
    factor(&mut e, "Quench bath temperature", "°C", "Continuous", "Environmental", 30.0, 70.0, "");
    response(&mut e, "Hardness", "HRC", "Target", "30", "32", "34", 1.0, 5.0);
    response(&mut e, "Charpy impact energy", "J", "Maximize", "40", "", "100", 1.0, 4.0);
    response(&mut e, "Runout", "mm", "Minimize", "0.03", "", "0.15", 1.0, 3.0);
    let path = save(&mut e, dir, "mfg-heat-treatment-template.sdoe");
    println!("    wrote {}", path.display());
    path
}

// ---------------------------------------------------------------------------
// Verification
// ---------------------------------------------------------------------------

fn verify(path: &Path, taguchi: &[(&str, &str)]) {
    let text = std::fs::read_to_string(path).expect("read back");
    let mut e = Engine::new();
    call(&mut e, "project.open", json!({"text": text}));
    let st = call(&mut e, "project.state", json!({}));
    println!(
        "\n== {} ==\n   \"{}\"\n   factors {}, responses {}, runs {}, done {}, design {}",
        path.file_name().unwrap().to_string_lossy(),
        st["title"].as_str().unwrap_or(""),
        st["factors"],
        st["responses"],
        st["runs"],
        st["done"],
        st["design"]["method"]
    );
    let factors = call(&mut e, "factors.list", json!({}));
    for f in factors.as_array().unwrap() {
        println!("   {} {} [{}] {} {}", f["symbol"].as_str().unwrap(), f["name"].as_str().unwrap(), f["units"].as_str().unwrap(), f["kind"].as_str().unwrap(), f["role"].as_str().unwrap());
    }
    let responses = call(&mut e, "responses.list", json!({}));
    for r in responses.as_array().unwrap() {
        let rid = r["id"].as_str().unwrap().to_string();
        let name = r["name"].as_str().unwrap();
        if r["count"].as_u64().unwrap_or(0) == 0 {
            println!("   response {name}: no data");
            continue;
        }
        let fit = call(&mut e, "analysis.fit", json!({"response": rid}));
        let stats: Vec<(String, String)> = serde_json::from_value(fit["stats"].clone()).unwrap();
        let get = |k: &str| stats.iter().find(|s| s.0 == k).map(|s| s.1.clone()).unwrap_or_default();
        println!(
            "   response {name}: n={} R²={} adjR²={} predR²={} sd={}  transform {}\n     formula {}",
            get("Runs used"),
            get("R²"),
            get("Adjusted R²"),
            get("Predicted R²"),
            get("Std. deviation"),
            fit["transform"].as_str().unwrap_or(""),
            fit["formula"].as_str().unwrap_or("")
        );
        let co = &fit["coefficients"];
        let sig: Vec<String> = co["rows"]
            .as_array()
            .unwrap()
            .iter()
            .zip(co["emphasis"].as_array().unwrap())
            .filter(|(_, em)| em.as_str() == Some("good"))
            .map(|(row, _)| format!("{} (p {})", row[0].as_str().unwrap(), row[4].as_str().unwrap()))
            .collect();
        println!("     significant (p<0.05): {}", sig.join(", "));
        for w in fit["warnings"].as_array().unwrap() {
            println!("     warning: {}", w.as_str().unwrap());
        }
        if let Some((_, sn)) = taguchi.iter().find(|(n, _)| *n == name) {
            let rb = call(&mut e, "analysis.robustness", json!({"response": rid, "sn": sn}));
            let best: Vec<String> = rb["best"].as_array().unwrap().iter().map(|b| b.as_str().unwrap().to_string()).collect();
            println!("     robustness ({sn}): {} inner settings; best S/N: {}", rb["table"]["rows"].as_array().unwrap().len(), best.join("; "));
            // Range of mean S/N per factor.
            for pl in rb["plots"].as_array().unwrap().iter().step_by(2) {
                let ys: Vec<f64> = pl["series"][0]["y"].as_array().unwrap().iter().map(|v| v.as_f64().unwrap()).collect();
                let lo = ys.iter().cloned().fold(f64::INFINITY, f64::min);
                let hi = ys.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
                println!("       S/N range {:>6.2} dB  {}", hi - lo, pl["x_label"].as_str().unwrap());
            }
        }
    }
    // Planned projects: environmental columns must be blank.
    let env: Vec<u64> = e.project.factors.iter().filter(|f| f.role == sdoe::project::Role::Environmental).map(|f| f.id).collect();
    let planned = e.project.runs.iter().filter(|r| r.status == RunStatus::Planned).count();
    if planned > 0 {
        let filled = e.project.runs.iter().filter(|r| env.iter().any(|id| r.settings.contains_key(id))).count();
        println!("   planned runs {planned}; runs with environmental values {filled}");
    }
    // Report and CSV export must work for every file.
    call(&mut e, "export.csv", json!({}));
    call(&mut e, "export.report", json!({}));
}

fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().expect("workspace root").to_path_buf();
    let dir = root.join("examples");
    std::fs::create_dir_all(&dir).expect("create examples directory");

    let files = vec![
        (injection_moulding(&dir), vec![("Shrinkage", "nominal"), ("Warpage", "smaller"), ("Part weight", "nominal")]),
        (fdm_printing(&dir), vec![]),
        (face_milling(&dir), vec![]),
        (concrete_mixture(&dir), vec![]),
        (laser_welding_planned(&dir), vec![]),
        (heat_treatment_template(&dir), vec![]),
    ];
    println!("\nVerification");
    for (path, tag) in &files {
        verify(path, tag);
    }
}
