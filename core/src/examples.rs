//! Example projects with simulated data, for learning and testing.

use crate::project::*;
use crate::rng::Rng;
use serde_json::json;

fn factor(p: &mut Project, name: &str, units: &str, kind: FactorKind, role: Role) -> u64 {
    let id = p.new_id();
    let symbol = p.next_symbol();
    p.factors.push(Factor { id, symbol, name: name.into(), units: units.into(), kind, role, condition: None });
    id
}

fn response(p: &mut Project, name: &str, units: &str, goal: Goal, lower: f64, upper: f64) -> u64 {
    let id = p.new_id();
    p.responses.push(Response {
        id,
        name: name.into(),
        units: units.into(),
        goal,
        lower: Some(lower),
        target: None,
        upper: Some(upper),
        weight: 1.0,
        importance: 3.0,
    });
    id
}

pub fn build(name: &str) -> Result<Project, String> {
    match name {
        "reactor" => Ok(reactor()),
        "blend" => Ok(blend()),
        _ => Err(format!("unknown example '{name}'")),
    }
}

/// Chemical reactor: continuous, discrete and categorical factors plus a
/// measured humidity that shifts the optimum.
fn reactor() -> Project {
    let mut p = Project { title: "Reactor yield study (example)".into(), ..Default::default() };
    let a = factor(&mut p, "Temperature", "°C", FactorKind::Continuous { low: 150.0, high: 200.0 }, Role::Controlled);
    let b = factor(&mut p, "Time", "min", FactorKind::Continuous { low: 10.0, high: 30.0 }, Role::Controlled);
    let c = factor(&mut p, "Catalyst", "", FactorKind::Categorical { levels: vec!["X".into(), "Y".into()] }, Role::Controlled);
    let d = factor(&mut p, "Stirring", "rpm", FactorKind::Discrete { levels: vec![200.0, 400.0, 600.0] }, Role::Controlled);
    let e = factor(&mut p, "Humidity", "%RH", FactorKind::Continuous { low: 30.0, high: 70.0 }, Role::Environmental);
    let y1 = response(&mut p, "Yield", "%", Goal::Maximize, 75.0, 92.0);
    let y2 = response(&mut p, "Impurity", "%", Goal::Minimize, 0.8, 2.5);
    let opts = json!({ "criterion": "D", "model": "quadratic", "runs": 26, "starts": 10, "seed": 5 });
    let g = crate::design::generate(&p, "optimal", opts.as_object().unwrap()).expect("example design");
    crate::api::apply_design(&mut p, &g, true, true, 11);
    let mut rng = Rng::new(31);
    for run in p.runs.iter_mut() {
        let hum = 30.0 + 40.0 * rng.uniform();
        run.settings.insert(e, Value::Num((hum * 10.0).round() / 10.0));
        let ca = (run.settings[&a].as_num().unwrap() - 175.0) / 25.0;
        let cb = (run.settings[&b].as_num().unwrap() - 20.0) / 10.0;
        let cc = if run.settings[&c] == Value::Text("X".into()) { 1.0 } else { -1.0 };
        let cd = (run.settings[&d].as_num().unwrap() - 400.0) / 200.0;
        let ce = (hum - 50.0) / 20.0;
        let yv = 85.0 + 4.0 * ca + 3.0 * cb - 3.0 * ca * ca - 2.0 * cb * cb + 1.5 * ca * cb + 1.8 * cc + 1.2 * cd - 0.8 * cd * cd
            - 2.5 * ce
            - 1.8 * ca * ce
            + 0.8 * rng.normal();
        let imp = 1.5 + 0.45 * ca - 0.2 * cb + 0.3 * ca * ca + 0.15 * cc + 0.35 * ce + 0.1 * rng.normal();
        run.responses.insert(y1, (yv * 100.0).round() / 100.0);
        run.responses.insert(y2, (imp.max(0.05) * 1000.0).round() / 1000.0);
        run.status = RunStatus::Done;
    }
    // Yield model: quadratic with environment interactions, reduced by stepwise AICc selection.
    let pool = crate::model::terms::preset_terms(&p, crate::model::terms::Preset::Quadratic, true);
    let start = crate::model::terms::preset_terms(&p, crate::model::terms::Preset::Linear, false);
    if let Ok((terms, _)) = crate::model::select::stepwise_aicc(&p, y1, &pool, Transform::None, &start) {
        crate::analysis::set_model(&mut p, y1, &terms, Transform::None);
    }
    p.notes = "Simulated data. Humidity is measured, not set: include it in the model and use the optimizer's environmental settings to adjust temperature for the day's humidity.".into();
    p
}

/// Three-component mixture with a process variable.
fn blend() -> Project {
    let mut p = Project { title: "Polymer blend (example)".into(), ..Default::default() };
    let a = factor(&mut p, "Resin", "", FactorKind::Mixture { low: 0.2, high: 0.8 }, Role::Controlled);
    let b = factor(&mut p, "Hardener", "", FactorKind::Mixture { low: 0.1, high: 0.6 }, Role::Controlled);
    let c = factor(&mut p, "Filler", "", FactorKind::Mixture { low: 0.0, high: 0.5 }, Role::Controlled);
    let y = response(&mut p, "Strength", "MPa", Goal::Maximize, 30.0, 55.0);
    let opts = json!({ "degree": 3, "centroid": true, "axial": true, "replicates": 1 });
    let g = crate::design::generate(&p, "simplex_lattice", opts.as_object().unwrap()).expect("example design");
    crate::api::apply_design(&mut p, &g, true, true, 3);
    let mut rng = Rng::new(8);
    for run in p.runs.iter_mut() {
        let x1 = run.settings[&a].as_num().unwrap();
        let x2 = run.settings[&b].as_num().unwrap();
        let x3 = run.settings[&c].as_num().unwrap();
        let s = 40.0 * x1 + 30.0 * x2 + 20.0 * x3 + 60.0 * x1 * x2 - 20.0 * x2 * x3 + 25.0 * x1 * x3 + 1.0 * rng.normal();
        run.responses.insert(y, (s * 100.0).round() / 100.0);
        run.status = RunStatus::Done;
    }
    p
}
