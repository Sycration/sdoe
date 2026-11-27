//! Experimental design generators.

pub mod evaluate;
pub mod factorial;
pub mod mixture;
pub mod optimal;
pub mod rsm;
pub mod screening;
pub mod space;
pub mod taguchi;

use crate::model::terms::Term;
use crate::project::{Coded, FactorKind, Project, Role, Value};
use serde::Serialize;
use serde_json::{Map, Value as Json, json};

/// One generated run, in coded units, indexed by project factor index.
#[derive(Clone, Debug)]
pub struct GenRun {
    pub coded: Vec<Option<Coded>>,
    pub point_type: String,
    pub block: usize,
    /// Known environmental values (actual units) per factor index.
    pub env: Vec<Option<Value>>,
}

impl GenRun {
    pub fn new(nf: usize, point_type: &str) -> GenRun {
        GenRun { coded: vec![None; nf], point_type: point_type.into(), block: 0, env: vec![None; nf] }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Generated {
    pub method: String,
    pub runs: Vec<GenRun>,
    pub notes: Vec<String>,
    pub blocks: usize,
    pub model: Vec<Term>,
    /// Runs that already exist in the project and stay as they are (augmentation).
    pub keeps_existing: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct OptSpec {
    pub key: String,
    pub label: String,
    pub kind: &'static str,
    pub default: Json,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub choices: Vec<(String, String)>,
    pub tooltip: String,
}

impl OptSpec {
    pub fn int(key: &str, label: &str, default: i64, min: i64, max: i64, tip: &str) -> OptSpec {
        OptSpec {
            key: key.into(),
            label: label.into(),
            kind: "int",
            default: json!(default),
            min: Some(min as f64),
            max: Some(max as f64),
            choices: vec![],
            tooltip: tip.into(),
        }
    }
    pub fn float(key: &str, label: &str, default: f64, tip: &str) -> OptSpec {
        OptSpec { key: key.into(), label: label.into(), kind: "float", default: json!(default), min: None, max: None, choices: vec![], tooltip: tip.into() }
    }
    pub fn boolean(key: &str, label: &str, default: bool, tip: &str) -> OptSpec {
        OptSpec { key: key.into(), label: label.into(), kind: "bool", default: json!(default), min: None, max: None, choices: vec![], tooltip: tip.into() }
    }
    pub fn text(key: &str, label: &str, default: &str, tip: &str) -> OptSpec {
        OptSpec { key: key.into(), label: label.into(), kind: "text", default: json!(default), min: None, max: None, choices: vec![], tooltip: tip.into() }
    }
    pub fn choice(key: &str, label: &str, default: &str, choices: &[(&str, &str)], tip: &str) -> OptSpec {
        OptSpec {
            key: key.into(),
            label: label.into(),
            kind: "choice",
            default: json!(default),
            min: None,
            max: None,
            choices: choices.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect(),
            tooltip: tip.into(),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct MethodSpec {
    pub id: &'static str,
    pub name: &'static str,
    pub group: &'static str,
    pub description: String,
    pub options: Vec<OptSpec>,
    pub available: bool,
    pub reason: String,
}

/// Accessor for design options with defaults.
pub struct Opts<'a>(pub &'a Map<String, Json>);

impl Opts<'_> {
    pub fn int(&self, k: &str, d: i64) -> i64 {
        match self.0.get(k) {
            Some(Json::Number(n)) => n.as_i64().or_else(|| n.as_f64().map(|f| f.round() as i64)).unwrap_or(d),
            Some(Json::String(s)) => s.trim().parse().unwrap_or(d),
            _ => d,
        }
    }
    pub fn float(&self, k: &str, d: f64) -> f64 {
        match self.0.get(k) {
            Some(Json::Number(n)) => n.as_f64().unwrap_or(d),
            Some(Json::String(s)) => crate::format::parse_num(s).unwrap_or(d),
            _ => d,
        }
    }
    pub fn bool(&self, k: &str, d: bool) -> bool {
        match self.0.get(k) {
            Some(Json::Bool(b)) => *b,
            Some(Json::Number(n)) => n.as_f64().map(|v| v != 0.0).unwrap_or(d),
            Some(Json::String(s)) => matches!(s.as_str(), "true" | "1" | "yes"),
            _ => d,
        }
    }
    pub fn str(&self, k: &str, d: &str) -> String {
        match self.0.get(k) {
            Some(Json::String(s)) => s.clone(),
            Some(Json::Number(n)) => n.to_string(),
            Some(Json::Bool(b)) => b.to_string(),
            _ => d.to_string(),
        }
    }
}

/// Indices of factors a design assigns (controlled and noise).
pub fn designed(p: &Project) -> Vec<usize> {
    p.factors.iter().enumerate().filter(|(_, f)| f.role.is_designed()).map(|(i, _)| i).collect()
}

pub fn designed_non_mixture(p: &Project) -> Vec<usize> {
    designed(p).into_iter().filter(|&i| !p.factors[i].is_mixture()).collect()
}

pub fn env_factors(p: &Project) -> Vec<usize> {
    p.factors.iter().enumerate().filter(|(_, f)| f.role == Role::Environmental).map(|(i, _)| i).collect()
}

/// Coded value of the low/high extreme of a factor for two-level designs.
pub fn two_level(p: &Project, fi: usize, high: bool) -> Result<Coded, String> {
    let f = &p.factors[fi];
    match &f.kind {
        FactorKind::Continuous { .. } | FactorKind::Discrete { .. } => Ok(Coded::Num(if high { 1.0 } else { -1.0 })),
        FactorKind::Categorical { levels } if levels.len() == 2 => Ok(Coded::Level(if high { 1 } else { 0 })),
        FactorKind::Categorical { levels } => Err(format!(
            "{} has {} levels; two-level designs need exactly 2 (use a full factorial or an optimal design)",
            f.name,
            levels.len()
        )),
        FactorKind::Mixture { .. } => Err(format!("{} is a mixture component; use a mixture design", f.name)),
    }
}

/// Coded values a factor can take on a grid (used by factorial and optimal designs).
pub fn level_grid(p: &Project, fi: usize, continuous_levels: usize) -> Vec<Coded> {
    let f = &p.factors[fi];
    match &f.kind {
        FactorKind::Continuous { .. } | FactorKind::Mixture { .. } => {
            let n = continuous_levels.max(2);
            (0..n).map(|i| Coded::Num(-1.0 + 2.0 * i as f64 / (n - 1) as f64)).collect()
        }
        FactorKind::Discrete { levels } => {
            let (c, h) = f.center_half();
            let mut v: Vec<f64> = levels.iter().map(|l| (l - c) / h).collect();
            v.sort_by(|a, b| a.partial_cmp(b).unwrap());
            v.dedup();
            v.into_iter().map(Coded::Num).collect()
        }
        FactorKind::Categorical { levels } => (0..levels.len()).map(Coded::Level).collect(),
    }
}

/// Snaps coded numeric values of discrete factors to their nearest level.
pub fn snap_discrete(p: &Project, runs: &mut [GenRun]) -> bool {
    let mut changed = false;
    for (fi, f) in p.factors.iter().enumerate() {
        if let FactorKind::Discrete { .. } = f.kind {
            let grid: Vec<f64> = level_grid(p, fi, 0).into_iter().map(|c| c.num()).collect();
            for r in runs.iter_mut() {
                if let Some(Coded::Num(x)) = r.coded[fi] {
                    let s = crate::project::nearest(&grid, x);
                    if (s - x).abs() > 1e-12 {
                        changed = true;
                    }
                    r.coded[fi] = Some(Coded::Num(s));
                }
            }
        }
    }
    changed
}

/// Full two-level factorial in standard (Yates) order: first column alternates fastest.
pub fn two_level_full(k: usize) -> Vec<Vec<i8>> {
    (0..1usize << k).map(|i| (0..k).map(|j| if (i >> j) & 1 == 1 { 1 } else { -1 }).collect()).collect()
}

/// Center points for continuous factors, cycling categorical factors through
/// their levels when present. The result is empty when a discrete factor has
/// no level at its center, because the run would repeat a non-center run.
pub fn center_runs(p: &Project, factors: &[usize], count: usize, nf: usize) -> Vec<GenRun> {
    let has_center = factors.iter().all(|&fi| match &p.factors[fi].kind {
        FactorKind::Discrete { .. } => level_grid(p, fi, 3).iter().any(|c| c.num().abs() < 1e-9),
        _ => true,
    });
    if !has_center {
        return Vec::new();
    }
    let mut out = Vec::new();
    for i in 0..count {
        let mut r = GenRun::new(nf, "center");
        for &fi in factors {
            let g = level_grid(p, fi, 3);
            r.coded[fi] = Some(match &p.factors[fi].kind {
                FactorKind::Categorical { .. } => g[i % g.len()],
                FactorKind::Discrete { .. } => {
                    let nums: Vec<f64> = g.iter().map(|c| c.num()).collect();
                    Coded::Num(crate::project::nearest(&nums, 0.0))
                }
                _ => Coded::Num(0.0),
            });
        }
        out.push(r);
    }
    out
}

pub fn replicate(runs: Vec<GenRun>, reps: usize, block_by_rep: bool) -> Vec<GenRun> {
    let mut out = Vec::new();
    for rep in 0..reps.max(1) {
        for r in &runs {
            let mut r = r.clone();
            if block_by_rep {
                r.block = rep;
            }
            out.push(r);
        }
    }
    out
}

/// Notes about environmental factors common to every design.
pub fn env_notes(p: &Project, notes: &mut Vec<String>) {
    let env = env_factors(p);
    if !env.is_empty() {
        let names: Vec<String> = env.iter().map(|&i| p.factors[i].name.clone()).collect();
        notes.push(format!(
            "Environmental factors ({}) are not set by the design; record their measured values for each run. The analysis includes them as covariates.",
            names.join(", ")
        ));
    }
}

/// Method catalogue with options tailored to the project.
pub fn methods(p: &Project) -> Vec<MethodSpec> {
    let d = designed(p);
    let dn = designed_non_mixture(p);
    let mix: Vec<usize> = d.iter().cloned().filter(|&i| p.factors[i].is_mixture()).collect();
    let k = dn.len();
    let cats_ok_2 = dn.iter().all(|&i| match &p.factors[i].kind {
        FactorKind::Categorical { levels } => levels.len() == 2,
        _ => true,
    });
    let numeric: Vec<usize> = dn.iter().cloned().filter(|&i| p.factors[i].is_numeric()).collect();
    let avail = |ok: bool, reason: &str| (ok, if ok { String::new() } else { reason.to_string() });
    let mut out = Vec::new();

    let (a, r) = avail(!dn.is_empty() && mix.is_empty(), "Requires controlled or noise factors and no mixture components.");
    out.push(MethodSpec {
        id: "full_factorial",
        name: "Full factorial",
        group: "Factorial",
        description: "Every combination of factor levels. Handles any mix of continuous, discrete and categorical factors.".into(),
        options: vec![
            OptSpec::int("levels", "Levels per continuous factor", 2, 2, 7, "Equally spaced levels between low and high"),
            OptSpec::int("center_points", "Center points", 0, 0, 50, "Added once per categorical level combination cycle"),
            OptSpec::int("replicates", "Replicates", 1, 1, 20, ""),
            OptSpec::boolean("block_replicates", "Block by replicate", false, "Each replicate becomes a block"),
        ],
        available: a,
        reason: r,
    });

    let (a, r) = avail(k >= 3 && mix.is_empty() && cats_ok_2, "Requires at least 3 two-level factors (categorical factors need exactly 2 levels).");
    let mut run_choices = vec![("auto".to_string(), "Smallest with minimum resolution".to_string())];
    let mut n = 2;
    while (1usize << n) <= (1usize << k.min(12)) && n <= 12 {
        if n < k && (1usize << n) > k {
            run_choices.push(((1usize << n).to_string(), format!("{} runs (1/{} fraction)", 1usize << n, 1usize << (k - n))));
        }
        n += 1;
    }
    out.push(MethodSpec {
        id: "fractional_factorial",
        name: "Fractional factorial (2-level)",
        group: "Factorial",
        description: "Regular 2^(k-p) fraction with minimum-aberration generators. Shows resolution and alias structure.".into(),
        options: vec![
            OptSpec {
                key: "runs".into(),
                label: "Runs".into(),
                kind: "choice",
                default: json!("auto"),
                min: None,
                max: None,
                choices: run_choices,
                tooltip: String::new(),
            },
            OptSpec::choice("resolution", "Minimum resolution", "4", &[("3", "III"), ("4", "IV"), ("5", "V")], "Used when runs are chosen automatically"),
            OptSpec::text("generators", "Generators", "", "Optional, e.g. D=ABC, E=-ABD"),
            OptSpec::int("center_points", "Center points", 0, 0, 50, ""),
            OptSpec::int("replicates", "Replicates", 1, 1, 20, ""),
            OptSpec::boolean("foldover", "Fold over", false, "Adds the mirror image of the fraction to de-alias main effects"),
        ],
        available: a,
        reason: r,
    });

    let (a, r) = avail(k >= 2 && mix.is_empty() && cats_ok_2, "Requires at least 2 two-level factors.");
    out.push(MethodSpec {
        id: "plackett_burman",
        name: "Plackett-Burman",
        group: "Screening",
        description: "Two-level orthogonal screening design for main effects in a multiple of 4 runs.".into(),
        options: vec![
            OptSpec::int("runs", "Runs (0 = smallest)", 0, 0, 96, "A supported multiple of 4"),
            OptSpec::int("center_points", "Center points", 0, 0, 50, ""),
        ],
        available: a,
        reason: r,
    });

    let dsd_ok = k >= 3 && mix.is_empty() && dn.iter().all(|&i| match &p.factors[i].kind {
        FactorKind::Categorical { levels } => levels.len() == 2,
        _ => true,
    });
    let (a, r) = avail(dsd_ok, "Requires at least 3 continuous or two-level categorical factors.");
    out.push(MethodSpec {
        id: "dsd",
        name: "Definitive screening",
        group: "Screening",
        description: "Three-level screening design: main effects are clear of two-factor interactions, and quadratic effects are estimable.".into(),
        options: vec![OptSpec::int("extra_center", "Extra center points", 0, 0, 10, "")],
        available: a,
        reason: r,
    });

    let ncont = numeric.len();
    let (a, r) = avail(ncont >= 2 && mix.is_empty(), "Requires at least 2 numeric factors.");
    let def_center = match ncont {
        2 => 5,
        3 => 6,
        4 => 7,
        5 => 6,
        6 => 9,
        _ => 10,
    };
    out.push(MethodSpec {
        id: "ccd",
        name: "Central composite",
        group: "Response surface",
        description: "Factorial core plus axial and center points for quadratic models. Categorical factors are crossed with the design.".into(),
        options: vec![
            OptSpec::choice(
                "alpha",
                "Axial distance",
                "rotatable",
                &[
                    ("rotatable", "Rotatable"),
                    ("orthogonal", "Orthogonal"),
                    ("face", "Face-centered (α = 1)"),
                    ("spherical", "Spherical (α = √k)"),
                    ("practical", "Practical (α = k^¼)"),
                ],
                "",
            ),
            OptSpec::boolean("inscribed", "Inscribed (keep all points within ranges)", false, ""),
            OptSpec::choice("fraction", "Factorial core", "auto", &[("auto", "Resolution V fraction when smaller"), ("full", "Full factorial")], ""),
            OptSpec::int("center_points", "Center points", def_center, 0, 50, ""),
            OptSpec::choice("blocks", "Blocks", "1", &[("1", "None"), ("2", "Factorial / axial")], ""),
        ],
        available: a,
        reason: r,
    });

    let (a, r) = avail(ncont >= 3 && mix.is_empty(), "Requires at least 3 numeric factors.");
    out.push(MethodSpec {
        id: "box_behnken",
        name: "Box-Behnken",
        group: "Response surface",
        description: "Three-level response surface design without extreme corner points.".into(),
        options: vec![OptSpec::int("center_points", "Center points", if ncont <= 5 { 5 } else { 6 }, 0, 50, "")],
        available: a,
        reason: r,
    });

    let env = env_factors(p);
    let mut opt_options = vec![
        OptSpec::choice("criterion", "Criterion", "D", &[("D", "D-optimal (parameter estimation)"), ("I", "I-optimal (prediction)"), ("A", "A-optimal")], ""),
        OptSpec::choice(
            "model",
            "Model",
            if k + mix.len() <= 6 { "quadratic" } else { "2fi" },
            &[("linear", "Linear"), ("2fi", "Two-factor interactions"), ("quadratic", "Quadratic"), ("cubic", "Cubic"), ("custom", "Custom formula")],
            "",
        ),
        OptSpec::text("formula", "Custom formula", "", "Used when the model is Custom, e.g. (A+B+C)^2 + A^2"),
        OptSpec::int("runs", "Runs (0 = model terms + 6)", 0, 0, 1000, ""),
        OptSpec::boolean("augment", "Augment existing runs", false, "Keeps the current runs and adds new runs that complement them"),
        OptSpec::int("starts", "Random starts", 20, 1, 500, ""),
        OptSpec::int("seed", "Seed", 1, 0, 1_000_000_000, ""),
    ];
    if !env.is_empty() {
        opt_options.push(OptSpec::boolean(
            "env_interactions",
            "Environment × factor interactions",
            false,
            "Adds interactions between environmental and designed factors to the model",
        ));
        for &e in &env {
            opt_options.push(OptSpec::text(
                &format!("env_{}", p.factors[e].id),
                &format!("Known values of {}", p.factors[e].name),
                "",
                "Optional list of forecast or scheduled values, one per run (separated by commas, spaces or new lines). The design then plans the other factors around them.",
            ));
        }
    }
    let (a, r) = avail(!d.is_empty(), "Requires controlled or noise factors.");
    out.push(MethodSpec {
        id: "optimal",
        name: "Optimal (custom)",
        group: "Optimal",
        description: "Coordinate-exchange optimal design for any combination of continuous, discrete, categorical and mixture factors, any model and any run count. Plans around known environmental values and augments existing runs.".into(),
        options: opt_options,
        available: a,
        reason: r,
    });

    let (a, r) = avail(!dn.is_empty() && mix.is_empty(), "Requires controlled or noise factors.");
    out.push(MethodSpec {
        id: "lhs",
        name: "Latin hypercube",
        group: "Space filling",
        description: "Space-filling design for computer experiments or flexible modelling. Discrete and categorical factors are balanced.".into(),
        options: vec![
            OptSpec::int("runs", "Runs", (10 * k.max(1)) as i64, 2, 5000, ""),
            OptSpec::int("iterations", "Maximin iterations", 2000, 0, 200_000, ""),
            OptSpec::int("seed", "Seed", 1, 0, 1_000_000_000, ""),
        ],
        available: a,
        reason: r,
    });

    let (a, r) = avail(mix.len() >= 2, "Requires at least 2 mixture components.");
    out.push(MethodSpec {
        id: "simplex_lattice",
        name: "Simplex lattice",
        group: "Mixture",
        description: "Mixture blends on a regular lattice of the (pseudo-component) simplex.".into(),
        options: vec![
            OptSpec::int("degree", "Lattice degree", 2, 1, 6, ""),
            OptSpec::boolean("centroid", "Add overall centroid", true, ""),
            OptSpec::boolean("axial", "Add axial check blends", false, ""),
            OptSpec::int("replicates", "Replicates", 1, 1, 10, ""),
        ],
        available: a,
        reason: r.clone(),
    });
    out.push(MethodSpec {
        id: "simplex_centroid",
        name: "Simplex centroid",
        group: "Mixture",
        description: "All vertex, edge, face and overall centroids; supports the special cubic model.".into(),
        options: vec![
            OptSpec::boolean("axial", "Add axial check blends", false, ""),
            OptSpec::int("replicates", "Replicates", 1, 1, 10, ""),
        ],
        available: a,
        reason: r,
    });

    let (a, r) = avail(!dn.is_empty() && mix.is_empty(), "Requires controlled or noise factors.");
    out.push(MethodSpec {
        id: "taguchi",
        name: "Taguchi (robust parameter)",
        group: "Robust",
        description: "Orthogonal-array inner design for controlled factors crossed with an outer design for noise factors. Analyse with signal-to-noise ratios.".into(),
        options: vec![
            OptSpec::choice("levels", "Levels per continuous factor", "2", &[("2", "2"), ("3", "3")], ""),
            OptSpec::choice(
                "inner",
                "Inner array",
                "auto",
                &[
                    ("auto", "Smallest that fits"),
                    ("L4", "L4"),
                    ("L8", "L8"),
                    ("L9", "L9"),
                    ("L12", "L12"),
                    ("L16", "L16"),
                    ("L16b", "L16 (4-level)"),
                    ("L18", "L18"),
                    ("L25", "L25"),
                    ("L27", "L27"),
                    ("L32", "L32"),
                ],
                "",
            ),
            OptSpec::choice("outer", "Outer design", "auto", &[("auto", "Full factorial when small, else array"), ("full", "Full factorial"), ("array", "Orthogonal array")], ""),
        ],
        available: a,
        reason: r,
    });
    out
}

/// Checks every numeric option against the range of its option specification.
fn check_options(p: &Project, method: &str, opts: &Map<String, Json>) -> Result<(), String> {
    let Some(spec) = methods(p).into_iter().find(|m| m.id == method) else { return Ok(()) };
    for s in spec.options.iter().filter(|s| s.kind == "int") {
        let v = match opts.get(&s.key) {
            Some(Json::Number(n)) => n.as_f64(),
            Some(Json::String(t)) => t.trim().parse::<f64>().ok(),
            _ => None,
        };
        let Some(v) = v else { continue };
        let (lo, hi) = (s.min.unwrap_or(f64::MIN), s.max.unwrap_or(f64::MAX));
        if !v.is_finite() || v.round() < lo || v.round() > hi {
            return Err(format!("option '{}' must be between {} and {}, not {}", s.key, lo, hi, v));
        }
    }
    Ok(())
}

/// Checks that every designed factor has a range the design can vary.
fn check_factors(p: &Project) -> Result<(), String> {
    for i in designed(p) {
        let f = &p.factors[i];
        match &f.kind {
            FactorKind::Categorical { levels } if levels.len() < 2 => {
                return Err(format!("{} is categorical with {} level(s); it needs at least 2", f.name, levels.len()));
            }
            FactorKind::Discrete { levels } => {
                let mut v = levels.clone();
                v.sort_by(|a, b| a.total_cmp(b));
                v.dedup();
                if v.len() < 2 {
                    return Err(format!("{} is discrete with fewer than 2 distinct levels; it needs at least 2", f.name));
                }
            }
            FactorKind::Continuous { low, high } | FactorKind::Mixture { low, high } if low >= high => {
                return Err(format!("{} has a low bound ({low}) that is not below its high bound ({high})", f.name));
            }
            _ => {}
        }
    }
    Ok(())
}

/// Generates a design.
pub fn generate(p: &Project, method: &str, opts: &Map<String, Json>) -> Result<Generated, String> {
    check_options(p, method, opts)?;
    check_factors(p)?;
    let o = Opts(opts);
    let mut g = match method {
        "full_factorial" => factorial::full_factorial(p, &o),
        "fractional_factorial" => factorial::fractional(p, &o),
        "plackett_burman" => screening::plackett_burman(p, &o),
        "dsd" => screening::definitive_screening(p, &o),
        "ccd" => rsm::ccd(p, &o),
        "box_behnken" => rsm::box_behnken(p, &o),
        "optimal" => optimal::generate(p, &o),
        "lhs" => space::latin_hypercube(p, &o),
        "simplex_lattice" => mixture::lattice(p, &o),
        "simplex_centroid" => mixture::centroid(p, &o),
        "taguchi" => taguchi::crossed(p, &o),
        _ => Err(format!("unknown design method '{method}'")),
    }?;
    g.method = methods(p).into_iter().find(|m| m.id == method).map(|m| m.name.to_string()).unwrap_or_default();
    if g.blocks == 0 {
        g.blocks = g.runs.iter().map(|r| r.block + 1).max().unwrap_or(1);
    }
    env_notes(p, &mut g.notes);
    apply_conditions(p, method, &mut g);
    Ok(g)
}

/// Clears the values of conditional factors in runs where they do not
/// apply. In full factorials, runs that become copies of other runs this
/// way are dropped, so every valid combination appears once per replicate.
fn apply_conditions(p: &Project, method: &str, g: &mut Generated) {
    use crate::condition::{activity, coded_conditions, describe};
    let conds = coded_conditions(p);
    if !crate::condition::any(&conds) {
        return;
    }
    let nf = p.factors.len();
    let mut cleared: Vec<(Vec<Option<Coded>>, GenRun)> = Vec::with_capacity(g.runs.len());
    for r in g.runs.drain(..) {
        let original = r.coded.clone();
        let mut r = r;
        let mut pt = r.coded.clone();
        for f in 0..nf {
            if pt[f].is_none() {
                pt[f] = r.env[f].as_ref().and_then(|v| p.code(f, v));
            }
        }
        for (f, a) in activity(&conds, &pt).into_iter().enumerate() {
            if a == Some(false) {
                r.coded[f] = None;
                r.env[f] = None;
            }
        }
        cleared.push((original, r));
    }
    let same = |a: &GenRun, b: &GenRun| a.coded == b.coded && a.block == b.block && a.point_type == b.point_type && a.env == b.env;
    let mut kept: Vec<(Vec<Option<Coded>>, GenRun)> = Vec::with_capacity(cleared.len());
    let mut dropped = 0;
    for (orig, r) in cleared {
        let duplicate = method == "full_factorial" && kept.iter().any(|(o, k)| same(k, &r) && *o != orig);
        if duplicate {
            dropped += 1;
        } else {
            kept.push((orig, r));
        }
    }
    g.runs = kept.into_iter().map(|(_, r)| r).collect();
    let n = g.runs.len();
    for f in designed(p) {
        if conds[f].is_none() {
            continue;
        }
        let set = g.runs.iter().filter(|r| r.coded[f].is_some()).count();
        let name = &p.factors[f].name;
        let cond = describe(p, f);
        g.notes.push(if set == 0 {
            format!("{name} applies only when {cond}, which holds in none of the runs; {name} is not set in this design.")
        } else {
            format!("{name} applies only when {cond}: set in {set} of {n} runs and left blank in the others.")
        });
    }
    if dropped > 0 {
        g.notes.push(format!("{dropped} combinations differ only in factors that do not apply and were left out."));
    }
    if !matches!(method, "optimal" | "full_factorial") {
        g.notes.push("In this design type, the points are placed without regard to the conditions. Optimal designs are built with the conditions taken into account.".into());
    }
}
