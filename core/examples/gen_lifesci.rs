//! Generates example projects for food science, pharmaceuticals and
//! biotechnology (files `examples/bio-*.sdoe` in the workspace root), then
//! reloads every file and runs the analyses as a check.
//!
//! Run with `cargo run --release -p sdoe-core --example gen_lifesci`.

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

enum Kind<'a> {
    Cont(f64, f64),
    Mix(f64, f64),
    Disc(&'a str),
    Cat(&'a str),
}

/// Adds a factor through the factor sheet and returns its id.
fn factor(e: &mut Engine, name: &str, units: &str, kind: Kind, role: &str) -> u64 {
    let row = e.project.factors.len();
    set(e, "factors", row, 1, name);
    if !units.is_empty() {
        set(e, "factors", row, 2, units);
    }
    match kind {
        Kind::Cont(lo, hi) | Kind::Mix(lo, hi) => {
            let t = if matches!(kind, Kind::Mix(..)) { "Mixture" } else { "Continuous" };
            set(e, "factors", row, 3, t);
            // High first: the default range of a new factor is -1..1 (0..total for mixtures).
            set(e, "factors", row, 6, &hi.to_string());
            set(e, "factors", row, 5, &lo.to_string());
        }
        Kind::Disc(levels) => {
            set(e, "factors", row, 3, "Discrete");
            set(e, "factors", row, 7, levels);
        }
        Kind::Cat(levels) => {
            set(e, "factors", row, 3, "Categorical");
            set(e, "factors", row, 7, levels);
        }
    }
    set(e, "factors", row, 4, role);
    e.project.factors[row].id
}

/// Adds a response through the response sheet and returns its id.
#[allow(clippy::too_many_arguments)]
fn response(e: &mut Engine, name: &str, units: &str, goal: &str, lower: Option<f64>, target: Option<f64>, upper: Option<f64>, importance: f64) -> u64 {
    let row = e.project.responses.len();
    set(e, "responses", row, 0, name);
    set(e, "responses", row, 1, units);
    set(e, "responses", row, 2, goal);
    for (c, v) in [(3, lower), (4, target), (5, upper)] {
        if let Some(v) = v {
            set(e, "responses", row, c, &v.to_string());
        }
    }
    set(e, "responses", row, 7, &importance.to_string());
    e.project.responses[row].id
}

fn design(e: &mut Engine, method: &str, options: Json, seed: u64) -> Json {
    let pv = call(e, "design.preview", json!({"method": method, "options": options}));
    call(e, "design.apply", json!({"mode": "replace", "randomize": true, "seed": seed}));
    pv
}

/// Indices of the runs in run order.
fn run_order(e: &Engine) -> Vec<usize> {
    let mut idx: Vec<usize> = (0..e.project.runs.len()).collect();
    idx.sort_by_key(|&i| e.project.runs[i].run_order);
    idx
}

fn num(e: &Engine, ri: usize, id: u64) -> f64 {
    e.project.runs[ri].settings[&id].as_num().expect("numeric setting")
}

fn text(e: &Engine, ri: usize, id: u64) -> String {
    match &e.project.runs[ri].settings[&id] {
        Value::Text(t) => t.clone(),
        Value::Num(v) => v.to_string(),
    }
}

/// Coded value (-1..1) of an actual value on [lo, hi].
fn coded(x: f64, lo: f64, hi: f64) -> f64 {
    (x - (lo + hi) / 2.0) / ((hi - lo) / 2.0)
}

/// Rounds to `d` decimals (negative `d` rounds to tens, hundreds…).
fn rnd(x: f64, d: i32) -> f64 {
    if d >= 0 {
        let f = 10f64.powi(d);
        (x * f).round() / f
    } else {
        let f = 10f64.powi(-d);
        (x / f).round() * f
    }
}

fn put(e: &mut Engine, ri: usize, rid: u64, v: f64) {
    e.project.runs[ri].responses.insert(rid, v);
}

fn put_setting(e: &mut Engine, ri: usize, fid: u64, v: Value) {
    e.project.runs[ri].settings.insert(fid, v);
}

fn done(e: &mut Engine, ri: usize) {
    e.project.runs[ri].status = RunStatus::Done;
}

fn exclude(e: &mut Engine, ri: usize, note: &str) {
    e.project.runs[ri].status = RunStatus::Excluded;
    e.project.runs[ri].note = note.into();
}

fn note(e: &mut Engine, ri: usize, note: &str) {
    e.project.runs[ri].note = note.into();
}

fn save(e: &mut Engine, dir: &Path, file: &str) -> PathBuf {
    let text = call(e, "project.save", json!({}))["text"].as_str().unwrap().to_string();
    let path = dir.join(file);
    std::fs::write(&path, text).expect("write project file");
    path
}

// ---------------------------------------------------------------------------
// 1. Tablet formulation: mixture + process factor + room humidity
// ---------------------------------------------------------------------------

fn tablet(dir: &Path) -> PathBuf {
    let mut e = Engine::new();
    call(&mut e, "project.set", json!({
        "title": "Immediate-release tablet: excipient blend and compression force",
        "mixture_total": 85,
        "notes": "Direct-compression tablet, 400 mg, with 14 % w/w API and 1 % w/w magnesium stearate held constant; \
the remaining 85 % w/w is split between microcrystalline cellulose (filler), povidone K30 (binder) and croscarmellose \
sodium (disintegrant). Each blend was compressed on a single-station instrumented press at the planned force. \
Room relative humidity was logged during compression because the binder and the disintegrant are hygroscopic. \
Hardness is the mean of 10 tablets, disintegration time the slowest of 6 tablets (Ph. Eur. 2.9.1, water, 37 °C) \
and dissolution the mean of 6 vessels (USP II, 50 rpm, pH 6.8). Quality targets: hardness at least 80 N, \
disintegration below 3 min, dissolution at 30 min near 90 % (not below 80 %)."
    }));
    let a = factor(&mut e, "Microcrystalline cellulose", "% w/w", Kind::Mix(72.0, 82.0), "Controlled");
    let b = factor(&mut e, "Povidone K30", "% w/w", Kind::Mix(2.0, 8.0), "Controlled");
    let c = factor(&mut e, "Croscarmellose sodium", "% w/w", Kind::Mix(1.0, 6.0), "Controlled");
    let d = factor(&mut e, "Compression force", "kN", Kind::Cont(8.0, 20.0), "Controlled");
    let rh = factor(&mut e, "Room relative humidity", "%RH", Kind::Cont(30.0, 60.0), "Environmental");
    let y_h = response(&mut e, "Hardness", "N", "Maximize", Some(80.0), None, Some(140.0), 3.0);
    let y_dt = response(&mut e, "Disintegration time", "s", "Minimize", Some(30.0), None, Some(180.0), 4.0);
    let y_ds = response(&mut e, "Dissolution at 30 min", "%", "Target", Some(80.0), Some(90.0), Some(100.0), 5.0);
    design(
        &mut e,
        "optimal",
        json!({"criterion": "I", "model": "custom", "formula": "A + B + C + A*B + A*C + B*C + A*D + B*D + C*D", "runs": 22, "starts": 30, "seed": 4}),
        101,
    );
    let order = run_order(&e);
    let mut rng = Rng::new(1101);
    let span = 85.0 - (72.0 + 2.0 + 1.0);
    let n = order.len();
    for (k, &ri) in order.iter().enumerate() {
        // Blends as weighed (0.01 % w/w), with the filler making up the balance.
        let bv = rnd(num(&e, ri, b), 2);
        let cv = rnd(num(&e, ri, c), 2);
        put_setting(&mut e, ri, b, Value::Num(bv));
        put_setting(&mut e, ri, c, Value::Num(cv));
        put_setting(&mut e, ri, a, Value::Num(rnd(85.0 - bv - cv, 2)));
        // Pseudo-components.
        let za = (num(&e, ri, a) - 72.0) / span;
        let zb = (num(&e, ri, b) - 2.0) / span;
        let zc = (num(&e, ri, c) - 1.0) / span;
        let f = coded(num(&e, ri, d), 8.0, 20.0);
        // Humidity: a humid spell over the second half of the campaign.
        let t = k as f64 / (n - 1) as f64;
        let hum = 34.0 + 21.0 * t + 3.0 * (6.0 * t).sin() + 1.5 * rng.normal();
        let hum = rnd(hum.clamp(28.0, 62.0), 0);
        put_setting(&mut e, ri, rh, Value::Num(hum));
        let hc = coded(hum, 30.0, 60.0);
        let hard = 78.0 * za + 158.0 * zb + 62.0 * zc + 30.0 * za * zb - 25.0 * zb * zc
            + (26.0 * za + 38.0 * zb + 20.0 * zc) * f
            - 12.0 * hc
            + 3.5 * rng.normal();
        let dt_log = (95.0f64).ln() * za + (460.0f64).ln() * zb + (22.0f64).ln() * zc + 0.6 * zb * zc
            + (0.32 * za + 0.45 * zb + 0.25 * zc) * f
            + 0.10 * hc
            + 0.07 * rng.normal();
        let diss = 93.0 * za + 71.0 * zb + 97.0 * zc + 12.0 * za * zc - 8.0 * za * zb
            + (-2.0 * za - 6.5 * zb - 1.0 * zc) * f
            + 1.6 * rng.normal();
        put(&mut e, ri, y_h, rnd(hard, 0));
        put(&mut e, ri, y_dt, rnd(dt_log.exp(), 0));
        put(&mut e, ri, y_ds, rnd(diss.min(100.4), 1));
        done(&mut e, ri);
    }
    // A dissolution vessel failure and a run with capping.
    let ri = order[8];
    e.project.runs[ri].responses.remove(&y_ds);
    note(&mut e, ri, "Dissolution not reported: vessel 4 paddle height out of tolerance");
    let ri = *order
        .iter()
        .filter(|&&ri| num(&e, ri, d) > 19.0)
        .max_by(|&&x, &&y| num(&e, x, b).partial_cmp(&num(&e, y, b)).unwrap())
        .unwrap();
    exclude(&mut e, ri, "Capping on ejection; tablets unsuitable for testing");
    e.project.runs[ri].responses.remove(&y_dt);
    e.project.runs[ri].responses.remove(&y_ds);
    save(&mut e, dir, "bio-tablet-formulation.sdoe")
}

// ---------------------------------------------------------------------------
// 2. CHO media screening: Plackett-Burman, log-normal titre
// ---------------------------------------------------------------------------

fn media(dir: &Path) -> PathBuf {
    let mut e = Engine::new();
    call(&mut e, "project.set", json!({
        "title": "CHO fed-batch media supplement screening",
        "notes": "Screening of ten supplements added to a chemically defined basal medium for a CHO-K1 cell line \
producing an IgG1 antibody. Each run is a 125 mL shake flask (30 mL working volume, 14-day fed batch, \
36.5 °C set point, 5 % CO2, 125 rpm). Flasks were run in three incubators over two weeks; the mean deviation \
of the logged flask-zone temperature from the set point is recorded for each flask. Titre is the day-14 \
Protein A HPLC result and peak viable cell density the highest daily count (trypan blue, automated counter). \
The goal is to find the few supplements worth carrying into a response-surface study."
    }));
    let f = [
        factor(&mut e, "Glucose", "g/L", Kind::Cont(4.0, 8.0), "Controlled"),
        factor(&mut e, "Glutamine", "mM", Kind::Cont(2.0, 6.0), "Controlled"),
        factor(&mut e, "Yeast hydrolysate", "g/L", Kind::Cont(0.0, 5.0), "Controlled"),
        factor(&mut e, "Soy hydrolysate", "g/L", Kind::Cont(0.0, 5.0), "Controlled"),
        factor(&mut e, "Insulin", "mg/L", Kind::Cont(5.0, 15.0), "Controlled"),
        factor(&mut e, "Ferric citrate", "µM", Kind::Cont(20.0, 100.0), "Controlled"),
        factor(&mut e, "Zinc sulfate", "µM", Kind::Cont(2.0, 20.0), "Controlled"),
        factor(&mut e, "Poloxamer 188", "g/L", Kind::Cont(0.5, 2.0), "Controlled"),
        factor(&mut e, "Sodium pyruvate", "mM", Kind::Cont(0.0, 2.0), "Controlled"),
        factor(&mut e, "Ethanolamine", "µM", Kind::Cont(10.0, 50.0), "Controlled"),
    ];
    let temp = factor(&mut e, "Incubator temperature deviation", "°C", Kind::Cont(-0.8, 0.8), "Environmental");
    let y_t = response(&mut e, "Titre", "mg/L", "Maximize", Some(500.0), None, Some(2000.0), 5.0);
    let y_v = response(&mut e, "Peak viable cell density", "10^6 cells/mL", "Maximize", Some(8.0), None, Some(25.0), 2.0);
    let ranges = [(4.0, 8.0), (2.0, 6.0), (0.0, 5.0), (0.0, 5.0), (5.0, 15.0), (20.0, 100.0), (2.0, 20.0), (0.5, 2.0), (0.0, 2.0), (10.0, 50.0)];
    design(&mut e, "plackett_burman", json!({"runs": 20, "center_points": 4}), 202);
    let order = run_order(&e);
    let mut rng = Rng::new(2207);
    let n = order.len();
    // Three incubators, each with a block of consecutive flasks, its own offset and its own drift.
    let offsets = [-0.35, 0.25, 0.05];
    for (k, &ri) in order.iter().enumerate() {
        let x: Vec<f64> = (0..10).map(|j| coded(num(&e, ri, f[j]), ranges[j].0, ranges[j].1)).collect();
        let inc = (k * 3) / n;
        let td = offsets[inc] + 0.05 * (k % (n / 3 + 1)) as f64 * if inc == 1 { -1.0 } else { 1.0 } + 0.12 * rng.normal();
        let td = rnd(td.clamp(-0.8, 0.8), 2);
        put_setting(&mut e, ri, temp, Value::Num(td));
        let tc = td / 0.8;
        let flask = 0.11 * rng.normal();
        let lt = (720.0f64).ln() + 0.30 * x[0] + 0.17 * x[1] + 0.36 * x[2] + 0.02 * x[3] + 0.13 * x[5] - 0.01 * x[6]
            - 0.24 * tc
            + flask;
        let lv = (14.0f64).ln() + 0.20 * x[0] + 0.15 * x[1] + 0.12 * x[2] + 0.08 * x[7] + 0.10 * tc + 0.5 * flask + 0.06 * rng.normal();
        put(&mut e, ri, y_t, rnd(lt.exp(), 0));
        put(&mut e, ri, y_v, rnd(lv.exp(), 1));
        done(&mut e, ri);
    }
    let ri = order[13];
    exclude(&mut e, ri, "Contaminated flask (bacterial, day 6); culture terminated");
    e.project.runs[ri].responses.clear();
    let ri = order[19];
    e.project.runs[ri].responses.remove(&y_v);
    note(&mut e, ri, "Cell counter fault on day 7 and 8; peak density not captured");
    save(&mut e, dir, "bio-cho-media-screening.sdoe")
}

// ---------------------------------------------------------------------------
// 3. Bread: CCD in two baking days, flour lot, sensory panel
// ---------------------------------------------------------------------------

fn bread(dir: &Path) -> PathBuf {
    let mut e = Engine::new();
    call(&mut e, "project.set", json!({
        "title": "Pan bread: water, mixing and proof time",
        "notes": "Development of a white pan bread (straight dough, 450 g dough pieces, baked 25 min at 220 °C). \
Water addition, spiral-mixer time and final proof time were varied in a central composite design run on two \
baking days (factorial points on day 1, axial points on day 2, centre points on both). The bakery switched to \
a new flour delivery partway through the trial; the lot used for each dough is recorded. Loaf volume was \
measured by rapeseed displacement 2 h after baking, crumb firmness by texture analyser (25 % compression of a \
25 mm slice, day 1) and overall liking by a panel of 10 trained assessors on a 9-point hedonic scale (mean score)."
    }));
    let a = factor(&mut e, "Water addition", "% flour basis", Kind::Cont(58.0, 66.0), "Controlled");
    let b = factor(&mut e, "Mixing time", "min", Kind::Cont(6.0, 12.0), "Controlled");
    let c = factor(&mut e, "Proof time", "min", Kind::Cont(40.0, 70.0), "Controlled");
    let lot = factor(&mut e, "Flour lot", "", Kind::Cat("Lot 24-117, Lot 24-131"), "Environmental");
    let y_v = response(&mut e, "Loaf volume", "mL", "Maximize", Some(1900.0), None, Some(2300.0), 3.0);
    let y_f = response(&mut e, "Crumb firmness", "N", "Minimize", Some(3.0), None, Some(6.0), 3.0);
    let y_l = response(&mut e, "Overall liking", "1-9", "Maximize", Some(5.5), None, Some(7.5), 4.0);
    design(&mut e, "ccd", json!({"alpha": "rotatable", "center_points": 6, "blocks": "2"}), 303);
    // The block factor added by design.apply is the baking day.
    let bi = e.project.factors.iter().position(|f| f.name == "Block").expect("block factor");
    set(&mut e, "factors", bi, 1, "Baking day");
    set(&mut e, "factors", bi, 7, "Day 1, Day 2");
    let day = e.project.factors[bi].id;
    let order = run_order(&e);
    let mut rng = Rng::new(3303);
    // Panel: each assessor has a personal offset per day.
    let panel: Vec<f64> = (0..20).map(|_| 0.6 * rng.normal()).collect();
    let mut day2_count = 0;
    for &ri in &order {
        // Axial settings as run: water to 0.1 %, mixing to 0.1 min, proof to the minute.
        for (id, d) in [(a, 1), (b, 1), (c, 0)] {
            let v = rnd(num(&e, ri, id), d);
            put_setting(&mut e, ri, id, Value::Num(v));
        }
        let xa = coded(num(&e, ri, a), 58.0, 66.0);
        let xb = coded(num(&e, ri, b), 6.0, 12.0);
        let xc = coded(num(&e, ri, c), 40.0, 70.0);
        let d2 = text(&e, ri, day) == "Day 2";
        // The bakery starts on the new flour lot after the third dough of day 2.
        let new_lot = if d2 {
            day2_count += 1;
            day2_count > 3
        } else {
            false
        };
        put_setting(&mut e, ri, lot, Value::Text(if new_lot { "Lot 24-131" } else { "Lot 24-117" }.into()));
        let dd = if d2 { 1.0 } else { -1.0 };
        let ll = if new_lot { 1.0 } else { -1.0 };
        let vol = 2140.0 + 55.0 * xa + 40.0 * xb + 85.0 * xc - 62.0 * xa * xa - 48.0 * xb * xb - 55.0 * xc * xc + 28.0 * xa * xb
            - 18.0 * xb * xc
            + 15.0 * dd
            - 35.0 * ll
            + 22.0 * rng.normal();
        let firm = 4.6 - 0.55 * xa - 0.25 * xb - 0.60 * xc + 0.30 * xa * xa + 0.22 * xc * xc + 0.12 * xa * xc + 0.25 * ll
            + 0.18 * rng.normal();
        let latent = 6.5 + 0.25 * xa + 0.18 * xb + 0.40 * xc - 0.38 * xa * xa - 0.30 * xb * xb - 0.42 * xc * xc - 0.12 * ll;
        let off = if d2 { 10 } else { 0 };
        let mut total = 0.0;
        for p in 0..10 {
            let s = (latent + panel[off + p] + 0.9 * rng.normal()).round().clamp(1.0, 9.0);
            total += s;
        }
        put(&mut e, ri, y_v, rnd(vol, -1));
        put(&mut e, ri, y_f, rnd(firm.max(1.0), 2));
        put(&mut e, ri, y_l, rnd(total / 10.0, 1));
        done(&mut e, ri);
    }
    let ri = order[4];
    exclude(&mut e, ri, "Oven door opened during the bake; loaf collapsed");
    let v = e.project.runs[ri].responses[&y_v];
    put(&mut e, ri, y_v, v - 260.0);
    e.project.runs[ri].responses.remove(&y_l);
    let ri = order[15];
    e.project.runs[ri].responses.remove(&y_l);
    note(&mut e, ri, "Not tasted: panel session ended early");
    save(&mut e, dir, "bio-bread-ccd.sdoe")
}

// ---------------------------------------------------------------------------
// 4. HPLC method robustness: 2^(6-2) fractional factorial
// ---------------------------------------------------------------------------

fn hplc(dir: &Path) -> PathBuf {
    let mut e = Engine::new();
    call(&mut e, "project.set", json!({
        "title": "HPLC assay method robustness (ICH Q2)",
        "notes": "Robustness study for a reversed-phase assay and related-substances method (C18, 150 × 4.6 mm, 3.5 µm; \
phosphate buffer / acetonitrile, isocratic; UV 254 nm). Six method parameters were varied around their nominal \
values in a resolution IV fraction with centre points; two columns from different packing lots were used. \
Each run is the mean of duplicate injections of the system suitability solution. Acceptance criteria: \
resolution between the API and impurity B of at least 2.0, API retention time 6.5 ± 0.5 min, tailing factor \
at most 1.5."
    }));
    let a = factor(&mut e, "Mobile phase pH", "", Kind::Cont(2.8, 3.2), "Controlled");
    let b = factor(&mut e, "Column temperature", "°C", Kind::Cont(28.0, 32.0), "Controlled");
    let c = factor(&mut e, "Flow rate", "mL/min", Kind::Cont(0.9, 1.1), "Controlled");
    let d = factor(&mut e, "Acetonitrile", "% v/v", Kind::Cont(38.0, 42.0), "Controlled");
    let bc = factor(&mut e, "Buffer concentration", "mM", Kind::Cont(20.0, 30.0), "Controlled");
    let lot = factor(&mut e, "Column lot", "", Kind::Cat("Lot 0821, Lot 1107"), "Controlled");
    let y_rs = response(&mut e, "Resolution API / impurity B", "", "Maximize", Some(2.0), None, Some(3.0), 5.0);
    let y_tr = response(&mut e, "API retention time", "min", "Target", Some(6.0), Some(6.5), Some(7.0), 3.0);
    let y_tf = response(&mut e, "API tailing factor", "", "Minimize", Some(1.0), None, Some(1.5), 2.0);
    design(&mut e, "fractional_factorial", json!({"runs": "16", "center_points": 4}), 404);
    let order = run_order(&e);
    let mut rng = Rng::new(4404);
    for &ri in &order {
        let ph = num(&e, ri, a);
        let temp = num(&e, ri, b);
        let flow = num(&e, ri, c);
        let acn = num(&e, ri, d);
        let buf = num(&e, ri, bc);
        let l2 = text(&e, ri, lot) == "Lot 1107";
        let (xa, xb, xd, xe) = (coded(ph, 2.8, 3.2), coded(temp, 28.0, 32.0), coded(acn, 38.0, 42.0), coded(buf, 20.0, 30.0));
        let ll = if l2 { 1.0 } else { -1.0 };
        // Retention: log k linear in organic fraction, van 't Hoff temperature term, dead time from flow.
        let k = 3.33 * 10f64.powf(-0.042 * (acn - 40.0)) * (-0.011 * (temp - 30.0)).exp() * (1.0 + 0.035 * xa) * (1.0 + 0.015 * ll);
        let t0 = 1.5 / flow;
        let tr = t0 * (1.0 + k) * (1.0 + 0.002 * rng.normal());
        let rs = 2.48 - 0.21 * xa - 0.09 * xb - 0.14 * xd + 0.05 * xe - 0.10 * ll + 0.07 * xa * xd - 0.03 * coded(flow, 0.9, 1.1)
            + 0.035 * rng.normal();
        let tf = 1.21 + 0.05 * xa - 0.03 * xe + 0.04 * ll + 0.018 * rng.normal();
        put(&mut e, ri, y_tr, rnd(tr, 3));
        put(&mut e, ri, y_rs, rnd(rs, 2));
        put(&mut e, ri, y_tf, rnd(tf, 2));
        done(&mut e, ri);
    }
    let ri = order[6];
    e.project.runs[ri].responses.remove(&y_tf);
    note(&mut e, ri, "Tailing not reported: baseline disturbance at the peak tail in injection 2");
    save(&mut e, dir, "bio-hplc-robustness.sdoe")
}

// ---------------------------------------------------------------------------
// 5. Planned: probiotic spray drying, Box-Behnken, humidity to be measured
// ---------------------------------------------------------------------------

fn spray(dir: &Path) -> PathBuf {
    let mut e = Engine::new();
    call(&mut e, "project.set", json!({
        "title": "Spray drying of a Lactobacillus probiotic powder (planned)",
        "notes": "Planned study on a laboratory spray dryer (two-fluid nozzle, co-current) for a Lactobacillus \
rhamnosus concentrate in a skim milk / maltodextrin carrier. Inlet temperature, feed rate, atomising air flow \
and carrier solids follow a Box-Behnken design in randomized order. The absolute humidity of the inlet air \
depends on the weather and is to be recorded at the start of each run. Viability loss is log10(CFU/g dry \
matter before drying) minus log10(CFU/g after drying), plated on MRS agar; moisture by Karl Fischer; yield \
as powder collected in the cyclone relative to solids fed."
    }));
    factor(&mut e, "Inlet temperature", "°C", Kind::Cont(130.0, 170.0), "Controlled");
    factor(&mut e, "Feed rate", "mL/min", Kind::Cont(4.0, 10.0), "Controlled");
    factor(&mut e, "Atomising air flow", "L/h", Kind::Cont(400.0, 600.0), "Controlled");
    factor(&mut e, "Carrier solids", "% w/w", Kind::Cont(15.0, 30.0), "Controlled");
    factor(&mut e, "Inlet air absolute humidity", "g/kg", Kind::Cont(4.0, 14.0), "Environmental");
    response(&mut e, "Viability loss", "log CFU/g", "Minimize", Some(0.3), None, Some(1.5), 5.0);
    response(&mut e, "Moisture content", "% w/w", "In range", Some(3.0), None, Some(5.0), 4.0);
    response(&mut e, "Powder yield", "%", "Maximize", Some(40.0), None, Some(75.0), 2.0);
    response(&mut e, "Outlet temperature", "°C", "None", None, None, None, 1.0);
    design(&mut e, "box_behnken", json!({"center_points": 3}), 505);
    save(&mut e, dir, "bio-spray-drying-planned.sdoe")
}

// ---------------------------------------------------------------------------
// 6. Empty template: set yoghurt fermentation
// ---------------------------------------------------------------------------

fn yoghurt(dir: &Path) -> PathBuf {
    let mut e = Engine::new();
    call(&mut e, "project.set", json!({
        "title": "Set yoghurt fermentation (template)",
        "notes": "Template for a set-yoghurt process study: starter culture, inoculation rate, incubation \
temperature and milk solids-non-fat are the planned factors; the protein content of each milk batch is \
measured (mid-infrared analyser) before inoculation. Fermentation ends at pH 4.6; texture and syneresis are \
measured after 24 h at 4 °C and acceptability by a consumer panel on a 9-point hedonic scale. Choose a design \
under Design (for example an optimal design with a quadratic model) to create the runs."
    }));
    factor(&mut e, "Starter culture", "", Kind::Cat("Culture A (mild), Culture B (high EPS)"), "Controlled");
    factor(&mut e, "Inoculation rate", "% v/v", Kind::Cont(1.0, 3.0), "Controlled");
    factor(&mut e, "Incubation temperature", "°C", Kind::Disc("40, 42, 44"), "Controlled");
    factor(&mut e, "Milk solids-non-fat", "% w/w", Kind::Cont(9.0, 13.0), "Controlled");
    factor(&mut e, "Milk protein content", "% w/w", Kind::Cont(3.1, 3.7), "Environmental");
    response(&mut e, "Time to pH 4.6", "min", "Minimize", Some(240.0), None, Some(420.0), 3.0);
    response(&mut e, "Gel firmness", "g", "Maximize", Some(60.0), None, Some(120.0), 3.0);
    response(&mut e, "Syneresis", "% w/w", "Minimize", Some(1.0), None, Some(6.0), 4.0);
    response(&mut e, "Acceptability", "1-9", "Maximize", Some(6.0), None, Some(8.0), 5.0);
    save(&mut e, dir, "bio-yoghurt-template.sdoe")
}

// ---------------------------------------------------------------------------
// Verification
// ---------------------------------------------------------------------------

fn stat(fit: &Json, name: &str) -> String {
    fit["stats"].as_array().unwrap().iter().find(|s| s[0] == json!(name)).map(|s| s[1].as_str().unwrap().to_string()).unwrap_or_default()
}

/// Terms with p < 0.05 in the coefficient table.
fn significant(fit: &Json) -> Vec<String> {
    let t = &fit["coefficients"];
    let rows = t["rows"].as_array().unwrap();
    rows.iter()
        .filter(|r| r[4].as_str().map(|p| p.trim_start_matches('<').trim().parse::<f64>().map(|v| v < 0.05).unwrap_or(false)).unwrap_or(false))
        .map(|r| r[0].as_str().unwrap().to_string())
        .filter(|l| l != "Intercept")
        .collect()
}

fn verify(path: &Path, expect: &[(&str, &[&str])], auto: &[&str]) -> bool {
    let mut ok = true;
    let text = std::fs::read_to_string(path).unwrap();
    let mut e = Engine::new();
    call(&mut e, "project.open", json!({"text": text}));
    let st = call(&mut e, "project.state", json!({}));
    let p = &e.project;
    let roles: Vec<String> = p.factors.iter().map(|f| format!("{}={}({})", f.symbol, f.name, f.role.label())).collect();
    let done_runs = p.runs.iter().filter(|r| r.status == RunStatus::Done).count();
    let excl = p.runs.iter().filter(|r| r.status == RunStatus::Excluded).count();
    println!("\n== {} ==", path.file_name().unwrap().to_string_lossy());
    println!("   {}", p.title);
    println!("   design: {}", p.design.as_ref().map(|d| d.method.clone()).unwrap_or_else(|| "none".into()));
    println!("   runs: {} ({} done, {} excluded)  state runs={}", p.runs.len(), done_runs, excl, st["runs"]);
    println!("   factors: {}", roles.join(", "));
    if let Some(d) = &p.design {
        for n in &d.notes {
            println!("   note: {n}");
        }
    }
    let responses = call(&mut e, "responses.list", json!({}));
    for r in responses.as_array().unwrap() {
        let rid = r["id"].as_str().unwrap().to_string();
        let name = r["name"].as_str().unwrap().to_string();
        if r["count"].as_u64().unwrap() == 0 {
            println!("   [{name}] no data");
            continue;
        }
        let out: Json = serde_json::from_str(&e.call("analysis.fit", &json!({"response": rid}).to_string())).unwrap();
        if out["ok"] != json!(true) {
            println!("   [{name}] FIT ERROR: {}", out["error"]);
            ok = false;
            continue;
        }
        let fit = &out["result"];
        let sig = significant(fit);
        println!(
            "   [{name}] {}  R²={} adjR²={} predR²={}  sig: {}",
            fit["formula"].as_str().unwrap(),
            stat(fit, "R²"),
            stat(fit, "Adjusted R²"),
            stat(fit, "Predicted R²"),
            sig.join(" ")
        );
        for w in fit["warnings"].as_array().unwrap() {
            println!("        warning: {}", w.as_str().unwrap());
        }
        if std::env::var_os("GEN_VERBOSE").is_some() {
            for row in fit["coefficients"]["rows"].as_array().unwrap() {
                println!("          {}", row.as_array().unwrap().iter().map(|c| c.as_str().unwrap()).collect::<Vec<_>>().join(" | "));
            }
        }
        let check = |sig: &[String], ok: &mut bool| {
            if let Some((_, terms)) = expect.iter().find(|(n, _)| *n == name) {
                for t in *terms {
                    if !sig.iter().any(|s| s == t || s.starts_with(&format!("{t}["))) {
                        println!("        CHECK FAILED: expected {t} to be significant");
                        *ok = false;
                    }
                }
            }
        };
        if !auto.contains(&name.as_str()) {
            check(&sig, &mut ok);
        }
        for plot in ["pred_actual", "resid_fitted", "boxcox", "pareto"] {
            let out: Json = serde_json::from_str(&e.call("analysis.plot", &json!({"response": rid, "kind": plot}).to_string())).unwrap();
            if out["ok"] != json!(true) {
                println!("        plot {plot} error: {}", out["error"]);
            }
        }
        if auto.contains(&name.as_str()) {
            let a = call(&mut e, "model.auto", json!({"response": rid}));
            println!("        model.auto: transform={} formula={}", a["transform"], a["formula"]);
            for row in a["table"]["rows"].as_array().unwrap().iter().take(4) {
                println!("          {}", row.as_array().unwrap().iter().map(|c| c.as_str().unwrap()).collect::<Vec<_>>().join(" | "));
            }
            if a["transform"] == json!("None") {
                println!("        CHECK FAILED: expected a transform for {name}");
                ok = false;
            }
            let fit = call(&mut e, "analysis.fit", json!({"response": rid}));
            println!(
                "        after auto: R²={} adjR²={} predR²={} sig: {}",
                stat(&fit, "R²"),
                stat(&fit, "Adjusted R²"),
                stat(&fit, "Predicted R²"),
                significant(&fit).join(" ")
            );
            check(&significant(&fit), &mut ok);
        }
    }
    if done_runs > 0 {
        let out: Json = serde_json::from_str(&e.call("optimize.run", "{}")).unwrap();
        match out["ok"].as_bool() {
            Some(true) => println!("   optimize: best d={}", out["result"]["solutions"][0]["d"]),
            _ => println!("   optimize error: {}", out["error"]),
        }
        let out: Json = serde_json::from_str(&e.call("export.report", "{}")).unwrap();
        if out["ok"] != json!(true) {
            println!("   report error: {}", out["error"]);
            ok = false;
        }
    }
    ok
}

fn main() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("examples");
    std::fs::create_dir_all(&dir).expect("create examples directory");
    let tablet = tablet(&dir);
    let media = media(&dir);
    let bread = bread(&dir);
    let hplc = hplc(&dir);
    let spray = spray(&dir);
    let yoghurt = yoghurt(&dir);
    let mut ok = true;
    ok &= verify(&tablet, &[("Hardness", &["A", "B", "A*D", "B*D", "E"]), ("Disintegration time", &["B", "D"])], &["Disintegration time"]);
    ok &= verify(&media, &[("Titre", &["A", "B", "C", "F", "L"]), ("Peak viable cell density", &["A", "B", "C"])], &["Titre"]);
    ok &= verify(&bread, &[("Loaf volume", &["A", "C", "A^2", "B^2", "C^2"]), ("Crumb firmness", &["A", "C"])], &[]);
    ok &= verify(&hplc, &[("Resolution API / impurity B", &["A", "B", "D", "F"]), ("API retention time", &["C", "D", "B"])], &[]);
    ok &= verify(&spray, &[], &[]);
    ok &= verify(&yoghurt, &[], &[]);
    println!("\n{}", if ok { "ALL CHECKS PASSED" } else { "SOME CHECKS FAILED" });
}
