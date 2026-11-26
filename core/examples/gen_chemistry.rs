//! Generates example projects for chemistry and process engineering into
//! `examples/chem-*.sdoe` at the workspace root, then reloads every file
//! and runs the analyses as a check.
//!
//! Run with `cargo run --release -p sdoe-core --example gen_chemistry`.
//!
//! The simulated responses come from fixed underlying models in coded units
//! plus measurement noise; all random numbers come from fixed seeds, so the
//! output is identical on every run.

use sdoe::Engine;
use sdoe::project::{RunStatus, Value as PV};
use sdoe::rng::Rng;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Engine helpers
// ---------------------------------------------------------------------------

fn call(e: &mut Engine, m: &str, p: Value) -> Value {
    let out: Value = serde_json::from_str(&e.call(m, &p.to_string())).expect("engine output is JSON");
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
    Disc(&'a str),
    Cat(&'a str),
}

/// Adds a factor through the factors sheet and returns its id.
fn factor(e: &mut Engine, name: &str, units: &str, kind: Kind, role: &str) -> u64 {
    let row = e.project.factors.len();
    set(e, "factors", row, 1, name);
    set(e, "factors", row, 2, units);
    match kind {
        Kind::Cont(lo, hi) => {
            set(e, "factors", row, 3, "Continuous");
            // The default range is -1..1: the order of the two writes keeps low < high.
            if hi > -1.0 {
                set(e, "factors", row, 6, &hi.to_string());
                set(e, "factors", row, 5, &lo.to_string());
            } else {
                set(e, "factors", row, 5, &lo.to_string());
                set(e, "factors", row, 6, &hi.to_string());
            }
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

/// Adds a response through the responses sheet and returns its id.
#[allow(clippy::too_many_arguments)]
fn response(e: &mut Engine, name: &str, units: &str, goal: &str, lower: Option<f64>, target: Option<f64>, upper: Option<f64>, importance: f64) -> u64 {
    let row = e.project.responses.len();
    set(e, "responses", row, 0, name);
    set(e, "responses", row, 1, units);
    set(e, "responses", row, 2, goal);
    let t = |v: Option<f64>| v.map(|x| x.to_string()).unwrap_or_default();
    set(e, "responses", row, 3, &t(lower));
    set(e, "responses", row, 4, &t(target));
    set(e, "responses", row, 5, &t(upper));
    set(e, "responses", row, 7, &importance.to_string());
    e.project.responses[row].id
}

fn round_to(x: f64, step: f64) -> f64 {
    let r = (x / step).round() * step;
    // Removes binary noise such as 0.30000000000000004.
    let digits = (-step.log10()).ceil().max(0.0) as i32 + 1;
    let f = 10f64.powi(digits);
    (r * f).round() / f
}

/// Numeric setting of a factor in a run.
fn num(e: &Engine, ri: usize, fid: u64) -> f64 {
    e.project.runs[ri].settings[&fid].as_num().expect("numeric setting")
}

/// Coded (-1..+1) value of a numeric factor in a run.
fn coded(e: &Engine, ri: usize, fid: u64) -> f64 {
    let fi = e.project.factor_index(fid).unwrap();
    let (c, h) = e.project.factors[fi].center_half();
    (num(e, ri, fid) - c) / h
}

fn text(e: &Engine, ri: usize, fid: u64) -> String {
    match &e.project.runs[ri].settings[&fid] {
        PV::Text(t) => t.clone(),
        PV::Num(x) => x.to_string(),
    }
}

/// Rounds the designed values of a continuous factor to a practical setting precision.
fn round_settings(e: &mut Engine, fid: u64, step: f64) {
    for run in e.project.runs.iter_mut() {
        if let Some(PV::Num(x)) = run.settings.get(&fid).cloned() {
            run.settings.insert(fid, PV::Num(round_to(x, step)));
        }
    }
}

fn save(e: &mut Engine, path: &Path) {
    let text = call(e, "project.save", json!({}))["text"].as_str().unwrap().to_string();
    std::fs::write(path, text).unwrap_or_else(|err| panic!("writing {}: {err}", path.display()));
    println!("wrote {}", path.display());
}

/// Slowly drifting measured quantity: linear trend over run order plus
/// first-order autocorrelated noise.
struct Drift {
    rng: Rng,
    state: f64,
}

impl Drift {
    fn new(seed: u64) -> Drift {
        Drift { rng: Rng::new(seed), state: 0.0 }
    }
    /// Next autocorrelated noise value with stationary standard deviation `sd`.
    fn next(&mut self, phi: f64, sd: f64) -> f64 {
        self.state = phi * self.state + (1.0 - phi * phi).sqrt() * sd * self.rng.normal();
        self.state
    }
}

// ---------------------------------------------------------------------------
// 1. Esterification: rotatable CCD with measured feedstock water
// ---------------------------------------------------------------------------

fn esterification(path: &Path) {
    let mut e = Engine::new();
    let ft = factor(&mut e, "Temperature", "°C", Kind::Cont(80.0, 100.0), "Controlled");
    let fb = factor(&mut e, "Reaction time", "h", Kind::Cont(2.0, 6.0), "Controlled");
    let fc = factor(&mut e, "Catalyst loading", "mol%", Kind::Cont(0.6, 1.8), "Controlled");
    let fw = factor(&mut e, "Feedstock water", "% w/w", Kind::Cont(0.05, 0.35), "Environmental");
    let ry = response(&mut e, "Ester yield", "%", "Maximize", Some(85.0), None, Some(97.0), 5.0);
    let ri = response(&mut e, "Ether impurity", "area %", "Minimize", Some(0.15), None, Some(0.8), 3.0);
    let rc = response(&mut e, "Colour", "Pt-Co", "Minimize", Some(20.0), None, Some(70.0), 2.0);
    call(&mut e, "project.set", json!({
        "title": "Esterification of oleic acid: response surface study",
        "notes": "Batch esterification of oleic acid with 2-ethylhexanol in toluene with p-toluenesulfonic acid as catalyst and \
azeotropic water removal (Dean-Stark trap). Jacket temperature, reaction time and catalyst loading are optimised with a \
rotatable central composite design with six centre points, run in random order in a 2 L jacketed reactor. \
The water content of the oleic acid feedstock is not controlled; it is measured by Karl Fischer titration before each \
batch and recorded as an environmental factor. A new feedstock drum was opened partway through the campaign. \
Ester yield is determined by GC against an internal standard, the bis(2-ethylhexyl) ether by-product as GC area %, \
and the colour of the crude product on the platinum-cobalt (APHA) scale."
    }));
    call(&mut e, "design.preview", json!({"method": "ccd", "options": {"alpha": "rotatable", "center_points": 6}}));
    call(&mut e, "design.apply", json!({"mode": "replace", "randomize": true, "seed": 4107}));
    round_settings(&mut e, ft, 0.5);
    round_settings(&mut e, fb, 0.05);
    round_settings(&mut e, fc, 0.01);

    let mut rng = Rng::new(9001);
    let mut drift = Drift::new(77);
    let n = e.project.runs.len();
    for i in 0..n {
        // Water content increases with moisture uptake after the drum is opened; drum 2 is used from run 12 on.
        let (base, k) = if i < 11 { (0.10, i as f64) } else { (0.08, (i - 11) as f64) };
        let water = round_to((base + 0.017 * k + drift.next(0.6, 0.015)).max(0.04), 0.01);
        let (a, b, c) = (coded(&e, i, ft), coded(&e, i, fb), coded(&e, i, fc));
        let w = (water - 0.20) / 0.15;
        let yield_ = 92.0 + 2.1 * a + 1.7 * b + 1.3 * c - 1.6 * a * a - 1.0 * b * b - 0.8 * c * c - 0.9 * a * b - 2.8 * w + 1.4 * b * w
            + 0.55 * rng.normal();
        let ln_imp = (0.32f64).ln() + 0.42 * a + 0.30 * c + 0.08 * b + 0.12 * a * c + 0.07 * rng.normal();
        let colour = 34.0 + 11.0 * a + 5.0 * b + 3.5 * a * b + 4.5 * a * a + 2.0 * c + 2.5 * rng.normal();
        let run = &mut e.project.runs[i];
        run.settings.insert(fw, PV::Num(water));
        run.responses.insert(ry, round_to(yield_.min(99.6), 0.1));
        run.responses.insert(ri, round_to(ln_imp.exp(), 0.01));
        run.responses.insert(rc, round_to(colour.max(5.0), 1.0));
        run.status = RunStatus::Done;
    }
    // A failed GC injection and one spoiled batch.
    let run = &mut e.project.runs[6];
    run.responses.remove(&ri);
    run.note = "Second GC injection failed; ether impurity not reported.".into();
    let run = &mut e.project.runs[14];
    run.status = RunStatus::Excluded;
    run.responses.insert(ry, 78.4);
    run.note = "Dean-Stark trap overflowed after 40 min; separated water ran back into the flask.".into();
    save(&mut e, path);
}

// ---------------------------------------------------------------------------
// 2. Hydrogenation screening: 2^(7-3) fractional factorial with centre points
// ---------------------------------------------------------------------------

fn hydrogenation(path: &Path) {
    let mut e = Engine::new();
    let fcat = factor(&mut e, "Catalyst", "", Kind::Cat("Pd/C, Pt/C"), "Controlled");
    let _fsol = factor(&mut e, "Solvent", "", Kind::Cat("MeOH, EtOAc"), "Controlled");
    let fp = factor(&mut e, "H2 pressure", "bar", Kind::Cont(2.0, 8.0), "Controlled");
    let ft = factor(&mut e, "Temperature", "°C", Kind::Cont(25.0, 55.0), "Controlled");
    let fl = factor(&mut e, "Catalyst loading", "wt%", Kind::Cont(2.0, 6.0), "Controlled");
    let fa = factor(&mut e, "Agitation", "rpm", Kind::Cont(600.0, 1200.0), "Controlled");
    let fs = factor(&mut e, "Substrate concentration", "mol/L", Kind::Cont(0.2, 0.6), "Controlled");
    let rconv = response(&mut e, "Conversion", "%", "Maximize", Some(90.0), None, Some(99.5), 4.0);
    let rdes = response(&mut e, "Des-chloro impurity", "area %", "Minimize", Some(0.05), None, Some(0.5), 5.0);
    call(&mut e, "project.set", json!({
        "title": "Nitro group hydrogenation: catalyst and solvent screening",
        "notes": "Screening study for the hydrogenation of 4-chloro-2-nitrotoluene to the corresponding aniline in a \
parallel pressure reactor block. Seven process variables are screened in a 16-run resolution IV fractional factorial \
with four centre points (two with Pd/C in methanol, two with Pt/C in ethyl acetate). \
Conversion is measured by HPLC after 4 h; the hydrodechlorinated by-product (o-toluidine) is reported as HPLC area %, \
since it is difficult to purge downstream. The aim is to identify the few variables with a large effect on conversion and \
dechlorination before a response surface study."
    }));
    call(&mut e, "design.preview", json!({"method": "fractional_factorial", "options": {"runs": "16", "resolution": "4", "center_points": 4}}));
    call(&mut e, "design.apply", json!({"mode": "replace", "randomize": true, "seed": 2219}));
    for (f, step) in [(fp, 0.1), (ft, 1.0), (fl, 0.1), (fa, 10.0), (fs, 0.01)] {
        round_settings(&mut e, f, step);
    }

    let mut rng = Rng::new(5150);
    let n = e.project.runs.len();
    for i in 0..n {
        let pd = if text(&e, i, fcat) == "Pd/C" { 1.0 } else { -1.0 };
        let (p, t, l) = (coded(&e, i, fp), coded(&e, i, ft), coded(&e, i, fl));
        let conv = 91.5 + 3.6 * p + 2.8 * l + 1.6 * pd - 1.2 * p * l + 0.7 * rng.normal();
        let ln_des = (0.14f64).ln() + 0.50 * pd + 0.32 * t + 0.10 * pd * t + 0.08 * rng.normal();
        let run = &mut e.project.runs[i];
        run.responses.insert(rconv, round_to(conv.min(99.9), 0.1));
        run.responses.insert(rdes, round_to(ln_des.exp(), 0.01));
        run.status = RunStatus::Done;
    }
    let run = &mut e.project.runs[9];
    run.responses.remove(&rdes);
    run.note = "Des-chloro peak co-eluted with a solvent impurity; not quantified.".into();
    save(&mut e, path);
}

// ---------------------------------------------------------------------------
// 3. Cooling crystallisation: D-optimal design planned around raw-material lots
// ---------------------------------------------------------------------------

fn crystallisation(path: &Path) {
    let mut e = Engine::new();
    let fr = factor(&mut e, "Cooling rate", "K/h", Kind::Cont(5.0, 30.0), "Controlled");
    let fs = factor(&mut e, "Seed loading", "% w/w", Kind::Cont(0.5, 2.5), "Controlled");
    let fa = factor(&mut e, "Agitator setting", "", Kind::Disc("1, 2, 3, 4, 5"), "Controlled");
    let lots = ["L24-031", "L24-044", "L24-058", "L24-071"];
    let flot = factor(&mut e, "Crude lot", "", Kind::Cat(&lots.join(", ")), "Environmental");
    let rd = response(&mut e, "D50", "µm", "Target", Some(140.0), Some(180.0), Some(220.0), 5.0);
    let ry = response(&mut e, "Isolated yield", "%", "Maximize", Some(84.0), None, Some(92.0), 3.0);
    let rf = response(&mut e, "Filtration time", "min", "Minimize", Some(12.0), None, Some(40.0), 2.0);
    call(&mut e, "project.set", json!({
        "title": "API cooling crystallisation: particle size and filtration",
        "notes": "Seeded cooling crystallisation of an active pharmaceutical ingredient from ethanol/water in a 5 L \
reactor, from 65 °C to 5 °C. Cooling rate, seed loading and the agitator speed setting (1-5 on the drive controller) \
are studied with a D-optimal design for a quadratic model. Four lots of crude material are available, each enough for \
six batches; the lot cannot be chosen freely, so it is an environmental factor whose schedule was entered before the \
design was generated, and the runs are randomised within each lot. \
D50 is measured by laser diffraction on the wet cake, isolated yield gravimetrically after drying, and filtration \
time as the time to deliquor the slurry on a 10 cm pressure filter at 1 bar."
    }));
    let schedule: Vec<&str> = lots.iter().flat_map(|l| std::iter::repeat_n(*l, 6)).collect();
    call(&mut e, "design.preview", json!({"method": "optimal", "options": {
        "criterion": "D", "model": "quadratic", "runs": 24, "starts": 30, "seed": 12,
        format!("env_{flot}"): schedule.join(", "),
    }}));
    call(&mut e, "design.apply", json!({"mode": "replace", "randomize": false}));
    round_settings(&mut e, fr, 0.5);
    round_settings(&mut e, fs, 0.05);
    // Random order within each lot; lots in delivery order.
    let mut rng = Rng::new(611);
    let mut order: Vec<usize> = Vec::new();
    for l in lots {
        let mut idx: Vec<usize> = (0..e.project.runs.len()).filter(|&i| text(&e, i, flot) == l).collect();
        rng.shuffle(&mut idx);
        order.extend(idx);
    }
    for (k, &i) in order.iter().enumerate() {
        e.project.runs[i].run_order = k as u32 + 1;
    }
    e.project.runs.sort_by_key(|r| r.run_order);

    let lot_d50 = [6.0, -3.0, -14.0, 9.0];
    let lot_yield = [0.4, -0.9, 0.1, 0.5];
    let mut rng = Rng::new(808);
    for i in 0..e.project.runs.len() {
        let li = lots.iter().position(|l| *l == text(&e, i, flot)).unwrap();
        let (a, b) = (coded(&e, i, fr), coded(&e, i, fs));
        let c = (num(&e, i, fa) - 3.0) / 2.0;
        let d50 = 182.0 - 26.0 * a - 14.0 * b - 9.0 * c - 6.0 * c * c + 7.0 * a * b + 5.0 * a * a + lot_d50[li] + 5.0 * rng.normal();
        let yld = 88.6 - 0.9 * a + 0.5 * b + 0.3 * c + lot_yield[li] + 0.45 * rng.normal();
        let filt = 21.0 * (180.0 / d50).powf(1.7) * (0.07 * rng.normal()).exp();
        let run = &mut e.project.runs[i];
        run.responses.insert(rd, round_to(d50, 1.0));
        run.responses.insert(ry, round_to(yld, 0.1));
        run.responses.insert(rf, round_to(filt, 0.5));
        run.status = RunStatus::Done;
    }
    let run = &mut e.project.runs[10];
    run.responses.remove(&rf);
    run.note = "Filter cloth replaced during deliquoring; filtration time not comparable.".into();
    let run = &mut e.project.runs[19];
    run.status = RunStatus::Excluded;
    // Uncontrolled primary nucleation results in fine crystals and slow filtration.
    run.responses.insert(rd, 104.0);
    run.responses.insert(rf, 52.5);
    run.note = "Seed slurry added 4 K below the planned seeding point (spontaneous nucleation observed).".into();
    save(&mut e, path);
}

// ---------------------------------------------------------------------------
// 4. Empty template: continuous solvent recovery column
// ---------------------------------------------------------------------------

fn distillation_template(path: &Path) {
    let mut e = Engine::new();
    factor(&mut e, "Reflux ratio", "", Kind::Cont(1.5, 4.0), "Controlled");
    factor(&mut e, "Reboiler steam", "kg/h", Kind::Cont(120.0, 220.0), "Controlled");
    factor(&mut e, "Feed temperature", "°C", Kind::Cont(55.0, 85.0), "Controlled");
    factor(&mut e, "Feed tray", "", Kind::Disc("8, 12, 16, 20"), "Controlled");
    factor(&mut e, "Cooling water inlet", "°C", Kind::Cont(10.0, 28.0), "Environmental");
    factor(&mut e, "IPA in feed", "% w/w", Kind::Cont(40.0, 70.0), "Environmental");
    response(&mut e, "Distillate IPA", "% w/w", "Maximize", Some(85.0), None, Some(87.5), 5.0);
    response(&mut e, "IPA in bottoms", "ppm", "Minimize", Some(200.0), None, Some(2000.0), 4.0);
    response(&mut e, "Specific steam use", "kg/kg", "Minimize", Some(1.2), None, Some(2.5), 3.0);
    response(&mut e, "Column pressure drop", "mbar", "In range", Some(20.0), None, Some(60.0), 2.0);
    call(&mut e, "project.set", json!({
        "title": "Solvent recovery column: template",
        "notes": "Template for a study of a continuous tray column for the recovery of isopropanol (IPA) from aqueous \
process waste as the IPA-water azeotrope. Reflux ratio, reboiler steam rate, feed temperature and the feed tray are \
the operating variables; the cooling water inlet temperature and the IPA content of the feed vary with season and \
upstream production and are recorded as environmental factors at each steady state. \
Distillate IPA is measured by GC or density, IPA in the bottoms by headspace GC, and specific steam use as steam mass \
per mass of recovered IPA. Choose a design (for example an optimal design for a quadratic model) under Design, \
then hold each setting until the column reaches steady state before sampling."
    }));
    save(&mut e, path);
}

// ---------------------------------------------------------------------------
// 5. Planned: emulsion polymerisation, Box-Behnken, results not yet entered
// ---------------------------------------------------------------------------

fn emulsion_planned(path: &Path) {
    let mut e = Engine::new();
    let fi = factor(&mut e, "Initiator (APS)", "wt% on monomer", Kind::Cont(0.2, 0.6), "Controlled");
    let fsu = factor(&mut e, "Surfactant (SDS)", "wt% on monomer", Kind::Cont(0.5, 2.0), "Controlled");
    let ff = factor(&mut e, "Monomer feed time", "min", Kind::Cont(120.0, 240.0), "Controlled");
    let ft = factor(&mut e, "Reaction temperature", "°C", Kind::Cont(75.0, 85.0), "Controlled");
    factor(&mut e, "MEHQ in monomer", "ppm", Kind::Cont(5.0, 30.0), "Environmental");
    factor(&mut e, "Cooling water", "°C", Kind::Cont(8.0, 25.0), "Environmental");
    response(&mut e, "Particle size Z-avg", "nm", "Target", Some(130.0), Some(150.0), Some(170.0), 5.0);
    response(&mut e, "Viscosity", "mPa·s", "In range", Some(200.0), None, Some(800.0), 3.0);
    response(&mut e, "Coagulum", "% on solids", "Minimize", Some(0.02), None, Some(0.5), 4.0);
    response(&mut e, "Residual monomer", "ppm", "Minimize", Some(200.0), None, Some(1500.0), 3.0);
    call(&mut e, "project.set", json!({
        "title": "Acrylic latex by emulsion polymerisation: planned study",
        "notes": "Planned Box-Behnken study for a semi-batch emulsion copolymerisation of butyl acrylate and methyl \
methacrylate (about 50 % solids) in a 3 L reactor. Initiator and surfactant levels, monomer feed time and reaction \
temperature are varied; the runs are in randomised order and have not been carried out yet. \
Before each batch, record the MEHQ inhibitor content of the monomer charge (HPLC) and the cooling water supply \
temperature in the environmental columns. After the batch, measure the Z-average particle size by dynamic light \
scattering, the Brookfield viscosity at 25 °C, the coagulum retained on a 100 µm sieve, and residual monomer by \
headspace GC."
    }));
    call(&mut e, "design.preview", json!({"method": "box_behnken", "options": {"center_points": 3}}));
    call(&mut e, "design.apply", json!({"mode": "replace", "randomize": true, "seed": 3301}));
    round_settings(&mut e, fi, 0.01);
    round_settings(&mut e, fsu, 0.05);
    round_settings(&mut e, ff, 5.0);
    round_settings(&mut e, ft, 0.5);
    save(&mut e, path);
}

// ---------------------------------------------------------------------------
// Verification
// ---------------------------------------------------------------------------

fn stat(fit: &Value, name: &str) -> String {
    fit["stats"].as_array().unwrap().iter().find(|r| r[0] == json!(name)).map(|r| r[1].as_str().unwrap().to_string()).unwrap_or_default()
}

/// Term labels marked significant (p < 0.05) in the coefficient table.
fn significant(fit: &Value) -> Vec<String> {
    let t = &fit["coefficients"];
    let rows = t["rows"].as_array().unwrap();
    let em = t["emphasis"].as_array().unwrap();
    rows.iter()
        .zip(em)
        .filter(|(r, m)| m.as_str() == Some("good") && r[0] != json!("Intercept"))
        .map(|(r, _)| r[0].as_str().unwrap().to_string())
        .collect()
}

fn print_fit(fit: &Value) {
    println!(
        "    {:<22} R²={} adjR²={} predR²={}  [{}]",
        fit["response"].as_str().unwrap(),
        stat(fit, "R²"),
        stat(fit, "Adjusted R²"),
        stat(fit, "Predicted R²"),
        fit["formula"].as_str().unwrap()
    );
    println!("      significant: {}", significant(fit).join(", "));
    for w in fit["warnings"].as_array().unwrap() {
        println!("      warning: {}", w.as_str().unwrap());
    }
}

fn check_contains(fit: &Value, terms: &[&str]) {
    let sig = significant(fit);
    for t in terms {
        assert!(sig.iter().any(|s| s == t), "{}: expected {t} to be significant, got {sig:?}", fit["response"]);
    }
}

fn verify(path: &Path) {
    let text = std::fs::read_to_string(path).unwrap();
    let mut e = Engine::new();
    call(&mut e, "project.open", json!({"text": text}));
    call(&mut e, "project.state", json!({}));
    let p = &e.project;
    let done = p.runs.iter().filter(|r| r.status == RunStatus::Done).count();
    let excl = p.runs.iter().filter(|r| r.status == RunStatus::Excluded).count();
    println!(
        "\n{}\n  \"{}\": {} factors, {} responses, {} runs ({} done, {} excluded), design: {}",
        path.file_name().unwrap().to_string_lossy(),
        p.title,
        p.factors.len(),
        p.responses.len(),
        p.runs.len(),
        done,
        excl,
        p.design.as_ref().map(|d| d.method.as_str()).unwrap_or("none")
    );
    let rids: Vec<u64> = p.responses.iter().map(|r| r.id).collect();
    let has_data = p.runs.iter().any(|r| !r.responses.is_empty());
    if !has_data {
        let runs = call(&mut e, "sheet.get", json!({"sheet": "runs"}));
        println!("  no results; runs sheet has {} rows", runs["rows"].as_array().unwrap().len());
        if e.project.runs.is_empty() {
            let pv = call(&mut e, "design.preview", json!({"method": "optimal", "options": {"model": "quadratic"}}));
            println!("  optimal design preview: {} runs for {}", pv["runs"], pv["model"].as_str().unwrap());
        } else {
            let ev = call(&mut e, "evaluate.run", json!({"formula": "(A+B+C+D)^2 + A^2 + B^2 + C^2 + D^2"}));
            println!("  design evaluation rows: {}", ev["table"]["rows"].as_array().unwrap().len());
        }
        return;
    }
    let name = path.file_name().unwrap().to_string_lossy().to_string();
    let mut fits = Vec::new();
    for &rid in &rids {
        let fit = call(&mut e, "analysis.fit", json!({"response": rid}));
        print_fit(&fit);
        fits.push(fit);
    }
    // Default-model checks of the active effects.
    if name.contains("hydrogenation") {
        check_contains(&fits[0], &["A[Pd/C]", "C", "E"]);
        check_contains(&fits[1], &["A[Pd/C]", "D"]);
        call(&mut e, "model.set", json!({"response": rids[1], "formula": "A + D", "transform": "Ln"}));
        let fit = call(&mut e, "analysis.fit", json!({"response": rids[1]}));
        println!("    des-chloro impurity, reduced model with Ln transform:");
        print_fit(&fit);
    }
    if name.contains("crystallisation") {
        check_contains(&fits[0], &["A", "B", "C"]);
        call(&mut e, "model.set", json!({"response": rids[2], "transform": "Ln"}));
        let fit = call(&mut e, "analysis.fit", json!({"response": rids[2]}));
        println!("    filtration time with Ln transform:");
        print_fit(&fit);
        let auto = call(&mut e, "model.auto", json!({"response": rids[2]}));
        let fit = call(&mut e, "analysis.fit", json!({"response": rids[2]}));
        println!("    filtration time, automatic model ({} transform):", auto["transform"].as_str().unwrap());
        print_fit(&fit);
    }
    if name.contains("esterification") {
        check_contains(&fits[0], &["A", "B", "C", "A^2"]);
        check_contains(&fits[1], &["A", "C"]);
        // Environment x factor interactions: the hint in the default fit and B*D in the extended model.
        let hint = fits[0]["warnings"].as_array().unwrap().iter().any(|w| w.as_str().unwrap().contains("Environment"));
        println!("    environment interaction hint in default yield fit: {hint}");
        call(&mut e, "model.preset", json!({"response": rids[0], "preset": "quadratic", "env_interactions": true}));
        let fit = call(&mut e, "analysis.fit", json!({"response": rids[0]}));
        println!("    yield with env interactions:");
        print_fit(&fit);
        check_contains(&fit, &["D", "B*D"]);
        // Selection from the quadratic + environment interaction pool.
        for method in ["backward", "aicc"] {
            call(&mut e, "model.preset", json!({"response": rids[0], "preset": "quadratic", "env_interactions": true}));
            let sel = call(&mut e, "model.select", json!({"response": rids[0], "method": method, "alpha": 0.05}));
            let fit = call(&mut e, "analysis.fit", json!({"response": rids[0]}));
            println!("    yield after {method} selection (AICc {}):", stat(&fit, "AICc"));
            print_fit(&fit);
            if method == "aicc" {
                println!("      steps: {}", sel["steps"]);
            }
        }
        // The backward-selected model is the one used for the adjustment law.
        call(&mut e, "model.preset", json!({"response": rids[0], "preset": "quadratic", "env_interactions": true}));
        call(&mut e, "model.select", json!({"response": rids[0], "method": "backward", "alpha": 0.05}));
        let water = e.project.factors[3].id.to_string();
        let law = call(&mut e, "optimize.control_law", json!({"factor": water}));
        println!("    control law: {}", law["formulas"]);
        call(&mut e, "model.set", json!({"response": rids[1], "transform": "Ln"}));
        let fit = call(&mut e, "analysis.fit", json!({"response": rids[1]}));
        println!("    impurity with Ln transform:");
        print_fit(&fit);
    }
    let opt = call(&mut e, "optimize.run", json!({}));
    println!("    optimum desirability: {}", opt["solutions"][0]["d"]);
    let html = call(&mut e, "export.report", json!({}))["html"].as_str().unwrap().len();
    println!("    report: {html} bytes");
}

fn main() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("examples");
    std::fs::create_dir_all(&root).expect("create examples directory");
    let root = root.canonicalize().unwrap();
    let files: Vec<(&str, fn(&Path))> = vec![
        ("chem-esterification-ccd.sdoe", esterification),
        ("chem-hydrogenation-screening.sdoe", hydrogenation),
        ("chem-crystallisation-optimal.sdoe", crystallisation),
        ("chem-solvent-recovery-template.sdoe", distillation_template),
        ("chem-emulsion-planned.sdoe", emulsion_planned),
    ];
    for (name, build) in &files {
        build(&root.join(name));
    }
    println!("\nVerification");
    for (name, _) in &files {
        verify(&root.join(name));
    }
}
