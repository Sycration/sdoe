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
    let total = grids.iter().try_fold(1usize, |acc, g| acc.checked_mul(g.len())).filter(|&t| t <= 100_000);
    let Some(total) = total else {
        return Err("too many runs for a full factorial (more than 100000); use a fraction or an optimal design".into());
    };
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
    /// With more than `FULL_WORDS_MAX_GENS` generators only the words up to
    /// length `SHORT_WORD_MAX` are listed (see `complete`).
    pub words: Vec<(u64, bool)>,
    /// True when `words` holds the whole defining relation.
    pub complete: bool,
    /// Resolution; with an incomplete word list, `SHORT_WORD_MAX + 1` stands
    /// for "above SHORT_WORD_MAX".
    pub resolution: usize,
    /// Word length pattern (counts by length; exact up to `SHORT_WORD_MAX`
    /// when the word list is incomplete).
    pub wlp: Vec<usize>,
}

/// Up to this many generators the defining relation is enumerated in full
/// (2^p words).
const FULL_WORDS_MAX_GENS: usize = 16;
/// Longest word listed for larger fractions; alias chains of effects up to
/// two-factor interactions with aliases up to three factors need words of up
/// to 5 letters.
const SHORT_WORD_MAX: usize = 6;

/// Column masks over the base factors: base factor i is bit i, generated
/// factor j has its generator mask.
fn column_masks(base: usize, gens: &[(u64, bool)]) -> Vec<u64> {
    (0..base).map(|i| 1u64 << i).chain(gens.iter().map(|g| g.0)).collect()
}

/// Words of up to `SHORT_WORD_MAX` letters: sets of columns whose masks XOR
/// to zero, found as single subsets of up to 3 columns or as unions of two
/// disjoint such subsets with equal XOR. Bit positions follow `Fraction`
/// (base factors first, then generated factors); the sign is the product of
/// the generator signs.
fn short_words(base: usize, gens: &[(u64, bool)]) -> Vec<(u64, bool)> {
    let masks = column_masks(base, gens);
    let k = masks.len();
    let sign = |set: u64| (base..k).filter(|&j| (set >> j) & 1 == 1).fold(false, |n, j| n ^ gens[j - base].1);
    let mut by_xor: std::collections::HashMap<u64, Vec<u64>> = std::collections::HashMap::new();
    let mut found = std::collections::BTreeSet::new();
    let mut add = |set: u64, x: u64, by_xor: &mut std::collections::HashMap<u64, Vec<u64>>| {
        if x == 0 {
            found.insert(set);
        }
        by_xor.entry(x).or_default().push(set);
    };
    for a in 0..k {
        add(1 << a, masks[a], &mut by_xor);
        for b in a + 1..k {
            add((1 << a) | (1 << b), masks[a] ^ masks[b], &mut by_xor);
            for c in b + 1..k {
                add((1 << a) | (1 << b) | (1 << c), masks[a] ^ masks[b] ^ masks[c], &mut by_xor);
            }
        }
    }
    for sets in by_xor.values() {
        for i in 0..sets.len() {
            for j in i + 1..sets.len() {
                if sets[i] & sets[j] == 0 {
                    found.insert(sets[i] | sets[j]);
                }
            }
        }
    }
    found.into_iter().filter(|w| w.count_ones() as usize <= SHORT_WORD_MAX).map(|w| (w, sign(w))).collect()
}

impl Fraction {
    pub fn new(k: usize, gens: Vec<(u64, bool)>) -> Fraction {
        let base = k - gens.len();
        let complete = gens.len() <= FULL_WORDS_MAX_GENS;
        let words = if complete {
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
            words
        } else {
            short_words(base, &gens)
        };
        let mut wlp = vec![0usize; k + 1];
        for (w, _) in &words {
            wlp[w.count_ones() as usize] += 1;
        }
        let shortest = words.iter().map(|(w, _)| w.count_ones() as usize).min();
        let resolution = match shortest {
            Some(r) => r,
            None if complete => usize::MAX,
            None => SHORT_WORD_MAX + 1,
        };
        Fraction { k, base, gens, words, complete, resolution, wlp }
    }

    pub fn full(k: usize) -> Fraction {
        Fraction { k, base: k, gens: vec![], words: vec![], complete: true, resolution: usize::MAX, wlp: vec![0; k + 1] }
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
    if p == 1 {
        // A half fraction is best with the generator that uses every base factor.
        return Some(Fraction::new(k, vec![((1u64 << n) - 1, false)]));
    }
    if n > 20 {
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
    // Large spaces: a greedy start and random starts with swap improvement,
    // ranked by the short-word counts, within a fixed evaluation budget.
    let mut counter = WordCounter::new(n);
    let key = |counter: &mut WordCounter, sel: &[u64]| {
        let masks: Vec<u64> = (0..n).map(|i| 1u64 << i).chain(sel.iter().cloned()).collect();
        counter.key(&masks)
    };
    const BUDGET: usize = 150_000;
    let per_pass = p * cands.len();
    let mut evals = 0usize;
    // Greedy: each generator in turn takes the candidate with the best key.
    let mut greedy: Vec<u64> = Vec::new();
    for _ in 0..p {
        let mut best_c: Option<(Vec<u64>, u64)> = None;
        for &c in &cands {
            if greedy.contains(&c) {
                continue;
            }
            greedy.push(c);
            let kk = key(&mut counter, &greedy);
            greedy.pop();
            if best_c.as_ref().is_none_or(|b| kk < b.0) {
                best_c = Some((kk, c));
            }
        }
        evals += cands.len();
        greedy.push(best_c?.1);
    }
    let mut rng = Rng::new(0x5D0E + (k * 64 + p) as u64);
    let mut starts: Vec<Vec<u64>> = vec![greedy];
    // Odd-weight columns: three masks of odd weight never XOR to zero, so
    // a selection of odd-weight generators (heaviest first) has no words of
    // length 3 (resolution IV for up to 2^(n-1) factors).
    let mut odd: Vec<u64> = cands.iter().cloned().filter(|m| m.count_ones() % 2 == 1).collect();
    if odd.len() >= p {
        odd.sort_by_key(|m| std::cmp::Reverse(m.count_ones()));
        starts.push(odd[..p].to_vec());
    }
    let n_random = (BUDGET / (4 * per_pass).max(1)).clamp(2, 40);
    for _ in 0..n_random {
        let mut c = cands.clone();
        rng.shuffle(&mut c);
        starts.push(c[..p].to_vec());
    }
    if let Some(sel) = no_short_word_columns(n, k, &mut rng) {
        starts.insert(0, sel);
    }
    let mut best_sel: Option<(Vec<u64>, Vec<u64>)> = None;
    for mut sel in starts {
        if evals > BUDGET && best_sel.is_some() {
            break;
        }
        let mut cur = key(&mut counter, &sel);
        let mut improved = true;
        while improved && evals <= BUDGET {
            improved = false;
            for i in 0..p {
                for &c in &cands {
                    if sel.contains(&c) {
                        continue;
                    }
                    let old = sel[i];
                    sel[i] = c;
                    let kk = key(&mut counter, &sel);
                    if kk < cur {
                        cur = kk;
                        improved = true;
                    } else {
                        sel[i] = old;
                    }
                }
            }
            evals += per_pass;
        }
        if best_sel.as_ref().is_none_or(|b| cur < b.0) {
            best_sel = Some((cur, sel));
        }
    }
    best_sel.map(|(_, sel)| Fraction::new(k, sel.into_iter().map(|m| (m, false)).collect()))
}

/// Generator masks of a 2^(k-p) fraction (n = k - p base factors) without
/// words of length 4 or less (resolution V or more), when random greedy
/// passes find one: each pass adds vectors of GF(2)^n in random order that
/// differ from every XOR of 1 to 3 vectors taken so far. Of the first k
/// vectors, n independent ones become the base factors and the others,
/// written in that basis, the generators (a change of basis keeps the XOR
/// relations, so the words).
fn no_short_word_columns(n: usize, k: usize, rng: &mut Rng) -> Option<Vec<u64>> {
    if n > 16 || k <= n {
        return None;
    }
    let size = 1usize << n;
    let mut order: Vec<u64> = (1..size as u64).collect();
    let mut forbidden = vec![false; size];
    for _ in 0..3000 {
        rng.shuffle(&mut order);
        forbidden.iter_mut().for_each(|f| *f = false);
        let mut cols: Vec<u64> = Vec::new();
        let mut sums2: Vec<u64> = Vec::new();
        for &c in &order {
            if forbidden[c as usize] {
                continue;
            }
            // New XORs of up to 3 vectors that contain c.
            let mut add: Vec<u64> = vec![c];
            add.extend(cols.iter().map(|&x| c ^ x));
            add.extend(sums2.iter().map(|&x| c ^ x));
            sums2.extend(cols.iter().map(|&x| c ^ x));
            for v in add {
                forbidden[v as usize] = true;
            }
            cols.push(c);
            if cols.len() == k {
                break;
            }
        }
        if cols.len() < k {
            continue;
        }
        // Pick n independent vectors (Gaussian elimination, with the
        // combination of picked vectors behind each reduced row).
        let mut basis: Vec<(u64, u64)> = Vec::new(); // (reduced vector, combination of picked indices)
        let mut picked: Vec<usize> = Vec::new();
        for (i, &c) in cols.iter().enumerate() {
            let (mut v, mut comb) = (c, 0u64);
            for &(b, bc) in &basis {
                if v ^ b < v {
                    v ^= b;
                    comb ^= bc;
                }
            }
            if v != 0 {
                basis.push((v, comb | (1u64 << picked.len())));
                basis.sort_by(|a, b| b.0.cmp(&a.0));
                picked.push(i);
            }
        }
        if picked.len() < n {
            continue;
        }
        // Coordinates of the other vectors over the picked ones.
        let gens: Vec<u64> = (0..k)
            .filter(|i| !picked.contains(i))
            .map(|i| {
                let (mut v, mut comb) = (cols[i], 0u64);
                for &(b, bc) in &basis {
                    if v ^ b < v {
                        v ^= b;
                        comb ^= bc;
                    }
                }
                comb
            })
            .collect();
        return Some(gens);
    }
    None
}

/// Largest number of factors of a regular 2^(k-p) fraction with `n` base
/// factors (2^n runs) and at least resolution `res`, where known: III takes
/// up to 2^n - 1, IV up to 2^(n-1), V the tabulated maxima. `None` when no
/// bound is used.
pub fn max_factors(n: usize, res: usize) -> Option<usize> {
    const MAX_V: [usize; 12] = [0, 0, 0, 0, 5, 6, 8, 11, 17, 23, 33, 47];
    if n >= 63 {
        return None;
    }
    match res {
        0..=3 => Some((1usize << n) - 1),
        4 => Some(1usize << (n - 1).min(62)),
        5 if n < MAX_V.len() => Some(if n < 4 { n } else { MAX_V[n] }),
        _ => None,
    }
}

/// Word-length counts A2 to A6 of a regular fraction from its column masks
/// over the base factors, via XOR histograms of column pairs and triples.
/// Each count is exact when no shorter word exists; otherwise the shorter
/// words dominate the ranking. The raw sums are scaled per length (A3 x3,
/// A4 x3, A5 x10, A6 x10), which keeps the lexicographic order.
struct WordCounter {
    ones: Vec<u32>,
    pairs: Vec<u32>,
    triples: Vec<u32>,
    touched: Vec<usize>,
}

impl WordCounter {
    fn new(n: usize) -> WordCounter {
        let size = 1usize << n;
        WordCounter { ones: vec![0; size], pairs: vec![0; size], triples: vec![0; size], touched: Vec::new() }
    }

    fn key(&mut self, masks: &[u64]) -> Vec<u64> {
        let k = masks.len();
        let m: Vec<usize> = masks.iter().map(|&x| x as usize).collect();
        for &x in &m {
            self.ones[x] += 1;
            self.touched.push(x);
        }
        let (mut a2, mut a3) = (0u64, 0u64);
        for &x in &m {
            a2 += (self.ones[x] - 1) as u64;
        }
        for a in 0..k {
            for b in a + 1..k {
                let x = m[a] ^ m[b];
                a3 += self.ones[x] as u64;
                self.pairs[x] += 1;
                self.touched.push(x);
            }
        }
        let mut a5 = 0u64;
        let mut a6 = 0u64;
        for a in 0..k {
            for b in a + 1..k {
                let ab = m[a] ^ m[b];
                for c in b + 1..k {
                    let x = ab ^ m[c];
                    a5 += self.pairs[x] as u64;
                    a6 += self.triples[x] as u64;
                    self.triples[x] += 1;
                    self.touched.push(x);
                }
            }
        }
        let mut a4 = 0u64;
        for &x in &self.touched {
            let c = self.pairs[x] as u64;
            a4 += c * c.saturating_sub(1) / 2;
            self.pairs[x] = 0;
        }
        for &x in &self.touched {
            self.ones[x] = 0;
            self.triples[x] = 0;
        }
        self.touched.clear();
        vec![a2 / 2, a3, a4, a5, a6]
    }
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

/// Rewrites generator words that use generated factors in terms of the base
/// factors only, so that "D=ABC, E=ABD" is read as D=ABC, E=C.
fn resolve_generators(mut gens: Vec<(usize, u64, bool)>, syms: &[String]) -> Result<Vec<(usize, u64, bool)>, String> {
    for g in &gens {
        if (g.1 >> g.0) & 1 == 1 {
            return Err(format!("the generator for {} uses {} itself", syms[g.0], syms[g.0]));
        }
    }
    for _ in 0..=gens.len() {
        let mut changed = false;
        for i in 0..gens.len() {
            for j in 0..gens.len() {
                if i != j && (gens[i].1 >> gens[j].0) & 1 == 1 {
                    gens[i].1 ^= (1u64 << gens[j].0) ^ gens[j].1;
                    gens[i].2 ^= gens[j].2;
                    changed = true;
                }
            }
        }
        if !changed {
            return Ok(gens);
        }
    }
    Err("the generators refer to each other in a circle".into())
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
        let mut targets: Vec<usize> = gens.iter().map(|g| g.0).collect();
        targets.sort_unstable();
        if let Some(w) = targets.windows(2).find(|w| w[0] == w[1]) {
            return Err(format!("{} is the target of more than one generator", syms[w[0]]));
        }
        let gens = resolve_generators(gens, &syms)?;
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
        let frac = Fraction::new(k, gm);
        if frac.resolution < 3 {
            return Err("these generators make two factors, or a factor and an interaction of two others, fully aliased (resolution below III); change the generators".into());
        }
        (frac, order)
    } else {
        let runs_choice = o.str("runs", "auto");
        let min_res = o.int("resolution", 4).clamp(3, 8) as usize;
        let frac = if runs_choice == "auto" {
            let mut found = None;
            let nmin = (usize::BITS - k.leading_zeros()) as usize; // 2^n > k
            // Run sizes up to 2^16; sizes below the known bounds for the
            // resolution are skipped.
            for n in nmin.max(2)..=k.min(16) {
                if max_factors(n, min_res).is_some_and(|m| k > m) {
                    continue;
                }
                if let Some(f) = best_fraction(k, k - n) {
                    if f.resolution >= min_res {
                        found = Some(f);
                        break;
                    }
                }
            }
            match found {
                Some(f) => f,
                None if k <= 16 => Fraction::full(k),
                None => {
                    return Err(format!(
                        "no regular fraction of resolution {} for {k} factors was found within 65536 runs; lower the resolution or use Plackett-Burman or definitive screening",
                        roman(min_res)
                    ));
                }
            }
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
        if frac.resolution >= 4 {
            return Err(format!(
                "folding over all factors of a resolution {} design repeats the fraction and adds no information; use a smaller fraction (resolution III) or an optimal design",
                roman(frac.resolution)
            ));
        }
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
        if frac.complete && words.len() <= 31 {
            notes.push(format!("Defining relation: I = {}", words.join(" = ")));
        } else if !frac.complete && !words.is_empty() && words.len() <= 31 {
            notes.push(format!("Shortest words of the defining relation: I = {} = …", words.join(" = ")));
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
