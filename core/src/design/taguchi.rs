//! Orthogonal arrays and Taguchi crossed (inner x outer) robust designs.

use super::screening::pb_matrix;
use super::{GenRun, Generated, Opts, level_grid};
use crate::model::terms::{Preset, preset_terms};
use crate::project::{Coded, FactorKind, Project, Role};

/// An orthogonal array: rows of level indices and per-column level counts.
#[derive(Clone, Debug)]
pub struct OArray {
    pub name: &'static str,
    pub rows: Vec<Vec<usize>>,
    pub levels: Vec<usize>,
}

fn gf_tables(s: usize) -> (Vec<Vec<usize>>, Vec<Vec<usize>>) {
    if s == 4 {
        let add = (0..4).map(|a| (0..4).map(|b| a ^ b).collect()).collect();
        let mul = vec![vec![0, 0, 0, 0], vec![0, 1, 2, 3], vec![0, 2, 3, 1], vec![0, 3, 1, 2]];
        (add, mul)
    } else {
        let add = (0..s).map(|a| (0..s).map(|b| (a + b) % s).collect()).collect();
        let mul = (0..s).map(|a| (0..s).map(|b| (a * b) % s).collect()).collect();
        (add, mul)
    }
}

/// Linear orthogonal array over GF(s) with `n` base columns:
/// s^n runs and (s^n - 1)/(s - 1) columns.
pub fn gf_array(name: &'static str, s: usize, n: usize) -> OArray {
    let (add, mul) = gf_tables(s);
    let total = s.pow(n as u32);
    let vecs: Vec<Vec<usize>> = (0..total)
        .map(|i| {
            let mut v = vec![0; n];
            let mut r = i;
            for k in (0..n).rev() {
                v[k] = r % s;
                r /= s;
            }
            v
        })
        .collect();
    let mut cols: Vec<Vec<usize>> = vecs
        .iter()
        .filter(|c| c.iter().find(|&&x| x != 0) == Some(&1))
        .cloned()
        .collect();
    // Base (unit) columns first, then higher weights first: assigning factors
    // in this order leaves main effects clear of low-order interactions.
    cols.sort_by_key(|c| {
        let w = c.iter().filter(|&&x| x != 0).count();
        (if w == 1 { 0 } else { 1 }, std::cmp::Reverse(w), c.clone())
    });
    let rows = vecs
        .iter()
        .map(|u| {
            cols.iter()
                .map(|c| {
                    let mut acc = 0;
                    for k in 0..n {
                        acc = add[acc][mul[u[k]][c[k]]];
                    }
                    acc
                })
                .collect()
        })
        .collect();
    let ncols = cols.len();
    OArray { name, rows, levels: vec![s; ncols] }
}

pub fn l18() -> OArray {
    const T: [[usize; 8]; 18] = [
        [1, 1, 1, 1, 1, 1, 1, 1],
        [1, 1, 2, 2, 2, 2, 2, 2],
        [1, 1, 3, 3, 3, 3, 3, 3],
        [1, 2, 1, 1, 2, 2, 3, 3],
        [1, 2, 2, 2, 3, 3, 1, 1],
        [1, 2, 3, 3, 1, 1, 2, 2],
        [1, 3, 1, 2, 1, 3, 2, 3],
        [1, 3, 2, 3, 2, 1, 3, 1],
        [1, 3, 3, 1, 3, 2, 1, 2],
        [2, 1, 1, 3, 3, 2, 2, 1],
        [2, 1, 2, 1, 1, 3, 3, 2],
        [2, 1, 3, 2, 2, 1, 1, 3],
        [2, 2, 1, 2, 3, 1, 3, 2],
        [2, 2, 2, 3, 1, 2, 1, 3],
        [2, 2, 3, 1, 2, 3, 2, 1],
        [2, 3, 1, 3, 2, 3, 1, 2],
        [2, 3, 2, 1, 3, 1, 2, 3],
        [2, 3, 3, 2, 1, 2, 3, 1],
    ];
    OArray {
        name: "L18",
        rows: T.iter().map(|r| r.iter().map(|v| v - 1).collect()).collect(),
        levels: vec![2, 3, 3, 3, 3, 3, 3, 3],
    }
}

pub fn l12() -> OArray {
    let m = pb_matrix(12).expect("PB12");
    OArray { name: "L12", rows: m.iter().map(|r| r.iter().map(|&v| if v > 0 { 1 } else { 0 }).collect()).collect(), levels: vec![2; 11] }
}

pub fn array_by_name(name: &str) -> Option<OArray> {
    Some(match name {
        "L4" => gf_array("L4", 2, 2),
        "L8" => gf_array("L8", 2, 3),
        "L9" => gf_array("L9", 3, 2),
        "L12" => l12(),
        "L16" => gf_array("L16", 2, 4),
        "L16b" => gf_array("L16b", 4, 2),
        "L18" => l18(),
        "L25" => gf_array("L25", 5, 2),
        "L27" => gf_array("L27", 3, 3),
        "L32" => gf_array("L32", 2, 5),
        _ => return None,
    })
}

const ORDER: [&str; 10] = ["L4", "L8", "L9", "L12", "L16", "L16b", "L18", "L25", "L27", "L32"];

/// Column assignment of factors (by required level count) to an array.
/// A two-level factor may use a three-level column with a dummy level.
fn assign(arr: &OArray, need: &[usize]) -> Option<(Vec<usize>, bool)> {
    let mut used = vec![false; arr.levels.len()];
    let mut out = vec![usize::MAX; need.len()];
    let mut dummy = false;
    // Larger level counts first.
    let mut order: Vec<usize> = (0..need.len()).collect();
    order.sort_by_key(|&i| std::cmp::Reverse(need[i]));
    for &i in &order {
        if let Some(c) = (0..arr.levels.len()).find(|&c| !used[c] && arr.levels[c] == need[i]) {
            used[c] = true;
            out[i] = c;
        } else if need[i] == 2 {
            let c = (0..arr.levels.len()).find(|&c| !used[c] && arr.levels[c] == 3)?;
            used[c] = true;
            out[i] = c;
            dummy = true;
        } else {
            return None;
        }
    }
    Some((out, dummy))
}

fn needed_levels(p: &Project, f: usize, cont_levels: usize) -> usize {
    match &p.factors[f].kind {
        FactorKind::Continuous { .. } => cont_levels,
        FactorKind::Discrete { levels } => levels.len(),
        FactorKind::Categorical { levels } => levels.len(),
        FactorKind::Mixture { .. } => 0,
    }
}

fn coded_level(p: &Project, f: usize, level: usize, nlev: usize, cont_levels: usize) -> Coded {
    let g = level_grid(p, f, cont_levels);
    // Dummy level: a 2-level factor in a 3-level column repeats its first level.
    let li = if level >= g.len() { level % nlev.min(g.len()).max(1) } else { level };
    g[li.min(g.len() - 1)]
}

/// Picks an array (by name or the smallest that fits).
fn choose(choice: &str, need: &[usize]) -> Result<(OArray, Vec<usize>, bool), String> {
    if choice != "auto" {
        let arr = array_by_name(choice).ok_or("unknown array")?;
        let (cols, dummy) = assign(&arr, need).ok_or_else(|| format!("{} cannot hold these factors", arr.name))?;
        return Ok((arr, cols, dummy));
    }
    let mut fallback = None;
    for name in ORDER {
        let arr = array_by_name(name).unwrap();
        if let Some((cols, dummy)) = assign(&arr, need) {
            if !dummy {
                return Ok((arr, cols, false));
            }
            if fallback.is_none() {
                fallback = Some((arr, cols, true));
            }
        }
    }
    fallback.ok_or_else(|| "no orthogonal array fits these factors; use an optimal design".into())
}

pub fn crossed(p: &Project, o: &Opts) -> Result<Generated, String> {
    let cont_levels: usize = o.str("levels", "2").parse().unwrap_or(2);
    let control: Vec<usize> = p
        .factors
        .iter()
        .enumerate()
        .filter(|(_, f)| f.role == Role::Controlled && !f.is_mixture())
        .map(|(i, _)| i)
        .collect();
    let noise: Vec<usize> = p
        .factors
        .iter()
        .enumerate()
        .filter(|(_, f)| f.role == Role::Noise && !f.is_mixture())
        .map(|(i, _)| i)
        .collect();
    if control.is_empty() {
        return Err("Taguchi designs need controlled factors".into());
    }
    let need: Vec<usize> = control.iter().map(|&f| needed_levels(p, f, cont_levels)).collect();
    let (inner, cols, dummy) = choose(&o.str("inner", "auto"), &need)?;
    let mut notes = vec![format!("Inner array {} ({} runs) for {} controlled factors.", inner.name, inner.rows.len(), control.len())];
    if dummy {
        notes.push("Some two-level factors use three-level columns with a repeated (dummy) level.".into());
    }
    // Outer design over noise factors.
    let outer_rows: Vec<Vec<Coded>> = if noise.is_empty() {
        vec![vec![]]
    } else {
        let nneed: Vec<usize> = noise.iter().map(|&f| needed_levels(p, f, cont_levels)).collect();
        let full: usize = nneed.iter().product();
        let mode = o.str("outer", "auto");
        if mode == "full" || (mode == "auto" && full <= 8) {
            let mut rows: Vec<Vec<Coded>> = vec![vec![]];
            for (j, &f) in noise.iter().enumerate() {
                let nl = nneed[j];
                let mut next = Vec::new();
                for r in &rows {
                    for l in 0..nl {
                        let mut w = r.clone();
                        w.push(coded_level(p, f, l, nl, cont_levels));
                        next.push(w);
                    }
                }
                rows = next;
            }
            notes.push(format!("Outer design: full factorial in {} noise factors ({} conditions).", noise.len(), rows.len()));
            rows
        } else {
            let (outer, ocols, _) = choose("auto", &nneed)?;
            notes.push(format!("Outer array {} ({} conditions) for {} noise factors.", outer.name, outer.rows.len(), noise.len()));
            outer
                .rows
                .iter()
                .map(|r| noise.iter().enumerate().map(|(j, &f)| coded_level(p, f, r[ocols[j]], nneed[j], cont_levels)).collect())
                .collect()
        }
    };
    let nf = p.factors.len();
    let mut runs = Vec::new();
    for (i, row) in inner.rows.iter().enumerate() {
        for (j, orow) in outer_rows.iter().enumerate() {
            let label = if noise.is_empty() { format!("I{}", i + 1) } else { format!("I{}·O{}", i + 1, j + 1) };
            let mut r = GenRun::new(nf, "inner");
            r.point_type = label;
            for (k, &f) in control.iter().enumerate() {
                r.coded[f] = Some(coded_level(p, f, row[cols[k]], need[k], cont_levels));
            }
            for (k, &f) in noise.iter().enumerate() {
                r.coded[f] = Some(orow[k]);
            }
            runs.push(r);
        }
    }
    if noise.is_empty() {
        notes.push("No noise factors are defined; replicate the inner runs to estimate variability for signal-to-noise analysis.".into());
    } else {
        notes.push("Analyse with signal-to-noise ratios (Analysis ▸ Robustness) or model control × noise interactions.".into());
    }
    let mut model = preset_terms(p, Preset::Linear, false);
    if !noise.is_empty() && runs.len() > 2 * (control.len() * noise.len() + control.len() + noise.len()) {
        for &c in &control {
            for &n in &noise {
                model.push(crate::model::terms::Term(vec![(c.min(n), 1), (c.max(n), 1)]));
            }
        }
        crate::model::terms::sort_terms(&mut model);
    }
    Ok(Generated { runs, notes, blocks: 1, model, ..Default::default() })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strength2(a: &OArray) {
        let n = a.rows.len();
        for i in 0..a.levels.len() {
            for j in i + 1..a.levels.len() {
                let mut counts = std::collections::HashMap::new();
                for r in &a.rows {
                    *counts.entry((r[i], r[j])).or_insert(0) += 1;
                }
                let expect = n / (a.levels[i] * a.levels[j]);
                assert_eq!(counts.len(), a.levels[i] * a.levels[j], "{} cols {i},{j}", a.name);
                assert!(counts.values().all(|&c| c == expect), "{} cols {i},{j}", a.name);
            }
        }
    }

    #[test]
    fn arrays_are_orthogonal() {
        for name in ORDER {
            strength2(&array_by_name(name).unwrap());
        }
        assert_eq!(array_by_name("L27").unwrap().levels.len(), 13);
        assert_eq!(array_by_name("L16b").unwrap().levels.len(), 5);
    }
}
