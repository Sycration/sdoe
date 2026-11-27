//! Space-filling designs: maximin Latin hypercube with balanced discrete and
//! categorical columns.

use super::{GenRun, Generated, Opts, designed_non_mixture, level_grid};
use crate::model::terms::{Preset, preset_terms};
use crate::project::{Coded, FactorKind, Project};
use crate::rng::Rng;

pub fn latin_hypercube(p: &Project, o: &Opts) -> Result<Generated, String> {
    let fs = designed_non_mixture(p);
    if fs.is_empty() {
        return Err("no controlled or noise factors".into());
    }
    let n = o.int("runs", 10).clamp(2, 5000) as usize;
    let iters = o.int("iterations", 2000).clamp(0, 200_000) as usize;
    let mut rng = Rng::new(o.int("seed", 1).max(0) as u64);
    let k = fs.len();
    for &f in &fs {
        if !matches!(p.factors[f].kind, FactorKind::Continuous { .. }) && level_grid(p, f, 2).is_empty() {
            return Err(format!("factor {} has no levels", p.factors[f].name));
        }
    }
    // Column values in coded units (categorical: level index).
    let mut cols: Vec<Vec<f64>> = Vec::with_capacity(k);
    let mut is_cat = Vec::with_capacity(k);
    for &f in &fs {
        let mut perm: Vec<usize> = (0..n).collect();
        rng.shuffle(&mut perm);
        let col: Vec<f64> = match &p.factors[f].kind {
            FactorKind::Continuous { .. } => perm.iter().map(|&j| -1.0 + 2.0 * (j as f64 + 0.5) / n as f64).collect(),
            _ => {
                let g = level_grid(p, f, 2);
                perm.iter()
                    .map(|&j| {
                        let li = j * g.len() / n;
                        match g[li] {
                            Coded::Num(x) => x,
                            Coded::Level(l) => l as f64,
                        }
                    })
                    .collect()
            }
        };
        is_cat.push(p.factors[f].is_categorical());
        cols.push(col);
    }
    let dist2 = |a: usize, b: usize, cols: &Vec<Vec<f64>>| -> f64 {
        let mut s = 0.0;
        for (j, c) in cols.iter().enumerate() {
            s += if is_cat[j] {
                if c[a] != c[b] { 1.0 } else { 0.0 }
            } else {
                (c[a] - c[b]).powi(2)
            };
        }
        s.max(1e-12)
    };
    // phi_p criterion (p = 15) as a smooth surrogate for maximin distance.
    const PW: i32 = 15;
    let term = |d2: f64| d2.sqrt().powi(-PW);
    let mut d = vec![vec![0.0; n]; n];
    let mut phi = 0.0;
    for a in 0..n {
        for b in a + 1..n {
            let v = dist2(a, b, &cols);
            d[a][b] = v;
            d[b][a] = v;
            phi += term(v);
        }
    }
    let mut temp = phi * 0.05;
    for _ in 0..iters {
        let j = rng.below(k);
        let a = rng.below(n);
        let b = rng.below(n);
        if a == b || cols[j][a] == cols[j][b] {
            continue;
        }
        cols[j].swap(a, b);
        let mut new_phi = phi;
        let mut da = vec![0.0; n];
        let mut db = vec![0.0; n];
        for r in 0..n {
            if r == a || r == b {
                continue;
            }
            da[r] = dist2(a, r, &cols);
            db[r] = dist2(b, r, &cols);
            new_phi += term(da[r]) - term(d[a][r]) + term(db[r]) - term(d[b][r]);
        }
        let accept = new_phi < phi || rng.uniform() < (-(new_phi - phi) / temp.max(1e-300)).exp();
        if accept {
            for r in 0..n {
                if r == a || r == b {
                    continue;
                }
                d[a][r] = da[r];
                d[r][a] = da[r];
                d[b][r] = db[r];
                d[r][b] = db[r];
            }
            phi = new_phi;
        } else {
            cols[j].swap(a, b);
        }
        temp *= 0.997;
    }
    let mut min_d = f64::INFINITY;
    for a in 0..n {
        for b in a + 1..n {
            min_d = min_d.min(d[a][b].sqrt());
        }
    }
    let nf = p.factors.len();
    let mut runs = Vec::new();
    for i in 0..n {
        let mut r = GenRun::new(nf, "space-filling");
        for (j, &f) in fs.iter().enumerate() {
            r.coded[f] = Some(if is_cat[j] { Coded::Level(cols[j][i] as usize) } else { Coded::Num(cols[j][i]) });
        }
        runs.push(r);
    }
    let notes = vec![
        format!("Latin hypercube with {n} runs; continuous factors take {n} distinct levels."),
        format!("Minimum distance between runs: {} (coded units).", crate::format::num(min_d, 4)),
    ];
    let preset = if n >= 3 * (k + 1) * (k + 2) / 2 { Preset::Quadratic } else { Preset::Linear };
    Ok(Generated { runs, notes, blocks: 1, model: preset_terms(p, preset, false), ..Default::default() })
}
