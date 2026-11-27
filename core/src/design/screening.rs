//! Screening designs: Plackett-Burman (Hadamard matrices) and definitive
//! screening designs (conference matrices).

use super::{GenRun, Generated, Opts, center_runs, designed_non_mixture, snap_discrete, two_level};
use crate::model::terms::{Preset, preset_terms};
use crate::project::{Coded, FactorKind, Project};

fn is_prime(n: usize) -> bool {
    n >= 2 && (2..).take_while(|d| d * d <= n).all(|d| n % d != 0)
}

/// Quadratic character modulo prime q.
fn chi(a: i64, q: i64) -> i64 {
    let a = a.rem_euclid(q);
    if a == 0 {
        return 0;
    }
    let mut r = 1i64;
    let mut b = a;
    let mut e = (q - 1) / 2;
    while e > 0 {
        if e & 1 == 1 {
            r = r * b % q;
        }
        b = b * b % q;
        e >>= 1;
    }
    if r == 1 { 1 } else { -1 }
}

/// Paley conference matrix of order q + 1 for prime q. Symmetric when
/// q = 1 (mod 4), antisymmetric when q = 3 (mod 4).
pub fn conference_prime(q: usize) -> Option<Vec<Vec<i8>>> {
    if !is_prime(q) || q == 2 {
        return None;
    }
    let m = q + 1;
    let eps: i8 = if q % 4 == 1 { 1 } else { -1 };
    let mut c = vec![vec![0i8; m]; m];
    for j in 1..m {
        c[0][j] = 1;
        c[j][0] = eps;
    }
    for i in 0..q {
        for j in 0..q {
            c[i + 1][j + 1] = chi(j as i64 - i as i64, q as i64) as i8;
        }
    }
    Some(c)
}

pub fn conference(m: usize) -> Option<Vec<Vec<i8>>> {
    if m == 2 {
        return Some(vec![vec![0, 1], vec![1, 0]]);
    }
    conference_prime(m.checked_sub(1)?)
}

/// Hadamard matrix of order n (Sylvester and Paley constructions).
pub fn hadamard(n: usize) -> Option<Vec<Vec<i8>>> {
    match n {
        1 => return Some(vec![vec![1]]),
        2 => return Some(vec![vec![1, 1], vec![1, -1]]),
        _ => {}
    }
    if n % 4 != 0 {
        return None;
    }
    // Paley I: q = n - 1 prime, q = 3 (mod 4).
    if is_prime(n - 1) && (n - 1) % 4 == 3 {
        let c = conference_prime(n - 1)?;
        let mut h = c.clone();
        for i in 0..n {
            h[i][i] = 1;
        }
        return Some(h);
    }
    // Sylvester doubling.
    if let Some(h) = hadamard(n / 2) {
        let m = n / 2;
        let mut out = vec![vec![0i8; n]; n];
        for i in 0..m {
            for j in 0..m {
                out[i][j] = h[i][j];
                out[i][j + m] = h[i][j];
                out[i + m][j] = h[i][j];
                out[i + m][j + m] = -h[i][j];
            }
        }
        return Some(out);
    }
    // Paley II: q = n/2 - 1 prime, q = 1 (mod 4).
    let q = n / 2 - 1;
    if is_prime(q) && q % 4 == 1 {
        let c = conference_prime(q)?;
        let m = q + 1;
        let mut out = vec![vec![0i8; n]; n];
        for i in 0..m {
            for j in 0..m {
                let blk: [[i8; 2]; 2] = match c[i][j] {
                    0 => [[1, -1], [-1, -1]],
                    1 => [[1, 1], [1, -1]],
                    _ => [[-1, -1], [-1, 1]],
                };
                for a in 0..2 {
                    for b in 0..2 {
                        out[2 * i + a][2 * j + b] = blk[a][b];
                    }
                }
            }
        }
        return Some(out);
    }
    None
}

/// Plackett-Burman design matrix: n runs, n - 1 columns.
pub fn pb_matrix(n: usize) -> Option<Vec<Vec<i8>>> {
    let mut h = hadamard(n)?;
    // Normalises the first column to +1.
    for row in h.iter_mut() {
        if row[0] < 0 {
            for v in row.iter_mut() {
                *v = -*v;
            }
        }
    }
    Some(h.into_iter().map(|r| r[1..].to_vec()).collect())
}

pub fn plackett_burman(p: &Project, o: &Opts) -> Result<Generated, String> {
    let fs = designed_non_mixture(p);
    let k = fs.len();
    if k < 2 {
        return Err("Plackett-Burman designs need at least 2 factors".into());
    }
    for &f in &fs {
        two_level(p, f, true)?;
    }
    let req = o.int("runs", 0).max(0) as usize;
    let mut n = if req > 0 { req } else { (k + 1).div_ceil(4) * 4 };
    if n < k + 1 {
        return Err(format!("{n} runs cannot hold {k} factors"));
    }
    let m = loop {
        if let Some(m) = pb_matrix(n) {
            break m;
        }
        if req > 0 {
            return Err(format!("no Plackett-Burman design with {req} runs is available"));
        }
        n += 4;
        if n > 1000 {
            return Err("no suitable Plackett-Burman design".into());
        }
    };
    let nf = p.factors.len();
    let mut runs = Vec::new();
    for row in &m {
        let mut r = GenRun::new(nf, "factorial");
        for (j, &f) in fs.iter().enumerate() {
            r.coded[f] = Some(two_level(p, f, row[j] > 0)?);
        }
        runs.push(r);
    }
    let cp = o.int("center_points", 0).max(0) as usize;
    if cp > 0 && fs.iter().any(|&f| p.factors[f].is_numeric()) {
        runs.extend(center_runs(p, &fs, cp, nf));
    }
    let mut notes = vec![format!("Plackett-Burman design with {n} runs for {k} factors.")];
    if n.is_power_of_two() {
        notes.push("Main effects are fully aliased with two-factor interactions (regular design, resolution III).".into());
    } else {
        notes.push("Main effects are partially aliased with two-factor interactions (non-regular design with complex aliasing).".into());
    }
    if n - 1 > k {
        notes.push(format!("{} unused columns estimate error for effect screening.", n - 1 - k));
    }
    Ok(Generated { runs, notes, blocks: 1, model: preset_terms(p, Preset::Linear, false), ..Default::default() })
}

pub fn definitive_screening(p: &Project, o: &Opts) -> Result<Generated, String> {
    let fs = designed_non_mixture(p);
    let k = fs.len();
    if k < 3 {
        return Err("definitive screening designs need at least 3 factors".into());
    }
    let mut m = if k % 2 == 0 { k } else { k + 1 };
    let c = loop {
        if let Some(c) = conference(m) {
            break c;
        }
        m += 2;
        if m > 200 {
            return Err("no conference matrix available".into());
        }
    };
    let nf = p.factors.len();
    let mut runs = Vec::new();
    for sign in [1i8, -1] {
        for (ri, row) in c.iter().enumerate() {
            let mut r = GenRun::new(nf, "screening");
            for (j, &f) in fs.iter().enumerate() {
                let v = row[j] * sign;
                r.coded[f] = Some(match &p.factors[f].kind {
                    FactorKind::Categorical { levels } => {
                        if levels.len() != 2 {
                            return Err(format!("{} needs exactly 2 levels for a definitive screening design", p.factors[f].name));
                        }
                        let hi = if v == 0 { (ri + if sign > 0 { 0 } else { 1 }) % 2 == 0 } else { v > 0 };
                        Coded::Level(hi as usize)
                    }
                    FactorKind::Mixture { .. } => return Err("mixture components are not supported".into()),
                    _ => Coded::Num(v as f64),
                });
            }
            runs.push(r);
        }
    }
    let extra = o.int("extra_center", 0).max(0) as usize;
    runs.extend(center_runs(p, &fs, 1 + extra, nf));
    let snapped = snap_discrete(p, &mut runs);
    let mut notes = vec![
        format!("Definitive screening design: {} runs for {k} factors (conference matrix of order {m}).", runs.len()),
        "Main effects are orthogonal to each other and to all two-factor interactions and quadratic effects.".into(),
        "Use stepwise selection to identify active quadratic and interaction terms.".into(),
    ];
    if fs.iter().any(|&f| p.factors[f].is_categorical()) {
        notes.push("Categorical factors take alternating levels where continuous factors are at their center.".into());
    }
    if snapped {
        notes.push("Discrete factors use their level nearest to the design value.".into());
    }
    Ok(Generated { runs, notes, blocks: 1, model: preset_terms(p, Preset::Linear, false), ..Default::default() })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check_hadamard(n: usize) {
        let h = hadamard(n).unwrap_or_else(|| panic!("no hadamard {n}"));
        for i in 0..n {
            for j in 0..n {
                let d: i32 = (0..n).map(|k| (h[i][k] * h[j][k]) as i32).sum();
                assert_eq!(d, if i == j { n as i32 } else { 0 }, "H{n} rows {i},{j}");
            }
        }
    }

    #[test]
    fn hadamard_orders() {
        for n in [4, 8, 12, 16, 20, 24, 28, 32, 36, 40, 44, 48, 60] {
            check_hadamard(n);
        }
    }

    #[test]
    fn conference_orders() {
        for m in [2, 4, 6, 8, 12, 14, 18, 20, 24, 30] {
            let c = conference(m).unwrap();
            for i in 0..m {
                for j in 0..m {
                    let d: i32 = (0..m).map(|k| (c[i][k] * c[j][k]) as i32).sum();
                    assert_eq!(d, if i == j { m as i32 - 1 } else { 0 }, "C{m}");
                }
            }
        }
    }
}
