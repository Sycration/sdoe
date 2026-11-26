//! Plot descriptions computed in the backend and drawn by any frontend:
//! axes with ticks, series, heat maps, contour segments and projected 3D
//! surfaces.

use crate::format;
use serde::Serialize;

#[derive(Clone, Debug, Serialize, Default)]
pub struct Tick {
    pub v: f64,
    pub label: String,
}

#[derive(Clone, Debug, Serialize, Default)]
pub struct Axis {
    pub min: f64,
    pub max: f64,
    pub ticks: Vec<Tick>,
    pub hidden: bool,
}

#[derive(Clone, Debug, Serialize, Default)]
pub struct Series {
    pub name: String,
    /// points | line | bars | band | hline | vline | segments | labels | polygons
    pub kind: &'static str,
    pub x: Vec<f64>,
    pub y: Vec<f64>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub y2: Vec<f64>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub labels: Vec<String>,
    /// Per-polygon or per-point values for colour mapping.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub values: Vec<f64>,
    /// Run indices for click-through.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub ids: Vec<i64>,
    /// Palette slot: 0.. series colours, -1 muted, -2 reference, -3 highlight.
    pub color: i32,
    pub dashed: bool,
}

#[derive(Clone, Debug, Serialize, Default)]
pub struct Heatmap {
    pub nx: usize,
    pub ny: usize,
    pub x0: f64,
    pub x1: f64,
    pub y0: f64,
    pub y1: f64,
    /// Row-major from (x0, y0).
    pub values: Vec<f64>,
    pub vmin: f64,
    pub vmax: f64,
    pub diverging: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub row_labels: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub col_labels: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Default)]
pub struct Plot {
    pub title: String,
    pub x_label: String,
    pub y_label: String,
    pub x: Axis,
    pub y: Axis,
    pub series: Vec<Series>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub heatmap: Option<Heatmap>,
    /// xy | 3d | matrix
    pub kind: &'static str,
    pub legend: bool,
    /// Colour-scale range for polygon/heat values.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value_range: Option<(f64, f64)>,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub note: String,
}

impl Series {
    pub fn new(name: &str, kind: &'static str, x: Vec<f64>, y: Vec<f64>, color: i32) -> Series {
        Series { name: name.into(), kind, x, y, color, ..Default::default() }
    }
}

fn nice_num(x: f64, round: bool) -> f64 {
    if x <= 0.0 || !x.is_finite() {
        return 1.0;
    }
    let e = x.log10().floor();
    let f = x / 10f64.powf(e);
    let nf = if round {
        if f < 1.5 {
            1.0
        } else if f < 3.0 {
            2.0
        } else if f < 7.0 {
            5.0
        } else {
            10.0
        }
    } else if f <= 1.0 {
        1.0
    } else if f <= 2.0 {
        2.0
    } else if f <= 5.0 {
        5.0
    } else {
        10.0
    };
    nf * 10f64.powf(e)
}

/// Axis covering [lo, hi] with about `n` nicely rounded ticks; with
/// `expand` the limits snap outward to tick values.
pub fn nice_axis(lo: f64, hi: f64, n: usize, expand: bool) -> Axis {
    let (mut lo, mut hi) = if lo.is_finite() && hi.is_finite() { (lo, hi) } else { (0.0, 1.0) };
    if hi < lo {
        std::mem::swap(&mut lo, &mut hi);
    }
    if (hi - lo).abs() < 1e-12 * lo.abs().max(1.0) {
        let d = if lo == 0.0 { 1.0 } else { lo.abs() * 0.1 };
        lo -= d;
        hi += d;
    }
    let range = nice_num(hi - lo, false);
    let step = nice_num(range / (n.max(2) - 1) as f64, true);
    let (min, max) = if expand { ((lo / step).floor() * step, (hi / step).ceil() * step) } else { (lo, hi) };
    let mut ticks = Vec::new();
    let mut v = (min / step).ceil() * step;
    let decimals = (-(step.log10().floor()) as i32).max(0) as usize;
    while v <= max + step * 1e-9 {
        let vv = if v.abs() < step * 1e-9 { 0.0 } else { v };
        let label = if vv.abs() >= 1e6 || (vv != 0.0 && vv.abs() < 1e-4) { format::num(vv, 3) } else { format!("{:.*}", decimals, vv) };
        ticks.push(Tick { v: vv, label });
        v += step;
    }
    Axis { min, max, ticks, hidden: false }
}

/// Axis with a little padding around data.
pub fn data_axis(values: &[f64], n: usize) -> Axis {
    let f: Vec<f64> = values.iter().cloned().filter(|v| v.is_finite()).collect();
    if f.is_empty() {
        return nice_axis(0.0, 1.0, n, true);
    }
    let lo = f.iter().cloned().fold(f64::INFINITY, f64::min);
    let hi = f.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let pad = (hi - lo).abs() * 0.05;
    nice_axis(lo - pad, hi + pad, n, true)
}

/// Axis for categorical positions 0..k-1.
pub fn category_axis(labels: &[String]) -> Axis {
    Axis {
        min: -0.5,
        max: labels.len() as f64 - 0.5,
        ticks: labels.iter().enumerate().map(|(i, l)| Tick { v: i as f64, label: l.clone() }).collect(),
        hidden: false,
    }
}

/// Contour segments of a grid by marching squares. `grid[j][i]` is the value
/// at (xs[i], ys[j]). Returns one segments series per level plus labels.
pub fn contours(xs: &[f64], ys: &[f64], grid: &[Vec<f64>], levels: &[f64]) -> (Vec<Series>, Series) {
    let mut out = Vec::new();
    let mut labels = Series::new("", "labels", vec![], vec![], -2);
    for &lev in levels {
        let mut s = Series::new(&format::num(lev, 4), "segments", vec![], vec![], -2);
        for j in 0..ys.len().saturating_sub(1) {
            for i in 0..xs.len().saturating_sub(1) {
                let v = [grid[j][i], grid[j][i + 1], grid[j + 1][i + 1], grid[j + 1][i]];
                if v.iter().any(|x| !x.is_finite()) {
                    continue;
                }
                let corners = [(xs[i], ys[j]), (xs[i + 1], ys[j]), (xs[i + 1], ys[j + 1]), (xs[i], ys[j + 1])];
                let mut pts = Vec::new();
                for e in 0..4 {
                    let (a, b) = (e, (e + 1) % 4);
                    let (va, vb) = (v[a], v[b]);
                    if (va < lev) != (vb < lev) {
                        let t = (lev - va) / (vb - va);
                        pts.push((corners[a].0 + t * (corners[b].0 - corners[a].0), corners[a].1 + t * (corners[b].1 - corners[a].1)));
                    }
                }
                if pts.len() == 2 {
                    s.x.extend([pts[0].0, pts[1].0]);
                    s.y.extend([pts[0].1, pts[1].1]);
                } else if pts.len() == 4 {
                    let center = (v[0] + v[1] + v[2] + v[3]) / 4.0;
                    let (p0, p1, p2, p3) = (pts[0], pts[1], pts[2], pts[3]);
                    if (center < lev) == (v[0] < lev) {
                        s.x.extend([p0.0, p3.0, p1.0, p2.0]);
                        s.y.extend([p0.1, p3.1, p1.1, p2.1]);
                    } else {
                        s.x.extend([p0.0, p1.0, p2.0, p3.0]);
                        s.y.extend([p0.1, p1.1, p2.1, p3.1]);
                    }
                }
            }
        }
        if !s.x.is_empty() {
            // Label at the segment closest to the plot center.
            let cx = (xs[0] + xs[xs.len() - 1]) / 2.0;
            let cy = (ys[0] + ys[ys.len() - 1]) / 2.0;
            let mut best = (f64::INFINITY, 0.0, 0.0);
            for k in (0..s.x.len()).step_by(2) {
                let mx = (s.x[k] + s.x[k + 1]) / 2.0;
                let my = (s.y[k] + s.y[k + 1]) / 2.0;
                let d = (mx - cx).powi(2) * 0.3 + (my - cy).powi(2);
                let d = d + ((k as f64) * 0.0);
                if d < best.0 {
                    best = (d, mx, my);
                }
            }
            labels.x.push(best.1);
            labels.y.push(best.2);
            labels.labels.push(format::num(lev, 4));
            out.push(s);
        }
    }
    (out, labels)
}

/// Projects a surface grid into 2D polygons drawn back to front.
pub fn surface_3d(
    xs: &[f64],
    ys: &[f64],
    grid: &[Vec<f64>],
    azimuth_deg: f64,
    elevation_deg: f64,
    labels: (&str, &str, &str),
) -> Plot {
    let zf: Vec<f64> = grid.iter().flatten().cloned().filter(|v| v.is_finite()).collect();
    let zmin = zf.iter().cloned().fold(f64::INFINITY, f64::min);
    let zmax = zf.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let zspan = if (zmax - zmin).abs() < 1e-300 { 1.0 } else { zmax - zmin };
    let (x0, x1) = (xs[0], xs[xs.len() - 1]);
    let (y0, y1) = (ys[0], ys[ys.len() - 1]);
    let nx = |x: f64| 2.0 * (x - x0) / (x1 - x0) - 1.0;
    let ny = |y: f64| 2.0 * (y - y0) / (y1 - y0) - 1.0;
    let nz = |z: f64| 1.6 * (z - zmin) / zspan - 0.8;
    let (th, ph) = (azimuth_deg.to_radians(), elevation_deg.to_radians());
    let proj = |x: f64, y: f64, z: f64| -> (f64, f64, f64) {
        let xr = x * th.cos() - y * th.sin();
        let yr = x * th.sin() + y * th.cos();
        (xr, z * ph.cos() + yr * ph.sin(), yr * ph.cos() - z * ph.sin())
    };
    let mut quads: Vec<(f64, [(f64, f64); 4], f64)> = Vec::new();
    for j in 0..ys.len() - 1 {
        for i in 0..xs.len() - 1 {
            let c = [
                (xs[i], ys[j], grid[j][i]),
                (xs[i + 1], ys[j], grid[j][i + 1]),
                (xs[i + 1], ys[j + 1], grid[j + 1][i + 1]),
                (xs[i], ys[j + 1], grid[j + 1][i]),
            ];
            if c.iter().any(|p| !p.2.is_finite()) {
                continue;
            }
            let mut pts = [(0.0, 0.0); 4];
            let mut depth = 0.0;
            for (k, &(x, y, z)) in c.iter().enumerate() {
                let (sx, sy, d) = proj(nx(x), ny(y), nz(z));
                pts[k] = (sx, sy);
                depth += d;
            }
            let val = (c[0].2 + c[1].2 + c[2].2 + c[3].2) / 4.0;
            quads.push((depth, pts, val));
        }
    }
    quads.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    let mut poly = Series::new("", "polygons", vec![], vec![], 0);
    for (_, pts, v) in &quads {
        for (x, y) in pts {
            poly.x.push(*x);
            poly.y.push(*y);
        }
        poly.values.push(*v);
    }
    // Floor box edges and axis labels.
    let mut edges = Series::new("", "segments", vec![], vec![], -1);
    let mut lab = Series::new("", "labels", vec![], vec![], -1);
    let floor = -0.8;
    let corners = [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)];
    for k in 0..4 {
        let (a, b) = (corners[k], corners[(k + 1) % 4]);
        let pa = proj(a.0, a.1, floor);
        let pb = proj(b.0, b.1, floor);
        edges.x.extend([pa.0, pb.0]);
        edges.y.extend([pa.1, pb.1]);
    }
    // Vertical axis at the corner with the largest depth (back corner).
    let back = corners
        .iter()
        .max_by(|a, b| proj(a.0, a.1, 0.0).2.partial_cmp(&proj(b.0, b.1, 0.0).2).unwrap())
        .cloned()
        .unwrap();
    let pv0 = proj(back.0, back.1, floor);
    let pv1 = proj(back.0, back.1, 0.8);
    edges.x.extend([pv0.0, pv1.0]);
    edges.y.extend([pv0.1, pv1.1]);
    let px = proj(0.0, -1.25, floor);
    let py = proj(1.25, 0.0, floor);
    let pz = proj(back.0 * 1.1, back.1 * 1.1, 0.95);
    lab.x.extend([px.0, py.0, pz.0]);
    lab.y.extend([px.1, py.1, pz.1]);
    lab.labels.extend([labels.0.to_string(), labels.1.to_string(), labels.2.to_string()]);
    let zl = proj(back.0 * 1.12, back.1 * 1.12, floor);
    let zh = proj(back.0 * 1.12, back.1 * 1.12, 0.8);
    lab.x.extend([zl.0, zh.0]);
    lab.y.extend([zl.1, zh.1]);
    lab.labels.extend([format::num(zmin, 4), format::num(zmax, 4)]);
    Plot {
        title: String::new(),
        kind: "3d",
        x: Axis { min: -1.7, max: 1.7, ticks: vec![], hidden: true },
        y: Axis { min: -1.7, max: 1.7, ticks: vec![], hidden: true },
        series: vec![poly, edges, lab],
        value_range: Some((zmin, zmax)),
        ..Default::default()
    }
}

/// Contour levels: about `n` nice values inside [lo, hi].
pub fn contour_levels(lo: f64, hi: f64, n: usize) -> Vec<f64> {
    let a = nice_axis(lo, hi, n, false);
    a.ticks.into_iter().map(|t| t.v).filter(|v| *v > lo && *v < hi).collect()
}
