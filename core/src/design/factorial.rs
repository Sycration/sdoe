//! Full factorial and regular two-level fractional factorial designs.

use super::{GenRun, Generated, Opts, center_runs, designed_non_mixture, level_grid, replicate, two_level, two_level_full};
use crate::model::terms::{Preset, preset_terms};
use crate::project::{Coded, Project};
use crate::rng::Rng;

pub fn full_factorial(p: &Project, o: &Opts) -> Result<Generated, String> {
    let fs = designed_non_mixture(p);
    if fs.is_empty() {
        return Err("no controlled or noise factors".into());
    }
    let nl = o.int("levels", 2).clamp(2, 7) as usize;
    let grids: Vec<Vec<Coded>> = fs.iter().map(|&f| level_grid(p, f, nl)).collect();
    let total: usize = grids.iter().map(|g| g.len()).product();
    if total > 100_000 {
        return Err(format!("{total} runs is too many for a full factorial; use a fraction or an optimal design"));
    }
    let nf = p.factors.len();
    let mut runs = Vec::with_capacity(total);
    for i in 0..total {
        let mut r = GenRun::new(nf, "factorial");
        let mut rem = i;
        for (j, &f) in fs.iter().enumerate() {
            let g = &grids[j];
            r.coded[f] = Some(g[rem % g.len()]);
            rem /= g.len();
        }
        runs.push(r);
    }
    let cp = o.int("center_points", 0).max(0) as usize;
    let has_cont = fs.iter().any(|&f| p.factors[f].is_numeric());
    let mut notes = vec![format!("{} level combinations.", total)];
    if cp > 0 && has_cont {
        runs.extend(center_runs(p, &fs, cp, nf));
    }
    let reps = o.int("replicates", 1).max(1) as usize;
    let blocks = o.bool("block_replicates", false) && reps > 1;
    let runs = replicate(runs, reps, blocks);
    let max_levels = grids.iter().map(|g| g.len()).max().unwrap_or(2);
    let preset = if max_levels >= 3 { Preset::Quadratic } else { Preset::Interactions };
    if reps > 1 {
        notes.push(format!("{reps} replicates provide a pure-error estimate."));
    }
    Ok(Generated { runs, notes, blocks: if blocks { reps } else { 1 }, model: preset_terms(p, preset, false), ..Default::default() })
}

/// A regular two-level fraction 2^(k-p).
#[derive(Clone, Debug)]
pub struct Fraction {
    pub k: usize,
    pub base: usize,
    /// Generator base masks and signs (true = negative).
    pub gens: Vec<(u64, bool)>,
    /// Words of the defining relation (bitmask over all k factors) with sign.
    pub words: Vec<(u64, bool)>,
    pub resolution: usize,
    pub wlp: Vec<usize>,
}

impl Fraction {
    pub fn new(k: usize, gens: Vec<(u64, bool)>) -> Fraction {
        let base = k - gens.len();
        let g: Vec<(u64, bool)> = gens.iter().enumerate().map(|(j, &(m, s))| (m | (1u64 << (base + j)), s)).collect();
        let mut words = Vec::new();
        for sel in 1u64..(1u64 << g.len()) {
            let mut w = 0u64;
            let mut neg = false;
            for (j, &(m, s)) in g.iter().enumerate() {
                if (sel >> j) & 1 == 1 {
                    w ^= m;
                    neg ^= s;
                }
            }
            words.push((w, neg));
        }
        let mut wlp = vec![0usize; k + 1];
        for (w, _) in &words {
            wlp[w.count_ones() as usize] += 1;
        }
        let resolution = words.iter().map(|(w, _)| w.count_ones() as usize).min().unwrap_or(usize::MAX);
        Fraction { k, base, gens, words, resolution, wlp }
    }

    pub fn full(k: usize) -> Fraction {
        Fraction { k, base: k, gens: vec![], words: vec![], resolution: usize::MAX, wlp: vec![0; k + 1] }
    }

    /// Design matrix in +/-1 units, base factors in standard order.
    pub fn matrix(&self) -> Vec<Vec<i8>> {
        two_level_full(self.base)
            .into_iter()
            .map(|mut row| {
                for &(m, neg) in &self.gens {
                    let mut v: i8 = if neg { -1 } else { 1 };
                    for b in 0..self.base {
                        if (m >> b) & 1 == 1 {
                            v *= row[b];
                        }
                    }
                    row.push(v);
                }
                row
            })
            .collect()
    }

    pub fn runs(&self) -> usize {
        1 << self.base
    }

    fn rank_key(&self) -> Vec<usize> {
        self.wlp.clone()
    }
}

pub fn roman(r: usize) -> String {
    match r {
        usize::MAX => "full".into(),
        1 => "I".into(),
        2 => "II".into(),
        3 => "III".into(),
        4 => "IV".into(),
        5 => "V".into(),
        6 => "VI".into(),
        7 => "VII".into(),
        8 => "VIII".into(),
        n => n.to_string(),
    }
}

fn binom(n: usize, k: usize) -> f64 {
    let mut r = 1.0;
    for i in 0..k {
        r *= (n - i) as f64 / (i + 1) as f64;
    }
    r
}

/// Minimum-aberration (or best found) 2^(k-p) fraction.
pub fn best_fraction(k: usize, p: usize) -> Option<Fraction> {
    if p == 0 {
        return Some(Fraction::full(k));
    }
    let n = k.checked_sub(p)?;
    if n < 2 || k > 63 {
        return None;
    }
    let cands: Vec<u64> = (1u64..(1u64 << n)).filter(|m| m.count_ones() >= 2).collect();
    if cands.len() < p {
        return None;
    }
    let mut best: Option<Fraction> = None;
    let consider = |f: Fraction, best: &mut Option<Fraction>| {
        if best.as_ref().is_none_or(|b| f.rank_key() < b.rank_key()) {
            *best = Some(f);
        }
    };
    if binom(cands.len(), p) <= 250_000.0 {
        let mut idx: Vec<usize> = (0..p).collect();
        loop {
            let gens = idx.iter().map(|&i| (cands[i], false)).collect();
            consider(Fraction::new(k, gens), &mut best);
            // Next combination.
            let mut i = p;
            loop {
                if i == 0 {
                    return best;
                }
                i -= 1;
                if idx[i] < cands.len() - p + i {
                    idx[i] += 1;
                    for j in i + 1..p {
                        idx[j] = idx[j - 1] + 1;
                    }
                    break;
                }
            }
        }
    }
    // Large spaces: random starts with greedy swap improvement.
    let mut rng = Rng::new(0x5D0E + (k * 64 + p) as u64);
    let mut sorted = cands.clone();
    sorted.sort_by_key(|m| std::cmp::Reverse(m.count_ones()));
    let mut starts: Vec<Vec<u64>> = vec![sorted[..p].to_vec()];
    for _ in 0..40 {
        let mut c = cands.clone();
        rng.shuffle(&mut c);
        starts.push(c[..p].to_vec());
    }
    for mut sel in starts {
        let mut cur = Fraction::new(k, sel.iter().map(|&m| (m, false)).collect());
        let mut improved = true;
        let mut guard = 0;
        while improved && guard < 50 {
            improved = false;
            guard += 1;
            for i in 0..p {
                for &c in &cands {
                    if sel.contains(&c) {
                        continue;
                    }
                    let old = sel[i];
                    sel[i] = c;
                    let f = Fraction::new(k, sel.iter().map(|&m| (m, false)).collect());
                    if f.rank_key() < cur.rank_key() {
                        cur = f;
                        improved = true;
                    } else {
                        sel[i] = old;
                    }
                }
            }
        }
        consider(cur, &mut best);
    }
    best
}

fn word_name(w: u64, syms: &[String]) -> String {
    let parts: Vec<&str> = (0..syms.len()).filter(|&b| (w >> b) & 1 == 1).map(|b| syms[b].as_str()).collect();
    if parts.iter().all(|s| s.chars().count() == 1) { parts.concat() } else { parts.join("*") }
}

/// Alias chains of main effects and two-factor interactions (aliases up to order 3).
pub fn alias_chains(f: &Fraction, syms: &[String]) -> Vec<String> {
    let k = f.k;
    let mut effects: Vec<u64> = (0..k).map(|i| 1u64 << i).collect();
    for i in 0..k {
        for j in i + 1..k {
            effects.push((1u64 << i) | (1u64 << j));
        }
    }
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for e in effects {
        if seen.contains(&e) {
            continue;
        }
        let mut chain: Vec<(u64, bool)> = vec![(e, false)];
        for &(w, neg) in &f.words {
            let a = e ^ w;
            if a.count_ones() <= 3 {
                chain.push((a, neg));
            }
        }
        if chain.len() == 1 {
            continue;
        }
        for (a, _) in &chain {
            seen.insert(*a);
        }
        let mut s = word_name(e, syms);
        for (a, neg) in &chain[1..] {
            s.push_str(if *neg { " - " } else { " + " });
            s.push_str(&word_name(*a, syms));
        }
        out.push(s);
    }
    out
}

/// Parses generators such as "D=ABC, E=-A*B*D" against factor symbols.
pub fn parse_generators(text: &str, syms: &[String]) -> Result<Vec<(usize, u64, bool)>, String> {
    let mut out = Vec::new();
    let find = |s: &str| syms.iter().position(|x| x.eq_ignore_ascii_case(s.trim()));
    for item in text.split([',', ';', '\n']).map(str::trim).filter(|s| !s.is_empty()) {
        let (lhs, rhs) = item.split_once('=').ok_or_else(|| format!("'{item}' is not of the form D=ABC"))?;
        let target = find(lhs).ok_or_else(|| format!("unknown factor '{}'", lhs.trim()))?;
        let mut r = rhs.trim().replace(' ', "");
        let mut neg = false;
        if let Some(rest) = r.strip_prefix('-') {
            neg = true;
            r = rest.to_string();
        } else if let Some(rest) = r.strip_prefix('+') {
            r = rest.to_string();
        }
        let mut mask = 0u64;
        let parts: Vec<String> = if r.contains('*') {
            r.split('*').map(String::from).collect()
        } else {
            // Greedy longest-symbol tokenisation of juxtaposed symbols.
            let mut v = Vec::new();
            let mut rest = r.as_str();
            let mut by_len: Vec<&String> = syms.iter().collect();
            by_len.sort_by_key(|s| std::cmp::Reverse(s.len()));
            while !rest.is_empty() {
                let m = by_len
                    .iter()
                    .find(|s| rest.len() >= s.len() && rest[..s.len()].eq_ignore_ascii_case(s))
                    .ok_or_else(|| format!("cannot read '{rest}' in generator '{item}'"))?;
                v.push(rest[..m.len()].to_string());
                rest = &rest[m.len()..];
            }
            v
        };
        for part in parts {
            let i = find(&part).ok_or_else(|| format!("unknown factor '{part}'"))?;
            mask ^= 1u64 << i;
        }
        if mask.count_ones() < 2 {
            return Err(format!("generator '{item}' needs an interaction of at least two factors"));
        }
        out.push((target, mask, neg));
    }
    Ok(out)
}

pub fn fractional(p: &Project, o: &Opts) -> Result<Generated, String> {
    let fs = designed_non_mixture(p);
    let k = fs.len();
    if k < 2 {
        return Err("fractional factorials need at least 2 factors".into());
    }
    if k > 40 {
        return Err("too many factors for a regular fraction; use Plackett-Burman or definitive screening".into());
    }
    for &f in &fs {
        two_level(p, f, true)?;
    }
    let syms: Vec<String> = fs.iter().map(|&f| p.factors[f].symbol.clone()).collect();
    let gen_text = o.str("generators", "");
    let (frac, order): (Fraction, Vec<usize>) = if !gen_text.trim().is_empty() {
        let gens = parse_generators(&gen_text, &syms)?;
        let targets: Vec<usize> = gens.iter().map(|g| g.0).collect();
        let base: Vec<usize> = (0..k).filter(|i| !targets.contains(i)).collect();
        // Re-expresses generator masks over base positions.
        let mut gm = Vec::new();
        for (t, mask, neg) in &gens {
            let mut m = 0u64;
            for b in 0..k {
                if (mask >> b) & 1 == 1 {
                    let pos = base.iter().position(|&x| x == b).ok_or_else(|| {
                        format!("generator for {} uses {}, which is itself generated", syms[*t], syms[b])
                    })?;
                    m |= 1 << pos;
                }
            }
            gm.push((m, *neg));
        }
        let mut order = base.clone();
        order.extend(&targets);
        (Fraction::new(k, gm), order)
    } else {
        let runs_choice = o.str("runs", "auto");
        let min_res = o.int("resolution", 4).clamp(3, 8) as usize;
        let frac = if runs_choice == "auto" {
            let mut found = None;
            let nmin = (usize::BITS - k.leading_zeros()) as usize; // 2^n > k
            for n in nmin.max(2)..=k {
                if let Some(f) = best_fraction(k, k - n) {
                    if f.resolution >= min_res {
                        found = Some(f);
                        break;
                    }
                }
            }
            found.unwrap_or_else(|| Fraction::full(k))
        } else {
            let runs: usize = runs_choice.parse().map_err(|_| "invalid run count".to_string())?;
            let n = runs.trailing_zeros() as usize;
            if 1usize << n != runs || n > k {
                return Err("the run count must be a power of two not above the full factorial".into());
            }
            if (1usize << n) <= k {
                return Err(format!("{runs} runs cannot estimate {k} main effects"));
            }
            best_fraction(k, k - n).ok_or("no fraction of that size exists")?
        };
        (frac, (0..k).collect())
    };
    let nf = p.factors.len();
    let mut runs = Vec::new();
    let add_matrix = |m: &Vec<Vec<i8>>, sign: i8, runs: &mut Vec<GenRun>| -> Result<(), String> {
        for row in m {
            let mut r = GenRun::new(nf, "factorial");
            for (col, &pos) in order.iter().enumerate() {
                r.coded[fs[pos]] = Some(two_level(p, fs[pos], row[col] * sign > 0)?);
            }
            runs.push(r);
        }
        Ok(())
    };
    let m = frac.matrix();
    add_matrix(&m, 1, &mut runs)?;
    let foldover = o.bool("foldover", false);
    if foldover {
        add_matrix(&m, -1, &mut runs)?;
    }
    let cp = o.int("center_points", 0).max(0) as usize;
    if cp > 0 && fs.iter().any(|&f| p.factors[f].is_numeric()) {
        runs.extend(center_runs(p, &fs, cp, nf));
    }
    let reps = o.int("replicates", 1).max(1) as usize;
    let runs = replicate(runs, reps, false);

    // Symbols in matrix-column order for the alias report.
    let osyms: Vec<String> = order.iter().map(|&i| syms[i].clone()).collect();
    let mut notes = Vec::new();
    let pfrac = frac.gens.len();
    if pfrac == 0 {
        notes.push(format!("Full 2^{k} factorial ({} runs).", frac.runs()));
    } else {
        notes.push(format!(
            "2^({k}-{pfrac}) fraction, {} runs, resolution {}.",
            frac.runs(),
            roman(frac.resolution)
        ));
        let gens: Vec<String> = frac
            .gens
            .iter()
            .enumerate()
            .map(|(j, &(m, neg))| {
                let target = &osyms[frac.base + j];
                format!("{target} = {}{}", if neg { "-" } else { "" }, word_name(m, &osyms))
            })
            .collect();
        notes.push(format!("Generators: {}", gens.join(", ")));
        let mut words: Vec<String> = frac
            .words
            .iter()
            .map(|&(w, neg)| format!("{}{}", if neg { "-" } else { "" }, word_name(w, &osyms)))
            .collect();
        words.sort_by_key(|w| w.trim_start_matches('-').len());
        if words.len() <= 31 {
            notes.push(format!("Defining relation: I = {}", words.join(" = ")));
        }
        if foldover {
            notes.push("The fold-over runs separate main effects from two-factor interactions.".into());
        } else {
            let chains = alias_chains(&frac, &osyms);
            if !chains.is_empty() {
                notes.push("Alias structure (up to three-factor interactions):".into());
                for c in chains.iter().take(60) {
                    notes.push(format!("  {c}"));
                }
            }
        }
    }
    let res = if foldover { frac.resolution.max(4) } else { frac.resolution };
    let preset = if res >= 5 { Preset::Interactions } else { Preset::Linear };
    Ok(Generated { runs, notes, blocks: 1, model: preset_terms(p, preset, false), ..Default::default() })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_resolutions() {
        let cases = [(4, 1, 4), (5, 1, 5), (5, 2, 3), (6, 2, 4), (7, 3, 4), (7, 4, 3), (8, 2, 5), (8, 4, 4), (10, 3, 5), (9, 4, 4)];
        for (k, p, r) in cases {
            let f = best_fraction(k, p).unwrap();
            assert_eq!(f.resolution, r, "2^({k}-{p})");
        }
    }

    #[test]
    fn fraction_is_balanced_and_orthogonal() {
        let f = best_fraction(7, 3).unwrap();
        let m = f.matrix();
        for i in 0..7 {
            assert_eq!(m.iter().map(|r| r[i] as i32).sum::<i32>(), 0);
            for j in i + 1..7 {
                assert_eq!(m.iter().map(|r| (r[i] * r[j]) as i32).sum::<i32>(), 0);
            }
        }
    }

    #[test]
    fn generator_parsing() {
        let syms: Vec<String> = ["A", "B", "C", "D"].iter().map(|s| s.to_string()).collect();
        let g = parse_generators("D=-ABC", &syms).unwrap();
        assert_eq!(g, vec![(3, 0b0111, true)]);
    }
}
