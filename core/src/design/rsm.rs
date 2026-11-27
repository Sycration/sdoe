//! Response-surface designs: central composite and Box-Behnken.

use super::factorial::{Fraction, best_fraction, roman};
use super::{GenRun, Generated, Opts, designed_non_mixture, level_grid, snap_discrete};
use crate::model::terms::{Preset, preset_terms};
use crate::project::{Coded, FactorKind, Project};

/// Splits designed factors into numeric (response-surface) and categorical.
fn split(p: &Project) -> (Vec<usize>, Vec<usize>) {
    let fs = designed_non_mixture(p);
    let num = fs.iter().cloned().filter(|&f| p.factors[f].is_numeric()).collect();
    let cat = fs.iter().cloned().filter(|&f| p.factors[f].is_categorical()).collect();
    (num, cat)
}

/// All level combinations of categorical factors (empty combination if none).
fn cat_combos(p: &Project, cats: &[usize]) -> Vec<Vec<(usize, usize)>> {
    let mut combos: Vec<Vec<(usize, usize)>> = vec![vec![]];
    for &c in cats {
        let k = p.factors[c].level_count();
        combos = combos
            .into_iter()
            .flat_map(|v| {
                (0..k).map(move |l| {
                    let mut w = v.clone();
                    w.push((c, l));
                    w
                })
            })
            .collect();
    }
    combos
}

fn res_v_core(k: usize, full: bool) -> Fraction {
    if full {
        return Fraction::full(k);
    }
    for pfrac in (1..k).rev() {
        if super::factorial::max_factors(k - pfrac, 5).is_some_and(|m| k > m) {
            continue;
        }
        if let Some(f) = best_fraction(k, pfrac) {
            if f.resolution >= 5 {
                return f;
            }
        }
    }
    Fraction::full(k)
}

pub fn ccd(p: &Project, o: &Opts) -> Result<Generated, String> {
    let (num, cats) = split(p);
    let k = num.len();
    if k < 2 {
        return Err("central composite designs need at least 2 numeric factors".into());
    }
    let core = res_v_core(k, o.str("fraction", "auto") == "full");
    let fm = core.matrix();
    let nfact = fm.len() as f64;
    let n0 = o.int("center_points", 5).max(0) as usize;
    let blocks = o.str("blocks", "1") == "2";
    let alpha_kind = o.str("alpha", "rotatable");
    // Center points of the factorial and the axial block.
    let c_fact = if blocks { n0.div_ceil(2) } else { n0 };
    let c_axial = n0 - c_fact;
    // Orthogonal blocking: the blocks are orthogonal to the quadratic terms
    // when sum(x²) / (block size) is equal in both blocks:
    // nf / (nf + c_f) = 2α² / (2k + c_a).
    let alpha_blocked = (nfact * (2.0 * k as f64 + c_axial as f64) / (2.0 * (nfact + c_fact as f64))).sqrt();
    let mut blocked_note = None;
    let alpha = match alpha_kind.as_str() {
        "rotatable" | "orthogonal" if blocks => {
            blocked_note = Some(format!(
                "Axial distance chosen for orthogonal blocking (the {} value would be {}).",
                alpha_kind,
                crate::format::num(
                    if alpha_kind == "rotatable" {
                        nfact.powf(0.25)
                    } else {
                        let t = 2.0 * k as f64 + n0 as f64;
                        (((nfact + t).sqrt() - nfact.sqrt()).powi(2) * nfact / 4.0).powf(0.25)
                    },
                    4
                )
            ));
            alpha_blocked
        }
        "face" => 1.0,
        "spherical" => (k as f64).sqrt(),
        "practical" => (k as f64).powf(0.25),
        "orthogonal" => {
            let t = 2.0 * k as f64 + n0 as f64;
            (((nfact + t).sqrt() - nfact.sqrt()).powi(2) * nfact / 4.0).powf(0.25)
        }
        "rotatable" => nfact.powf(0.25),
        other => {
            let a = crate::format::parse_num(other).ok_or("invalid axial distance")?;
            if !(a > 0.0 && a.is_finite()) {
                return Err("the axial distance must be a positive number".into());
            }
            a
        }
    };
    if blocks && blocked_note.is_none() && (alpha - alpha_blocked).abs() > 1e-9 {
        blocked_note = Some(format!(
            "The blocks are not orthogonal to the quadratic terms with this axial distance; α = {} would make them orthogonal.",
            crate::format::num(alpha_blocked, 4)
        ));
    }
    let inscribed = o.bool("inscribed", false);
    let scale = if inscribed { 1.0 / alpha } else { 1.0 };
    let nf = p.factors.len();
    let combos = cat_combos(p, &cats);
    let mut runs = Vec::new();
    for combo in &combos {
        let set_cats = |r: &mut GenRun| {
            for &(c, l) in combo {
                r.coded[c] = Some(Coded::Level(l));
            }
        };
        for row in &fm {
            let mut r = GenRun::new(nf, "factorial");
            for (j, &f) in num.iter().enumerate() {
                r.coded[f] = Some(Coded::Num(row[j] as f64 * scale));
            }
            set_cats(&mut r);
            runs.push(r);
        }
        for _ in 0..c_fact {
            let mut r = GenRun::new(nf, "center");
            for &f in &num {
                r.coded[f] = Some(Coded::Num(0.0));
            }
            set_cats(&mut r);
            runs.push(r);
        }
        let blk = if blocks { 1 } else { 0 };
        for (j, _) in num.iter().enumerate() {
            for s in [-1.0, 1.0] {
                let mut r = GenRun::new(nf, "axial");
                for (jj, &f) in num.iter().enumerate() {
                    r.coded[f] = Some(Coded::Num(if jj == j { s * alpha * scale } else { 0.0 }));
                }
                r.block = blk;
                set_cats(&mut r);
                runs.push(r);
            }
        }
        if blocks {
            for _ in 0..c_axial {
                let mut r = GenRun::new(nf, "center");
                for &f in &num {
                    r.coded[f] = Some(Coded::Num(0.0));
                }
                r.block = 1;
                set_cats(&mut r);
                runs.push(r);
            }
        }
    }
    let snapped = snap_discrete(p, &mut runs);
    let mut notes = vec![format!(
        "Central composite design: {} factorial, {} axial and {} center points{}.",
        fm.len(),
        2 * k,
        n0,
        if combos.len() > 1 { format!(" per categorical combination ({} combinations)", combos.len()) } else { String::new() }
    )];
    if core.gens.is_empty() {
        notes.push("Factorial core: full 2-level factorial.".into());
    } else {
        notes.push(format!("Factorial core: resolution {} fraction.", roman(core.resolution)));
    }
    notes.push(format!("Axial distance α = {} (coded units){}.", crate::format::num(alpha, 4), if inscribed { ", inscribed" } else { "" }));
    if !inscribed && alpha > 1.0 + 1e-9 {
        let outside: Vec<String> = num
            .iter()
            .filter(|&&f| matches!(p.factors[f].kind, FactorKind::Continuous { .. }))
            .map(|&f| {
                let (c, h) = p.factors[f].center_half();
                format!(
                    "{} {}…{}",
                    p.factors[f].name,
                    crate::format::num(c - alpha * h, 5),
                    crate::format::num(c + alpha * h, 5)
                )
            })
            .collect();
        if !outside.is_empty() {
            notes.push(format!("Axial points extend beyond the factor ranges: {}.", outside.join(", ")));
        }
    }
    if snapped {
        notes.push("Discrete factors use their level nearest to each design value.".into());
    }
    if blocks {
        notes.push("Block 1 holds the factorial points, block 2 the axial points; center points are split between them.".into());
        notes.extend(blocked_note);
    }
    Ok(Generated { runs, notes, blocks: if blocks { 2 } else { 1 }, model: preset_terms(p, Preset::Quadratic, false), ..Default::default() })
}

pub fn box_behnken(p: &Project, o: &Opts) -> Result<Generated, String> {
    let (num, cats) = split(p);
    let k = num.len();
    if k < 3 {
        return Err("Box-Behnken designs need at least 3 numeric factors".into());
    }
    for &f in &num {
        if let FactorKind::Discrete { levels } = &p.factors[f].kind {
            if levels.len() < 3 {
                return Err(format!("{} needs at least 3 levels", p.factors[f].name));
            }
        }
    }
    // Blocks of factors varied together (incomplete block designs from Box & Behnken 1960).
    let groups: Vec<Vec<usize>> = match k {
        6 => vec![vec![0, 1, 3], vec![1, 2, 4], vec![2, 3, 5], vec![0, 3, 4], vec![1, 4, 5], vec![0, 2, 5]],
        7 => vec![
            vec![3, 4, 5],
            vec![0, 5, 6],
            vec![1, 4, 6],
            vec![0, 1, 3],
            vec![2, 3, 6],
            vec![0, 2, 4],
            vec![1, 2, 5],
        ],
        _ => {
            let mut g = Vec::new();
            for i in 0..k {
                for j in i + 1..k {
                    g.push(vec![i, j]);
                }
            }
            g
        }
    };
    let nf = p.factors.len();
    let combos = cat_combos(p, &cats);
    let n0 = o.int("center_points", 5).max(0) as usize;
    let mut runs = Vec::new();
    for combo in &combos {
        for g in &groups {
            let m = g.len();
            for s in 0..(1usize << m) {
                let mut r = GenRun::new(nf, "edge");
                for &f in &num {
                    r.coded[f] = Some(Coded::Num(0.0));
                }
                for (bi, &gi) in g.iter().enumerate() {
                    r.coded[num[gi]] = Some(Coded::Num(if (s >> bi) & 1 == 1 { 1.0 } else { -1.0 }));
                }
                for &(c, l) in combo {
                    r.coded[c] = Some(Coded::Level(l));
                }
                runs.push(r);
            }
        }
        for _ in 0..n0 {
            let mut r = GenRun::new(nf, "center");
            for &f in &num {
                r.coded[f] = Some(Coded::Num(0.0));
            }
            for &(c, l) in combo {
                r.coded[c] = Some(Coded::Level(l));
            }
            runs.push(r);
        }
    }
    // Discrete factors map 0 to their middle level.
    for &f in &num {
        if let FactorKind::Discrete { .. } = p.factors[f].kind {
            let g: Vec<f64> = level_grid(p, f, 0).iter().map(|c| c.num()).collect();
            for r in runs.iter_mut() {
                if let Some(Coded::Num(x)) = r.coded[f] {
                    r.coded[f] = Some(Coded::Num(crate::project::nearest(&g, x)));
                }
            }
        }
    }
    let notes = vec![format!(
        "Box-Behnken design: {} edge points and {} center points{}.",
        groups.iter().map(|g| 1usize << g.len()).sum::<usize>(),
        n0,
        if combos.len() > 1 { format!(" per categorical combination ({})", combos.len()) } else { String::new() }
    )];
    Ok(Generated { runs, notes, blocks: 1, model: preset_terms(p, Preset::Quadratic, false), ..Default::default() })
}
