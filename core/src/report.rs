//! Self-contained HTML report with inline SVG plots.

use crate::analysis::model_for;
use crate::format::num;
use crate::model::equation::{Syntax, equations};
use crate::plot::Plot;
use crate::project::{FactorKind, Project};
use crate::results;
use crate::table::Table;

pub fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

const PALETTE: [&str; 6] = ["#2f6fb2", "#d0782a", "#3c9a5f", "#9b59b6", "#c0392b", "#7f8c8d"];

fn color(c: i32) -> &'static str {
    match c {
        -1 => "#999999",
        -2 => "#555555",
        -3 => "#c0392b",
        n => PALETTE[(n.max(0) as usize) % PALETTE.len()],
    }
}

fn heat(t: f64) -> String {
    // Blue-to-yellow sequential ramp.
    let t = t.clamp(0.0, 1.0);
    let stops = [(0.0, (49, 54, 149)), (0.35, (69, 117, 180)), (0.6, (116, 173, 209)), (0.8, (254, 224, 144)), (1.0, (244, 109, 67))];
    for w in stops.windows(2) {
        let (t0, c0) = w[0];
        let (t1, c1) = w[1];
        if t <= t1 {
            let u = (t - t0) / (t1 - t0);
            let l = |a: i32, b: i32| (a as f64 + u * (b - a) as f64).round() as i32;
            return format!("rgb({},{},{})", l(c0.0, c1.0), l(c0.1, c1.1), l(c0.2, c1.2));
        }
    }
    "rgb(244,109,67)".into()
}

/// Renders a plot as SVG.
pub fn svg(p: &Plot, w: f64, h: f64) -> String {
    let (ml, mr, mt, mb) = if p.kind == "3d" { (10.0, 10.0, 24.0, 10.0) } else { (64.0, 16.0, 28.0, 46.0) };
    let pw = w - ml - mr;
    let ph = h - mt - mb;
    let (x0, x1, y0, y1) = (p.x.min, p.x.max, p.y.min, p.y.max);
    let sx = |x: f64| ml + (x - x0) / (x1 - x0) * pw;
    let sy = |y: f64| mt + ph - (y - y0) / (y1 - y0) * ph;
    let mut s = format!("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}\" height=\"{h}\" font-family=\"sans-serif\" font-size=\"11\">");
    s.push_str(&format!("<text x=\"{}\" y=\"16\" text-anchor=\"middle\" font-size=\"13\">{}</text>", w / 2.0, esc(&p.title)));
    if let Some(hm) = &p.heatmap {
        let span = (hm.vmax - hm.vmin).max(1e-300);
        let cw = pw / hm.nx as f64;
        let ch = ph / hm.ny as f64;
        for j in 0..hm.ny {
            for i in 0..hm.nx {
                let v = hm.values[j * hm.nx + i];
                if !v.is_finite() {
                    continue;
                }
                let px = ml + i as f64 * cw;
                let py = if p.kind == "matrix" { mt + j as f64 * ch } else { mt + ph - (j + 1) as f64 * ch };
                s.push_str(&format!("<rect x=\"{px:.1}\" y=\"{py:.1}\" width=\"{:.1}\" height=\"{:.1}\" fill=\"{}\"/>", cw + 0.5, ch + 0.5, heat((v - hm.vmin) / span)));
            }
        }
    }
    if !p.x.hidden {
        s.push_str(&format!("<rect x=\"{ml}\" y=\"{mt}\" width=\"{pw}\" height=\"{ph}\" fill=\"none\" stroke=\"#888\"/>"));
        for t in &p.x.ticks {
            let x = sx(t.v);
            s.push_str(&format!("<line x1=\"{x:.1}\" y1=\"{}\" x2=\"{x:.1}\" y2=\"{}\" stroke=\"#888\"/>", mt + ph, mt + ph + 4.0));
            s.push_str(&format!("<text x=\"{x:.1}\" y=\"{}\" text-anchor=\"middle\">{}</text>", mt + ph + 16.0, esc(&t.label)));
        }
        for t in &p.y.ticks {
            let y = sy(t.v);
            s.push_str(&format!("<line x1=\"{}\" y1=\"{y:.1}\" x2=\"{ml}\" y2=\"{y:.1}\" stroke=\"#888\"/>", ml - 4.0));
            s.push_str(&format!("<text x=\"{}\" y=\"{:.1}\" text-anchor=\"end\">{}</text>", ml - 6.0, y + 4.0, esc(&t.label)));
        }
        s.push_str(&format!("<text x=\"{}\" y=\"{}\" text-anchor=\"middle\">{}</text>", ml + pw / 2.0, h - 8.0, esc(&p.x_label)));
        s.push_str(&format!(
            "<text x=\"14\" y=\"{}\" text-anchor=\"middle\" transform=\"rotate(-90 14 {})\">{}</text>",
            mt + ph / 2.0,
            mt + ph / 2.0,
            esc(&p.y_label)
        ));
    }
    let vr = p.value_range.unwrap_or((0.0, 1.0));
    for se in &p.series {
        let col = color(se.color);
        let dash = if se.dashed { " stroke-dasharray=\"5,4\"" } else { "" };
        match se.kind {
            "points" => {
                for (x, y) in se.x.iter().zip(&se.y) {
                    if x.is_finite() && y.is_finite() {
                        s.push_str(&format!("<circle cx=\"{:.1}\" cy=\"{:.1}\" r=\"3.2\" fill=\"{col}\"/>", sx(*x), sy(*y)));
                    }
                }
            }
            "line" | "linepoints" => {
                let pts: Vec<String> = se.x.iter().zip(&se.y).filter(|(x, y)| x.is_finite() && y.is_finite()).map(|(x, y)| format!("{:.1},{:.1}", sx(*x), sy(*y))).collect();
                s.push_str(&format!("<polyline points=\"{}\" fill=\"none\" stroke=\"{col}\" stroke-width=\"1.8\"{dash}/>", pts.join(" ")));
                if se.kind == "linepoints" {
                    for (x, y) in se.x.iter().zip(&se.y) {
                        s.push_str(&format!("<circle cx=\"{:.1}\" cy=\"{:.1}\" r=\"3\" fill=\"{col}\"/>", sx(*x), sy(*y)));
                    }
                }
            }
            "band" => {
                let mut pts: Vec<String> = se.x.iter().zip(&se.y).map(|(x, y)| format!("{:.1},{:.1}", sx(*x), sy(*y))).collect();
                pts.extend(se.x.iter().zip(&se.y2).rev().map(|(x, y)| format!("{:.1},{:.1}", sx(*x), sy(*y))));
                s.push_str(&format!("<polygon points=\"{}\" fill=\"{col}\" fill-opacity=\"0.18\" stroke=\"none\"/>", pts.join(" ")));
            }
            "errorbars" => {
                for ((x, lo), hi) in se.x.iter().zip(&se.y).zip(&se.y2) {
                    s.push_str(&format!("<line x1=\"{0:.1}\" y1=\"{1:.1}\" x2=\"{0:.1}\" y2=\"{2:.1}\" stroke=\"{col}\"/>", sx(*x), sy(*lo), sy(*hi)));
                }
            }
            "bars" => {
                let bw = (pw / se.x.len().max(1) as f64 * 0.6).min(24.0);
                for (x, y) in se.x.iter().zip(&se.y) {
                    let (ya, yb) = (sy(y.max(0.0)), sy(y.min(0.0).max(y0)));
                    s.push_str(&format!("<rect x=\"{:.1}\" y=\"{ya:.1}\" width=\"{bw:.1}\" height=\"{:.1}\" fill=\"{col}\"/>", sx(*x) - bw / 2.0, (yb - ya).abs()));
                }
            }
            "hbars" => {
                let bh = (ph / se.y.len().max(1) as f64 * 0.65).min(22.0);
                for (k, (x, y)) in se.x.iter().zip(&se.y).enumerate() {
                    let c = if se.values.get(k).copied().unwrap_or(1.0) < 0.0 { color(1) } else { col };
                    s.push_str(&format!("<rect x=\"{:.1}\" y=\"{:.1}\" width=\"{:.1}\" height=\"{bh:.1}\" fill=\"{c}\"/>", sx(0.0), sy(*y) - bh / 2.0, sx(*x) - sx(0.0)));
                }
            }
            "hline" => {
                for y in &se.y {
                    s.push_str(&format!("<line x1=\"{ml}\" y1=\"{0:.1}\" x2=\"{1}\" y2=\"{0:.1}\" stroke=\"{col}\"{dash}/>", sy(*y), ml + pw));
                }
            }
            "vline" => {
                for x in &se.x {
                    if x.is_finite() {
                        s.push_str(&format!("<line x1=\"{0:.1}\" y1=\"{mt}\" x2=\"{0:.1}\" y2=\"{1}\" stroke=\"{col}\"{dash}/>", sx(*x), mt + ph));
                    }
                }
            }
            "segments" => {
                let mut d = String::new();
                for k in (0..se.x.len()).step_by(2) {
                    d.push_str(&format!("M{:.1} {:.1}L{:.1} {:.1}", sx(se.x[k]), sy(se.y[k]), sx(se.x[k + 1]), sy(se.y[k + 1])));
                }
                s.push_str(&format!("<path d=\"{d}\" stroke=\"{col}\" fill=\"none\"/>"));
            }
            "labels" => {
                for ((x, y), l) in se.x.iter().zip(&se.y).zip(&se.labels) {
                    s.push_str(&format!("<text x=\"{:.1}\" y=\"{:.1}\" fill=\"{col}\">{}</text>", sx(*x) + 4.0, sy(*y) - 4.0, esc(l)));
                }
            }
            "polygons" => {
                let span = (vr.1 - vr.0).max(1e-300);
                for (k, v) in se.values.iter().enumerate() {
                    let pts: Vec<String> = (0..4).map(|j| format!("{:.1},{:.1}", sx(se.x[4 * k + j]), sy(se.y[4 * k + j]))).collect();
                    s.push_str(&format!("<polygon points=\"{}\" fill=\"{}\" stroke=\"#333\" stroke-width=\"0.3\"/>", pts.join(" "), heat((v - vr.0) / span)));
                }
            }
            _ => {}
        }
    }
    s.push_str("</svg>");
    s
}

fn kv(pairs: &[(String, String)]) -> String {
    let mut s = String::from("<table class=\"kv\">");
    for (k, v) in pairs {
        s.push_str(&format!("<tr><th>{}</th><td>{}</td></tr>", esc(k), esc(v)));
    }
    s.push_str("</table>");
    s
}

pub fn html(p: &Project) -> String {
    let mut b = String::new();
    let title = if p.title.is_empty() { "Experiment report".to_string() } else { p.title.clone() };
    b.push_str(&format!(
        "<!DOCTYPE html><html><head><meta charset=\"utf-8\"><title>{}</title><style>\
body{{font-family:system-ui,sans-serif;max-width:1000px;margin:24px auto;padding:0 16px;color:#222}}\
table{{border-collapse:collapse;margin:8px 0 16px}}th,td{{border-bottom:1px solid #ddd;padding:3px 8px;text-align:left}}\
td.n,th.n{{text-align:right;font-variant-numeric:tabular-nums}}tr.good td{{font-weight:600}}tr.muted td{{color:#888}}tr.strong td{{font-weight:600}}tr.bad td{{color:#b03030}}\
pre{{background:#f5f5f5;padding:8px;overflow-x:auto}}.warn{{color:#a04000}}h2{{border-bottom:2px solid #ccc;padding-bottom:4px;margin-top:32px}}\
.plots svg{{margin:4px}}</style></head><body>",
        esc(&title)
    ));
    b.push_str(&format!("<h1>{}</h1>", esc(&title)));
    if !p.notes.is_empty() {
        b.push_str(&format!("<p>{}</p>", esc(&p.notes)));
    }
    b.push_str("<h2>Factors</h2>");
    let mut t = Table::new(&["Symbol", "Name", "Units", "Type", "Role", "Range / levels"], &[false; 6]);
    for f in &p.factors {
        let r = match &f.kind {
            FactorKind::Continuous { low, high } | FactorKind::Mixture { low, high } => format!("{} … {}", num(*low, 6), num(*high, 6)),
            FactorKind::Discrete { levels } => levels.iter().map(|v| num(*v, 6)).collect::<Vec<_>>().join(", "),
            FactorKind::Categorical { levels } => levels.join(", "),
        };
        let kind = match f.kind {
            FactorKind::Continuous { .. } => "Continuous",
            FactorKind::Discrete { .. } => "Discrete",
            FactorKind::Categorical { .. } => "Categorical",
            FactorKind::Mixture { .. } => "Mixture",
        };
        t.push(vec![f.symbol.clone(), f.name.clone(), f.units.clone(), kind.into(), f.role.label().into(), r], "", "");
    }
    b.push_str(&t.to_html());
    if let Some(d) = &p.design {
        b.push_str(&format!("<h2>Design: {}</h2>", esc(&d.method)));
        b.push_str("<pre>");
        b.push_str(&esc(&d.notes.join("\n")));
        b.push_str("</pre>");
    }
    b.push_str(&format!("<p>{} runs, {} completed.</p>", p.runs.len(), p.runs.iter().filter(|r| r.status == crate::project::RunStatus::Done).count()));
    for r in &p.responses {
        b.push_str(&format!("<h2>Response: {}</h2>", esc(&r.name)));
        let (terms, tr) = model_for(p, r.id);
        let f = match results::fit_for(p, r.id) {
            Ok(f) => f,
            Err(e) => {
                b.push_str(&format!("<p class=\"warn\">{}</p>", esc(&e)));
                continue;
            }
        };
        b.push_str(&format!(
            "<p>Model: <code>{}</code>{}</p>",
            esc(&crate::model::terms::format_formula(p, &terms)),
            if tr != crate::project::Transform::None { format!(", transform {}", esc(&tr.label())) } else { String::new() }
        ));
        for w in results::warnings(p, &f) {
            b.push_str(&format!("<p class=\"warn\">{}</p>", esc(&w)));
        }
        b.push_str(&kv(&results::stats_pairs(&f)));
        b.push_str("<h3>Analysis of variance</h3>");
        b.push_str(&results::anova_table(&f).to_html());
        b.push_str("<h3>Coefficients (coded units)</h3>");
        b.push_str(&results::coefficient_table(&f).to_html());
        b.push_str("<h3>Equation in actual units</h3><pre>");
        for e in equations(p, &f, &r.name, true, Syntax::Plain, 6) {
            if !e.condition.is_empty() {
                b.push_str(&esc(&e.condition));
                b.push_str(":\n");
            }
            b.push_str(&esc(&e.text));
            b.push('\n');
            if let Some(x) = e.explicit {
                b.push_str(&esc(&x));
                b.push('\n');
            }
        }
        b.push_str("</pre>");
        for line in results::canonical(p, &f) {
            b.push_str(&format!("<p>{}</p>", esc(&line)));
        }
        b.push_str("<div class=\"plots\">");
        b.push_str(&svg(&results::plot_pred_actual(p, &f, r.id), 420.0, 340.0));
        b.push_str(&svg(&results::plot_resid_normal(p, &f), 420.0, 340.0));
        b.push_str(&svg(&results::plot_pareto(&f), 420.0, 340.0));
        b.push_str(&svg(&results::plot_resid_fitted(p, &f), 420.0, 340.0));
        b.push_str("</div>");
    }
    b.push_str("</body></html>");
    b
}
