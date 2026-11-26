//! Generates example projects for field trials, buildings/energy and digital
//! experiments, where uncontrollable environmental conditions dominate.
//!
//! Run with `cargo run --release -p sdoe-core --example gen_field`. The
//! program writes `examples/field-*.sdoe` at the workspace root, then reloads
//! every file in a fresh engine and runs the analyses as a check.

use sdoe::Engine;
use sdoe::project::{RunStatus, Value};
use sdoe::rng::Rng;
use serde_json::{Value as Json, json};
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// Engine helpers
// ---------------------------------------------------------------------------

fn try_call(e: &mut Engine, m: &str, p: Json) -> Result<Json, String> {
    let out: Json = serde_json::from_str(&e.call(m, &p.to_string())).expect("engine output is JSON");
    if out["ok"] == json!(true) { Ok(out["result"].clone()) } else { Err(out["error"].as_str().unwrap_or("?").to_string()) }
}

fn call(e: &mut Engine, m: &str, p: Json) -> Json {
    try_call(e, m, p.clone()).unwrap_or_else(|err| panic!("{m} {p} failed: {err}"))
}

fn set(e: &mut Engine, sheet: &str, row: usize, col: usize, text: &str) {
    call(e, "sheet.set", json!({"sheet": sheet, "row": row, "col": col, "text": text}));
}

/// Adds a factor through the factors sheet and returns its id.
/// `kind` is Continuous, Discrete or Categorical; `range` applies to continuous
/// factors, `levels` to discrete and categorical ones.
fn factor(e: &mut Engine, name: &str, units: &str, kind: &str, role: &str, range: Option<(f64, f64)>, levels: Option<&str>) -> u64 {
    let row = e.project.factors.len();
    set(e, "factors", row, 1, name);
    set(e, "factors", row, 2, units);
    set(e, "factors", row, 3, kind);
    set(e, "factors", row, 4, role);
    if let Some((lo, hi)) = range {
        // The new range has to stay valid after each single edit.
        let cur_lo = e.project.factors[row].range().0;
        if hi > cur_lo {
            set(e, "factors", row, 6, &hi.to_string());
            set(e, "factors", row, 5, &lo.to_string());
        } else {
            set(e, "factors", row, 5, &lo.to_string());
            set(e, "factors", row, 6, &hi.to_string());
        }
    }
    if let Some(l) = levels {
        set(e, "factors", row, 7, l);
    }
    e.project.factors[row].id
}

/// Adds a response through the responses sheet and returns its id.
#[allow(clippy::too_many_arguments)]
fn response(e: &mut Engine, name: &str, units: &str, goal: &str, lower: &str, target: &str, upper: &str, weight: f64, importance: f64) -> u64 {
    let row = e.project.responses.len();
    set(e, "responses", row, 0, name);
    set(e, "responses", row, 1, units);
    set(e, "responses", row, 2, goal);
    set(e, "responses", row, 3, lower);
    set(e, "responses", row, 4, target);
    set(e, "responses", row, 5, upper);
    set(e, "responses", row, 6, &weight.to_string());
    set(e, "responses", row, 7, &importance.to_string());
    e.project.responses[row].id
}

fn rnd(x: f64, decimals: i32) -> f64 {
    let m = 10f64.powi(decimals);
    (x * m).round() / m
}

fn num(run: &sdoe::project::Run, id: u64) -> f64 {
    run.settings[&id].as_num().expect("numeric setting")
}

fn text(run: &sdoe::project::Run, id: u64) -> String {
    match &run.settings[&id] {
        Value::Text(t) => t.clone(),
        Value::Num(x) => x.to_string(),
    }
}

/// Smooth version of max(0, x) with transition width `t`.
fn softplus(x: f64, t: f64) -> f64 {
    let z = x / t;
    if z > 30.0 { x } else { t * (1.0 + z.exp()).ln() }
}

fn logistic(x: f64) -> f64 {
    1.0 / (1.0 + (-x).exp())
}

/// Calendar label for a 0-based day of year in 2026 (1 January 2026 is a Thursday).
fn date_2026(doy: usize) -> String {
    const DAYS: [&str; 7] = ["Thu", "Fri", "Sat", "Sun", "Mon", "Tue", "Wed"];
    const MONTHS: [(&str, usize); 12] = [
        ("Jan", 31),
        ("Feb", 28),
        ("Mar", 31),
        ("Apr", 30),
        ("May", 31),
        ("Jun", 30),
        ("Jul", 31),
        ("Aug", 31),
        ("Sep", 30),
        ("Oct", 31),
        ("Nov", 30),
        ("Dec", 31),
    ];
    let mut d = doy;
    for (m, len) in MONTHS {
        if d < len {
            return format!("{} {} {} 2026", DAYS[doy % 7], d + 1, m);
        }
        d -= len;
    }
    unreachable!()
}

fn save(e: &mut Engine, dir: &PathBuf, file: &str) -> PathBuf {
    let text = call(e, "project.save", json!({}))["text"].as_str().unwrap().to_string();
    let path = dir.join(file);
    std::fs::write(&path, text).expect("write project file");
    path
}

fn design(e: &mut Engine, method: &str, options: Json, randomize: bool, seed: u64) -> Json {
    let pv = call(e, "design.preview", json!({"method": method, "options": options}));
    call(e, "design.apply", json!({"mode": "replace", "randomize": randomize, "seed": seed}));
    pv
}

// ---------------------------------------------------------------------------
// 1. Winter wheat agronomy trial on three fields
// ---------------------------------------------------------------------------

fn crop_trial(dir: &PathBuf) -> PathBuf {
    let mut e = Engine::new();
    call(
        &mut e,
        "project.set",
        json!({
            "title": "Winter wheat N × irrigation × seeding trial",
            "notes": "Agronomy trial at a research station: 48 plots of 12 m × 2 m on three fields (North, River and Hill, 16 plots each), harvest 2026. \
Nitrogen rate (split into two dressings), irrigation level (drip, as % of crop evapotranspiration), seeding density and variety vary between plots. \
The fields differ in soil type, so the field enters the design as an environmental factor with known values. \
Soil moisture at sowing (TDR probe, 0–30 cm) and pre-plant soil mineral nitrate (0–90 cm cores) were measured on every plot before any treatment. \
Grain yield is reported at 15 % moisture; protein by NIR on a grain subsample. \
Plot H-11 was hit by hail on 14 June and is excluded. Questions: which nitrogen rate pays off on each field, and does the best rate depend on the residual soil nitrate?"
        }),
    );
    let n = factor(&mut e, "Nitrogen rate", "kg N/ha", "Continuous", "Controlled", Some((0.0, 240.0)), None);
    let irr = factor(&mut e, "Irrigation", "% ETc", "Continuous", "Controlled", Some((40.0, 100.0)), None);
    let dens = factor(&mut e, "Seeding density", "seeds/m²", "Discrete", "Controlled", None, Some("250, 350, 450"));
    let var = factor(&mut e, "Variety", "", "Categorical", "Controlled", None, Some("Arcadia, Brenton, Calder"));
    let field = factor(&mut e, "Field", "", "Categorical", "Environmental", None, Some("North, River, Hill"));
    let moist = factor(&mut e, "Soil moisture at sowing", "% vol", "Continuous", "Environmental", Some((15.0, 40.0)), None);
    let nitrate = factor(&mut e, "Soil nitrate", "kg N/ha", "Continuous", "Environmental", Some((10.0, 100.0)), None);
    let y_yield = response(&mut e, "Grain yield", "t/ha", "Maximize", "6", "", "11", 1.0, 4.0);
    let y_prot = response(&mut e, "Grain protein", "%", "Maximize", "10.5", "", "13", 1.0, 2.0);

    let fields: Vec<&str> = ["North", "River", "Hill"].iter().flat_map(|f| std::iter::repeat_n(*f, 16)).collect();
    let opts = json!({
        "criterion": "D", "model": "quadratic", "runs": 48, "starts": 30, "seed": 4,
        format!("env_{field}"): fields.join(", "),
    });
    design(&mut e, "optimal", opts, true, 2026);

    // Field properties: mean moisture, mean nitrate, yield offset, plot code prefix.
    let props = |f: &str| match f {
        "North" => (27.0, 42.0, 0.25, "N"),
        "River" => (31.0, 55.0, 0.05, "R"),
        _ => (23.0, 30.0, -0.35, "H"),
    };
    let mut rng = Rng::new(1931);
    // Plot layout order within each field follows the standard order; the
    // soil properties drift along the row of plots (spatial autocorrelation).
    let mut order: Vec<usize> = (0..e.project.runs.len()).collect();
    order.sort_by_key(|&i| e.project.runs[i].std_order);
    for fname in ["North", "River", "Hill"] {
        let (m_mean, n_mean, f_eff, code) = props(fname);
        let (mut ar_m, mut ar_n) = (0.0f64, 0.0f64);
        let mut plot = 0;
        for &i in &order {
            if text(&e.project.runs[i], field) != fname {
                continue;
            }
            plot += 1;
            ar_m = 0.65 * ar_m + 1.9 * rng.normal();
            ar_n = 0.45 * ar_n + 17.0 * rng.normal();
            let m = (m_mean + ar_m).clamp(12.0, 42.0);
            let no3 = (n_mean + ar_n).max(6.0);
            let run = &mut e.project.runs[i];
            run.settings.insert(moist, Value::Num(rnd(m, 1)));
            run.settings.insert(nitrate, Value::Num(rnd(no3, 0)));
            let nr = num(run, n);
            let ir = num(run, irr);
            let d = num(run, dens);
            let v = text(run, var);
            let (ymax, nscale, p_base) = match v.as_str() {
                "Arcadia" => (10.3, 100.0, 11.7),
                "Brenton" => (11.0, 125.0, 11.0),
                _ => (9.8, 95.0, 12.9),
            };
            let navail = 55.0 + nr + 0.8 * no3;
            // Lodging and disease at high available N, worse in dense stands.
            let lodging = if navail > 250.0 { 0.12 * ((navail - 250.0) / 100.0).powi(2) * (1.0 + 0.5 * (d - 350.0) / 100.0) } else { 0.0 };
            let f_n = 1.0 - (-navail / nscale).exp() - lodging;
            let water = ir + 2.2 * (m - 27.0);
            let mut f_w = 1.0 - 0.30 * ((100.0 - water.min(100.0)) / 60.0).powi(2);
            if water > 108.0 {
                f_w -= 0.025 * (water - 108.0) / 10.0;
            }
            let f_d = 1.0 - 0.06 * ((d - 370.0) / 100.0).powi(2);
            let y = ymax * f_n * f_w.max(0.3) * f_d + f_eff + 0.38 * rng.normal();
            let prot = p_base + 0.0095 * (navail - 200.0) - 0.32 * (y - 8.5) + 0.22 * rng.normal();
            run.responses.insert(y_yield, rnd(y, 2));
            run.responses.insert(y_prot, rnd(prot, 1));
            run.status = RunStatus::Done;
            run.note = format!("Plot {code}-{plot:02}");
            if fname == "Hill" && plot == 11 {
                run.status = RunStatus::Excluded;
                let lost = run.responses[&y_yield] * 0.62;
                run.responses.insert(y_yield, rnd(lost, 2));
                run.note = format!("Plot {code}-{plot:02}: hail damage on 14 June, about 40 % of the plot lodged");
            }
            if fname == "River" && plot == 6 {
                run.responses.remove(&y_prot);
                run.note = format!("Plot {code}-{plot:02}: NIR subsample lost in the lab");
            }
            if fname == "North" && plot == 13 {
                run.settings.remove(&nitrate);
                run.note = format!("Plot {code}-{plot:02}: soil core not analysed (sample bag split)");
            }
        }
    }
    save(&mut e, dir, "field-crop-nitrogen-trial.sdoe")
}

// ---------------------------------------------------------------------------
// 2. Heat pump heating curve in an office building
// ---------------------------------------------------------------------------

fn hvac(dir: &PathBuf) -> PathBuf {
    let mut e = Engine::new();
    call(
        &mut e,
        "project.set",
        json!({
            "title": "Office heat pump: supply temperature and fan-coil study",
            "notes": "Two-storey office (about 900 m², 24 fan-coil units) heated by an air-to-water heat pump. \
Over eight working weeks (5 January to 27 February 2026) the building management system applied one combination of supply water temperature, \
fan-coil speed and preheat start per working day. The settings were planned as an optimal design around the day-ahead forecast of the daily mean outdoor temperature; \
the run notes give the date and the forecast, the Outdoor temperature column holds the measured daily mean. Solar irradiation is the measured daily global horizontal total. \
Electricity covers the heat pump, the fan coils and the circulation pumps (sub-meter). Discomfort is the sum of degree-hours below 20.5 °C over all zone sensors during occupancy (07:00–18:00), averaged over zones. \
Goal: a heating curve (supply temperature as a function of outdoor temperature) and fan speed that keep discomfort low at the lowest electricity use."
        }),
    );
    let sup = factor(&mut e, "Supply temperature", "°C", "Continuous", "Controlled", Some((30.0, 50.0)), None);
    let fan = factor(&mut e, "Fan speed", "%", "Continuous", "Controlled", Some((30.0, 100.0)), None);
    let pre = factor(&mut e, "Preheat start", "h", "Continuous", "Controlled", Some((0.5, 3.5)), None);
    let tout = factor(&mut e, "Outdoor temperature", "°C", "Continuous", "Environmental", Some((-10.0, 12.0)), None);
    let sol = factor(&mut e, "Solar irradiation", "kWh/m²", "Continuous", "Environmental", Some((0.2, 3.5)), None);
    let y_el = response(&mut e, "Electricity", "kWh", "Minimize", "50", "", "250", 1.0, 3.0);
    let y_dis = response(&mut e, "Discomfort", "K·h", "Minimize", "0", "", "6", 1.0, 4.0);

    // Daily weather over 56 calendar days from Monday 5 January (day of year 4).
    let mut rng = Rng::new(20260105);
    let days = 56;
    let mut t_true = Vec::new();
    let mut solar = Vec::new();
    let mut anom = 1.5f64;
    let mut cloud = 0.0f64;
    for d in 0..days {
        let clim = 0.6 + 2.6 * d as f64 / days as f64;
        anom = 0.75 * anom + 2.5 * rng.normal();
        // A cold spell in the third week (continental high pressure).
        let spell = if (15..22).contains(&d) { -5.5 * (1.0 - ((d as f64 - 18.0) / 4.0).powi(2)).max(0.0) } else { 0.0 };
        let t = clim + anom + spell;
        t_true.push(t);
        // Cold anomalies come with clearer skies here (anticyclonic weather).
        cloud = 0.55 * cloud + 0.9 * rng.normal();
        let clearness = logistic(cloud - 0.35 * (anom + spell) / 3.0 - 0.6);
        let clear_sky = 1.25 + 1.75 * d as f64 / days as f64;
        solar.push(clear_sky * (0.18 + 0.82 * clearness));
    }
    let weekdays: Vec<usize> = (0..days).filter(|d| d % 7 < 5).collect();
    let forecast: Vec<f64> = weekdays.iter().map(|&d| rnd(t_true[d] + 0.9 * rng.normal(), 1)).collect();
    let fc_text: Vec<String> = forecast.iter().map(|v| v.to_string()).collect();
    let opts = json!({
        "criterion": "D", "model": "quadratic", "env_interactions": true, "starts": 40, "seed": 12,
        format!("env_{tout}"): fc_text.join(", "),
    });
    // Run order follows the calendar, so the design is applied without randomisation.
    design(&mut e, "optimal", opts, false, 1);
    assert_eq!(e.project.runs.len(), weekdays.len());

    let ua = 1.25; // kW/K
    for (k, run) in e.project.runs.iter_mut().enumerate() {
        let d = weekdays[k];
        let to = rnd(t_true[d], 1);
        let es = rnd(solar[d], 2);
        run.settings.insert(tout, Value::Num(to));
        run.settings.insert(sol, Value::Num(es));
        let a = num(run, sup);
        let b = num(run, fan);
        let c = num(run, pre);
        // Heat delivered over the day.
        let q = (ua * (21.0 - to) * 24.0 * 0.62 + 6.0 * c - 20.0 * es - 40.0).max(20.0) * (1.0 + 0.006 * (a - 40.0));
        let cop = 0.42 * (a + 3.0 + 273.15) / (a + 3.0 - (to - 6.0));
        let hours = 11.0 + c;
        let fan_kwh = 2.9 * (b / 100.0).powf(2.5) * hours;
        let aux = 18.0 + 1.2 * c;
        let el = (q / cop + fan_kwh + aux) * (1.0 + 0.025 * rng.normal());
        // Comfort: emitter capacity against the occupied-hours load, plus the
        // morning recovery after the night setback.
        let cap = 52.0 * ((a - 23.0) / 27.0).max(0.01).powf(1.2) * (0.35 + 0.65 * (b / 100.0).powf(0.8));
        let p_req = ua * (21.0 - to + 2.0) - 2.5 * es - 4.0;
        // Indoor temperature deficit during occupancy; thermal mass and
        // internal gains limit how far the zones fall below the setpoint.
        let deficit = 4.5 * (1.0 - (-softplus((p_req - cap) / (3.0 * ua), 0.3) / 4.5).exp());
        let steady = 0.6 * 11.0 * deficit;
        let dt_night = 0.18 * (21.0 - to);
        let rec_h = dt_night * 6.0 / (cap - 0.8 * p_req).max(1.0);
        let morning = 0.5 * dt_night * softplus(rec_h - c, 0.3).min(5.0);
        let dis = (steady + morning + 0.25 * rng.normal()).max(0.0);
        run.responses.insert(y_el, rnd(el, 1));
        run.responses.insert(y_dis, rnd(dis, 1));
        run.status = RunStatus::Done;
        run.note = format!("{}; forecast {} °C", date_2026(4 + d), forecast[k]);
    }
    // Data problems on two days.
    let r = &mut e.project.runs[13];
    r.responses.remove(&y_el);
    r.note = format!("{}; electricity sub-meter logger restarted, no daily total", r.note);
    let r = &mut e.project.runs[27];
    r.status = RunStatus::Excluded;
    r.note = format!("{}; building closed in the afternoon for a staff event, occupancy not representative", r.note);
    save(&mut e, dir, "field-heat-pump-heating-curve.sdoe")
}

// ---------------------------------------------------------------------------
// 3. Hyperparameter study on a shared GPU cluster
// ---------------------------------------------------------------------------

fn ml_lhs(dir: &PathBuf) -> PathBuf {
    let mut e = Engine::new();
    call(
        &mut e,
        "project.set",
        json!({
            "title": "Image classifier hyperparameter study",
            "notes": "ResNet-18 trained from scratch for 30 epochs on a 100-class image set (120 000 training images, 10 000 validation images) on the shared GPU cluster. \
A 48-run Latin hypercube covers learning rate (entered as log10, e.g. −3 = 0.001), batch size, dropout and optimizer; each run is one training job with a fixed random seed. \
The scheduler assigns each job to whichever GPU node type is free, and the shared storage throughput (mean read rate logged by the data loader) varies with cluster load over the day; both are recorded as environmental factors. \
Jobs whose loss became NaN were stopped and are excluded. Goal: highest validation top-1 accuracy at a reasonable training time."
        }),
    );
    let lr = factor(&mut e, "Learning rate", "log10", "Continuous", "Controlled", Some((-4.5, -1.0)), None);
    let bs = factor(&mut e, "Batch size", "", "Discrete", "Controlled", None, Some("16, 32, 64, 128"));
    let dr = factor(&mut e, "Dropout", "", "Continuous", "Controlled", Some((0.0, 0.5)), None);
    let opt = factor(&mut e, "Optimizer", "", "Categorical", "Controlled", None, Some("AdamW, SGD, RMSprop"));
    let gpu = factor(&mut e, "GPU node", "", "Categorical", "Environmental", None, Some("A100, V100, L4"));
    let io = factor(&mut e, "Storage throughput", "GB/s", "Continuous", "Environmental", Some((0.3, 2.5)), None);
    let y_acc = response(&mut e, "Validation accuracy", "%", "Maximize", "70", "", "80", 1.0, 4.0);
    let y_time = response(&mut e, "Training time", "min", "Minimize", "25", "", "120", 1.0, 2.0);
    design(&mut e, "lhs", json!({"runs": 48, "iterations": 4000, "seed": 21}), true, 77);

    let mut rng = Rng::new(4242);
    let mut ar = 0.0f64;
    for (k, run) in e.project.runs.iter_mut().enumerate() {
        // Jobs start about every 1.5 h from 09:00 on day 1.
        let hour = (9.0 + 1.5 * k as f64) % 24.0;
        let busy = (-((hour - 14.0) / 4.0).powi(2)).exp();
        ar = 0.6 * ar + 0.13 * rng.normal();
        let thr = (1.75 - 1.05 * busy + ar).clamp(0.32, 2.45);
        let u = rng.uniform();
        let node = if u < 0.45 { "A100" } else if u < 0.8 { "V100" } else { "L4" };
        run.settings.insert(gpu, Value::Text(node.into()));
        run.settings.insert(io, Value::Num(rnd(thr, 2)));
        let l = num(run, lr);
        let b = num(run, bs);
        let p = num(run, dr);
        let o = text(run, opt);
        let lb = (b / 32.0).log2();
        let (lr_opt, acc_max, diverge_at, step_cost) = match o.as_str() {
            "AdamW" => (-3.0 + 0.25 * lb, 79.4, -1.3, 1.06),
            "SGD" => (-1.65 + 0.3 * lb, 80.3, 9.0, 1.0),
            _ => (-3.3 + 0.2 * lb, 77.6, -1.55, 1.04),
        };
        let dl = l - lr_opt;
        let lr_pen = if dl > 0.0 { 2.6 * dl * dl } else { 1.7 * dl * dl };
        let acc = acc_max - lr_pen - 8.0 * (p - 0.18).powi(2) - 0.3 * (b / 16.0).log2() + 0.2 * rng.normal();
        // Training time: GPU-bound and I/O-bound phases partly overlap.
        let peak = match node {
            "A100" => 2600.0,
            "V100" => 1500.0,
            _ => 1100.0,
        };
        let gpu_rate = peak * b / (b + 20.0) / step_cost;
        let io_rate = thr * 1e9 / 600e3;
        let images = 120_000.0 * 30.0;
        let t_gpu = images / gpu_rate;
        let t_io = images / io_rate;
        let secs = t_gpu.max(t_io) + 0.2 * t_gpu.min(t_io) + 30.0 * 15.0;
        let minutes = secs / 60.0 * (1.0 + 0.03 * rng.normal());
        run.status = RunStatus::Done;
        if l > diverge_at {
            let epoch = 2 + rng.below(3);
            run.responses.insert(y_acc, 1.0);
            run.responses.insert(y_time, rnd(minutes * epoch as f64 / 30.0, 1));
            run.status = RunStatus::Excluded;
            run.note = format!("Loss became NaN in epoch {epoch}; job stopped");
        } else {
            run.responses.insert(y_acc, rnd(acc, 2));
            run.responses.insert(y_time, rnd(minutes, 1));
        }
        if k == 31 {
            run.responses.remove(&y_time);
            run.note = "Job requeued after node maintenance; wall-clock time not comparable".into();
        }
    }
    save(&mut e, dir, "field-ml-hyperparameter-lhs.sdoe")
}

// ---------------------------------------------------------------------------
// 4. Data centre cooling: historical operating log for sequential runs
// ---------------------------------------------------------------------------

fn datacentre(dir: &PathBuf) -> PathBuf {
    let mut e = Engine::new();
    call(
        &mut e,
        "project.set",
        json!({
            "title": "Data centre chilled-water setpoint log",
            "notes": "Operating log of an 800 kW data hall with chillers, a waterside economizer and computer-room air handlers (CRAH), May–June 2026. \
Each run is one day: the chilled-water supply setpoint and CRAH fan speed the operators chose, the daily mean outdoor wet-bulb temperature and the mean IT load. \
The settings were not designed; the operators changed them by judgement, often lowering the setpoint on warm days, so settings and weather are correlated. \
PUE is the daily facility energy divided by IT energy; the inlet column is the highest 15-minute rack inlet temperature of the day (limit 27 °C). \
Use Next run with today's wet-bulb forecast and IT load to choose an informative or the best predicted setting for the next day, then add the result to the table."
        }),
    );
    let chw = factor(&mut e, "Chilled-water setpoint", "°C", "Continuous", "Controlled", Some((7.0, 16.0)), None);
    let fan = factor(&mut e, "CRAH fan speed", "%", "Continuous", "Controlled", Some((50.0, 100.0)), None);
    let wb = factor(&mut e, "Outdoor wet-bulb", "°C", "Continuous", "Environmental", Some((0.0, 22.0)), None);
    let load = factor(&mut e, "IT load", "kW", "Continuous", "Environmental", Some((550.0, 900.0)), None);
    let y_pue = response(&mut e, "PUE", "", "Minimize", "1.15", "", "1.45", 1.0, 3.0);
    let y_inlet = response(&mut e, "Max rack inlet", "°C", "Minimize", "24", "", "27", 1.0, 5.0);

    // Operator settings over 18 days (setpoint °C, fan %).
    let settings: [(f64, f64); 18] = [
        (9.0, 75.0),
        (9.0, 75.0),
        (9.0, 75.0),
        (9.0, 75.0),
        (9.0, 80.0),
        (9.5, 80.0),
        (10.0, 80.0),
        (11.0, 80.0),
        (11.0, 85.0),
        (11.0, 85.0),
        (9.5, 85.0),
        (12.0, 85.0),
        (12.0, 85.0),
        (12.0, 90.0),
        (13.0, 90.0),
        (12.0, 90.0),
        (12.5, 70.0),
        (11.0, 95.0),
    ];
    let mut rng = Rng::new(510);
    let mut anom = 0.5f64;
    let start = 130; // 11 May 2026 (day of year, 0-based)
    for (d, &(a, b)) in settings.iter().enumerate() {
        anom = 0.7 * anom + 1.8 * rng.normal();
        // Warm spell around day 10 that led the operators to lower the setpoint.
        let warm = if (9..12).contains(&d) { 4.0 } else { 0.0 };
        let w = 8.5 + 0.22 * d as f64 + anom + warm;
        let doy = start + d;
        // doy % 7: 0 = Thursday, 2 = Saturday, 3 = Sunday.
        let weekend = matches!(doy % 7, 2 | 3);
        let it = 700.0 + 2.0 * d as f64 - if weekend { 45.0 } else { 0.0 } + 12.0 * rng.normal();
        let mut vals = serde_json::Map::new();
        vals.insert(chw.to_string(), json!(a.to_string()));
        vals.insert(fan.to_string(), json!(b.to_string()));
        vals.insert(wb.to_string(), json!(rnd(w, 1).to_string()));
        vals.insert(load.to_string(), json!(rnd(it, 0).to_string()));
        let row = call(&mut e, "design.add_run", json!({"values": vals, "type": "historical"}))["row"].as_u64().unwrap() as usize;
        let run = &mut e.project.runs[row];
        let w = num(run, wb);
        let it = num(run, load);
        let q = it;
        let cop = 5.5 + 0.25 * (a - 10.0) - 0.12 * (w - 12.0);
        let ff = logistic((a - 4.0 - w) / 1.5);
        let chiller = (1.0 - ff) * q / cop + ff * q * 0.03;
        let fans = 42.0 * (b / 100.0).powi(3);
        let towers = 15.0 + 0.5 * w.max(0.0);
        let other = 0.06 * it + 20.0;
        let pue = (it + chiller + fans + towers + other) / it + 0.006 * rng.normal();
        let inlet = a + 8.0 + 14.0 * (1.0 - b / 100.0).powf(1.1) * (it / 700.0) + 0.8 * (it - 700.0) / 100.0 + 0.3 * rng.normal();
        run.responses.insert(y_pue, rnd(pue, 3));
        run.responses.insert(y_inlet, rnd(inlet, 1));
        run.status = RunStatus::Done;
        run.note = date_2026(doy);
        if d == 6 {
            run.responses.remove(&y_pue);
            run.note = format!("{}; facility power meter offline 03:00–15:00, no daily PUE", run.note);
        }
        if d == 16 {
            run.note = format!("{}; hot-aisle alarm in row C at 14:20, fan speed raised the next day", run.note);
        }
    }
    save(&mut e, dir, "field-datacentre-cooling-log.sdoe")
}

// ---------------------------------------------------------------------------
// 5. Landing page test: planned, no results yet
// ---------------------------------------------------------------------------

fn landing_page(dir: &PathBuf) -> PathBuf {
    let mut e = Engine::new();
    call(
        &mut e,
        "project.set",
        json!({
            "title": "Pricing page multivariate test (planned)",
            "notes": "Time-sliced test of the SaaS pricing page: the content system serves a single page version to all visitors on a given day, \
so each run is one calendar day with one combination of headline, button colour and price display. \
Full factorial (12 combinations) in 3 replicates over 36 days in random order. \
For each day, record the weekday and the share of visitors from paid campaigns (web analytics), then the conversion rate (trial sign-ups per visitor) and revenue per visitor after 30 days."
        }),
    );
    factor(&mut e, "Headline", "", "Categorical", "Controlled", None, Some("Benefit, Question, Social proof"));
    factor(&mut e, "Button colour", "", "Categorical", "Controlled", None, Some("Green, Orange"));
    factor(&mut e, "Price display", "", "Categorical", "Controlled", None, Some("Monthly, Annual"));
    factor(&mut e, "Weekday", "", "Categorical", "Environmental", None, Some("Mon, Tue, Wed, Thu, Fri, Sat, Sun"));
    factor(&mut e, "Paid traffic share", "%", "Continuous", "Environmental", Some((10.0, 60.0)), None);
    response(&mut e, "Conversion rate", "%", "Maximize", "2", "", "4", 1.0, 4.0);
    response(&mut e, "Revenue per visitor", "EUR", "Maximize", "0.8", "", "1.6", 1.0, 3.0);
    design(&mut e, "full_factorial", json!({"replicates": 3}), true, 3131);
    save(&mut e, dir, "field-pricing-page-test-planned.sdoe")
}

// ---------------------------------------------------------------------------
// 6. PV soiling study: empty template
// ---------------------------------------------------------------------------

fn pv_template(dir: &PathBuf) -> PathBuf {
    let mut e = Engine::new();
    call(
        &mut e,
        "project.set",
        json!({
            "title": "PV soiling and cleaning study (template)",
            "notes": "Template for a field study at a ground-mounted PV plant in a dusty, semi-arid location. \
Each run is one string of modules over one cleaning cycle. Cleaning interval, cleaning method and an anti-soiling coating are set per string; \
rainfall since the last cleaning, mean PM10 concentration and mean module temperature are recorded from the on-site weather station. \
Performance ratio and soiling loss come from the string monitoring compared with a reference string cleaned daily. \
Choose a design on the Design page (an optimal design with environmental interactions suits a study over one dry season)."
        }),
    );
    factor(&mut e, "Cleaning interval", "days", "Discrete", "Controlled", None, Some("7, 14, 28, 56"));
    factor(&mut e, "Cleaning method", "", "Categorical", "Controlled", None, Some("Water rinse, Dry brush, Robot"));
    factor(&mut e, "Coating", "", "Categorical", "Controlled", None, Some("None, Anti-soiling"));
    factor(&mut e, "Rainfall since cleaning", "mm", "Continuous", "Environmental", Some((0.0, 80.0)), None);
    factor(&mut e, "PM10", "µg/m³", "Continuous", "Environmental", Some((5.0, 120.0)), None);
    factor(&mut e, "Module temperature", "°C", "Continuous", "Environmental", Some((15.0, 65.0)), None);
    response(&mut e, "Performance ratio", "%", "Maximize", "78", "", "88", 1.0, 4.0);
    response(&mut e, "Soiling loss", "%", "Minimize", "0.5", "", "6", 1.0, 3.0);
    response(&mut e, "Water use", "L/kWp", "Minimize", "0", "", "4", 1.0, 2.0);
    save(&mut e, dir, "field-pv-soiling-template.sdoe")
}

// ---------------------------------------------------------------------------
// Verification
// ---------------------------------------------------------------------------

fn stat(fit: &Json, name: &str) -> String {
    fit["stats"].as_array().unwrap().iter().find(|s| s[0] == json!(name)).map(|s| s[1].as_str().unwrap().to_string()).unwrap_or_default()
}

fn fid(e: &mut Engine, name: &str) -> String {
    let list = call(e, "factors.list", json!({}));
    list.as_array().unwrap().iter().find(|f| f["name"] == json!(name)).unwrap_or_else(|| panic!("factor {name}"))["id"].as_str().unwrap().to_string()
}

fn verify(path: &PathBuf) {
    let text = std::fs::read_to_string(path).unwrap();
    let mut e = Engine::new();
    call(&mut e, "project.open", json!({"text": text}));
    let st = call(&mut e, "project.state", json!({}));
    println!(
        "\n== {} ==\n   \"{}\": {} factors, {} responses, {} runs ({} done), design: {}",
        path.file_name().unwrap().to_string_lossy(),
        st["title"].as_str().unwrap(),
        st["factors"],
        st["responses"],
        st["runs"],
        st["done"],
        st["design"]["method"].as_str().unwrap_or("none")
    );
    let roles: Vec<String> = call(&mut e, "factors.list", json!({}))
        .as_array()
        .unwrap()
        .iter()
        .map(|f| format!("{}={} ({}, {})", f["symbol"].as_str().unwrap(), f["name"].as_str().unwrap(), f["kind"].as_str().unwrap(), f["role"].as_str().unwrap()))
        .collect();
    println!("   factors: {}", roles.join("; "));
    if st["done"].as_u64().unwrap() == 0 {
        return;
    }
    let resp = call(&mut e, "responses.list", json!({}));
    for r in resp.as_array().unwrap() {
        let rid = r["id"].as_str().unwrap();
        let fit = call(&mut e, "analysis.fit", json!({"response": rid}));
        let idn: u64 = rid.parse().unwrap();
        let vals: Vec<f64> = e.project.runs.iter().filter_map(|r| r.responses.get(&idn).copied()).collect();
        let (lo, hi) = vals.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |a, v| (a.0.min(*v), a.1.max(*v)));
        println!("   {} observed {lo} .. {hi} ({} values)", r["name"].as_str().unwrap(), vals.len());
        println!(
            "   {} ~ {}\n      R² {}  adj R² {}  pred R² {}  n {}",
            r["name"].as_str().unwrap(),
            fit["formula"].as_str().unwrap(),
            stat(&fit, "R²"),
            stat(&fit, "Adjusted R²"),
            stat(&fit, "Predicted R²"),
            stat(&fit, "Runs used")
        );
        for w in fit["warnings"].as_array().unwrap() {
            println!("      ! {}", w.as_str().unwrap());
        }
        let hint = fit["warnings"].as_array().unwrap().iter().any(|w| w.as_str().unwrap().contains("Environment × factor interactions"));
        if hint {
            // Model with environment interactions: the adjusted R² comparison.
            let mut e2 = Engine::new();
            call(&mut e2, "project.open", json!({"text": text}));
            call(&mut e2, "model.preset", json!({"response": rid, "preset": "quadratic", "env_interactions": true}));
            call(&mut e2, "model.select", json!({"response": rid, "method": "aicc"}));
            let f2 = call(&mut e2, "analysis.fit", json!({"response": rid}));
            println!(
                "      with env interactions + AICc: {}\n      R² {}  adj R² {}  pred R² {}",
                f2["formula"].as_str().unwrap(),
                stat(&f2, "R²"),
                stat(&f2, "Adjusted R²"),
                stat(&f2, "Predicted R²")
            );
        }
        // Coefficient table of the significant terms.
        let coefs = &fit["coefficients"];
        if let Some(rows) = coefs["rows"].as_array() {
            let sig: Vec<String> = rows
                .iter()
                .filter_map(|row| {
                    let cells = row.as_array()?;
                    let label = cells.first()?.as_str()?.to_string();
                    let pv = cells.get(4)?.as_str()?;
                    (pv.starts_with("< ") || pv.parse::<f64>().map(|v| v < 0.01).unwrap_or(false)).then_some(label)
                })
                .collect();
            if !sig.is_empty() {
                println!("      p < 0.01: {}", sig.join(", "));
            }
        }
    }
    let name = path.file_name().unwrap().to_string_lossy().to_string();
    if name.contains("heat-pump") {
        let t = fid(&mut e, "Outdoor temperature");
        for temp in ["-6", "2", "9"] {
            let res = call(&mut e, "optimize.run", json!({"env": {t.clone(): {"mode": "fixed", "value": temp}}}));
            let sol = &res["solutions"][0];
            println!("   optimum at {temp} °C: d = {:.3}, values {}", sol["d"].as_f64().unwrap(), sol["values"]);
            for n in res["notes"].as_array().unwrap() {
                println!("      note: {}", n.as_str().unwrap());
            }
        }
        let law = call(&mut e, "optimize.control_law", json!({"factor": t}));
        for f in law["formulas"].as_array().unwrap() {
            println!("   law: {}", f.as_str().unwrap());
        }
        let rob = call(&mut e, "optimize.run", json!({"env": {t.clone(): {"mode": "random", "mean": "1.5", "sd": "4"}}}));
        println!("   robust optimum: d = {:.3}, values {}", rob["solutions"][0]["d"].as_f64().unwrap(), rob["solutions"][0]["values"]);
    }
    if name.contains("crop") {
        let res = call(&mut e, "optimize.run", json!({}));
        println!("   optimum: d = {:.3}, values {}", res["solutions"][0]["d"].as_f64().unwrap(), res["solutions"][0]["values"]);
        let no3 = fid(&mut e, "Soil nitrate");
        // Models with environment interactions, reduced by stepwise AICc.
        for r in call(&mut e, "responses.list", json!({})).as_array().unwrap() {
            let rid = r["id"].as_str().unwrap();
            call(&mut e, "model.preset", json!({"response": rid, "preset": "quadratic", "env_interactions": true}));
            call(&mut e, "model.select", json!({"response": rid, "method": "aicc"}));
            let f2 = call(&mut e, "analysis.fit", json!({"response": rid}));
            println!("   {} (env interactions + AICc): {}  adj R² {}  pred R² {}", r["name"].as_str().unwrap(), f2["formula"].as_str().unwrap(), stat(&f2, "Adjusted R²"), stat(&f2, "Predicted R²"));
        }
        for v in ["20", "80"] {
            let res = call(&mut e, "optimize.run", json!({"env": {no3.clone(): {"mode": "fixed", "value": v}}}));
            println!("   optimum at soil nitrate {v}: values {}", res["solutions"][0]["values"]);
        }
    }
    if name.contains("ml-") {
        let res = call(&mut e, "optimize.run", json!({}));
        println!("   optimum: d = {:.3}, values {}", res["solutions"][0]["d"].as_f64().unwrap(), res["solutions"][0]["values"]);
    }
    if name.contains("datacentre") {
        let w = fid(&mut e, "Outdoor wet-bulb");
        let l = fid(&mut e, "IT load");
        let nr = call(&mut e, "design.next_run", json!({"env": {w.clone(): "15.5", l.clone(): "730"}, "mode": "info"}));
        println!("   next run (info) at wet-bulb 15.5, load 730: {}", nr["values"]);
        for p in nr["predictions"].as_array().unwrap() {
            println!("      {}", p.as_str().unwrap());
        }
        let nb = call(&mut e, "design.next_run", json!({"env": {w.clone(): "15.5", l.clone(): "730"}, "mode": "best"}));
        println!("   next run (best): {}", nb["values"]);
        for p in nb["predictions"].as_array().unwrap() {
            println!("      {}", p.as_str().unwrap());
        }
        call(&mut e, "design.add_run", json!({"values": nb["values"]}));
    }
    // Report and CSV for every project with data.
    let html = call(&mut e, "export.report", json!({}))["html"].as_str().unwrap().len();
    let csv = call(&mut e, "export.csv", json!({}))["text"].as_str().unwrap().lines().count();
    println!("   report {html} bytes, CSV {csv} lines");
}

fn main() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("examples");
    std::fs::create_dir_all(&dir).expect("create examples directory");
    let files = vec![crop_trial(&dir), hvac(&dir), ml_lhs(&dir), datacentre(&dir), landing_page(&dir), pv_template(&dir)];
    for f in &files {
        println!("wrote {}", f.display());
    }
    for f in &files {
        verify(f);
    }
}
