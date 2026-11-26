//! Glue between the project and the modelling layer: model choice per
//! response, fitting and point construction from user values.

use crate::format;
use crate::model::fit::{Fit, fit, gather};
use crate::model::terms::{Preset, Term, from_specs, preset_terms};
use crate::project::{Coded, ModelSpec, Project, RunStatus, Transform, Value};

/// Model for a response: the stored spec, else the design's model, else a
/// preset sized to the data.
pub fn model_for(p: &Project, rid: u64) -> (Vec<Term>, Transform) {
    if let Some(spec) = p.models.get(&rid) {
        let t = from_specs(p, &spec.terms);
        if !t.is_empty() {
            return (t, spec.transform);
        }
    }
    (default_terms(p), Transform::None)
}

pub fn default_terms(p: &Project) -> Vec<Term> {
    if let Some(d) = &p.design {
        let mut t = from_specs(p, &d.model);
        // Measured environmental and block factors join the design model.
        for (i, f) in p.factors.iter().enumerate() {
            if !f.role.is_designed() && !t.iter().any(|x| x.0.len() == 1 && x.0[0].0 == i) {
                t.push(Term::single(i, 1));
            }
        }
        if !t.is_empty() {
            crate::model::terms::sort_terms(&mut t);
            return t;
        }
    }
    let n = p.runs.iter().filter(|r| r.status != RunStatus::Excluded).count();
    for preset in [Preset::Quadratic, Preset::Interactions, Preset::Linear] {
        let t = preset_terms(p, preset, false);
        if n >= t.len() + 4 || preset == Preset::Linear {
            return t;
        }
    }
    preset_terms(p, Preset::Linear, false)
}

pub fn set_model(p: &mut Project, rid: u64, terms: &[Term], transform: Transform) {
    let specs = crate::model::terms::to_specs(p, terms);
    p.models.insert(rid, ModelSpec { terms: specs, transform });
}

pub fn fit_response(p: &Project, rid: u64) -> Result<Fit, String> {
    let (terms, tr) = model_for(p, rid);
    fit(gather(p, rid, &terms, tr))
}

/// Parses a user-entered value for a factor.
pub fn parse_value(p: &Project, fi: usize, text: &str) -> Result<Value, String> {
    let f = &p.factors[fi];
    let t = text.trim();
    if f.is_categorical() {
        let i = f.level_index(t).ok_or_else(|| format!("'{t}' is not a level of {} ({})", f.name, f.cat_levels().join(", ")))?;
        Ok(Value::Text(f.cat_levels()[i].clone()))
    } else {
        format::parse_num(t).map(Value::Num).ok_or_else(|| format!("'{t}' is not a number"))
    }
}

/// Builds a coded point from (factor id -> text) pairs; unspecified factors
/// sit at their center (numeric) or first level (categorical).
pub fn point_from_json(p: &Project, values: Option<&serde_json::Value>) -> Result<Vec<Option<Coded>>, String> {
    let mut pt = center_point(p);
    if let Some(serde_json::Value::Object(m)) = values {
        for (k, v) in m {
            let Ok(id) = k.parse::<u64>() else { continue };
            let Some(fi) = p.factor_index(id) else { continue };
            let text = match v {
                serde_json::Value::String(s) => s.clone(),
                serde_json::Value::Number(n) => n.to_string(),
                _ => continue,
            };
            if text.trim().is_empty() {
                continue;
            }
            let val = parse_value(p, fi, &text)?;
            pt[fi] = p.code(fi, &val);
        }
    }
    Ok(pt)
}

pub fn center_point(p: &Project) -> Vec<Option<Coded>> {
    let mc = p.mixture_coding();
    let nmix = p.factors.iter().filter(|f| f.is_mixture()).count().max(1);
    p.factors
        .iter()
        .map(|f| {
            Some(if f.is_categorical() {
                Coded::Level(0)
            } else if f.is_mixture() {
                let _ = &mc;
                Coded::Num(1.0 / nmix as f64)
            } else if let crate::project::FactorKind::Discrete { levels } = &f.kind {
                let (c, h) = f.center_half();
                Coded::Num((crate::project::nearest(levels, c) - c) / h)
            } else {
                Coded::Num(0.0)
            })
        })
        .collect()
}

/// Actual-unit display text of a coded value.
pub fn display_value(p: &Project, fi: usize, c: Option<Coded>) -> String {
    match c {
        None => String::new(),
        Some(c) => match p.decode(fi, c) {
            Value::Num(x) => format::num(x, 5),
            Value::Text(t) => t,
        },
    }
}

/// Range of observed response values.
pub fn response_range(p: &Project, rid: u64) -> Option<(f64, f64)> {
    let v: Vec<f64> = p
        .runs
        .iter()
        .filter(|r| r.status != RunStatus::Excluded)
        .filter_map(|r| r.responses.get(&rid).copied())
        .filter(|v| v.is_finite())
        .collect();
    if v.is_empty() {
        return None;
    }
    Some((v.iter().cloned().fold(f64::INFINITY, f64::min), v.iter().cloned().fold(f64::NEG_INFINITY, f64::max)))
}

/// Observed mean and standard deviation of a numeric factor (actual units).
pub fn factor_stats(p: &Project, fi: usize) -> Option<(f64, f64)> {
    let id = p.factors[fi].id;
    let v: Vec<f64> = p.runs.iter().filter_map(|r| r.settings.get(&id).and_then(|v| v.as_num())).collect();
    if v.len() < 2 {
        return None;
    }
    Some((crate::stats::mean(&v), crate::stats::std_dev(&v)))
}
