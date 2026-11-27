//! Model terms, formula parsing/formatting, model presets and model matrices.

use crate::condition::{CodedCond, activity, coded_conditions};
use crate::project::{Coded, Factor, FactorKind, Project, Role, TermSpec};
use std::collections::BTreeSet;

/// A model term as a product of factor powers, keyed by factor index and
/// sorted by factor index. The empty product is the intercept.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Term(pub Vec<(usize, u32)>);

impl Term {
    pub fn single(f: usize, p: u32) -> Term {
        Term(vec![(f, p)])
    }

    pub fn degree(&self) -> u32 {
        self.0.iter().map(|(_, p)| p).sum()
    }

    pub fn max_power(&self) -> u32 {
        self.0.iter().map(|(_, p)| *p).max().unwrap_or(0)
    }

    pub fn is_intercept(&self) -> bool {
        self.0.is_empty()
    }

    pub fn power_of(&self, f: usize) -> u32 {
        self.0.iter().find(|(g, _)| *g == f).map(|(_, p)| *p).unwrap_or(0)
    }

    /// Product of two terms (powers of shared factors add).
    pub fn mul(&self, o: &Term) -> Term {
        let mut v = self.0.clone();
        for &(f, p) in &o.0 {
            if let Some(e) = v.iter_mut().find(|(g, _)| *g == f) {
                e.1 += p;
            } else {
                v.push((f, p));
            }
        }
        v.sort();
        Term(v)
    }

    /// True if `sub` is a proper divisor of this term (hierarchy parent).
    pub fn contains(&self, sub: &Term) -> bool {
        sub != self && !sub.is_intercept() && sub.0.iter().all(|&(f, p)| self.power_of(f) >= p)
    }

    /// Immediate parents for hierarchy: terms obtained by lowering one power by one.
    pub fn parents(&self) -> Vec<Term> {
        let mut out = Vec::new();
        for i in 0..self.0.len() {
            let mut v = self.0.clone();
            if v[i].1 > 1 {
                v[i].1 -= 1;
            } else {
                v.remove(i);
            }
            if !v.is_empty() {
                out.push(Term(v));
            }
        }
        out
    }

    pub fn factors(&self) -> impl Iterator<Item = usize> + '_ {
        self.0.iter().map(|(f, _)| *f)
    }
}

pub fn sort_terms(terms: &mut Vec<Term>) {
    terms.sort_by(|a, b| {
        (a.degree(), a.max_power(), a.0.iter().map(|x| x.0).collect::<Vec<_>>(), &a.0).cmp(&(
            b.degree(),
            b.max_power(),
            b.0.iter().map(|x| x.0).collect::<Vec<_>>(),
            &b.0,
        ))
    });
    terms.dedup();
}

pub fn from_specs(p: &Project, specs: &[TermSpec]) -> Vec<Term> {
    let mut out: Vec<Term> = specs
        .iter()
        .filter_map(|s| {
            let mut v = Vec::new();
            for &(id, pw) in &s.0 {
                v.push((p.factor_index(id)?, pw));
            }
            v.sort();
            (!v.is_empty()).then_some(Term(v))
        })
        .collect();
    sort_terms(&mut out);
    out
}

pub fn to_specs(p: &Project, terms: &[Term]) -> Vec<TermSpec> {
    terms.iter().map(|t| TermSpec(t.0.iter().map(|&(f, pw)| (p.factors[f].id, pw)).collect())).collect()
}

pub fn term_label(p: &Project, t: &Term) -> String {
    if t.is_intercept() {
        return "Intercept".into();
    }
    t.0.iter()
        .map(|&(f, pw)| {
            let s = &p.factors[f].symbol;
            if pw > 1 { format!("{s}^{pw}") } else { s.clone() }
        })
        .collect::<Vec<_>>()
        .join("*")
}

pub fn format_formula(p: &Project, terms: &[Term]) -> String {
    terms.iter().map(|t| term_label(p, t)).collect::<Vec<_>>().join(" + ")
}

/// True when the model has an intercept (Scheffé mixture models do not).
pub fn has_intercept(p: &Project, terms: &[Term]) -> bool {
    !terms.iter().any(|t| t.0.len() == 1 && p.factors[t.0[0].0].is_mixture())
}

/// Highest power of one factor in a model term.
pub const MAX_POWER: u32 = 10;
/// Deepest nesting of parentheses in a formula.
const MAX_DEPTH: usize = 64;
/// Most terms a formula may expand to while it is parsed.
const MAX_TERMS: usize = 10_000;

pub fn validate_term(p: &Project, t: &Term) -> Result<(), String> {
    for &(f, pw) in &t.0 {
        let fac = &p.factors[f];
        if pw > MAX_POWER {
            return Err(format!("powers of {} above {MAX_POWER} are not supported", fac.symbol));
        }
        if pw > 1 {
            match &fac.kind {
                FactorKind::Categorical { .. } => {
                    return Err(format!("{} is categorical and has no powers", fac.symbol));
                }
                FactorKind::Mixture { .. } => {
                    return Err(format!("{} is a mixture component; Scheffé models use products, not powers", fac.symbol));
                }
                FactorKind::Discrete { levels } if (pw as usize) >= levels.len() => {
                    return Err(format!(
                        "{} has {} levels, which supports powers up to {}",
                        fac.symbol,
                        levels.len(),
                        levels.len().saturating_sub(1)
                    ));
                }
                _ => {}
            }
        }
    }
    Ok(())
}

/// Adds every missing hierarchy parent of the listed terms.
pub fn complete_hierarchy(terms: &[Term]) -> Vec<Term> {
    let mut set: BTreeSet<Term> = terms.iter().cloned().collect();
    let mut stack: Vec<Term> = terms.to_vec();
    while let Some(t) = stack.pop() {
        for par in t.parents() {
            if set.insert(par.clone()) {
                stack.push(par);
            }
        }
    }
    let mut v: Vec<Term> = set.into_iter().collect();
    sort_terms(&mut v);
    v
}

// ---------------------------------------------------------------------------
// Formula parsing
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Ident(String),
    Int(u32),
    Plus,
    Minus,
    Star,
    Caret,
    LParen,
    RParen,
}

fn tokenize(s: &str) -> Result<Vec<Tok>, String> {
    let mut out = Vec::new();
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        match c {
            ' ' | '\t' | '\n' | '\r' => i += 1,
            '+' => {
                out.push(Tok::Plus);
                i += 1
            }
            '-' => {
                out.push(Tok::Minus);
                i += 1
            }
            '*' | ':' | '×' | '·' => {
                out.push(Tok::Star);
                i += 1
            }
            '^' => {
                out.push(Tok::Caret);
                i += 1
            }
            '²' => {
                out.push(Tok::Caret);
                out.push(Tok::Int(2));
                i += 1
            }
            '³' => {
                out.push(Tok::Caret);
                out.push(Tok::Int(3));
                i += 1
            }
            '(' => {
                out.push(Tok::LParen);
                i += 1
            }
            ')' => {
                out.push(Tok::RParen);
                i += 1
            }
            '`' | '"' => {
                let q = c;
                let start = i + 1;
                let mut j = start;
                while j < chars.len() && chars[j] != q {
                    j += 1;
                }
                if j >= chars.len() {
                    return Err("unterminated quoted name".into());
                }
                out.push(Tok::Ident(chars[start..j].iter().collect()));
                i = j + 1;
            }
            c if c.is_ascii_digit() => {
                let start = i;
                while i < chars.len() && chars[i].is_ascii_digit() {
                    i += 1;
                }
                let t: String = chars[start..i].iter().collect();
                out.push(Tok::Int(t.parse().map_err(|_| format!("bad number {t}"))?));
            }
            c if c.is_alphanumeric() || c == '_' || c == '.' => {
                let start = i;
                while i < chars.len() && ((chars[i].is_alphanumeric() && chars[i] != '²' && chars[i] != '³') || chars[i] == '_' || chars[i] == '.') {
                    i += 1;
                }
                out.push(Tok::Ident(chars[start..i].iter().collect()));
            }
            _ => return Err(format!("unexpected character '{c}'")),
        }
    }
    Ok(out)
}

struct Parser<'a> {
    toks: Vec<Tok>,
    pos: usize,
    depth: usize,
    project: &'a Project,
}

impl Parser<'_> {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos)
    }

    fn next(&mut self) -> Option<Tok> {
        let t = self.toks.get(self.pos).cloned();
        self.pos += 1;
        t
    }

    fn lookup(&self, name: &str) -> Result<usize, String> {
        let fs = &self.project.factors;
        fs.iter()
            .position(|f| f.symbol == name)
            .or_else(|| fs.iter().position(|f| f.symbol.eq_ignore_ascii_case(name)))
            .or_else(|| fs.iter().position(|f| f.name == name))
            .or_else(|| fs.iter().position(|f| f.name.eq_ignore_ascii_case(name)))
            .ok_or_else(|| format!("unknown factor '{name}'"))
    }

    /// expr := ['-'] product (('+'|'-') product)*
    fn expr(&mut self) -> Result<Vec<Term>, String> {
        let mut add: Vec<Term> = Vec::new();
        let mut remove: Vec<Term> = Vec::new();
        let mut neg = false;
        if self.peek() == Some(&Tok::Minus) {
            self.next();
            neg = true;
        }
        loop {
            let set = self.product()?;
            if neg { remove.extend(set) } else { add.extend(set) }
            match self.peek() {
                Some(Tok::Plus) => {
                    self.next();
                    neg = false;
                }
                Some(Tok::Minus) => {
                    self.next();
                    neg = true;
                }
                _ => break,
            }
        }
        add.retain(|t| !remove.contains(t));
        Ok(add)
    }

    /// product := power ('*' power)*
    fn product(&mut self) -> Result<Vec<Term>, String> {
        let mut acc = self.power()?;
        while self.peek() == Some(&Tok::Star) {
            self.next();
            let rhs = self.power()?;
            if acc.len().saturating_mul(rhs.len()) > MAX_TERMS {
                return Err(format!("the formula expands to more than {MAX_TERMS} terms"));
            }
            let mut out = Vec::new();
            for a in &acc {
                for b in &rhs {
                    out.push(a.mul(b));
                }
            }
            acc = out;
        }
        Ok(acc)
    }

    /// power := primary ('^' INT)?
    fn power(&mut self) -> Result<Vec<Term>, String> {
        let (set, grouped) = self.primary()?;
        if self.peek() == Some(&Tok::Caret) {
            self.next();
            let n = match self.next() {
                Some(Tok::Int(n)) if n >= 1 => n,
                _ => return Err("expected a positive integer after '^'".into()),
            };
            if n > MAX_POWER {
                return Err(format!("powers above {MAX_POWER} are not supported"));
            }
            if grouped {
                // (A + B + C)^n: every product of up to n distinct terms.
                let mut out: Vec<Term> = set.clone();
                let mut frontier: Vec<(Term, usize)> = set.iter().cloned().zip(0..).collect();
                for _ in 1..n {
                    let mut next = Vec::new();
                    for (t, last) in &frontier {
                        for (j, s) in set.iter().enumerate().skip(last + 1) {
                            if out.len() >= MAX_TERMS {
                                return Err(format!("the formula expands to more than {MAX_TERMS} terms"));
                            }
                            let m = t.mul(s);
                            out.push(m.clone());
                            next.push((m, j));
                        }
                    }
                    frontier = next;
                }
                return Ok(out);
            }
            return Ok(set
                .into_iter()
                .map(|t| Term(t.0.into_iter().map(|(f, p)| (f, p.saturating_mul(n))).collect()))
                .collect());
        }
        Ok(set)
    }

    fn primary(&mut self) -> Result<(Vec<Term>, bool), String> {
        match self.next() {
            Some(Tok::Ident(name)) => {
                let f = self.lookup(&name)?;
                Ok((vec![Term::single(f, 1)], false))
            }
            Some(Tok::Int(1)) => Ok((vec![], false)),
            Some(Tok::LParen) => {
                if self.depth >= MAX_DEPTH {
                    return Err(format!("parentheses are nested more than {MAX_DEPTH} deep"));
                }
                self.depth += 1;
                let e = self.expr();
                self.depth -= 1;
                let e = e?;
                if self.next() != Some(Tok::RParen) {
                    return Err("missing ')'".into());
                }
                Ok((e, true))
            }
            Some(t) => Err(format!("unexpected {t:?}")),
            None => Err("unexpected end of formula".into()),
        }
    }
}

/// Parses a model formula such as `A + B + A*B + A^2` or `(A+B+C)^2 - B*C`.
pub fn parse_formula(p: &Project, text: &str) -> Result<Vec<Term>, String> {
    let toks = tokenize(text)?;
    if toks.is_empty() {
        return Ok(Vec::new());
    }
    let mut parser = Parser { toks, pos: 0, depth: 0, project: p };
    let mut terms = parser.expr()?;
    if parser.pos < parser.toks.len() {
        return Err(format!("unexpected {:?}", parser.toks[parser.pos]));
    }
    terms.retain(|t| !t.is_intercept());
    for t in &terms {
        validate_term(p, t)?;
    }
    sort_terms(&mut terms);
    Ok(terms)
}

// ---------------------------------------------------------------------------
// Presets
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Preset {
    Linear,
    Interactions,
    Quadratic,
    Cubic,
}

impl Preset {
    pub fn parse(s: &str) -> Option<Preset> {
        Some(match s.to_ascii_lowercase().as_str() {
            "linear" | "main" => Preset::Linear,
            "2fi" | "interactions" => Preset::Interactions,
            "quadratic" => Preset::Quadratic,
            "cubic" => Preset::Cubic,
            _ => return None,
        })
    }
    pub fn all() -> [(&'static str, &'static str); 4] {
        [
            ("linear", "Linear"),
            ("2fi", "Two-factor interactions"),
            ("quadratic", "Quadratic"),
            ("cubic", "Cubic"),
        ]
    }
}

fn supports_power(f: &Factor, pw: u32) -> bool {
    match &f.kind {
        FactorKind::Continuous { .. } => true,
        FactorKind::Discrete { levels } => levels.len() > pw as usize,
        _ => false,
    }
}

/// Builds a standard model.
///
/// Designed factors (controlled and noise) receive the full polynomial
/// structure; environmental factors enter as main effects, and with
/// `env_interactions` also through interactions with designed factors;
/// block factors enter as main effects only.
pub fn preset_terms(p: &Project, preset: Preset, env_interactions: bool) -> Vec<Term> {
    let idx = |pred: &dyn Fn(&Factor) -> bool| -> Vec<usize> {
        p.factors.iter().enumerate().filter(|(_, f)| pred(f)).map(|(i, _)| i).collect()
    };
    let design = idx(&|f| f.role.is_designed() && !f.is_mixture());
    let mix = idx(&|f| f.is_mixture());
    let env = idx(&|f| f.role == Role::Environmental && !f.is_mixture());
    let block = idx(&|f| f.role == Role::Block && !f.is_mixture());
    let order = match preset {
        Preset::Linear => 1,
        Preset::Interactions | Preset::Quadratic => 2,
        Preset::Cubic => 3,
    };
    let mut terms: Vec<Term> = Vec::new();
    for &f in design.iter().chain(&mix).chain(&env).chain(&block) {
        terms.push(Term::single(f, 1));
    }
    // Interactions among designed factors and among mixture components.
    for group in [&design, &mix] {
        for k in 2..=order {
            for combo in combinations(group, k) {
                terms.push(Term(combo.iter().map(|&f| (f, 1)).collect()));
            }
        }
    }
    // Mixture x process interactions are left to the formula editor.
    if matches!(preset, Preset::Quadratic | Preset::Cubic) {
        for &f in &design {
            if supports_power(&p.factors[f], 2) {
                terms.push(Term::single(f, 2));
            }
        }
    }
    if preset == Preset::Cubic {
        for &f in &design {
            if supports_power(&p.factors[f], 3) {
                terms.push(Term::single(f, 3));
            }
            if supports_power(&p.factors[f], 2) {
                for &g in &design {
                    if g != f {
                        terms.push(Term::single(f, 2).mul(&Term::single(g, 1)));
                    }
                }
            }
        }
    }
    if env_interactions && order >= 2 {
        for &e in &env {
            for &d in &design {
                terms.push(Term::single(e, 1).mul(&Term::single(d, 1)));
            }
        }
        if matches!(preset, Preset::Quadratic | Preset::Cubic) {
            for &e in &env {
                if supports_power(&p.factors[e], 2) {
                    terms.push(Term::single(e, 2));
                }
            }
        }
    }
    // Interactions of a conditional factor with a parent that takes a single
    // value wherever the factor applies duplicate its main effect.
    let conds = coded_conditions(p);
    if crate::condition::any(&conds) {
        terms.retain(|t| {
            t.0.len() < 2 || !t.factors().any(|f| crate::condition::fixed_ancestors(&conds, f).iter().any(|&a| t.power_of(a) > 0))
        });
    }
    sort_terms(&mut terms);
    terms
}

pub fn combinations(items: &[usize], k: usize) -> Vec<Vec<usize>> {
    let mut out = Vec::new();
    let mut cur = Vec::new();
    fn rec(items: &[usize], k: usize, start: usize, cur: &mut Vec<usize>, out: &mut Vec<Vec<usize>>) {
        if cur.len() == k {
            out.push(cur.clone());
            return;
        }
        for i in start..items.len() {
            cur.push(items[i]);
            rec(items, k, i + 1, cur, out);
            cur.pop();
        }
    }
    rec(items, k, 0, &mut cur, &mut out);
    out
}

// ---------------------------------------------------------------------------
// Model matrix layout
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub enum ColPart {
    Num { factor: usize, power: u32 },
    /// Effect (sum-to-zero) coding column for `level` of a factor with `k` levels.
    Cat { factor: usize, level: usize, k: usize },
}

#[derive(Clone, Debug)]
pub struct Column {
    /// Term index, or None for the intercept.
    pub term: Option<usize>,
    pub label: String,
    pub parts: Vec<ColPart>,
}

#[derive(Clone, Debug)]
pub struct ModelLayout {
    pub terms: Vec<Term>,
    pub term_labels: Vec<String>,
    pub intercept: bool,
    pub columns: Vec<Column>,
    /// Conditions of conditional factors, per factor index.
    pub conds: Vec<Option<CodedCond>>,
    /// Indices of the terms that are a single mixture component (the
    /// linear blending terms of a Scheffé model).
    pub mixture_linear: Vec<usize>,
}

impl ModelLayout {
    pub fn new(p: &Project, terms: &[Term]) -> ModelLayout {
        let intercept = has_intercept(p, terms);
        let mut columns = Vec::new();
        if intercept {
            columns.push(Column { term: None, label: "Intercept".into(), parts: vec![] });
        }
        for (ti, t) in terms.iter().enumerate() {
            // Expands each factor into its column options, then takes the
            // Cartesian product across the factors of the term.
            let mut combos: Vec<(Vec<ColPart>, Vec<String>)> = vec![(vec![], vec![])];
            for &(f, pw) in &t.0 {
                let fac = &p.factors[f];
                let opts: Vec<(ColPart, String)> = match &fac.kind {
                    FactorKind::Categorical { levels } => {
                        let k = levels.len();
                        (0..k.saturating_sub(1))
                            .map(|l| (ColPart::Cat { factor: f, level: l, k }, format!("{}[{}]", fac.symbol, levels[l])))
                            .collect()
                    }
                    _ => {
                        let lab = if pw > 1 { format!("{}^{}", fac.symbol, pw) } else { fac.symbol.clone() };
                        vec![(ColPart::Num { factor: f, power: pw }, lab)]
                    }
                };
                let mut next = Vec::new();
                for (parts, labs) in &combos {
                    for (op, ol) in &opts {
                        let mut p2 = parts.clone();
                        p2.push(op.clone());
                        let mut l2 = labs.clone();
                        l2.push(ol.clone());
                        next.push((p2, l2));
                    }
                }
                combos = next;
            }
            for (parts, labs) in combos {
                columns.push(Column { term: Some(ti), label: labs.join("*"), parts });
            }
        }
        let term_labels = terms.iter().map(|t| term_label(p, t)).collect();
        let conds = coded_conditions(p);
        let conds = if crate::condition::any(&conds) { conds } else { Vec::new() };
        let mixture_linear =
            (0..terms.len()).filter(|&ti| terms[ti].0.len() == 1 && terms[ti].0[0].1 == 1 && p.factors[terms[ti].0[0].0].is_mixture()).collect();
        ModelLayout { terms: terms.to_vec(), term_labels, intercept, columns, conds, mixture_linear }
    }

    pub fn ncols(&self) -> usize {
        self.columns.len()
    }

    pub fn factors_used(&self) -> Vec<usize> {
        let mut s: BTreeSet<usize> = BTreeSet::new();
        for t in &self.terms {
            s.extend(t.factors());
        }
        s.into_iter().collect()
    }

    /// Model-matrix row for a point in coded units; None if a used factor is
    /// missing. Columns that contain an inactive conditional factor are 0,
    /// and inactive factors need no value.
    pub fn row(&self, point: &[Option<Coded>]) -> Option<Vec<f64>> {
        let act = (!self.conds.is_empty()).then(|| activity(&self.conds, point));
        let mut row = Vec::with_capacity(self.columns.len());
        for c in &self.columns {
            let mut v = 1.0;
            for part in &c.parts {
                if let Some(act) = &act {
                    let f = match *part {
                        ColPart::Num { factor, .. } | ColPart::Cat { factor, .. } => factor,
                    };
                    match act.get(f).copied().flatten() {
                        Some(true) => {}
                        Some(false) => {
                            v = 0.0;
                            break;
                        }
                        None => return None,
                    }
                }
                match *part {
                    ColPart::Num { factor, power } => {
                        let x = point.get(factor).copied().flatten()?.num();
                        v *= x.powi(power as i32);
                    }
                    ColPart::Cat { factor, level, k } => {
                        let li = match point.get(factor).copied().flatten()? {
                            Coded::Level(i) => i,
                            Coded::Num(x) => x.round().max(0.0) as usize,
                        };
                        v *= if li == level {
                            1.0
                        } else if li == k - 1 {
                            -1.0
                        } else {
                            0.0
                        };
                    }
                }
            }
            row.push(v);
        }
        Some(row)
    }

    /// Indices of the columns that belong to term `ti`.
    pub fn term_columns(&self, ti: usize) -> Vec<usize> {
        self.columns.iter().enumerate().filter(|(_, c)| c.term == Some(ti)).map(|(i, _)| i).collect()
    }
}

pub fn term_label_from_layout(layout: &ModelLayout, ti: usize) -> String {
    layout.term_labels[ti].clone()
}
