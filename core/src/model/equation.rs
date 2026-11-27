//! Model equations in coded and actual units, in several output syntaxes.

use super::fit::Fit;
use super::terms::{ColPart, Column, MAX_POWER};
use crate::format;
use crate::condition::{coded_conditions, format_named};
use crate::project::{Coded, FactorKind, Project, Transform};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Syntax {
    Plain,
    Latex,
    Excel,
    Python,
    C,
}

impl Syntax {
    pub fn parse(s: &str) -> Syntax {
        match s.to_ascii_lowercase().as_str() {
            "latex" => Syntax::Latex,
            "excel" => Syntax::Excel,
            "python" => Syntax::Python,
            "c" => Syntax::C,
            _ => Syntax::Plain,
        }
    }
    pub fn all() -> [(&'static str, &'static str); 5] {
        [("plain", "Plain text"), ("latex", "LaTeX"), ("excel", "Excel"), ("python", "Python"), ("c", "C / C++ / Rust")]
    }
}

/// Polynomial in factor variables: monomial (factor index, power) -> coefficient.
type Poly = BTreeMap<Vec<(usize, u32)>, f64>;

fn poly_mul(a: &Poly, b: &Poly) -> Poly {
    let mut out = Poly::new();
    for (ma, ca) in a {
        for (mb, cb) in b {
            let mut m = ma.clone();
            for &(f, p) in mb {
                if let Some(e) = m.iter_mut().find(|(g, _)| *g == f) {
                    e.1 += p;
                } else {
                    m.push((f, p));
                }
            }
            m.sort();
            *out.entry(m).or_insert(0.0) += ca * cb;
        }
    }
    out
}

fn poly_const(c: f64) -> Poly {
    let mut p = Poly::new();
    p.insert(vec![], c);
    p
}

/// True for a column whose power is above what the engine expands, as can
/// come from a project file; such columns are left out of the equations.
fn has_excess_power(col: &Column) -> bool {
    col.parts.iter().any(|part| matches!(*part, ColPart::Num { power, .. } if power > MAX_POWER))
}

/// Variable name of a factor in the given syntax.
pub fn var_name(p: &Project, fi: usize, syntax: Syntax, coded: bool) -> String {
    let f = &p.factors[fi];
    let base = if coded { f.symbol.clone() } else { f.name.clone() };
    match syntax {
        Syntax::Plain => {
            if base.chars().all(|c| c.is_alphanumeric() || c == '_') { base } else { format!("[{base}]") }
        }
        Syntax::Latex => {
            let esc = base.replace('\\', "\\textbackslash{}").replace('_', "\\_").replace('%', "\\%").replace('&', "\\&").replace('#', "\\#");
            if coded && esc.chars().count() == 1 { esc } else { format!("\\text{{{esc}}}") }
        }
        _ => sanitize_ident(&base),
    }
}

pub fn sanitize_ident(s: &str) -> String {
    let mut out: String = s.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect();
    while out.contains("__") {
        out = out.replace("__", "_");
    }
    let out = out.trim_matches('_').to_string();
    if out.is_empty() {
        "x".into()
    } else if out.chars().next().unwrap().is_ascii_digit() {
        format!("x{out}")
    } else {
        out
    }
}

fn fmt_num(x: f64, sig: usize, syntax: Syntax) -> String {
    let s = format::num(x, sig);
    if syntax == Syntax::Latex {
        if let Some((m, e)) = s.split_once('e') {
            let e = e.trim_start_matches('+');
            return format!("{m} \\times 10^{{{e}}}");
        }
    }
    s
}

fn power(var: &str, pw: u32, syntax: Syntax) -> String {
    if pw == 1 {
        return var.to_string();
    }
    match syntax {
        Syntax::Plain | Syntax::Excel => format!("{var}^{pw}"),
        Syntax::Latex => format!("{var}^{{{pw}}}"),
        Syntax::Python => format!("{var}**{pw}"),
        Syntax::C => vec![var; pw as usize].join("*"),
    }
}

fn mul_sym(syntax: Syntax) -> &'static str {
    match syntax {
        Syntax::Plain => " * ",
        Syntax::Latex => " \\cdot ",
        _ => "*",
    }
}

/// Renders a sum of (coefficient, factor list) terms.
fn render_sum(items: &[(f64, Vec<String>)], sig: usize, syntax: Syntax) -> String {
    let mut s = String::new();
    for (i, (c, vars)) in items.iter().enumerate() {
        if *c == 0.0 && !vars.is_empty() {
            continue;
        }
        let mag = fmt_num(c.abs(), sig, syntax);
        let body = if vars.is_empty() {
            mag
        } else {
            format!("{mag}{}{}", mul_sym(syntax), vars.join(mul_sym(syntax)))
        };
        if i == 0 || s.is_empty() {
            if *c < 0.0 {
                s.push('-');
            }
            s.push_str(&body);
        } else {
            s.push_str(if *c < 0.0 { " - " } else { " + " });
            s.push_str(&body);
        }
    }
    if s.is_empty() { "0".into() } else { s }
}

fn lhs(name: &str, tr: Transform, syntax: Syntax) -> String {
    let n = name.to_string();
    match (tr, syntax) {
        (Transform::None, _) => n,
        (Transform::Ln, Syntax::Latex) => format!("\\ln({n})"),
        (Transform::Ln, _) => format!("ln({n})"),
        (Transform::Log10, Syntax::Latex) => format!("\\log_{{10}}({n})"),
        (Transform::Log10, _) => format!("log10({n})"),
        (Transform::Sqrt, Syntax::Latex) => format!("\\sqrt{{{n}}}"),
        (Transform::Sqrt, _) => format!("sqrt({n})"),
        (Transform::Inverse, Syntax::Latex) => format!("\\frac{{1}}{{{n}}}"),
        (Transform::Inverse, _) => format!("1/{n}"),
        (Transform::Power(l), Syntax::Latex) => format!("{n}^{{{}}}", format::num(l, 4)),
        (Transform::Power(l), Syntax::Python) => format!("{n}**{}", format::num(l, 4)),
        (Transform::Power(l), _) => format!("{n}^{}", format::num(l, 4)),
    }
}

/// Wraps `expr` (transformed scale) in the inverse transform.
fn explicit(expr: &str, tr: Transform, syntax: Syntax) -> String {
    let e = format!("({expr})");
    match (tr, syntax) {
        (Transform::None, _) => expr.to_string(),
        (Transform::Ln, Syntax::Excel) => format!("EXP{e}"),
        (Transform::Ln, Syntax::Python) => format!("math.exp{e}"),
        (Transform::Ln, Syntax::Latex) => format!("e^{{{expr}}}"),
        (Transform::Ln, _) => format!("exp{e}"),
        (Transform::Log10, Syntax::Python) => format!("10**{e}"),
        (Transform::Log10, Syntax::C) => format!("pow(10.0, {expr})"),
        (Transform::Log10, Syntax::Latex) => format!("10^{{{expr}}}"),
        (Transform::Log10, _) => format!("10^{e}"),
        (Transform::Sqrt, Syntax::Python) => format!("{e}**2"),
        (Transform::Sqrt, Syntax::C) => format!("{e}*{e}"),
        (Transform::Sqrt, Syntax::Latex) => format!("\\left({expr}\\right)^{{2}}"),
        (Transform::Sqrt, _) => format!("{e}^2"),
        (Transform::Inverse, Syntax::Latex) => format!("\\frac{{1}}{{{expr}}}"),
        (Transform::Inverse, _) => format!("1/{e}"),
        (Transform::Power(l), Syntax::Python) => format!("{e}**(1/{})", format::num(l, 6)),
        (Transform::Power(l), Syntax::C) => format!("pow({expr}, 1.0/{})", format::num(l, 6)),
        (Transform::Power(l), Syntax::Latex) => format!("\\left({expr}\\right)^{{1/{}}}", format::num(l, 6)),
        (Transform::Power(l), _) => format!("{e}^(1/{})", format::num(l, 6)),
    }
}

#[derive(Clone, Debug)]
pub struct Equation {
    /// Categorical level combination this equation applies to (empty if none).
    pub condition: String,
    /// Equation on the modelled (possibly transformed) scale.
    pub text: String,
    /// Equation solved for the response (only differs with a transform).
    pub explicit: Option<String>,
}

fn wrap(syntax: Syntax, lhs: &str, rhs: &str) -> String {
    match syntax {
        Syntax::Excel => format!("={rhs}"),
        Syntax::C => format!("{lhs} = {rhs};"),
        _ => format!("{lhs} = {rhs}"),
    }
}

/// Note for coded-unit equations of models with conditional factors.
pub fn coded_note(p: &Project, fit: &Fit) -> Option<String> {
    let names: Vec<String> = fit.layout().factors_used().into_iter().filter(|&f| p.factors[f].condition.is_some()).map(|f| p.factors[f].symbol.clone()).collect();
    (!names.is_empty()).then(|| format!("Terms with {} are 0 where the factor does not apply.", names.join(", ")))
}

/// Model equation(s). In coded units there is one equation with effect-coded
/// categorical columns; in actual units there is one equation per
/// combination of categorical levels.
pub fn equations(p: &Project, fit: &Fit, response_name: &str, actual: bool, syntax: Syntax, sig: usize) -> Vec<Equation> {
    let layout = fit.layout();
    let tr = fit.input.transform;
    let resp = match syntax {
        Syntax::Plain | Syntax::Latex => {
            if syntax == Syntax::Latex { format!("\\text{{{}}}", response_name.replace('_', "\\_")) } else { response_name.to_string() }
        }
        _ => sanitize_ident(response_name),
    };
    let lhs_t = lhs(&resp, tr, syntax);
    let make = |rhs: String, cond: String| {
        let text = if tr == Transform::None || matches!(syntax, Syntax::Plain | Syntax::Latex) {
            wrap(syntax, &lhs_t, &rhs)
        } else {
            wrap(syntax, &resp, &explicit(&rhs, tr, syntax))
        };
        let expl = (tr != Transform::None && matches!(syntax, Syntax::Plain | Syntax::Latex))
            .then(|| wrap(syntax, &resp, &explicit(&rhs, tr, syntax)));
        Equation { condition: cond, text, explicit: expl }
    };

    if !actual {
        let mut items = Vec::new();
        for (ci, col) in layout.columns.iter().enumerate() {
            if fit.aliased[ci] || has_excess_power(col) {
                continue;
            }
            let vars: Vec<String> = col
                .parts
                .iter()
                .map(|part| match *part {
                    ColPart::Num { factor, power: pw } => power(&var_name(p, factor, syntax, true), pw, syntax),
                    ColPart::Cat { factor, level, .. } => {
                        let f = &p.factors[factor];
                        let lev = &f.cat_levels()[level];
                        match syntax {
                            Syntax::Latex => format!("{}_{{[\\text{{{}}}]}}", f.symbol, lev),
                            Syntax::Plain => format!("{}[{}]", f.symbol, lev),
                            _ => sanitize_ident(&format!("{}_{}", f.symbol, lev)),
                        }
                    }
                })
                .collect();
            items.push((fit.coef[ci], vars));
        }
        return vec![make(render_sum(&items, sig, syntax), String::new())];
    }

    // Actual units: expand each column polynomial for each categorical
    // combination and, with conditional factors, for each pattern of which
    // factors apply.
    let used = layout.factors_used();
    let cats: Vec<usize> = used.iter().cloned().filter(|&f| p.factors[f].is_categorical()).collect();
    let conds = coded_conditions(p);
    // Conditional factors of the model and their conditional ancestors.
    let mut rel: Vec<usize> = Vec::new();
    for &f in &used {
        let mut cur = f;
        for _ in 0..p.factors.len() {
            let Some(c) = &conds[cur] else { break };
            if !rel.contains(&cur) {
                rel.push(cur);
            }
            cur = c.parent;
        }
    }
    rel.sort_unstable();
    // Conditions on a categorical factor of the model follow from the level
    // combination; the others branch into "applies" and "does not apply".
    let branch: Vec<usize> = rel.iter().cloned().filter(|&f| conds[f].as_ref().is_some_and(|c| !cats.contains(&c.parent))).collect();
    let mut combos: Vec<Vec<usize>> = vec![vec![]];
    for &c in &cats {
        let k = p.factors[c].level_count();
        let mut next = Vec::new();
        for combo in &combos {
            for l in 0..k {
                let mut v = combo.clone();
                v.push(l);
                next.push(v);
            }
        }
        combos = next;
        if combos.len() > 64 {
            break;
        }
    }
    if combos.len() << branch.len().min(16) > 64 {
        return vec![Equation {
            condition: String::new(),
            text: "Too many categorical level combinations for actual-unit equations; use coded units.".into(),
            explicit: None,
        }];
    }
    let mc = p.mixture_coding();
    let mut out = Vec::new();
    let mut seen: Vec<(Vec<bool>, Vec<Option<usize>>)> = Vec::new();
    for combo in combos.iter().flat_map(|c| (0..1usize << branch.len()).map(move |bits| (c, bits))) {
        let (combo, bits) = combo;
        let level_of = |f: usize| cats.iter().position(|&c| c == f).map(|i| combo[i]);
        // Activity of every factor under this combination and pattern.
        let mut active = vec![true; p.factors.len()];
        let mut order = rel.clone();
        // Parents before children: sort by chain depth.
        let depth = |f: usize| {
            let mut d = 0;
            let mut cur = f;
            while let Some(c) = &conds[cur] {
                d += 1;
                cur = c.parent;
                if d > p.factors.len() {
                    break;
                }
            }
            d
        };
        order.sort_by_key(|&f| depth(f));
        for &f in &order {
            let c = conds[f].as_ref().unwrap();
            let holds = match branch.iter().position(|&b| b == f) {
                Some(i) => bits >> i & 1 == 1,
                None => c.holds(Coded::Level(level_of(c.parent).unwrap_or(0))),
            };
            active[f] = active[c.parent] && holds;
        }
        // Patterns that differ only where a factor does not apply give the
        // same equation.
        let key = (
            used.iter().map(|&f| active[f]).collect::<Vec<_>>(),
            cats.iter().zip(combo).map(|(&f, &l)| active[f].then_some(l)).collect::<Vec<_>>(),
        );
        let branch_ok = branch.iter().enumerate().all(|(i, &f)| active[conds[f].as_ref().unwrap().parent] || bits >> i & 1 == 0);
        if !branch_ok || seen.contains(&key) {
            continue;
        }
        seen.push(key);
        let mut total = Poly::new();
        for (ci, col) in layout.columns.iter().enumerate() {
            if fit.aliased[ci] || has_excess_power(col) {
                continue;
            }
            if col.parts.iter().any(|part| match *part {
                ColPart::Num { factor, .. } | ColPart::Cat { factor, .. } => !active[factor],
            }) {
                continue;
            }
            let mut poly = poly_const(fit.coef[ci]);
            for part in &col.parts {
                match *part {
                    ColPart::Num { factor, power: pw } => {
                        let f = &p.factors[factor];
                        let (a, b) = match f.kind {
                            FactorKind::Mixture { low, .. } => (1.0 / mc.span(), -low / mc.span()),
                            _ => {
                                let (c, h) = f.center_half();
                                (1.0 / h, -c / h)
                            }
                        };
                        let mut lin = Poly::new();
                        lin.insert(vec![(factor, 1)], a);
                        if b != 0.0 {
                            lin.insert(vec![], b);
                        }
                        for _ in 0..pw {
                            poly = poly_mul(&poly, &lin);
                        }
                    }
                    ColPart::Cat { factor, level, k } => {
                        let li = level_of(factor).unwrap_or(0);
                        let v = if li == level {
                            1.0
                        } else if li == k - 1 {
                            -1.0
                        } else {
                            0.0
                        };
                        poly = poly_mul(&poly, &poly_const(v));
                    }
                }
            }
            for (m, c) in poly {
                *total.entry(m).or_insert(0.0) += c;
            }
        }
        // Orders monomials by degree, then factor order.
        let mut mons: Vec<(Vec<(usize, u32)>, f64)> = total.into_iter().collect();
        mons.sort_by_key(|(m, _)| (m.iter().map(|x| x.1).sum::<u32>(), m.clone()));
        let scale = mons.iter().map(|(_, c)| c.abs()).fold(0.0, f64::max);
        let items: Vec<(f64, Vec<String>)> = mons
            .into_iter()
            .filter(|(m, c)| m.is_empty() || c.abs() > 1e-13 * scale)
            .map(|(m, c)| {
                (c, m.iter().map(|&(f, pw)| power(&var_name(p, f, syntax, false), pw, syntax)).collect())
            })
            .collect();
        let mut labels: Vec<String> = cats
            .iter()
            .zip(combo)
            .filter(|(f, _)| active[**f])
            .map(|(&f, &l)| format!("{} = {}", p.factors[f].name, p.factors[f].cat_levels()[l]))
            .collect();
        for (i, &f) in branch.iter().enumerate() {
            let c = p.factors[f].condition.as_ref().unwrap();
            if active[p.factor_index(c.factor).unwrap_or(f)] {
                labels.push(format_named(p, c, bits >> i & 1 == 0));
            }
        }
        out.push(make(render_sum(&items, sig, syntax), labels.join(", ")));
    }
    out
}
