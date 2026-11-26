//! Conditional factors: a factor that applies only when the value of another
//! factor (its parent) meets a condition, e.g. a defrosting method that only
//! matters when the temperature is below 0.
//!
//! Where the condition fails, the factor is inactive: the run stores no
//! value for it, designs leave it blank, and every model column that
//! contains it takes the value 0 (the coded centre, or no contrast for
//! categorical factors). Its effects are therefore estimated from the runs
//! where it applies.

use crate::format;
use crate::project::{Coded, FactorKind, Project, Role, Value};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum CondOp {
    #[serde(rename = "<")]
    Lt,
    #[serde(rename = "<=")]
    Le,
    #[serde(rename = ">")]
    Gt,
    #[serde(rename = ">=")]
    Ge,
    #[serde(rename = "=")]
    Eq,
    #[serde(rename = "!=")]
    Ne,
}

impl CondOp {
    pub fn text(self) -> &'static str {
        match self {
            CondOp::Lt => "<",
            CondOp::Le => "<=",
            CondOp::Gt => ">",
            CondOp::Ge => ">=",
            CondOp::Eq => "=",
            CondOp::Ne => "!=",
        }
    }

    fn is_comparison(self) -> bool {
        matches!(self, CondOp::Lt | CondOp::Le | CondOp::Gt | CondOp::Ge)
    }
}

/// The factor applies only when factor `factor` (by id) satisfies
/// `op values`. `=` and `!=` take a list (any of / none of).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Condition {
    pub factor: u64,
    pub op: CondOp,
    #[serde(default)]
    pub values: Vec<Value>,
}

fn value_text(v: &Value) -> String {
    match v {
        Value::Num(x) => format::num(*x, 10),
        Value::Text(t) => t.clone(),
    }
}

/// Display text such as `T < 0` or `M = Freeze, Flash`.
pub fn format(p: &Project, c: &Condition) -> String {
    let Some(pi) = p.factor_index(c.factor) else { return String::new() };
    let vals: Vec<String> = c.values.iter().map(value_text).collect();
    format!("{} {} {}", p.factors[pi].symbol, c.op.text(), vals.join(", "))
}

/// Text such as `Temperature < 0`, or its negation `Temperature >= 0`.
pub fn format_named(p: &Project, c: &Condition, negate: bool) -> String {
    let Some(pi) = p.factor_index(c.factor) else { return String::new() };
    let op = if !negate {
        c.op
    } else {
        match c.op {
            CondOp::Lt => CondOp::Ge,
            CondOp::Le => CondOp::Gt,
            CondOp::Gt => CondOp::Le,
            CondOp::Ge => CondOp::Lt,
            CondOp::Eq => CondOp::Ne,
            CondOp::Ne => CondOp::Eq,
        }
    };
    let vals: Vec<String> = c.values.iter().map(value_text).collect();
    format!("{} {} {}", p.factors[pi].name, op.text(), vals.join(", "))
}

/// Display text of the condition of factor `fi` (empty without one).
pub fn describe(p: &Project, fi: usize) -> String {
    p.factors[fi].condition.as_ref().map(|c| format(p, c)).unwrap_or_default()
}

/// Splits `left op right` at the first comparison operator.
fn split_op(text: &str) -> Option<(&str, CondOp, &str)> {
    const OPS: [(&str, CondOp); 11] = [
        ("<=", CondOp::Le),
        (">=", CondOp::Ge),
        ("!=", CondOp::Ne),
        ("<>", CondOp::Ne),
        ("==", CondOp::Eq),
        ("≤", CondOp::Le),
        ("≥", CondOp::Ge),
        ("≠", CondOp::Ne),
        ("<", CondOp::Lt),
        (">", CondOp::Gt),
        ("=", CondOp::Eq),
    ];
    let first = text.char_indices().find(|(_, ch)| "<>=!≤≥≠".contains(*ch))?.0;
    let rest = &text[first..];
    let (tok, op) = OPS.iter().find(|(t, _)| rest.starts_with(t))?;
    Some((&text[..first], *op, &rest[tok.len()..]))
}

fn lookup(p: &Project, name: &str) -> Option<usize> {
    let fs = &p.factors;
    fs.iter()
        .position(|f| f.symbol == name)
        .or_else(|| fs.iter().position(|f| f.symbol.eq_ignore_ascii_case(name)))
        .or_else(|| fs.iter().position(|f| f.name.eq_ignore_ascii_case(name)))
}

/// Parses condition text for factor `fi`, e.g. `T < 0`, `Temp >= -5` or
/// `Method = Freeze, Flash`, and checks it against the project.
pub fn parse(p: &Project, fi: usize, text: &str) -> Result<Condition, String> {
    let (left, op, right) = split_op(text).ok_or("write a condition such as T < 0 or M = A, B")?;
    let left = left.trim();
    let pi = lookup(p, left).ok_or_else(|| format!("unknown factor '{left}'"))?;
    let parent = &p.factors[pi];
    let items: Vec<&str> = right.split([',', ';']).map(str::trim).filter(|s| !s.is_empty()).collect();
    if items.is_empty() {
        return Err(format!("enter a value of {} after {}", parent.symbol, op.text()));
    }
    if parent.is_categorical() && op.is_comparison() {
        return Err(format!("{} is categorical; use = or != with its levels", parent.symbol));
    }
    let values = match &parent.kind {
        FactorKind::Categorical { levels } => items
            .iter()
            .map(|t| {
                parent
                    .level_index(t)
                    .map(|i| Value::Text(levels[i].clone()))
                    .ok_or_else(|| format!("'{t}' is not a level of {} ({})", parent.name, levels.join(", ")))
            })
            .collect::<Result<Vec<_>, _>>()?,
        _ => items
            .iter()
            .map(|t| format::parse_num(t).map(Value::Num).ok_or_else(|| format!("'{t}' is not a number")))
            .collect::<Result<Vec<_>, _>>()?,
    };
    let c = Condition { factor: parent.id, op, values };
    check(p, fi, &c)?;
    check_extent(p, &c)?;
    Ok(c)
}

/// Structural checks: the parent exists, differs from the factor, creates
/// no cycle, and the operator and values suit the parent's type.
pub fn check(p: &Project, fi: usize, c: &Condition) -> Result<(), String> {
    let f = &p.factors[fi];
    if f.role == Role::Block {
        return Err("block factors cannot be conditional".into());
    }
    if f.is_mixture() {
        return Err("mixture components cannot be conditional".into());
    }
    let pi = p.factor_index(c.factor).ok_or("the condition refers to a deleted factor")?;
    if pi == fi {
        return Err("a factor cannot depend on itself".into());
    }
    let parent = &p.factors[pi];
    if parent.role == Role::Block {
        return Err("conditions cannot refer to block factors".into());
    }
    // Walks up the chain of parents; reaching `fi` again is a cycle.
    let mut cur = pi;
    for _ in 0..=p.factors.len() {
        match p.factors[cur].condition.as_ref().and_then(|c| p.factor_index(c.factor)) {
            Some(up) if up == fi => {
                return Err(format!("{} already depends on {}", parent.symbol, f.symbol));
            }
            Some(up) => cur = up,
            None => break,
        }
    }
    if c.values.is_empty() {
        return Err("the condition has no value".into());
    }
    match &parent.kind {
        FactorKind::Categorical { levels } => {
            if c.op.is_comparison() {
                return Err(format!("{} is categorical; use = or != with its levels", parent.symbol));
            }
            for v in &c.values {
                match v {
                    Value::Text(t) if levels.contains(t) => {}
                    v => return Err(format!("'{}' is not a level of {}", value_text(v), parent.name)),
                }
            }
        }
        FactorKind::Discrete { levels } => {
            if c.op.is_comparison() && c.values.len() != 1 {
                return Err(format!("{} takes a single value", c.op.text()));
            }
            if !c.op.is_comparison() {
                for v in &c.values {
                    let x = v.as_num().ok_or("discrete levels are numbers")?;
                    if !levels.iter().any(|l| (l - x).abs() <= 1e-9 * l.abs().max(1.0)) {
                        return Err(format!("{} is not a level of {}", format::num(x, 10), parent.name));
                    }
                }
            }
        }
        FactorKind::Continuous { .. } | FactorKind::Mixture { .. } => {
            if !c.op.is_comparison() {
                return Err(format!("{} is continuous; use <, <=, > or >=", parent.symbol));
            }
            if c.values.len() != 1 || c.values[0].as_num().is_none() {
                return Err(format!("{} takes a single number", c.op.text()));
            }
        }
    }
    Ok(())
}

/// Checks that the condition holds in part, but not all, of the parent's
/// range or levels.
fn check_extent(p: &Project, c: &Condition) -> Result<(), String> {
    let pi = p.factor_index(c.factor).ok_or("unknown factor")?;
    let parent = &p.factors[pi];
    let (always, never) = match &parent.kind {
        FactorKind::Categorical { levels } => {
            let n = levels.iter().filter(|l| c.values.iter().any(|v| matches!(v, Value::Text(t) if t == *l))).count();
            let hit = if c.op == CondOp::Eq { n } else { levels.len() - n };
            (hit == levels.len(), hit == 0)
        }
        FactorKind::Discrete { levels } => {
            let hit = levels.iter().filter(|&&l| holds_actual(c, l)).count();
            (hit == levels.len(), hit == 0)
        }
        _ => {
            let (lo, hi) = parent.range();
            let t = c.values[0].as_num().unwrap_or(0.0);
            let inside = t > lo && t < hi;
            let edge_ok = match c.op {
                CondOp::Le => t >= lo && t < hi,
                CondOp::Ge => t > lo && t <= hi,
                _ => inside,
            };
            if !edge_ok {
                return Err(format!(
                    "the value must lie inside the range of {} ({} to {})",
                    parent.symbol,
                    format::num(lo, 10),
                    format::num(hi, 10)
                ));
            }
            (false, false)
        }
    };
    if always {
        return Err(format!("the condition holds for every value of {}", parent.symbol));
    }
    if never {
        return Err(format!("the condition holds for no value of {}", parent.symbol));
    }
    Ok(())
}

fn holds_actual(c: &Condition, x: f64) -> bool {
    let eq = |v: &Value| v.as_num().is_some_and(|t| (t - x).abs() <= 1e-9 * t.abs().max(1.0));
    let t = c.values.first().and_then(|v| v.as_num()).unwrap_or(f64::NAN);
    match c.op {
        CondOp::Lt => x < t,
        CondOp::Le => x <= t,
        CondOp::Gt => x > t,
        CondOp::Ge => x >= t,
        CondOp::Eq => c.values.iter().any(eq),
        CondOp::Ne => !c.values.iter().any(eq),
    }
}

/// Removes conditions that no longer pass the structural checks (after a
/// factor is deleted or its type, levels or role change).
pub fn prune(p: &mut Project) {
    for fi in 0..p.factors.len() {
        if let Some(c) = p.factors[fi].condition.clone() {
            if check(p, fi, &c).is_err() {
                p.factors[fi].condition = None;
            }
        }
    }
}

/// Renames categorical levels of factor `parent_id` inside conditions.
pub fn rename_levels(p: &mut Project, parent_id: u64, old: &[String], new: &[String]) {
    for f in p.factors.iter_mut() {
        if let Some(c) = &mut f.condition {
            if c.factor != parent_id {
                continue;
            }
            for v in c.values.iter_mut() {
                if let Value::Text(t) = v {
                    if let Some(i) = old.iter().position(|o| o == t) {
                        if let Some(n) = new.get(i) {
                            *t = n.clone();
                        }
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Evaluation on coded points
// ---------------------------------------------------------------------------

const EPS: f64 = 1e-9;

#[derive(Clone, Debug)]
enum Test {
    Below(f64, bool),
    Above(f64, bool),
    Levels(Vec<usize>),
    Nums(Vec<f64>),
}

/// A condition translated to coded units of its parent.
#[derive(Clone, Debug)]
pub struct CodedCond {
    pub parent: usize,
    test: Test,
}

impl CodedCond {
    pub fn holds(&self, c: Coded) -> bool {
        match &self.test {
            Test::Below(t, incl) => {
                let x = c.num();
                if *incl { x <= t + EPS } else { x < t - EPS }
            }
            Test::Above(t, incl) => {
                let x = c.num();
                if *incl { x >= t - EPS } else { x > t + EPS }
            }
            Test::Levels(ls) => {
                let i = match c {
                    Coded::Level(i) => i,
                    Coded::Num(x) => x.round().max(0.0) as usize,
                };
                ls.contains(&i)
            }
            Test::Nums(vs) => vs.iter().any(|v| (v - c.num()).abs() <= 1e-6),
        }
    }
}

/// Coded conditions per factor index (None for unconditional factors and
/// for conditions that do not pass the structural checks).
pub fn coded_conditions(p: &Project) -> Vec<Option<CodedCond>> {
    (0..p.factors.len())
        .map(|fi| {
            let c = p.factors[fi].condition.as_ref()?;
            check(p, fi, c).ok()?;
            let pi = p.factor_index(c.factor)?;
            let parent = &p.factors[pi];
            let code_num = |x: f64| p.code(pi, &Value::Num(x)).map(|c| c.num());
            let test = match &parent.kind {
                FactorKind::Categorical { levels } => {
                    let sel: Vec<usize> = (0..levels.len())
                        .filter(|&i| c.values.iter().any(|v| matches!(v, Value::Text(t) if *t == levels[i])))
                        .collect();
                    Test::Levels(if c.op == CondOp::Eq { sel } else { (0..levels.len()).filter(|i| !sel.contains(i)).collect() })
                }
                FactorKind::Discrete { levels } if !c.op.is_comparison() => {
                    Test::Nums(levels.iter().filter(|&&l| holds_actual(c, l)).filter_map(|&l| code_num(l)).collect())
                }
                _ => {
                    let t = code_num(c.values[0].as_num()?)?;
                    match c.op {
                        CondOp::Lt => Test::Below(t, false),
                        CondOp::Le => Test::Below(t, true),
                        CondOp::Gt => Test::Above(t, false),
                        CondOp::Ge => Test::Above(t, true),
                        _ => return None,
                    }
                }
            };
            Some(CodedCond { parent: pi, test })
        })
        .collect()
}

/// Ancestors of factor `f` that take a single value wherever `f` applies.
/// Interactions of `f` with these factors duplicate the main effect of `f`.
pub fn fixed_ancestors(conds: &[Option<CodedCond>], f: usize) -> Vec<usize> {
    let mut out = Vec::new();
    let mut cur = f;
    for _ in 0..conds.len() {
        let Some(c) = &conds[cur] else { break };
        let single = match &c.test {
            Test::Levels(v) => v.len() == 1,
            Test::Nums(v) => v.len() == 1,
            _ => false,
        };
        if single {
            out.push(c.parent);
        }
        cur = c.parent;
    }
    out
}

pub fn any(conds: &[Option<CodedCond>]) -> bool {
    conds.iter().any(Option::is_some)
}

/// Activity of every factor at a coded point: `Some(true)` applies,
/// `Some(false)` does not apply, `None` is unknown because a parent value
/// is missing. A factor whose parent is inactive is inactive too.
pub fn activity(conds: &[Option<CodedCond>], pt: &[Option<Coded>]) -> Vec<Option<bool>> {
    let n = conds.len();
    let mut memo: Vec<Option<Option<bool>>> = vec![None; n];
    fn eval(f: usize, conds: &[Option<CodedCond>], pt: &[Option<Coded>], memo: &mut [Option<Option<bool>>], depth: usize) -> Option<bool> {
        if let Some(v) = memo[f] {
            return v;
        }
        let v = match &conds[f] {
            None => Some(true),
            // A cycle in a hand-edited file leaves the factor active.
            Some(_) if depth > conds.len() => Some(true),
            Some(c) => match eval(c.parent, conds, pt, memo, depth + 1) {
                Some(false) => Some(false),
                None => None,
                Some(true) => pt.get(c.parent).copied().flatten().map(|v| c.holds(v)),
            },
        };
        memo[f] = Some(v);
        v
    }
    (0..n).map(|f| eval(f, conds, pt, &mut memo, 0)).collect()
}

/// Clears the values of inactive factors in a coded point.
pub fn blank_inactive(conds: &[Option<CodedCond>], pt: &mut [Option<Coded>]) {
    if !any(conds) {
        return;
    }
    for (f, a) in activity(conds, pt).into_iter().enumerate() {
        if a == Some(false) {
            pt[f] = None;
        }
    }
}

/// Activity of every factor in run `ri`.
pub fn run_activity(p: &Project, conds: &[Option<CodedCond>], ri: usize) -> Vec<Option<bool>> {
    activity(conds, &p.coded_run(&p.runs[ri]))
}
