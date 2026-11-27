mod common;

use sdoe::design::{self, Generated};
use sdoe::project::*;
use serde_json::{Map, Value as J, json};
use std::collections::HashSet;

fn proj(k: usize, role: Role) -> Project {
    let mut p = Project::default();
    for i in 0..k {
        let id = p.new_id();
        p.factors.push(Factor {
            id,
            symbol: symbol_for(i),
            name: format!("F{i}"),
            units: String::new(),
            kind: FactorKind::Continuous { low: 0.0, high: 10.0 },
            role,
            condition: None,
        });
    }
    p
}

fn opts(v: J) -> Map<String, J> {
    v.as_object().unwrap().clone()
}

fn set_kind(p: &mut Project, i: usize, kind: FactorKind) {
    p.factors[i].kind = kind;
}

fn cat2(p: &mut Project, i: usize) {
    set_kind(p, i, FactorKind::Categorical { levels: vec!["a".into(), "b".into()] });
}

fn unique_runs(g: &Generated) -> usize {
    g.runs.iter().map(|r| format!("{:?}", r.coded)).collect::<HashSet<_>>().len()
}

fn mk(p: &Project, method: &str, o: J) -> Result<Generated, String> {
    design::generate(p, method, &opts(o))
}

fn describe(r: &Result<Generated, String>) -> String {
    match r {
        Ok(g) => format!("Ok with {} runs, notes {:?}", g.runs.len(), g.notes),
        Err(e) => format!("Err({e})"),
    }
}

isolated_test!(auto_fractional_factorial_20_factors_finishes_quickly, 20, 2048, {
    // Guards: the automatic fraction search for 20 factors takes longer than 60 s.
    let _ = mk(&proj(20, Role::Controlled), "fractional_factorial", json!({}));
});

isolated_test!(auto_fractional_factorial_25_factors_resolution_5_finishes_quickly, 20, 2048, {
    // Guards: the fraction search for 25 factors at resolution 5 takes longer than 120 s.
    let _ = mk(&proj(25, Role::Controlled), "fractional_factorial", json!({"resolution": 5}));
});

isolated_test!(full_factorial_64_two_level_factors_is_an_error, 20, 2048, {
    // Guards: the run count 2^64 wraps to 0 and the design is returned as Ok with no runs.
    let r = mk(&proj(64, Role::Controlled), "full_factorial", json!({}));
    assert!(r.is_err(), "expected Err for 64 two-level factors, got {}", describe(&r));
});

isolated_test!(full_factorial_70_two_level_factors_is_an_error, 20, 2048, {
    // Guards: the run count for 70 two-level factors wraps and the design is returned as Ok.
    let r = mk(&proj(70, Role::Controlled), "full_factorial", json!({}));
    assert!(r.is_err(), "expected Err for 70 two-level factors, got {}", describe(&r));
});

isolated_test!(duplicate_generator_targets_return_error_without_panic, 20, 2048, {
    // Guards: generators "C=AB, C=-AB" index out of bounds instead of returning an error.
    let r = mk(&proj(4, Role::Controlled), "fractional_factorial", json!({"generators": "C=AB, C=-AB"}));
    assert!(r.is_err(), "expected Err for duplicate generator target, got {}", describe(&r));
});

#[test]
fn foldover_of_resolution_4_fraction_adds_new_runs_or_drops_separation_claim() {
    // Guards: the fold-over of a resolution IV fraction repeats the base runs while the notes claim separated main effects.
    let r = mk(&proj(6, Role::Controlled), "fractional_factorial", json!({"foldover": true}));
    if let Ok(g) = &r {
        let claims = g.notes.iter().any(|n| {
            let n = n.to_lowercase();
            n.contains("fold") && (n.contains("separat") || n.contains("clear") || n.contains("de-alias") || n.contains("dealias"))
        });
        let uniq = unique_runs(g);
        assert!(
            uniq > 16 || !claims,
            "fold-over has {} runs, {} unique (base fraction has 16) and claims separation = {}; notes {:?}",
            g.runs.len(),
            uniq,
            claims,
            g.notes
        );
    }
}

#[test]
fn generators_giving_fully_aliased_factors_are_rejected() {
    // Guards: "D=AB, E=AB" is accepted although D and E are fully aliased.
    let r = mk(&proj(5, Role::Controlled), "fractional_factorial", json!({"generators": "D=AB, E=AB"}));
    assert!(r.is_err(), "expected Err for D=AB, E=AB, got {}", describe(&r));
}

#[test]
fn generators_with_equal_words_giving_identity_bd_are_rejected() {
    // Guards: "B=-ACE, D=-ACE" is accepted although the defining relation contains I=BD.
    let r = mk(&proj(5, Role::Controlled), "fractional_factorial", json!({"generators": "B=-ACE, D=-ACE"}));
    assert!(r.is_err(), "expected Err for B=-ACE, D=-ACE, got {}", describe(&r));
}

#[test]
fn generator_through_a_generated_factor_resolving_to_an_alias_is_rejected() {
    // Guards: with D=ABC, "E=ABD" means E = AB·ABC = C, so E and C share one column (resolution II).
    let r = mk(&proj(5, Role::Controlled), "fractional_factorial", json!({"generators": "D=ABC, E=ABD"}));
    assert!(r.is_err(), "D=ABC, E=ABD aliases E with C and must be rejected, got {}", describe(&r));
}

#[test]
fn valid_generator_through_a_generated_factor_is_accepted() {
    // Guards: with E=ABC, "F=ADE" means F = AD·ABC = BCD, a valid resolution IV fraction (I = ABCE = BCDF = ADEF).
    let k = 6;
    let r = mk(&proj(k, Role::Controlled), "fractional_factorial", json!({"generators": "E=ABC, F=ADE"}));
    let g = match &r {
        Ok(g) => g,
        Err(e) => panic!("expected Ok for E=ABC, F=ADE, got Err({e})"),
    };
    let n = g.runs.len();
    assert_eq!(n, 16, "expected 16 runs for 2^(6-2), got {n}");
    let col = |f: usize| -> Vec<f64> { g.runs.iter().map(|r| r.coded[f].map(|c| c.num()).unwrap()).collect() };
    let cols: Vec<Vec<f64>> = (0..k).map(col).collect();
    // Resolution: length of the shortest word whose column product is constant.
    let mut res = usize::MAX;
    for mask in 1u32..(1 << k) {
        let s: f64 = (0..n).map(|i| (0..k).filter(|b| mask >> b & 1 == 1).map(|b| cols[b][i]).product::<f64>()).sum();
        if (s.abs() - n as f64).abs() < 1e-9 {
            res = res.min(mask.count_ones() as usize);
        }
    }
    assert_eq!(res, 4, "expected resolution IV from the generated columns, got {res}");
}

isolated_test!(full_factorial_replicates_are_limited_to_documented_range, 20, 2048, {
    // Guards: replicates 2000000 on 3 factors builds 16 million runs although the documented range is 1 to 20.
    let r = mk(&proj(3, Role::Controlled), "full_factorial", json!({"replicates": 2000000}));
    if let Ok(g) = &r {
        assert!(g.runs.len() <= 8 * 20 + 100, "replicates 2000000 returned {} runs, expected Err or at most 160", g.runs.len());
    }
});

isolated_test!(taguchi_full_outer_array_with_21_noise_factors_is_limited, 20, 2048, {
    // Guards: a full outer array for 21 noise factors builds millions of runs without any cap.
    let mut p = proj(24, Role::Controlled);
    for i in 3..24 {
        p.factors[i].role = Role::Noise;
    }
    let r = mk(&p, "taguchi", json!({"outer": "full"}));
    if let Ok(g) = &r {
        assert!(g.runs.len() <= 100_000, "returned {} runs, expected Err or at most 100000", g.runs.len());
    }
});

isolated_test!(plackett_burman_runs_are_limited_to_documented_range, 20, 2048, {
    // Guards: runs 100000 is accepted although the documented range is 0 to 96.
    let r = mk(&proj(5, Role::Controlled), "plackett_burman", json!({"runs": 100000}));
    if let Ok(g) = &r {
        assert!(g.runs.len() <= 96 + 100, "runs 100000 returned {} runs, expected Err or at most 96", g.runs.len());
    }
});

#[test]
fn full_factorial_with_zero_level_categorical_is_an_error() {
    // Guards: a categorical factor with no levels yields Ok with 0 runs.
    let mut p = proj(2, Role::Controlled);
    set_kind(&mut p, 1, FactorKind::Categorical { levels: vec![] });
    let r = mk(&p, "full_factorial", json!({}));
    assert!(r.is_err(), "expected Err for a 0-level categorical factor, got {}", describe(&r));
}

#[test]
fn full_factorial_with_one_level_categorical_is_an_error() {
    // Guards: a categorical factor with one level cannot be varied but is accepted.
    let mut p = proj(2, Role::Controlled);
    set_kind(&mut p, 1, FactorKind::Categorical { levels: vec!["x".into()] });
    let r = mk(&p, "full_factorial", json!({}));
    assert!(r.is_err(), "expected Err for a 1-level categorical factor, got {}", describe(&r));
}

#[test]
fn full_factorial_with_one_level_discrete_is_an_error() {
    // Guards: a discrete factor with one level cannot be varied but is accepted.
    let mut p = proj(2, Role::Controlled);
    set_kind(&mut p, 1, FactorKind::Discrete { levels: vec![5.0] });
    let r = mk(&p, "full_factorial", json!({}));
    assert!(r.is_err(), "expected Err for a 1-level discrete factor, got {}", describe(&r));
}

#[test]
fn full_factorial_with_equal_continuous_bounds_is_an_error() {
    // Guards: a continuous factor with low equal to high yields runs that repeat each other.
    let mut p = proj(2, Role::Controlled);
    set_kind(&mut p, 1, FactorKind::Continuous { low: 3.0, high: 3.0 });
    let r = mk(&p, "full_factorial", json!({}));
    assert!(r.is_err(), "expected Err for low == high, got {}", describe(&r));
}

#[test]
fn plackett_burman_24_run_notes_do_not_claim_resolution_3() {
    // Guards: the 24-run Plackett-Burman note claims resolution III although the design is non-regular.
    let g = mk(&proj(20, Role::Controlled), "plackett_burman", json!({})).expect("PB with 20 factors");
    assert!(g.runs.len() >= 24, "expected at least 24 runs, got {}", g.runs.len());
    let claims = g.notes.iter().any(|n| n.to_lowercase().contains("resolution iii") || n.to_lowercase().contains("resolution 3"));
    assert!(!claims, "notes claim resolution III for a non-regular design: {:?}", g.notes);
}

#[test]
fn plackett_burman_all_categorical_adds_no_center_points() {
    // Guards: three 2-level categorical factors with center_points 3 give 7 runs instead of 4.
    let mut p = proj(3, Role::Controlled);
    for i in 0..3 {
        cat2(&mut p, i);
    }
    let g = mk(&p, "plackett_burman", json!({"center_points": 3})).expect("PB all categorical");
    assert_eq!(g.runs.len(), 4, "expected 4 runs with no center points, got {} runs", g.runs.len());
}

#[test]
fn full_factorial_center_points_do_not_duplicate_corners_for_two_level_discrete() {
    // Guards: the center run of two-level discrete factors is a corner of the factorial.
    let mut p = proj(2, Role::Controlled);
    set_kind(&mut p, 0, FactorKind::Discrete { levels: vec![1.0, 2.0] });
    set_kind(&mut p, 1, FactorKind::Discrete { levels: vec![10.0, 20.0] });
    let g = mk(&p, "full_factorial", json!({"center_points": 2})).expect("full factorial with discrete factors");
    let corners: HashSet<String> = g.runs.iter().filter(|r| r.point_type != "center").map(|r| format!("{:?}", r.coded)).collect();
    let bad: Vec<String> = g
        .runs
        .iter()
        .filter(|r| r.point_type == "center")
        .map(|r| format!("{:?}", r.coded))
        .filter(|c| corners.contains(c))
        .collect();
    assert!(bad.is_empty(), "center runs equal to corner runs: {bad:?}");
}

/// Default of option `key` of design `method` as offered to the user.
fn spec_default(p: &Project, method: &str, key: &str) -> J {
    let m = design::methods(p).into_iter().find(|m| m.id == method).unwrap();
    m.options.into_iter().find(|o| o.key == key).unwrap().default
}

/// Without a center_points option, central composite and Box-Behnken designs used 5 center
/// points instead of the default the Design page offers.
#[test]
fn omitted_center_points_use_the_offered_default() {
    for (method, ks) in [("ccd", 2..=7), ("box_behnken", 3..=7)] {
        for k in ks {
            let p = proj(k, Role::Controlled);
            let def = spec_default(&p, method, "center_points");
            let omitted = mk(&p, method, json!({})).unwrap();
            let given = mk(&p, method, json!({ "center_points": def })).unwrap();
            assert_eq!(omitted.runs.len(), given.runs.len(), "{method} with {k} factors: default {def}");
        }
    }
}

/// The Generators tooltip example "D=ABC, E=-ABD" resolves to E = -C and is rejected.
#[test]
fn generators_tooltip_example_is_accepted() {
    let tip = {
        let p = proj(6, Role::Controlled);
        let m = design::methods(&p).into_iter().find(|m| m.id == "fractional_factorial").unwrap();
        m.options.into_iter().find(|o| o.key == "generators").unwrap().tooltip
    };
    let example = tip.split("e.g.").nth(1).expect("tooltip example").trim().to_string();
    let k = example.chars().filter(|c| c.is_ascii_uppercase()).map(|c| (c as u8 - b'A') as usize + 1).max().unwrap();
    let r = mk(&proj(k, Role::Controlled), "fractional_factorial", json!({ "generators": example }));
    assert!(r.is_ok(), "tooltip example {example:?} with {k} factors: {}", describe(&r));
}
