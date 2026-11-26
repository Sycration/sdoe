//! Mixture designs: simplex lattice and simplex centroid in pseudo-components,
//! optionally crossed with a factorial in process factors.

use super::{GenRun, Generated, Opts, designed, designed_non_mixture, level_grid, two_level_full};
use crate::model::terms::{Preset, preset_terms};
use crate::project::{Coded, FactorKind, Project};

fn mixture_factors(p: &Project) -> Result<Vec<usize>, String> {
    let mix: Vec<usize> = designed(p).into_iter().filter(|&f| p.factors[f].is_mixture()).collect();
    if mix.len() < 2 {
        return Err("mixture designs need at least 2 mixture components".into());
    }
    let mc = p.mixture_coding();
    if mc.lower_sum >= mc.total - 1e-12 {
        return Err("the lower bounds of the mixture components add up to the mixture total or more".into());
    }
    Ok(mix)
}

fn finish(p: &Project, mix: &[usize], blends: Vec<(Vec<f64>, &'static str)>, reps: usize, preset: Preset, mut notes: Vec<String>) -> Generated {
    let mc = p.mixture_coding();
    let mut kept = Vec::new();
    let mut dropped = 0;
    for (z, kind) in blends {
        let ok = mix.iter().zip(&z).all(|(&f, &v)| match p.factors[f].kind {
            FactorKind::Mixture { low, high } => low + v * mc.span() <= high + 1e-9,
            _ => true,
        });
        if ok {
            kept.push((z, kind));
        } else {
            dropped += 1;
        }
    }
    if dropped > 0 {
        notes.push(format!(
            "{dropped} blends exceed component upper bounds and were left out; an optimal design covers constrained regions better."
        ));
    }
    // Crossing with process factors.
    let proc = designed_non_mixture(p);
    let proc_runs: Vec<Vec<(usize, Coded)>> = if proc.is_empty() {
        vec![vec![]]
    } else {
        let all_two = proc.iter().all(|&f| !p.factors[f].is_categorical());
        if all_two {
            two_level_full(proc.len())
                .into_iter()
                .map(|row| proc.iter().zip(row).map(|(&f, s)| (f, Coded::Num(s as f64))).collect())
                .collect()
        } else {
            let mut combos: Vec<Vec<(usize, Coded)>> = vec![vec![]];
            for &f in &proc {
                let g = level_grid(p, f, 2);
                combos = combos
                    .into_iter()
                    .flat_map(|c| {
                        g.iter().map(move |&v| {
                            let mut w = c.clone();
                            w.push((f, v));
                            w
                        })
                    })
                    .collect();
            }
            combos
        }
    };
    if !proc.is_empty() {
        notes.push(format!("Crossed with {} process-factor combinations.", proc_runs.len()));
    }
    let nf = p.factors.len();
    let mut runs = Vec::new();
    for _ in 0..reps.max(1) {
        for pr in &proc_runs {
            for (z, kind) in &kept {
                let mut r = GenRun::new(nf, kind);
                for (&f, &v) in mix.iter().zip(z) {
                    r.coded[f] = Some(Coded::Num(v));
                }
                for &(f, v) in pr {
                    r.coded[f] = Some(v);
                }
                runs.push(r);
            }
        }
    }
    if mc.lower_sum > 0.0 {
        notes.push("Blends are laid out in pseudo-components because of the lower bounds.".into());
    }
    Generated { runs, notes, blocks: 1, model: preset_terms(p, preset, false), ..Default::default() }
}

pub fn lattice(p: &Project, o: &Opts) -> Result<Generated, String> {
    let mix = mixture_factors(p)?;
    let q = mix.len();
    let m = o.int("degree", 2).clamp(1, 6) as usize;
    let mut blends = Vec::new();
    let mut cur = vec![0usize; q];
    fn rec(i: usize, left: usize, q: usize, m: usize, cur: &mut Vec<usize>, out: &mut Vec<(Vec<f64>, &'static str)>) {
        if i == q - 1 {
            cur[i] = left;
            let z: Vec<f64> = cur.iter().map(|&c| c as f64 / m as f64).collect();
            let nz = cur.iter().filter(|&&c| c > 0).count();
            out.push((z, if nz == 1 { "vertex" } else if nz == 2 { "edge" } else { "interior" }));
            return;
        }
        for v in (0..=left).rev() {
            cur[i] = v;
            rec(i + 1, left - v, q, m, cur, out);
        }
    }
    rec(0, m, q, m, &mut cur, &mut blends);
    if o.bool("centroid", true) && !(m % q == 0 && m / q >= 1) {
        blends.push((vec![1.0 / q as f64; q], "centroid"));
    }
    if o.bool("axial", false) {
        for i in 0..q {
            blends.push((
                (0..q).map(|j| if i == j { (1.0 + q as f64) / (2.0 * q as f64) } else { 1.0 / (2.0 * q as f64) }).collect(),
                "axial",
            ));
        }
    }
    let preset = match m {
        1 => Preset::Linear,
        2 => Preset::Interactions,
        _ => Preset::Cubic,
    };
    let notes = vec![format!("{{{q}, {m}}} simplex lattice.")];
    Ok(finish(p, &mix, blends, o.int("replicates", 1).max(1) as usize, preset, notes))
}

pub fn centroid(p: &Project, o: &Opts) -> Result<Generated, String> {
    let mix = mixture_factors(p)?;
    let q = mix.len();
    if q > 12 {
        return Err("too many components for a simplex centroid design".into());
    }
    let mut blends: Vec<(Vec<f64>, &'static str)> = Vec::new();
    for s in 1u64..(1u64 << q) {
        let c = s.count_ones() as f64;
        let kind = match s.count_ones() {
            1 => "vertex",
            2 => "edge",
            n if n as usize == q => "centroid",
            _ => "interior",
        };
        blends.push(((0..q).map(|i| if (s >> i) & 1 == 1 { 1.0 / c } else { 0.0 }).collect::<Vec<f64>>(), kind));
    }
    blends.sort_by_key(|(z, _)| z.iter().filter(|v| **v > 0.0).count());
    if o.bool("axial", false) {
        for i in 0..q {
            blends.push((
                (0..q).map(|j| if i == j { (1.0 + q as f64) / (2.0 * q as f64) } else { 1.0 / (2.0 * q as f64) }).collect(),
                "axial",
            ));
        }
    }
    let notes = vec![format!("Simplex centroid design for {q} components ({} blends).", (1usize << q) - 1)];
    Ok(finish(p, &mix, blends, o.int("replicates", 1).max(1) as usize, if q >= 3 { Preset::Cubic } else { Preset::Interactions }, notes))
}
