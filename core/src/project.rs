//! Project data model: factors, responses, runs and per-response model specs.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const FORMAT_VERSION: u32 = 1;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FactorKind {
    /// Real-valued factor on the interval [low, high].
    Continuous { low: f64, high: f64 },
    /// Numeric factor restricted to a finite set of values.
    Discrete { levels: Vec<f64> },
    /// Unordered named levels.
    Categorical { levels: Vec<String> },
    /// Mixture component (proportion of the mixture total) on [low, high].
    Mixture { low: f64, high: f64 },
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    /// Set by the experimenter during the experiment and in production.
    Controlled,
    /// Set during the experiment, but random in production (robust design).
    Noise,
    /// Not settable; measured and recorded for each run (covariate).
    Environmental,
    /// Blocking variable created by a design.
    Block,
}

impl Role {
    pub fn label(self) -> &'static str {
        match self {
            Role::Controlled => "Controlled",
            Role::Noise => "Noise",
            Role::Environmental => "Environmental",
            Role::Block => "Block",
        }
    }
    pub fn all() -> [Role; 4] {
        [Role::Controlled, Role::Noise, Role::Environmental, Role::Block]
    }
    pub fn from_label(s: &str) -> Option<Role> {
        let s = s.trim().to_ascii_lowercase();
        Role::all().into_iter().find(|r| r.label().to_ascii_lowercase().starts_with(&s) && !s.is_empty())
    }
    /// True for roles whose values a design assigns.
    pub fn is_designed(self) -> bool {
        matches!(self, Role::Controlled | Role::Noise)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Factor {
    pub id: u64,
    pub symbol: String,
    pub name: String,
    #[serde(default)]
    pub units: String,
    pub kind: FactorKind,
    pub role: Role,
    /// Applies the factor only when another factor meets this condition.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub condition: Option<crate::condition::Condition>,
}

/// Value of a factor in coded (model) units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Coded {
    Num(f64),
    Level(usize),
}

impl Coded {
    pub fn num(self) -> f64 {
        match self {
            Coded::Num(v) => v,
            Coded::Level(i) => i as f64,
        }
    }
}

/// Value of a factor in actual units, as stored in runs.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum Value {
    Num(f64),
    Text(String),
}

impl Value {
    pub fn as_num(&self) -> Option<f64> {
        match self {
            Value::Num(v) => Some(*v),
            Value::Text(_) => None,
        }
    }
}

impl Factor {
    pub fn is_numeric(&self) -> bool {
        !matches!(self.kind, FactorKind::Categorical { .. })
    }

    pub fn is_categorical(&self) -> bool {
        matches!(self.kind, FactorKind::Categorical { .. })
    }

    pub fn is_mixture(&self) -> bool {
        matches!(self.kind, FactorKind::Mixture { .. })
    }

    pub fn level_count(&self) -> usize {
        match &self.kind {
            FactorKind::Categorical { levels } => levels.len(),
            FactorKind::Discrete { levels } => levels.len(),
            _ => 0,
        }
    }

    pub fn cat_levels(&self) -> &[String] {
        match &self.kind {
            FactorKind::Categorical { levels } => levels,
            _ => &[],
        }
    }

    /// Lower and upper bound in actual units for numeric factors.
    pub fn range(&self) -> (f64, f64) {
        match &self.kind {
            FactorKind::Continuous { low, high } | FactorKind::Mixture { low, high } => (*low, *high),
            FactorKind::Discrete { levels } => {
                let lo = levels.iter().cloned().fold(f64::INFINITY, f64::min);
                let hi = levels.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
                if lo.is_finite() { (lo, hi) } else { (0.0, 1.0) }
            }
            FactorKind::Categorical { .. } => (0.0, 1.0),
        }
    }

    /// Linear coding `coded = (actual - center) / half` for numeric factors
    /// (excluding mixture components, which use pseudo-component coding).
    pub fn center_half(&self) -> (f64, f64) {
        let (lo, hi) = self.range();
        let half = (hi - lo) / 2.0;
        ((lo + hi) / 2.0, if half.abs() < 1e-300 { 1.0 } else { half })
    }

    pub fn level_index(&self, text: &str) -> Option<usize> {
        let t = text.trim();
        self.cat_levels()
            .iter()
            .position(|l| l == t)
            .or_else(|| self.cat_levels().iter().position(|l| l.eq_ignore_ascii_case(t)))
    }
}

/// Pseudo-component coding parameters for mixture factors.
#[derive(Clone, Debug, Default)]
pub struct MixtureCoding {
    pub total: f64,
    pub lower_sum: f64,
}

impl MixtureCoding {
    pub fn span(&self) -> f64 {
        let s = self.total - self.lower_sum;
        if s.abs() < 1e-300 { 1.0 } else { s }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum Goal {
    #[default]
    None,
    Maximize,
    Minimize,
    Target,
    InRange,
}

impl Goal {
    pub fn label(self) -> &'static str {
        match self {
            Goal::None => "None",
            Goal::Maximize => "Maximize",
            Goal::Minimize => "Minimize",
            Goal::Target => "Target",
            Goal::InRange => "In range",
        }
    }
    pub fn all() -> [Goal; 5] {
        [Goal::None, Goal::Maximize, Goal::Minimize, Goal::Target, Goal::InRange]
    }
    pub fn from_label(s: &str) -> Option<Goal> {
        let s = s.trim().to_ascii_lowercase();
        Goal::all().into_iter().find(|g| g.label().to_ascii_lowercase() == s)
            .or_else(|| Goal::all().into_iter().find(|g| !s.is_empty() && g.label().to_ascii_lowercase().starts_with(&s)))
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Response {
    pub id: u64,
    pub name: String,
    #[serde(default)]
    pub units: String,
    #[serde(default)]
    pub goal: Goal,
    #[serde(default)]
    pub lower: Option<f64>,
    #[serde(default)]
    pub target: Option<f64>,
    #[serde(default)]
    pub upper: Option<f64>,
    /// Desirability shape exponent.
    #[serde(default = "one")]
    pub weight: f64,
    /// Relative importance in the overall desirability (1..5).
    #[serde(default = "three")]
    pub importance: f64,
}

fn one() -> f64 {
    1.0
}
fn three() -> f64 {
    3.0
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    #[default]
    Planned,
    Done,
    Excluded,
}

impl RunStatus {
    pub fn label(self) -> &'static str {
        match self {
            RunStatus::Planned => "Planned",
            RunStatus::Done => "Done",
            RunStatus::Excluded => "Excluded",
        }
    }
    pub fn from_label(s: &str) -> Option<RunStatus> {
        let s = s.trim().to_ascii_lowercase();
        [RunStatus::Planned, RunStatus::Done, RunStatus::Excluded]
            .into_iter()
            .find(|r| !s.is_empty() && r.label().to_ascii_lowercase().starts_with(&s))
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Run {
    pub id: u64,
    pub std_order: u32,
    pub run_order: u32,
    #[serde(default)]
    pub point_type: String,
    #[serde(default)]
    pub status: RunStatus,
    /// Factor values keyed by factor id; an absent key is a missing value.
    #[serde(default)]
    pub settings: BTreeMap<u64, Value>,
    /// Response values keyed by response id.
    #[serde(default)]
    pub responses: BTreeMap<u64, f64>,
    #[serde(default)]
    pub note: String,
}

/// One model term: a product of factor powers, keyed by factor id.
/// The empty product is the intercept.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TermSpec(pub Vec<(u64, u32)>);

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(tag = "type", content = "lambda", rename_all = "snake_case")]
pub enum Transform {
    #[default]
    None,
    Ln,
    Log10,
    Sqrt,
    Inverse,
    Power(f64),
}

impl Transform {
    pub fn label(self) -> String {
        match self {
            Transform::None => "None".into(),
            Transform::Ln => "Ln".into(),
            Transform::Log10 => "Log10".into(),
            Transform::Sqrt => "Square root".into(),
            Transform::Inverse => "Inverse".into(),
            Transform::Power(l) => format!("Power {}", crate::format::num(l, 4)),
        }
    }

    pub fn parse(s: &str) -> Option<Transform> {
        let t = s.trim().to_ascii_lowercase();
        Some(match t.as_str() {
            "none" | "" => Transform::None,
            "ln" | "log" => Transform::Ln,
            "log10" => Transform::Log10,
            "sqrt" | "square root" => Transform::Sqrt,
            "inverse" | "1/y" => Transform::Inverse,
            _ => {
                let rest = t.strip_prefix("power").unwrap_or(&t).trim();
                Transform::Power(rest.parse().ok()?)
            }
        })
    }

    pub fn apply(self, y: f64) -> f64 {
        match self {
            Transform::None => y,
            Transform::Ln => if y > 0.0 { y.ln() } else { f64::NAN },
            Transform::Log10 => if y > 0.0 { y.log10() } else { f64::NAN },
            Transform::Sqrt => if y >= 0.0 { y.sqrt() } else { f64::NAN },
            Transform::Inverse => if y != 0.0 { 1.0 / y } else { f64::NAN },
            Transform::Power(l) => {
                if l == 0.0 {
                    if y > 0.0 { y.ln() } else { f64::NAN }
                } else if y > 0.0 || (l.fract() == 0.0 && y != 0.0) {
                    y.powf(l)
                } else if y == 0.0 && l > 0.0 {
                    0.0
                } else {
                    f64::NAN
                }
            }
        }
    }

    pub fn inverse(self, z: f64) -> f64 {
        match self {
            Transform::None => z,
            Transform::Ln => z.exp(),
            Transform::Log10 => 10f64.powf(z),
            Transform::Sqrt => if z >= 0.0 { z * z } else { 0.0 },
            Transform::Inverse => 1.0 / z,
            Transform::Power(l) => {
                if l == 0.0 {
                    z.exp()
                } else if z > 0.0 {
                    z.powf(1.0 / l)
                } else if l.fract() == 0.0 && (l as i64) % 2 != 0 {
                    -(-z).powf(1.0 / l)
                } else {
                    0.0
                }
            }
        }
    }

    /// Smallest interval that holds the back-transformed values of every z in
    /// [a, b]. Inverse and negative powers have a pole at z = 0, so an
    /// interval that contains 0 maps to an unbounded one; the other inverses
    /// are non-decreasing and map the end points.
    pub fn inverse_interval(self, a: f64, b: f64) -> (f64, f64) {
        let (a, b) = if a <= b { (a, b) } else { (b, a) };
        let pole = match self {
            Transform::Inverse => Some(true),
            Transform::Power(l) if l < 0.0 => Some(l.fract() == 0.0 && (l as i64) % 2 != 0),
            _ => None,
        };
        match pole {
            // Decreasing on each side of the pole; `odd`: negative z maps to
            // negative values (otherwise to 0).
            Some(odd) if a <= 0.0 && b >= 0.0 => {
                let lo = if a < 0.0 && odd { f64::NEG_INFINITY } else if odd { self.inverse(b) } else { 0.0 };
                let hi = if b > 0.0 { f64::INFINITY } else { self.inverse(a) };
                (lo.min(hi), hi)
            }
            Some(_) => (self.inverse(b), self.inverse(a)),
            None => (self.inverse(a), self.inverse(b)),
        }
    }

    /// True when the inverse transform reverses ordering.
    pub fn decreasing(self) -> bool {
        matches!(self, Transform::Inverse) || matches!(self, Transform::Power(l) if l < 0.0)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
pub struct ModelSpec {
    pub terms: Vec<TermSpec>,
    #[serde(default)]
    pub transform: Transform,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
pub struct DesignInfo {
    pub method: String,
    #[serde(default)]
    pub notes: Vec<String>,
    /// Model the design supports, used as the default analysis model.
    #[serde(default)]
    pub model: Vec<TermSpec>,
}

/// Settings for one environmental or noise factor during optimisation.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct EnvSetting {
    /// "fixed" or "random".
    pub mode: String,
    #[serde(default)]
    pub value: Option<Value>,
    #[serde(default)]
    pub mean: Option<f64>,
    #[serde(default)]
    pub sd: Option<f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
pub struct OptimizeSettings {
    #[serde(default)]
    pub env: BTreeMap<u64, EnvSetting>,
    /// Controlled factors held at a fixed value during optimisation.
    #[serde(default)]
    pub fixed: BTreeMap<u64, Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Project {
    pub format: u32,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub factors: Vec<Factor>,
    #[serde(default)]
    pub responses: Vec<Response>,
    #[serde(default)]
    pub runs: Vec<Run>,
    #[serde(default)]
    pub models: BTreeMap<u64, ModelSpec>,
    #[serde(default = "one")]
    pub mixture_total: f64,
    #[serde(default)]
    pub design: Option<DesignInfo>,
    #[serde(default)]
    pub optimize: OptimizeSettings,
    pub next_id: u64,
}

impl Default for Project {
    fn default() -> Self {
        Project {
            format: FORMAT_VERSION,
            title: String::new(),
            notes: String::new(),
            factors: Vec::new(),
            responses: Vec::new(),
            runs: Vec::new(),
            models: BTreeMap::new(),
            mixture_total: 1.0,
            design: None,
            optimize: OptimizeSettings::default(),
            next_id: 1,
        }
    }
}

impl Project {
    pub fn new_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    pub fn factor_index(&self, id: u64) -> Option<usize> {
        self.factors.iter().position(|f| f.id == id)
    }

    pub fn response_index(&self, id: u64) -> Option<usize> {
        self.responses.iter().position(|r| r.id == id)
    }

    pub fn mixture_coding(&self) -> MixtureCoding {
        let lower_sum = self
            .factors
            .iter()
            .filter_map(|f| match f.kind {
                FactorKind::Mixture { low, .. } => Some(low),
                _ => None,
            })
            .sum();
        MixtureCoding { total: self.mixture_total, lower_sum }
    }

    /// Converts an actual value to coded units.
    pub fn code(&self, fi: usize, v: &Value) -> Option<Coded> {
        let f = &self.factors[fi];
        match (&f.kind, v) {
            (FactorKind::Categorical { .. }, Value::Text(t)) => f.level_index(t).map(Coded::Level),
            (FactorKind::Categorical { levels }, Value::Num(n)) => {
                let t = crate::format::num(*n, 12);
                levels.iter().position(|l| *l == t).map(Coded::Level)
            }
            (FactorKind::Mixture { low, .. }, Value::Num(x)) => {
                let mc = self.mixture_coding();
                Some(Coded::Num((x - low) / mc.span()))
            }
            (_, Value::Num(x)) => {
                let (c, h) = f.center_half();
                Some(Coded::Num((x - c) / h))
            }
            (_, Value::Text(t)) => t.trim().parse::<f64>().ok().and_then(|x| self.code(fi, &Value::Num(x))),
        }
    }

    /// Converts a coded value back to actual units.
    pub fn decode(&self, fi: usize, c: Coded) -> Value {
        let f = &self.factors[fi];
        match (&f.kind, c) {
            (FactorKind::Categorical { levels }, Coded::Level(i)) => {
                Value::Text(levels.get(i).cloned().unwrap_or_default())
            }
            (FactorKind::Categorical { levels }, Coded::Num(x)) => {
                let i = (x.round().max(0.0) as usize).min(levels.len().saturating_sub(1));
                Value::Text(levels.get(i).cloned().unwrap_or_default())
            }
            (FactorKind::Mixture { low, .. }, c) => {
                let mc = self.mixture_coding();
                Value::Num(low + c.num() * mc.span())
            }
            (FactorKind::Discrete { levels }, c) => {
                let (ce, h) = f.center_half();
                let x = ce + c.num() * h;
                Value::Num(nearest(levels, x))
            }
            (_, c) => {
                let (ce, h) = f.center_half();
                Value::Num(ce + c.num() * h)
            }
        }
    }

    /// Coded values for every factor of a run (None for missing values).
    pub fn coded_run(&self, run: &Run) -> Vec<Option<Coded>> {
        (0..self.factors.len())
            .map(|fi| run.settings.get(&self.factors[fi].id).and_then(|v| self.code(fi, v)))
            .collect()
    }

    pub fn next_symbol(&self) -> String {
        for i in 0.. {
            let s = symbol_for(i);
            if !self.factors.iter().any(|f| f.symbol.eq_ignore_ascii_case(&s)) {
                return s;
            }
        }
        unreachable!()
    }

    pub fn renumber_std_order_if_empty(&mut self) {
        if self.runs.iter().all(|r| r.std_order == 0) {
            for (i, r) in self.runs.iter_mut().enumerate() {
                r.std_order = i as u32 + 1;
            }
        }
    }
}

pub fn nearest(levels: &[f64], x: f64) -> f64 {
    levels
        .iter()
        .cloned()
        .min_by(|a, b| (a - x).abs().partial_cmp(&(b - x).abs()).unwrap_or(std::cmp::Ordering::Equal))
        .unwrap_or(x)
}

/// Symbols A..Z, then AA, AB, ... (I and O are skipped as in common DOE
/// practice to avoid confusion with 1 and 0).
pub fn symbol_for(i: usize) -> String {
    const LETTERS: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ";
    let n = LETTERS.len();
    if i < n {
        (LETTERS[i] as char).to_string()
    } else {
        let a = (i / n) - 1;
        let b = i % n;
        format!("{}{}", symbol_for(a), LETTERS[b] as char)
    }
}
