mod common;
use common::*;
use sdoe::Engine;
use serde_json::{Value, json};

/// Numeric value of the evaluation statistic whose label starts with `prefix`.
fn stat(ev: &Value, prefix: &str) -> f64 {
    let row = ev["stats"].as_array().unwrap().iter().find(|r| r[0].as_str().unwrap().starts_with(prefix)).unwrap_or_else(|| panic!("no stat {prefix}: {}", ev["stats"]));
    row[1].as_str().unwrap().trim_end_matches(" %").parse().unwrap()
}

fn cell(row: &Value, i: usize) -> f64 {
    row[i].as_str().unwrap().parse().unwrap()
}

/// A criterion of an optimal design for three continuous factors.
fn a_value_of_optimal(criterion: &str) -> f64 {
    let mut e = engine_with_project(&project(vec![cont(1, 0.0, 10.0), cont(2, 0.0, 10.0), cont(3, 0.0, 10.0)]));
    call(&mut e, "design.preview", json!({"method": "optimal", "options": {"criterion": criterion, "model": "quadratic", "runs": 12, "starts": 5, "seed": 1}}));
    stat(&call(&mut e, "evaluate.run", json!({"source": "preview", "preset": "quadratic"})), "A (")
}

/// The A-optimal design is worse on the A criterion than the D-optimal design, because the exchange accepts no move for criterion A.
#[test]
fn a_optimal_design_is_not_worse_on_a_than_d_optimal_design() {
    let a = a_value_of_optimal("A");
    let d = a_value_of_optimal("D");
    assert!(a <= d * 1.05, "A criterion of the A-optimal design is {a}, expected at most {} (D-optimal design: {d})", d * 1.05);
}

/// A simplex lattice with component uppers that sum to less than the total returns Ok with no runs.
#[test]
fn simplex_lattice_rejects_infeasible_mixture_bounds() {
    let mut e = engine_with_project(&project(vec![mix(1, 0.0, 0.3), mix(2, 0.0, 0.3), mix(3, 0.0, 0.3)]));
    let out = reply(&mut e, "design.preview", json!({"method": "simplex_lattice", "options": {}}));
    if out["ok"] == json!(true) {
        panic!("expected an error for upper bounds summing to 0.9, got {} runs: {}", out["result"]["runs"], out["result"]["notes"]);
    }
}

/// An optimal design with component uppers that sum to less than the total returns blends that violate the upper bounds.
#[test]
fn optimal_design_rejects_infeasible_mixture_bounds() {
    let mut e = engine_with_project(&project(vec![mix(1, 0.0, 0.3), mix(2, 0.0, 0.3), mix(3, 0.0, 0.3)]));
    let out = reply(&mut e, "design.preview", json!({"method": "optimal", "options": {"model": "linear", "runs": 4}}));
    if out["ok"] == json!(true) {
        let rows = out["result"]["table"]["rows"].as_array().unwrap();
        let worst = rows.iter().flat_map(|r| (2..5).map(|i| cell(r, i))).fold(0.0, f64::max);
        panic!("expected an error for upper bounds summing to 0.9, got {} runs with a component up to {worst} (upper 0.3)", rows.len());
    }
}

/// A degree-2 lattice on bounded components returns one run, too few to fit the six-term model.
#[test]
fn bounded_simplex_lattice_has_enough_runs_for_its_model() {
    let mut e = engine_with_project(&project(vec![mix(1, 0.2, 0.5), mix(2, 0.1, 0.4), mix(3, 0.0, 0.6)]));
    let out = reply(&mut e, "design.preview", json!({"method": "simplex_lattice", "options": {"degree": 2}}));
    if out["ok"] == json!(true) {
        let runs = out["result"]["runs"].as_u64().unwrap();
        assert!(runs >= 6, "the lattice returned {runs} runs, expected an error or at least 6 runs for the model {}", out["result"]["model"]);
    }
}

/// Log determinant of X'X for the Scheffe quadratic model on three components (raw proportions).
fn scheffe_log_det(rows: &[[f64; 3]]) -> f64 {
    let x: Vec<[f64; 6]> = rows.iter().map(|&[a, b, c]| [a, b, c, a * b, a * c, b * c]).collect();
    let mut m = [[0.0f64; 6]; 6];
    for r in &x {
        for i in 0..6 {
            for j in 0..6 {
                m[i][j] += r[i] * r[j];
            }
        }
    }
    let mut ld = 0.0;
    for c in 0..6 {
        let piv = (c..6).max_by(|&a, &b| m[a][c].abs().total_cmp(&m[b][c].abs())).unwrap();
        m.swap(c, piv);
        if m[c][c].abs() < 1e-300 {
            return f64::NEG_INFINITY;
        }
        ld += m[c][c].abs().ln();
        for r in c + 1..6 {
            let f = m[r][c] / m[c][c];
            for k in c..6 {
                m[r][k] -= f * m[c][k];
            }
        }
    }
    ld
}

/// The constrained mixture D-optimal design has log det(X'X) near -25.7, far from the -23.0 of a design on extreme vertices and edge midpoints.
#[test]
fn constrained_mixture_d_optimal_design_is_near_extreme_vertices_design() {
    let mut e = engine_with_project(&project(vec![mix(1, 0.2, 0.5), mix(2, 0.1, 0.4), mix(3, 0.0, 0.6)]));
    let r = call(&mut e, "design.preview", json!({"method": "optimal", "options": {"model": "interactions", "runs": 8}}));
    let rows: Vec<[f64; 3]> = r["table"]["rows"].as_array().unwrap().iter().map(|x| [cell(x, 2), cell(x, 3), cell(x, 4)]).collect();
    assert_eq!(rows.len(), 8);
    let ld = scheffe_log_det(&rows);
    // Reference: five extreme vertices plus three edge midpoints give -22.99 in the same parametrisation.
    let reference = scheffe_log_det(&[[0.2, 0.2, 0.6], [0.2, 0.4, 0.4], [0.5, 0.1, 0.4], [0.5, 0.4, 0.1], [0.3, 0.1, 0.6], [0.35, 0.25, 0.4], [0.35, 0.4, 0.25], [0.5, 0.25, 0.25]]);
    assert!((reference + 22.99).abs() < 0.01, "reference log det {reference}");
    assert!(ld >= -23.5, "log det(X'X) of the engine design is {ld}, expected at least -23.5 (extreme-vertices design: {reference})");
}

/// An optimal design with runs = 1e12 (documented range 0-1000) aborts the process in the allocator.
#[test]
fn optimal_design_with_huge_run_count_returns_without_abort() {
    if !in_isolated_child("optimal_design_with_huge_run_count_returns_without_abort") {
        return run_isolated("optimal_design_with_huge_run_count_returns_without_abort", 20, 2048);
    }
    let mut e = engine_with_project(&project(vec![cont(1, 0.0, 10.0), cont(2, 0.0, 10.0), cont(3, 0.0, 10.0)]));
    let out = reply(&mut e, "design.preview", json!({"method": "optimal", "options": {"model": "linear", "runs": 1_000_000_000_000u64}}));
    assert!(out["ok"] == json!(false) || out["result"]["runs"].as_u64().unwrap() <= 1000, "unexpected reply with runs far outside 0-1000");
}

/// An optimal design with 26 mixture components builds 2^26 centroid candidates and exhausts memory.
#[test]
fn optimal_design_with_26_mixture_components_returns_within_limits() {
    if !in_isolated_child("optimal_design_with_26_mixture_components_returns_within_limits") {
        return run_isolated("optimal_design_with_26_mixture_components_returns_within_limits", 20, 2048);
    }
    let mut e = engine_with_project(&project((1..=26).map(|i| mix(i, 0.0, 1.0)).collect()));
    let _ = reply(&mut e, "design.preview", json!({"method": "optimal", "options": {"model": "linear", "starts": 1}}));
}

/// A simplex lattice with 30 components and degree 6 enumerates hundreds of millions of blends and exhausts memory.
#[test]
fn simplex_lattice_with_30_components_degree_6_returns_within_limits() {
    if !in_isolated_child("simplex_lattice_with_30_components_degree_6_returns_within_limits") {
        return run_isolated("simplex_lattice_with_30_components_degree_6_returns_within_limits", 20, 2048);
    }
    let mut e = engine_with_project(&project((1..=30).map(|i| mix(i, 0.0, 1.0)).collect()));
    let _ = reply(&mut e, "design.preview", json!({"method": "simplex_lattice", "options": {"degree": 6}}));
}

fn blocked_ccd_block_quadratic_contrasts(k: u64) -> Vec<f64> {
    let mut e = engine_with_project(&project((1..=k).map(|i| cont(i, 0.0, 10.0)).collect()));
    let r = call(&mut e, "design.preview", json!({"method": "ccd", "options": {"blocks": "2", "center_points": 4}}));
    let rows = r["table"]["rows"].as_array().unwrap();
    (0..k as usize)
        .map(|j| {
            let q: Vec<f64> = rows.iter().map(|x| ((cell(x, 3 + j) - 5.0) / 5.0).powi(2)).collect();
            let mean = q.iter().sum::<f64>() / q.len() as f64;
            rows.iter().zip(&q).map(|(x, q)| if x[2].as_str().unwrap() == "1" { 1.0 } else { -1.0 } * (q - mean)).sum()
        })
        .collect()
}

/// The two-block CCD with 4 center points has a block contrast correlated with the quadratic columns of three factors.
#[test]
fn blocked_ccd_three_factors_is_orthogonally_blocked() {
    let c = blocked_ccd_block_quadratic_contrasts(3);
    // The preview table holds values rounded to 5 significant digits, so the contrast is 0 only to about 1e-3.
    assert!(c.iter().all(|v| v.abs() < 1e-2), "block contrast against centred squares is {c:?}, expected all 0");
}

/// The two-block CCD with 4 center points has a block contrast correlated with the quadratic columns of four factors.
#[test]
fn blocked_ccd_four_factors_is_orthogonally_blocked() {
    let c = blocked_ccd_block_quadratic_contrasts(4);
    // The preview table holds values rounded to 5 significant digits, so the contrast is 0 only to about 1e-3.
    assert!(c.iter().all(|v| v.abs() < 1e-2), "block contrast against centred squares is {c:?}, expected all 0");
}

/// The G-efficiency of the rotatable three-factor CCD with 3 center points is reported as 94.2 % while the true value over the cube is 87.8 %.
#[test]
fn ccd_g_efficiency_matches_dense_grid_maximum() {
    let mut e = engine_with_project(&project(vec![cont(1, 0.0, 10.0), cont(2, 0.0, 10.0), cont(3, 0.0, 10.0)]));
    call(&mut e, "design.preview", json!({"method": "ccd", "options": {"center_points": 3, "alpha": "rotatable"}}));
    let g = stat(&call(&mut e, "evaluate.run", json!({"source": "preview", "preset": "quadratic"})), "G-efficiency");
    assert!((g - 87.8).abs() <= 1.5, "G-efficiency is {g} %, expected 87.8 % (maximum over the coded cube on a 81^3 grid)");
}

/// A custom CCD axial distance of zero or below is accepted instead of being rejected.
#[test]
fn ccd_rejects_non_positive_custom_alpha() {
    for alpha in ["0", "-1"] {
        let mut e = engine_with_project(&project(vec![cont(1, 0.0, 10.0), cont(2, 0.0, 10.0)]));
        let out = reply(&mut e, "design.preview", json!({"method": "ccd", "options": {"alpha": alpha, "inscribed": true}}));
        assert_eq!(out["ok"], json!(false), "alpha {alpha} was accepted: {}", out["result"]["notes"]);
    }
}

/// A condition written with a decimal comma such as `A<=1,5` is rejected as having more than one number.
#[test]
fn condition_accepts_decimal_comma_for_single_valued_operator() {
    let mut e = engine_with_project(&project(vec![cont(1, 0.0, 10.0), cont(2, 0.0, 10.0)]));
    let out = reply(&mut e, "sheet.set", json!({"sheet": "factors", "row": 1, "col": 8, "text": "A<=1,5"}));
    assert_eq!(out["ok"], json!(true), "condition `A<=1,5` was rejected: {}", out["error"]);
    let list = call(&mut e, "factors.list", json!({}));
    let cond = list[1]["condition"].as_str().unwrap();
    assert!(cond.contains("1.5") || cond.contains("1,5"), "stored condition is {cond:?}, expected a threshold of 1.5");
}

/// An optimal design for a continuous factor with low equal to high returns values other than that single value.
#[test]
fn optimal_design_with_fixed_range_factor_keeps_the_single_value() {
    let mut e = Engine::new();
    let open = reply(&mut e, "project.open", json!({"text": project(vec![cont(1, 5.0, 5.0), cont(2, 0.0, 1.0)]).to_string()}));
    if open["ok"] != json!(true) {
        return;
    }
    let out = reply(&mut e, "design.preview", json!({"method": "optimal", "options": {"model": "linear", "runs": 4}}));
    if out["ok"] == json!(true) {
        let vals: Vec<f64> = out["result"]["table"]["rows"].as_array().unwrap().iter().map(|r| cell(r, 2)).collect();
        assert!(vals.iter().all(|v| *v == 5.0), "values of the factor fixed at 5 are {vals:?}, expected all 5");
    }
}
