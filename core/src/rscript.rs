//! R script export. The script holds the runs in actual units, codes the
//! factors as the engine does, fits each response's model with `lm()` and
//! computes the statistics of `analysis.fit`. Its last section holds the
//! engine's own results and compares them with the R results.

use crate::analysis::center_point;
use crate::condition::{CondOp, coded_conditions, run_activity};
use crate::model::fit::{AnovaRow, Fit};
use crate::model::terms::{ColPart, format_formula};
use crate::project::{Coded, FactorKind, Project, Transform, Value};
use crate::results::fit_for;
use crate::stats;
use std::fmt::Write;

const RESERVED: &[&str] = &[
    "if", "else", "repeat", "while", "function", "for", "next", "break", "in", "TRUE", "FALSE", "NULL", "Inf", "NaN", "NA",
    "NA_integer_", "NA_real_", "NA_complex_", "NA_character_",
];

/// R identifier for `raw`: valid ASCII identifiers stay as they are, other
/// text becomes letters, digits and `_`. Reserved words gain a trailing `_`,
/// and names already in `taken` (case-insensitive) gain a number.
fn ident(raw: &str, taken: &mut Vec<String>) -> String {
    let valid = raw.starts_with(|c: char| c.is_ascii_alphabetic()) && raw.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
    let mut s = if valid {
        raw.to_string()
    } else {
        let mut s = String::new();
        for c in raw.chars() {
            if c.is_ascii_alphanumeric() {
                s.push(c);
            } else if !s.ends_with('_') {
                s.push('_');
            }
        }
        let mut s = s.trim_matches('_').to_string();
        if !s.starts_with(|c: char| c.is_ascii_alphabetic()) {
            s.insert(0, 'X');
        }
        s
    };
    if RESERVED.contains(&s.as_str()) {
        s.push('_');
    }
    let base = s.clone();
    let mut k = 2;
    while taken.iter().any(|t| t.eq_ignore_ascii_case(&s)) {
        s = format!("{base}_{k}");
        k += 1;
    }
    taken.push(s.clone());
    s
}

/// Number as R source text, exact to the last bit.
fn num(x: f64) -> String {
    if x.is_nan() {
        "NA".into()
    } else if x.is_infinite() {
        if x > 0.0 { "Inf".into() } else { "-Inf".into() }
    } else if x == 0.0 {
        "0".into()
    } else if (1e-4..1e15).contains(&x.abs()) {
        format!("{x}")
    } else {
        format!("{x:e}")
    }
}

/// R string literal.
fn string(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn strings(v: &[String]) -> String {
    format!("c({})", v.iter().map(|s| string(s)).collect::<Vec<_>>().join(", "))
}

/// Single-line comment text.
fn comment(s: &str) -> String {
    s.replace(['\n', '\r'], " ")
}

/// Section heading comment, padded to 80 columns.
fn heading(out: &mut String, title: &str) {
    let t = format!("# {} ", comment(title));
    let pad = 80usize.saturating_sub(t.chars().count()).max(4);
    let _ = writeln!(out, "\n{t}{}\n", "-".repeat(pad));
}

/// Joins items with `sep`, breaking lines before 80 columns.
fn wrap(first: &str, items: &[String], sep: &str, indent: usize, last: &str) -> String {
    let mut out = first.to_string();
    let mut col = out.chars().count();
    for (i, it) in items.iter().enumerate() {
        let piece = if i + 1 < items.len() { format!("{it}{sep}") } else { format!("{it}{last}") };
        let w = piece.chars().count();
        if i > 0 && col + w > 80 {
            out = out.trim_end().to_string();
            out.push('\n');
            out.push_str(&" ".repeat(indent));
            col = indent;
        }
        out.push_str(&piece);
        col += w;
    }
    out
}

/// Formula pieces of a transform: the left-hand side, the expression that
/// must be finite (on the runs table) and the inverse as an R function.
fn transform_parts(t: Transform, y: &str) -> (String, Option<String>, String, String) {
    let ln = || (format!("log({y})"), Some(format!("log(runs${y})")), "exp".to_string(), "natural logarithm".to_string());
    match t {
        Transform::None => (y.to_string(), None, "identity".into(), String::new()),
        Transform::Ln => ln(),
        Transform::Power(l) if l == 0.0 => ln(),
        Transform::Log10 => (format!("log10({y})"), Some(format!("log10(runs${y})")), "function(z) 10^z".into(), "base-10 logarithm".into()),
        Transform::Sqrt => (format!("sqrt({y})"), Some(format!("sqrt(runs${y})")), "function(z) pmax(z, 0)^2".into(), "square root".into()),
        Transform::Inverse => (format!("I(1 / {y})"), Some(format!("1 / runs${y}")), "function(z) 1 / z".into(), "inverse".into()),
        Transform::Power(l) => {
            let e = if l < 0.0 { format!("({})", num(l)) } else { num(l) };
            // As in sdoe: z <= 0 gives the odd root for odd integer powers, 0 otherwise.
            let neg = if power_is_odd(l) { format!("-(-z)^(1 / {e})") } else { "0".to_string() };
            (
                format!("I({y}^{e})"),
                Some(format!("runs${y}^{e}")),
                format!("function(z) ifelse(z > 0, z^(1 / {e}), {neg})"),
                format!("power {}", num(l)),
            )
        }
    }
}

fn power_is_odd(l: f64) -> bool {
    l.fract() == 0.0 && (l as i64) % 2 != 0
}

/// `analyse()` arguments for transforms whose inverse has a pole at 0
/// (inverse and negative powers): intervals that contain 0 back-transform to
/// unbounded ones.
fn pole_args(t: Transform) -> Vec<String> {
    match t {
        Transform::Inverse => vec!["pole = TRUE".into(), "odd = TRUE".into()],
        Transform::Power(l) if l < 0.0 => {
            vec!["pole = TRUE".into(), format!("odd = {}", if power_is_odd(l) { "TRUE" } else { "FALSE" })]
        }
        _ => vec![],
    }
}

struct Names {
    factors: Vec<String>,
    responses: Vec<String>,
    run: String,
    std: String,
    kind: String,
    status: String,
}

fn names(p: &Project) -> Names {
    let mut taken = Vec::new();
    let factors = p.factors.iter().map(|f| ident(&f.symbol, &mut taken)).collect();
    let run = ident("Run", &mut taken);
    let std = ident("Std", &mut taken);
    let kind = ident("Type", &mut taken);
    let status = ident("Status", &mut taken);
    let responses = p.responses.iter().map(|r| ident(&r.name, &mut taken)).collect();
    Names { factors, responses, run, std, kind, status }
}

/// R variable of a factor power in a formula: `A` or `I(A^2)`.
fn variable(n: &Names, f: usize, pw: u32) -> String {
    if pw > 1 { format!("I({}^{pw})", n.factors[f]) } else { n.factors[f].clone() }
}

/// Text of a factor value in actual units as the engine reads it, or None
/// for a missing or unreadable value.
fn cell(p: &Project, fi: usize, v: Option<&Value>) -> Option<String> {
    let v = v?;
    let c = p.code(fi, v)?;
    Some(match (&p.factors[fi].kind, v) {
        (FactorKind::Categorical { levels }, _) => match c {
            Coded::Level(i) => levels.get(i)?.clone(),
            Coded::Num(_) => return None,
        },
        (_, Value::Num(x)) => num(*x),
        (_, Value::Text(t)) => num(t.trim().parse::<f64>().ok()?),
    })
}

fn csv_field(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "\"\""))
}

fn runs_table(out: &mut String, p: &Project, n: &Names) {
    let conds = coded_conditions(p);
    let mut header = vec![n.run.clone(), n.std.clone(), n.kind.clone(), n.status.clone()];
    let mut classes = vec!["integer", "integer", "character", "character"];
    for (fi, f) in p.factors.iter().enumerate() {
        header.push(n.factors[fi].clone());
        classes.push(if f.is_categorical() { "character" } else { "numeric" });
    }
    for r in &n.responses {
        header.push(r.clone());
        classes.push("numeric");
    }
    let mut rows: Vec<Vec<String>> = vec![header];
    for (ri, run) in p.runs.iter().enumerate() {
        let act = run_activity(p, &conds, ri);
        let mut row = vec![run.run_order.to_string(), run.std_order.to_string(), csv_field(&run.point_type), csv_field(run.status.label())];
        for (fi, f) in p.factors.iter().enumerate() {
            let text = if act[fi] == Some(false) { None } else { cell(p, fi, run.settings.get(&f.id)) };
            row.push(match text {
                None => "NA".into(),
                Some(t) if f.is_categorical() => csv_field(&t),
                Some(t) => t,
            });
        }
        for r in &p.responses {
            row.push(run.responses.get(&r.id).map(|v| num(*v)).unwrap_or_else(|| "NA".into()));
        }
        rows.push(row);
    }
    let cols = rows[0].len();
    let width: Vec<usize> = (0..cols).map(|c| rows.iter().map(|r| r[c].chars().count()).max().unwrap_or(0)).collect();
    let mut body = String::new();
    for r in &rows {
        let mut line = String::new();
        for (c, v) in r.iter().enumerate() {
            if c + 1 < cols {
                let _ = write!(line, "{v},{}", " ".repeat(width[c] - v.chars().count() + 1));
            } else {
                line.push_str(v);
            }
        }
        body.push_str(line.trim_end());
        body.push('\n');
    }
    // A raw string delimiter that does not occur in the table.
    let mut dashes = String::new();
    while body.contains(&format!("){dashes}\"")) {
        dashes.push('-');
    }
    let classes: Vec<String> = classes.iter().map(|c| string(c)).collect();
    out.push_str("runs <- read.csv(\n  strip.white = TRUE,\n");
    out.push_str(&wrap("  colClasses = c(", &classes, ", ", 4, "),\n"));
    let _ = write!(out, "  text = r\"{dashes}(\n{body}){dashes}\"\n)\n");
}

/// R expression for the condition of factor `fi` on data frame `d`, in
/// actual units, including the activity of its parent.
fn condition_expr(p: &Project, n: &Names, fi: usize, conditional: &[bool]) -> Option<String> {
    let c = p.factors[fi].condition.as_ref()?;
    let pi = p.factor_index(c.factor)?;
    let parent = &p.factors[pi];
    let x = format!("d${}", n.factors[pi]);
    let list = || match &parent.kind {
        FactorKind::Categorical { .. } => strings(&c.values.iter().map(|v| match v {
            Value::Text(t) => t.clone(),
            Value::Num(x) => num(*x),
        }).collect::<Vec<_>>()),
        _ => format!("c({})", c.values.iter().filter_map(|v| v.as_num()).map(num).collect::<Vec<_>>().join(", ")),
    };
    let first = || c.values.first().and_then(|v| v.as_num()).map(num).unwrap_or_else(|| "NA".into());
    let test = match c.op {
        CondOp::Lt => format!("{x} < {}", first()),
        CondOp::Le => format!("{x} <= {}", first()),
        CondOp::Gt => format!("{x} > {}", first()),
        CondOp::Ge => format!("{x} >= {}", first()),
        CondOp::Eq => format!("is_in({x}, {})", list()),
        CondOp::Ne => format!("!is_in({x}, {})", list()),
    };
    Some(if conditional[pi] { format!("ifelse(a_{}, {test}, FALSE)", n.factors[pi]) } else { test })
}

fn coding(out: &mut String, p: &Project, n: &Names) {
    let conds = coded_conditions(p);
    let conditional: Vec<bool> = conds.iter().map(Option::is_some).collect();
    let any_cat = p.factors.iter().any(|f| f.is_categorical() && f.level_count() >= 2);
    let any_cond = conditional.iter().any(|&c| c);
    let any_mix = p.factors.iter().any(|f| f.is_mixture());
    let mc = p.mixture_coding();

    out.push_str("# Model columns are in coded units:\n");
    out.push_str("# - continuous and discrete factors: (value - centre) / half-range, so the\n#   range maps to -1 ... +1;\n");
    if any_mix {
        let _ = writeln!(
            out,
            "# - mixture components: pseudo-components (value - low) / {}, where {}\n#   is the mixture total minus the sum of the lower bounds;",
            num(mc.span()),
            num(mc.span())
        );
    }
    if any_cat {
        out.push_str("# - categorical factors: effect (sum-to-zero) coding, one column per level\n#   except the last, which is -1 in every column;\n");
    }
    if any_cond {
        out.push_str("# - conditional factors: 0 in every column where the factor does not apply.\n");
    }
    out.push_str("# code() also codes new points, e.g. for predict().\n\n");
    if any_cat {
        out.push_str(
            "effect <- function(x, levels) {\n  k <- length(levels)\n  m <- contr.sum(k)[match(x, levels), , drop = FALSE]\n  dimnames(m) <- list(NULL, levels[-k])\n  m\n}\n\n",
        );
    }
    if any_cond {
        out.push_str("# 0 where the factor does not apply, NA where that is unknown (missing parent value).\n");
        out.push_str("applies <- function(x, active) {\n  off <- active %in% FALSE\n  unknown <- is.na(active)\n  if (is.matrix(x)) {\n    x[off, ] <- 0\n    x[unknown, ] <- NA\n  } else {\n    x[off] <- 0\n    x[unknown] <- NA\n  }\n  x\n}\n\n");
        out.push_str("is_in <- function(x, values) ifelse(is.na(x), NA, x %in% values)\n\n");
    }
    out.push_str("code <- function(d) {\n");
    if any_cond {
        // Parents first, so that chained conditions can use the parent's activity.
        let mut done = vec![false; p.factors.len()];
        fn emit(fi: usize, p: &Project, n: &Names, conditional: &[bool], done: &mut [bool], out: &mut String, depth: usize) {
            if done[fi] || !conditional[fi] || depth > p.factors.len() {
                return;
            }
            done[fi] = true;
            if let Some(pi) = p.factors[fi].condition.as_ref().and_then(|c| p.factor_index(c.factor)) {
                emit(pi, p, n, conditional, done, out, depth + 1);
            }
            if let Some(e) = condition_expr(p, n, fi, conditional) {
                let _ = writeln!(out, "  a_{} <- {e}", n.factors[fi]);
            }
        }
        for fi in 0..p.factors.len() {
            emit(fi, p, n, &conditional, &mut done, out, 0);
        }
    }
    out.push_str("  x <- data.frame(row.names = seq_len(nrow(d)))\n");
    for (fi, f) in p.factors.iter().enumerate() {
        let name = &n.factors[fi];
        let expr = match &f.kind {
            FactorKind::Categorical { levels } if levels.len() < 2 => {
                let _ = writeln!(out, "  # {name} has a single level and no model columns.");
                continue;
            }
            FactorKind::Categorical { levels } => format!("effect(d${name}, {})", strings(levels)),
            FactorKind::Mixture { low, .. } => format!("(d${name} - {}) / {}", num(*low), num(mc.span())),
            _ => {
                let (c, h) = f.center_half();
                format!("(d${name} - {}) / {}", num(c), num(h))
            }
        };
        let expr = if conditional[fi] { format!("applies({expr}, a_{name})") } else { expr };
        let _ = writeln!(out, "  x${name} <- {expr}");
    }
    out.push_str("  x\n}\n\n");
    out.push_str("coded <- code(runs)\n");
    out.push_str("# Design points (coded settings of every factor) for the lack-of-fit test.\n");
    out.push_str("points <- as.matrix(coded)\n");
    for r in &n.responses {
        let _ = writeln!(out, "coded${r} <- runs${r}");
    }
    let _ = writeln!(out, "keep <- runs${} != \"Excluded\"", n.status);
}

fn centre(out: &mut String, p: &Project, n: &Names) {
    let pt = center_point(p);
    let items: Vec<String> = p
        .factors
        .iter()
        .enumerate()
        .map(|(fi, f)| {
            let v = pt[fi].map(|c| p.decode(fi, c));
            let text = match v {
                Some(Value::Num(x)) => num(x),
                Some(Value::Text(t)) => string(&t),
                None if f.is_categorical() => "NA_character_".into(),
                None => "NA".into(),
            };
            format!("{} = {text}", n.factors[fi])
        })
        .collect();
    out.push_str("# Centre point (first level of categorical factors), used for the\n# predictions below.\n");
    out.push_str(&wrap("centre <- data.frame(", &items, ", ", 2, ")\n"));
}

const ANALYSE: &str = r#"# analyse() prints the statistics, the analysis of variance and the
# coefficients of a fit as sdoe reports them, and returns them as a list.
# - Term rows of the analysis of variance are partial (drop1) tests.
# - `mixture` holds the linear blending terms of a model without an intercept
#   (mixture models); they are tested together against an intercept-only
#   model. R² uses the corrected total sum of squares in every model
#   (summary.lm uses the uncorrected one in models without an intercept).
# - Lack of fit uses replicates: runs with identical settings of every factor
#   (rows of `points`), including factors the model leaves out.
# - AICc and BIC leave out the constant n * (log(2 * pi) + 1) of -2 log L;
#   BIC is NaN without residual degrees of freedom.
# - The prediction at `at` is back-transformed with `inverse`.
analyse <- function(fit, at, inverse = identity, mixture = character(), points = NULL,
                    pole = FALSE, odd = FALSE) {
  X <- model.matrix(fit)
  y <- model.response(model.frame(fit))
  b <- coef(fit)
  n <- nrow(X)
  p <- fit$rank
  df <- fit$df.residual
  e <- residuals(fit)
  h <- hatvalues(fit)
  sse <- sum(e^2)
  sst <- sum((y - mean(y))^2)
  mse <- if (df > 0) sse / df else NaN
  press <- if (all(1 - h >= 1e-10)) sum((e / (1 - h))^2) else NaN
  k <- p + 1
  const <- n * (log(2 * pi) + 1)
  # -2 log L without the constant; an SSE below 1e-24 of sum(y^2) is the
  # rounding noise of a perfect fit and is raised to that floor.
  floor <- 1e-24 * sum(y^2)
  ll <- if (sse > floor) AIC(fit) - const - 2 * k else n * log(floor / n)
  stats <- c(
    runs = n,
    sd = sqrt(mse),
    mean = mean(y),
    cv = 100 * sqrt(mse) / abs(mean(y)),
    r2 = 1 - sse / sst,
    adj_r2 = 1 - mse / (sst / (n - 1)),
    pred_r2 = 1 - press / sst,
    adeq_precision = diff(range(fitted(fit))) / sqrt(p * mse / n),
    press = press,
    aicc = if (n - k - 1 > 0) ll + 2 * k + 2 * k * (k + 1) / (n - k - 1) else NaN,
    bic = if (df > 0) ll + k * log(n) else NaN
  )

  test <- function(ss, d) {
    f <- ss / d / mse
    c(SS = ss, df = d, MS = ss / d, F = f, p = pf(f, d, df, lower.tail = FALSE))
  }
  rows <- list(Model = test(sst - sse, p - 1))
  labels <- attr(terms(fit), "term.labels")
  if (length(mixture)) {
    linear <- attr(X, "assign") %in% match(mixture, labels)
    r <- lm.fit(cbind(1, X[, !linear & !is.na(b), drop = FALSE]), y)
    if (p - r$rank > 0) rows[["Linear mixture"]] <- test(sum(r$residuals^2) - sse, p - r$rank)
  }
  d1 <- drop1(fit, scope = setdiff(labels, mixture), test = "F")
  for (t in rownames(d1)[-1]) {
    rows[[t]] <- c(SS = d1[t, "Sum of Sq"], df = d1[t, "Df"], MS = d1[t, "Sum of Sq"] / d1[t, "Df"],
                   F = d1[t, "F value"], p = d1[t, "Pr(>F)"])
  }
  rows$Residual <- c(SS = sse, df = df, MS = mse, F = NA, p = NA)
  P <- if (is.null(points)) X else points[as.integer(rownames(model.frame(fit))), , drop = FALSE]
  g <- apply(round(P, 7), 1, paste, collapse = " ")
  ss_pe <- sum(tapply(y, g, function(v) sum((v - mean(v))^2)))
  df_pe <- n - length(unique(g))
  df_lof <- df - df_pe
  if (df_pe > 0 && df_lof > 0) {
    f <- ((sse - ss_pe) / df_lof) / (ss_pe / df_pe)
    rows[["Lack of fit"]] <- c(SS = sse - ss_pe, df = df_lof, MS = (sse - ss_pe) / df_lof, F = f,
                               p = pf(f, df_lof, df_pe, lower.tail = FALSE))
    rows[["Pure error"]] <- c(SS = ss_pe, df = df_pe, MS = ss_pe / df_pe, F = NA, p = NA)
  }
  rows[["Total (corrected)"]] <- c(SS = sst, df = n - 1, MS = NA, F = NA, p = NA)
  anova <- as.data.frame(do.call(rbind, rows))

  cf <- summary(fit)$coefficients
  ci <- confint(fit)
  coefficients <- cbind(estimate = b, se = NA, t = NA, p = NA, lwr = ci[, 1], upr = ci[, 2], vif = NA)
  coefficients[rownames(cf), c("se", "t", "p")] <- cf[, 2:4]
  Xk <- X[, !is.na(b) & colnames(X) != "(Intercept)", drop = FALSE]
  if (ncol(Xk) > 0 && n > 1) {
    if ("(Intercept)" %in% colnames(X)) Xk <- scale(Xk, scale = FALSE)
    G <- crossprod(Xk)
    coefficients[colnames(Xk), "vif"] <- tryCatch(diag(solve(G)) * diag(G), error = function(err) NA)
  }

  conf <- predict(fit, at, interval = "confidence")
  pred <- predict(fit, at, interval = "prediction")
  # Back-transformed interval; with a pole at 0 (`pole`) an interval that
  # contains 0 is unbounded (`odd`: negative values map to negative ones).
  back <- function(lo, hi) {
    if (pole && lo <= 0 && hi >= 0) {
      upper <- if (hi > 0) Inf else inverse(lo)
      lower <- if (lo < 0 && odd) -Inf else if (odd) inverse(hi) else 0
      c(min(lower, upper), upper)
    } else {
      range(inverse(c(lo, hi)))
    }
  }
  prediction <- c(y = unname(inverse(conf[1, "fit"])), ci = back(conf[1, 2], conf[1, 3]),
                  pi = back(pred[1, 2], pred[1, 3]))

  cat("\nStatistics\n")
  print(signif(stats, 6))
  cat("\nAnalysis of variance\n")
  print(signif(as.matrix(anova), 6), na.print = "")
  cat("\nCoefficients (coded units) with 95% confidence intervals\n")
  print(signif(coefficients, 6), na.print = "")
  cat("\nPrediction at the centre point with 95% confidence and prediction intervals\n")
  print(signif(prediction, 6))
  invisible(list(stats = stats, anova = anova, coefficients = coefficients, prediction = prediction))
}
"#;

const CHECK: &str = r#"# check() compares the R results with sdoe's and lists the values that differ
# by more than 1e-6 (relative to the value, or to 1e-6 times the largest
# value of the same kind for values near 0).
check <- function(name, res, ref) {
  pairs <- list()
  for (key in c("stats", "prediction")) pairs[[key]] <- list(ref[[key]], res[[key]][names(ref[[key]])])
  for (tab in c("anova", "coefficients")) {
    for (col in names(ref[[tab]])) {
      want <- ref[[tab]][[col]]
      pairs[[paste(tab, col)]] <- list(want, res[[tab]][match(names(want), rownames(res[[tab]])), col])
    }
  }
  bad <- NULL
  total <- 0
  for (key in names(pairs)) {
    want <- pairs[[key]][[1]]
    have <- unname(pairs[[key]][[2]])
    tiny <- 1e-6 * max(0, abs(want[is.finite(want)]))
    same <- ifelse(is.na(want) | is.na(have), is.na(want) & is.na(have),
                   want == have | abs(have - want) <= 1e-6 * pmax(abs(want), tiny))
    total <- total + length(want)
    if (!all(same)) {
      bad <- rbind(bad, data.frame(value = key, term = names(want)[!same], sdoe = want[!same],
                                   R = have[!same], row.names = NULL))
    }
  }
  if (is.null(bad)) {
    cat(sprintf("%s: all %d values agree with sdoe\n", name, total))
  } else {
    cat(sprintf("%s: %d of %d values differ from sdoe\n", name, nrow(bad), total))
    print(bad)
  }
  invisible(bad)
}
"#;

fn named(items: &[(String, f64)]) -> Vec<String> {
    items.iter().map(|(k, v)| format!("{} = {}", string(k), num(*v))).collect()
}

fn vector(out: &mut String, lead: &str, items: &[(String, f64)], last: &str) {
    out.push_str(&wrap(&format!("{lead}c("), &named(items), ", ", 4, &format!("){last}")));
}

/// Fitted model of one response: R formula and the engine's reference values.
struct Model {
    formula: String,
    mixture: Vec<String>,
    left_out: Vec<String>,
}

fn model(p: &Project, n: &Names, fit: &Fit, lhs: &str) -> (Model, Vec<String>, Vec<Option<String>>) {
    let layout = fit.layout();
    // Terms without estimable columns stay out of the R formula.
    let mut included = Vec::new();
    let mut left_out = Vec::new();
    for ti in 0..layout.terms.len() {
        let cols = layout.term_columns(ti);
        if cols.iter().any(|&c| !fit.aliased[c]) {
            included.push(ti);
        } else {
            left_out.push(layout.term_labels[ti].clone());
        }
    }
    // R orders the variables of an interaction label by their first
    // appearance in the formula.
    let written: Vec<Vec<String>> =
        included.iter().map(|&ti| layout.terms[ti].0.iter().map(|&(f, pw)| variable(n, f, pw)).collect()).collect();
    let mut order: Vec<String> = Vec::new();
    for t in &written {
        for v in t {
            if !order.contains(v) {
                order.push(v.clone());
            }
        }
    }
    let pos = |v: &str| order.iter().position(|o| o == v).unwrap_or(usize::MAX);
    let label = |parts: &[(String, String)]| {
        let mut parts = parts.to_vec();
        parts.sort_by_key(|(var, _)| pos(var));
        parts.into_iter().map(|(_, text)| text).collect::<Vec<_>>().join(":")
    };
    let mut term_labels: Vec<Option<String>> = vec![None; layout.terms.len()];
    for &ti in &included {
        let parts: Vec<(String, String)> = layout.terms[ti]
            .0
            .iter()
            .map(|&(f, pw)| {
                let v = variable(n, f, pw);
                (v.clone(), v)
            })
            .collect();
        term_labels[ti] = Some(label(&parts));
    }
    let col_names: Vec<String> = layout
        .columns
        .iter()
        .map(|c| {
            if c.term.is_none() {
                return "(Intercept)".to_string();
            }
            let parts: Vec<(String, String)> = c
                .parts
                .iter()
                .map(|part| match *part {
                    ColPart::Num { factor, power } => {
                        let v = variable(n, factor, power);
                        (v.clone(), v)
                    }
                    // R names a one-column matrix by the variable alone.
                    ColPart::Cat { factor, k: 2, .. } => {
                        let v = variable(n, factor, 1);
                        (v.clone(), v)
                    }
                    ColPart::Cat { factor, level, .. } => {
                        let v = variable(n, factor, 1);
                        let text = format!("{v}{}", p.factors[factor].cat_levels()[level]);
                        (v, text)
                    }
                })
                .collect();
            label(&parts)
        })
        .collect();
    let rhs: Vec<String> = written.iter().map(|t| t.join(":")).collect();
    let mut rhs = if layout.intercept { rhs } else { [vec!["0".to_string()], rhs].concat() };
    if rhs.is_empty() {
        rhs.push("1".into());
    }
    let formula = wrap(&format!("{lhs} ~ "), &rhs, " + ", 4, "");
    let mixture = if layout.intercept {
        Vec::new()
    } else {
        included
            .iter()
            .filter(|&&ti| {
                let t = &layout.terms[ti];
                t.0.len() == 1 && t.max_power() == 1 && p.factors[t.0[0].0].is_mixture()
            })
            .filter_map(|&ti| term_labels[ti].clone())
            .collect()
    };
    (Model { formula, mixture, left_out }, col_names, term_labels)
}

fn reference(out: &mut String, p: &Project, fit: &Fit, var: &str, col_names: &[String], term_labels: &[Option<String>]) {
    let stats = [
        ("runs", fit.n as f64),
        ("sd", fit.rmse()),
        ("mean", fit.mean_y),
        ("cv", fit.cv),
        ("r2", fit.r2),
        ("adj_r2", fit.adj_r2),
        ("pred_r2", fit.pred_r2),
        ("adeq_precision", fit.adeq_precision),
        ("press", fit.press),
        ("aicc", fit.aicc),
        ("bic", fit.bic),
    ];
    let stats: Vec<(String, f64)> = stats.iter().map(|(k, v)| (k.to_string(), *v)).collect();
    let _ = writeln!(out, "sdoe${var} <- list(");
    vector(out, "  stats = ", &stats, ",\n");

    let mut rows: Vec<(String, &AnovaRow)> = vec![("Model".into(), &fit.model_row)];
    for r in &fit.anova {
        match r.term {
            Some(ti) => {
                if let Some(l) = &term_labels[ti] {
                    rows.push((l.clone(), r));
                }
            }
            None => rows.push((r.source.clone(), r)),
        }
    }
    rows.push(("Residual".into(), &fit.resid_row));
    if let Some((lof, pe)) = &fit.lack_of_fit {
        rows.push(("Lack of fit".into(), lof));
        rows.push(("Pure error".into(), pe));
    }
    let total = AnovaRow { source: String::new(), ss: fit.sst, df: fit.n as f64 - 1.0, ms: f64::NAN, f: f64::NAN, p: f64::NAN, term: None };
    rows.push(("Total (corrected)".into(), &total));
    out.push_str("  anova = list(\n");
    let col = |f: &dyn Fn(&AnovaRow) -> f64| rows.iter().map(|(k, r)| (k.clone(), f(r))).collect::<Vec<_>>();
    vector(out, "    SS = ", &col(&|r| r.ss), ",\n");
    vector(out, "    df = ", &col(&|r| r.df), ",\n");
    vector(out, "    F = ", &col(&|r| r.f), ",\n");
    vector(out, "    p = ", &col(&|r| r.p), "\n");
    out.push_str("  ),\n");

    let tq = if fit.df_resid > 0.0 { stats::t_quantile(0.975, fit.df_resid) } else { f64::NAN };
    let coef = |f: &dyn Fn(usize) -> f64| col_names.iter().enumerate().map(|(i, k)| (k.clone(), f(i))).collect::<Vec<_>>();
    let layout = fit.layout();
    out.push_str("  coefficients = list(\n");
    vector(out, "    estimate = ", &coef(&|i| fit.coef[i]), ",\n");
    vector(out, "    se = ", &coef(&|i| fit.se[i]), ",\n");
    vector(out, "    t = ", &coef(&|i| fit.t[i]), ",\n");
    vector(out, "    p = ", &coef(&|i| fit.p[i]), ",\n");
    vector(out, "    lwr = ", &coef(&|i| fit.coef[i] - tq * fit.se[i]), ",\n");
    vector(out, "    upr = ", &coef(&|i| fit.coef[i] + tq * fit.se[i]), ",\n");
    vector(out, "    vif = ", &coef(&|i| if layout.columns[i].term.is_some() { fit.vif[i] } else { f64::NAN }), "\n");
    out.push_str("  )");
    match fit.predict_point(&center_point(p), 0.05) {
        Some(pr) => {
            out.push_str(",\n");
            let items = [("y", pr.y), ("ci1", pr.ci.0), ("ci2", pr.ci.1), ("pi1", pr.pi.0), ("pi2", pr.pi.1)];
            let items: Vec<(String, f64)> = items.iter().map(|(k, v)| (k.to_string(), *v)).collect();
            vector(out, "  prediction = ", &items, "\n");
        }
        None => out.push('\n'),
    }
    out.push_str(")\n");
}

fn kind_text(k: &FactorKind) -> String {
    match k {
        FactorKind::Continuous { low, high } => format!("continuous, {} to {}", num(*low), num(*high)),
        FactorKind::Mixture { low, high } => format!("mixture component, {} to {}", num(*low), num(*high)),
        FactorKind::Discrete { levels } => format!("discrete: {}", levels.iter().map(|v| num(*v)).collect::<Vec<_>>().join(", ")),
        FactorKind::Categorical { levels } => format!("categorical: {}", levels.join(", ")),
    }
}

fn with_units(name: &str, units: &str) -> String {
    if units.is_empty() { name.to_string() } else { format!("{name} [{units}]") }
}

/// The R script for the project.
pub fn script(p: &Project) -> String {
    let n = names(p);
    let mut out = String::new();
    if !p.title.is_empty() {
        let _ = writeln!(out, "# {}", comment(&p.title));
    }
    let _ = writeln!(out, "# R analysis exported from sdoe {}.", env!("CARGO_PKG_VERSION"));
    for line in p.notes.lines() {
        let _ = writeln!(out, "# {}", comment(line).trim_end());
    }
    out.push_str("#\n# Running the script fits each response's model with lm() and prints the\n");
    out.push_str("# statistics sdoe reports. The last section compares them with the values\n# sdoe computed. Base R only.\n");

    heading(&mut out, "Runs (actual units)");
    out.push_str("# Factors\n");
    for (fi, f) in p.factors.iter().enumerate() {
        let mut d = format!("#   {}: {}, {}, {}", n.factors[fi], with_units(&f.name, &f.units), kind_text(&f.kind), f.role.label().to_lowercase());
        let cond = crate::condition::describe(p, fi);
        if !cond.is_empty() {
            let _ = write!(d, ", applies when {cond}");
        }
        let _ = writeln!(out, "{}", comment(&d));
    }
    if !p.responses.is_empty() {
        out.push_str("# Responses\n");
        for (i, r) in p.responses.iter().enumerate() {
            let _ = writeln!(out, "#   {}: {}", n.responses[i], comment(&with_units(&r.name, &r.units)));
        }
    }
    if let Some(d) = &p.design {
        let _ = writeln!(out, "# Design: {}", comment(&d.method));
    }
    if p.factors.iter().any(|f| f.condition.is_some()) {
        out.push_str("# Values of conditional factors are NA where the factor does not apply.\n");
    }
    out.push('\n');
    runs_table(&mut out, p, &n);

    heading(&mut out, "Coding");
    coding(&mut out, p, &n);
    out.push('\n');
    centre(&mut out, p, &n);

    heading(&mut out, "Analysis");
    out.push_str(ANALYSE);

    let mut fitted: Vec<(usize, Fit, Vec<String>, Vec<Option<String>>)> = Vec::new();
    for (i, r) in p.responses.iter().enumerate() {
        let var = &n.responses[i];
        heading(&mut out, &with_units(&r.name, &r.units));
        let fit = match fit_for(p, r.id) {
            Ok(f) => f,
            Err(e) => {
                let _ = writeln!(out, "# No fit in sdoe: {}.", comment(&e));
                continue;
            }
        };
        let tr = fit.input.transform;
        let (lhs, finite, inverse, tr_name) = transform_parts(tr, var);
        let (m, col_names, term_labels) = model(p, &n, &fit, &lhs);
        let _ = writeln!(out, "# Model in sdoe: {}", comment(&format_formula(p, &fit.layout().terms)));
        if !tr_name.is_empty() {
            let _ = writeln!(out, "# Transform: {tr_name}");
        }
        if !m.left_out.is_empty() {
            let _ = writeln!(out, "# Left out (aliased with these runs): {}", m.left_out.join(", "));
        }
        let data = match finite {
            Some(e) => {
                let _ = writeln!(out, "use <- keep & is.finite(suppressWarnings({e}))");
                "coded[use, ]"
            }
            None => "coded[keep, ]",
        };
        let _ = writeln!(out, "fit_{var} <- lm(\n  {},\n  data = {data}\n)", m.formula.replace('\n', "\n  "));
        let _ = writeln!(out, "cat(\"\\n==\", {}, \"==\\n\")", string(&with_units(&r.name, &r.units)));
        let mut args = vec![format!("fit_{var}"), "at = code(centre)".to_string(), "points = points".to_string()];
        if inverse != "identity" {
            args.push(format!("inverse = {inverse}"));
        }
        if !m.mixture.is_empty() {
            args.push(format!("mixture = {}", strings(&m.mixture)));
        }
        args.extend(pole_args(tr));
        out.push_str(&wrap(&format!("res_{var} <- analyse("), &args, ", ", 2, ")\n"));
        fitted.push((i, fit, col_names, term_labels));
    }

    if !fitted.is_empty() {
        heading(&mut out, "Comparison with sdoe");
        out.push_str(CHECK);
        out.push_str("\nsdoe <- list()\n\n");
        for (i, fit, col_names, term_labels) in &fitted {
            reference(&mut out, p, fit, &n.responses[*i], col_names, term_labels);
            out.push('\n');
        }
        out.push_str("cat(\"\\n\")\n");
        for (i, _, _, _) in &fitted {
            let var = &n.responses[*i];
            let _ = writeln!(out, "check({}, res_{var}, sdoe${var})", string(&p.responses[*i].name));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifiers() {
        let mut taken = Vec::new();
        assert_eq!(ident("A", &mut taken), "A");
        assert_eq!(ident("a", &mut taken), "a_2");
        assert_eq!(ident("in", &mut taken), "in_");
        assert_eq!(ident("Tensile strength (MPa)", &mut taken), "Tensile_strength_MPa");
        assert_eq!(ident("2nd pass", &mut taken), "X2nd_pass");
        assert_eq!(ident("Ä", &mut taken), "X");
        assert_eq!(ident("", &mut taken), "X_2");
    }

    #[test]
    fn numbers_round_trip() {
        for x in [0.1, -3.0, 1e-7, 123456789012345680000.0, 2.0f64.sqrt()] {
            assert_eq!(num(x).parse::<f64>().unwrap(), x);
        }
        assert_eq!(num(f64::NAN), "NA");
    }
}
