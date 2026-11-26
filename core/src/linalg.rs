//! Dense linear algebra for the small and medium matrices that occur in
//! experimental design: Gram-Schmidt least squares, Cholesky factorisation,
//! general inversion and the Jacobi symmetric eigen decomposition.

use std::ops::{Index, IndexMut};

/// Row-major dense matrix.
#[derive(Clone, Debug, PartialEq)]
pub struct Mat {
    pub rows: usize,
    pub cols: usize,
    pub data: Vec<f64>,
}

impl Mat {
    pub fn zeros(rows: usize, cols: usize) -> Self {
        Mat { rows, cols, data: vec![0.0; rows * cols] }
    }

    pub fn identity(n: usize) -> Self {
        let mut m = Mat::zeros(n, n);
        for i in 0..n {
            m[(i, i)] = 1.0;
        }
        m
    }

    pub fn from_rows(rows: &[Vec<f64>]) -> Self {
        let r = rows.len();
        let c = rows.first().map(|x| x.len()).unwrap_or(0);
        let mut data = Vec::with_capacity(r * c);
        for row in rows {
            assert_eq!(row.len(), c, "ragged matrix rows");
            data.extend_from_slice(row);
        }
        Mat { rows: r, cols: c, data }
    }

    pub fn row(&self, r: usize) -> &[f64] {
        &self.data[r * self.cols..(r + 1) * self.cols]
    }

    pub fn row_mut(&mut self, r: usize) -> &mut [f64] {
        let c = self.cols;
        &mut self.data[r * c..(r + 1) * c]
    }

    pub fn col(&self, c: usize) -> Vec<f64> {
        (0..self.rows).map(|r| self[(r, c)]).collect()
    }

    pub fn transpose(&self) -> Mat {
        let mut t = Mat::zeros(self.cols, self.rows);
        for r in 0..self.rows {
            for c in 0..self.cols {
                t[(c, r)] = self[(r, c)];
            }
        }
        t
    }

    pub fn matmul(&self, o: &Mat) -> Mat {
        assert_eq!(self.cols, o.rows);
        let mut m = Mat::zeros(self.rows, o.cols);
        for i in 0..self.rows {
            for k in 0..self.cols {
                let a = self[(i, k)];
                if a == 0.0 {
                    continue;
                }
                let orow = o.row(k);
                let mrow = m.row_mut(i);
                for j in 0..o.cols {
                    mrow[j] += a * orow[j];
                }
            }
        }
        m
    }

    pub fn mul_vec(&self, v: &[f64]) -> Vec<f64> {
        assert_eq!(self.cols, v.len());
        (0..self.rows).map(|r| dot(self.row(r), v)).collect()
    }

    /// Computes `self' * v`.
    pub fn t_mul_vec(&self, v: &[f64]) -> Vec<f64> {
        assert_eq!(self.rows, v.len());
        let mut out = vec![0.0; self.cols];
        for r in 0..self.rows {
            let row = self.row(r);
            for c in 0..self.cols {
                out[c] += row[c] * v[r];
            }
        }
        out
    }

    /// Computes the Gram matrix `self' * self`.
    pub fn gram(&self) -> Mat {
        let p = self.cols;
        let mut g = Mat::zeros(p, p);
        for r in 0..self.rows {
            let row = self.row(r);
            for i in 0..p {
                let a = row[i];
                if a == 0.0 {
                    continue;
                }
                for j in i..p {
                    g.data[i * p + j] += a * row[j];
                }
            }
        }
        for i in 0..p {
            for j in 0..i {
                g.data[i * p + j] = g.data[j * p + i];
            }
        }
        g
    }

    /// Returns the sub-matrix of the listed columns.
    pub fn select_cols(&self, cols: &[usize]) -> Mat {
        let mut m = Mat::zeros(self.rows, cols.len());
        for r in 0..self.rows {
            for (j, &c) in cols.iter().enumerate() {
                m[(r, j)] = self[(r, c)];
            }
        }
        m
    }

    pub fn push_row(&mut self, row: &[f64]) {
        if self.rows == 0 && self.cols == 0 {
            self.cols = row.len();
        }
        assert_eq!(row.len(), self.cols);
        self.data.extend_from_slice(row);
        self.rows += 1;
    }

    /// Quadratic form `v' * self * v`.
    pub fn quad_form(&self, v: &[f64]) -> f64 {
        let mut s = 0.0;
        for i in 0..self.rows {
            let row = self.row(i);
            s += v[i] * dot(row, v);
        }
        s
    }

    /// Bilinear form `a' * self * b`.
    pub fn bilinear(&self, a: &[f64], b: &[f64]) -> f64 {
        let mut s = 0.0;
        for i in 0..self.rows {
            s += a[i] * dot(self.row(i), b);
        }
        s
    }

    pub fn trace(&self) -> f64 {
        (0..self.rows.min(self.cols)).map(|i| self[(i, i)]).sum()
    }
}

impl Index<(usize, usize)> for Mat {
    type Output = f64;
    #[inline]
    fn index(&self, (r, c): (usize, usize)) -> &f64 {
        &self.data[r * self.cols + c]
    }
}

impl IndexMut<(usize, usize)> for Mat {
    #[inline]
    fn index_mut(&mut self, (r, c): (usize, usize)) -> &mut f64 {
        &mut self.data[r * self.cols + c]
    }
}

#[inline]
pub fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

/// Cholesky factorisation `a = L L'`. Returns `None` for a matrix that is
/// not numerically positive definite.
pub fn cholesky(a: &Mat) -> Option<Mat> {
    let n = a.rows;
    let mut l = Mat::zeros(n, n);
    let scale = (0..n).map(|i| a[(i, i)].abs()).fold(0.0, f64::max).max(1e-300);
    for j in 0..n {
        let mut d = a[(j, j)];
        for k in 0..j {
            d -= l[(j, k)] * l[(j, k)];
        }
        if d <= 1e-12 * scale {
            return None;
        }
        let d = d.sqrt();
        l[(j, j)] = d;
        for i in j + 1..n {
            let mut s = a[(i, j)];
            for k in 0..j {
                s -= l[(i, k)] * l[(j, k)];
            }
            l[(i, j)] = s / d;
        }
    }
    Some(l)
}

pub fn chol_logdet(l: &Mat) -> f64 {
    2.0 * (0..l.rows).map(|i| l[(i, i)].ln()).sum::<f64>()
}

/// Inverse of a lower-triangular matrix.
pub fn lower_inverse(l: &Mat) -> Mat {
    let n = l.rows;
    let mut inv = Mat::zeros(n, n);
    for j in 0..n {
        inv[(j, j)] = 1.0 / l[(j, j)];
        for i in j + 1..n {
            let mut s = 0.0;
            for k in j..i {
                s -= l[(i, k)] * inv[(k, j)];
            }
            inv[(i, j)] = s / l[(i, i)];
        }
    }
    inv
}

/// Inverse of a symmetric positive definite matrix via Cholesky.
pub fn inverse_spd(a: &Mat) -> Option<Mat> {
    let l = cholesky(a)?;
    let li = lower_inverse(&l);
    Some(li.transpose().matmul(&li))
}

/// General inverse by Gauss-Jordan elimination with partial pivoting.
pub fn inverse(a: &Mat) -> Option<Mat> {
    let n = a.rows;
    assert_eq!(n, a.cols);
    let mut m = a.clone();
    let mut inv = Mat::identity(n);
    let scale = a.data.iter().fold(0.0f64, |s, x| s.max(x.abs())).max(1e-300);
    for c in 0..n {
        let mut piv = c;
        for r in c + 1..n {
            if m[(r, c)].abs() > m[(piv, c)].abs() {
                piv = r;
            }
        }
        if m[(piv, c)].abs() <= 1e-13 * scale {
            return None;
        }
        if piv != c {
            for j in 0..n {
                m.data.swap(piv * n + j, c * n + j);
                inv.data.swap(piv * n + j, c * n + j);
            }
        }
        let d = m[(c, c)];
        for j in 0..n {
            m[(c, j)] /= d;
            inv[(c, j)] /= d;
        }
        for r in 0..n {
            if r != c {
                let f = m[(r, c)];
                if f != 0.0 {
                    for j in 0..n {
                        m[(r, j)] -= f * m[(c, j)];
                        inv[(r, j)] -= f * inv[(c, j)];
                    }
                }
            }
        }
    }
    Some(inv)
}

/// Solves `a x = b` for square `a`.
pub fn solve(a: &Mat, b: &[f64]) -> Option<Vec<f64>> {
    inverse(a).map(|inv| inv.mul_vec(b))
}

/// Determinant via LU with partial pivoting.
pub fn determinant(a: &Mat) -> f64 {
    let n = a.rows;
    let mut m = a.clone();
    let mut det = 1.0;
    for c in 0..n {
        let mut piv = c;
        for r in c + 1..n {
            if m[(r, c)].abs() > m[(piv, c)].abs() {
                piv = r;
            }
        }
        if m[(piv, c)] == 0.0 {
            return 0.0;
        }
        if piv != c {
            for j in 0..n {
                m.data.swap(piv * n + j, c * n + j);
            }
            det = -det;
        }
        let d = m[(c, c)];
        det *= d;
        for r in c + 1..n {
            let f = m[(r, c)] / d;
            if f != 0.0 {
                for j in c..n {
                    m[(r, j)] -= f * m[(c, j)];
                }
            }
        }
    }
    det
}

/// Thin orthogonal decomposition of a model matrix for least squares.
///
/// Columns are processed in order; a column that lies (numerically) in the
/// span of the earlier kept columns is marked as aliased and dropped. This
/// mirrors the usual DOE convention of reporting later terms as aliased with
/// earlier ones.
#[derive(Clone, Debug)]
pub struct LsqDecomp {
    /// n x r matrix with orthonormal columns.
    pub q: Mat,
    /// r x r upper-triangular factor.
    pub r: Mat,
    /// Original indices of the kept columns.
    pub kept: Vec<usize>,
    /// Original indices of the aliased columns.
    pub dropped: Vec<usize>,
}

impl LsqDecomp {
    pub fn new(x: &Mat, rel_tol: f64) -> Self {
        let n = x.rows;
        let mut qcols: Vec<Vec<f64>> = Vec::new();
        let mut rcols: Vec<Vec<f64>> = Vec::new();
        let mut kept = Vec::new();
        let mut dropped = Vec::new();
        for j in 0..x.cols {
            let mut v = x.col(j);
            let orig = dot(&v, &v).sqrt();
            let mut coeffs = vec![0.0; qcols.len()];
            for _pass in 0..2 {
                for (k, q) in qcols.iter().enumerate() {
                    let c = dot(q, &v);
                    coeffs[k] += c;
                    for i in 0..n {
                        v[i] -= c * q[i];
                    }
                }
            }
            let norm = dot(&v, &v).sqrt();
            if orig == 0.0 || norm <= rel_tol * orig.max(1.0) || norm <= 1e-300 {
                dropped.push(j);
                continue;
            }
            for vi in v.iter_mut() {
                *vi /= norm;
            }
            coeffs.push(norm);
            qcols.push(v);
            rcols.push(coeffs);
            kept.push(j);
        }
        let r_dim = kept.len();
        let mut q = Mat::zeros(n, r_dim);
        for (k, col) in qcols.iter().enumerate() {
            for i in 0..n {
                q[(i, k)] = col[i];
            }
        }
        let mut r = Mat::zeros(r_dim, r_dim);
        for (k, col) in rcols.iter().enumerate() {
            for (i, &v) in col.iter().enumerate() {
                r[(i, k)] = v;
            }
        }
        LsqDecomp { q, r, kept, dropped }
    }

    pub fn rank(&self) -> usize {
        self.kept.len()
    }

    /// Coefficients of the kept columns for response `y`.
    pub fn solve(&self, y: &[f64]) -> Vec<f64> {
        let qty = self.q.t_mul_vec(y);
        back_substitute(&self.r, &qty)
    }

    pub fn r_inverse(&self) -> Mat {
        let n = self.r.rows;
        let mut inv = Mat::zeros(n, n);
        for j in 0..n {
            let mut e = vec![0.0; n];
            e[j] = 1.0;
            let col = back_substitute(&self.r, &e);
            for i in 0..n {
                inv[(i, j)] = col[i];
            }
        }
        inv
    }

    /// `(X'X)^-1` restricted to the kept columns.
    pub fn xtx_inverse(&self) -> Mat {
        let ri = self.r_inverse();
        ri.matmul(&ri.transpose())
    }

    /// Diagonal of the hat matrix.
    pub fn leverage(&self) -> Vec<f64> {
        (0..self.q.rows).map(|i| self.q.row(i).iter().map(|v| v * v).sum()).collect()
    }
}

/// Solves `r x = b` for upper-triangular `r`.
pub fn back_substitute(r: &Mat, b: &[f64]) -> Vec<f64> {
    let n = r.rows;
    let mut x = vec![0.0; n];
    for i in (0..n).rev() {
        let mut s = b[i];
        for j in i + 1..n {
            s -= r[(i, j)] * x[j];
        }
        x[i] = s / r[(i, i)];
    }
    x
}

/// Eigen decomposition of a symmetric matrix by cyclic Jacobi rotations.
/// Returns eigenvalues in descending order and eigenvectors as columns.
pub fn sym_eigen(a: &Mat) -> (Vec<f64>, Mat) {
    let n = a.rows;
    let mut m = a.clone();
    let mut v = Mat::identity(n);
    for _sweep in 0..100 {
        let mut off = 0.0;
        for i in 0..n {
            for j in i + 1..n {
                off += m[(i, j)] * m[(i, j)];
            }
        }
        if off < 1e-22 {
            break;
        }
        for p in 0..n {
            for q in p + 1..n {
                let apq = m[(p, q)];
                if apq.abs() < 1e-300 {
                    continue;
                }
                let theta = (m[(q, q)] - m[(p, p)]) / (2.0 * apq);
                let t = theta.signum() / (theta.abs() + (theta * theta + 1.0).sqrt());
                let t = if theta == 0.0 { 1.0 } else { t };
                let c = 1.0 / (t * t + 1.0).sqrt();
                let s = t * c;
                for k in 0..n {
                    let mkp = m[(k, p)];
                    let mkq = m[(k, q)];
                    m[(k, p)] = c * mkp - s * mkq;
                    m[(k, q)] = s * mkp + c * mkq;
                }
                for k in 0..n {
                    let mpk = m[(p, k)];
                    let mqk = m[(q, k)];
                    m[(p, k)] = c * mpk - s * mqk;
                    m[(q, k)] = s * mpk + c * mqk;
                }
                for k in 0..n {
                    let vkp = v[(k, p)];
                    let vkq = v[(k, q)];
                    v[(k, p)] = c * vkp - s * vkq;
                    v[(k, q)] = s * vkp + c * vkq;
                }
            }
        }
    }
    let mut idx: Vec<usize> = (0..n).collect();
    idx.sort_by(|&i, &j| m[(j, j)].partial_cmp(&m[(i, i)]).unwrap_or(std::cmp::Ordering::Equal));
    let vals = idx.iter().map(|&i| m[(i, i)]).collect();
    let vecs = v.select_cols(&idx);
    (vals, vecs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inverse_roundtrip() {
        let a = Mat::from_rows(&[vec![4.0, 1.0, 2.0], vec![1.0, 3.0, 0.5], vec![2.0, 0.5, 5.0]]);
        let i1 = inverse(&a).unwrap();
        let i2 = inverse_spd(&a).unwrap();
        let p = a.matmul(&i1);
        for r in 0..3 {
            for c in 0..3 {
                let e = if r == c { 1.0 } else { 0.0 };
                assert!((p[(r, c)] - e).abs() < 1e-12);
                assert!((i1[(r, c)] - i2[(r, c)]).abs() < 1e-12);
            }
        }
        let l = cholesky(&a).unwrap();
        assert!((chol_logdet(&l) - determinant(&a).ln()).abs() < 1e-12);
    }

    #[test]
    fn lsq_detects_alias() {
        let x = Mat::from_rows(&[
            vec![1.0, 1.0, 2.0, 0.3],
            vec![1.0, 2.0, 4.0, 0.1],
            vec![1.0, 3.0, 6.0, 0.7],
            vec![1.0, 4.0, 8.0, 0.2],
        ]);
        let d = LsqDecomp::new(&x, 1e-9);
        assert_eq!(d.kept, vec![0, 1, 3]);
        assert_eq!(d.dropped, vec![2]);
        let y = vec![1.0, 2.0, 2.5, 4.5];
        let b = d.solve(&y);
        let xs = x.select_cols(&d.kept);
        let fitted = xs.mul_vec(&b);
        let resid: Vec<f64> = y.iter().zip(&fitted).map(|(a, b)| a - b).collect();
        let g = xs.t_mul_vec(&resid);
        assert!(g.iter().all(|v| v.abs() < 1e-10));
    }

    #[test]
    fn jacobi_eigen() {
        let a = Mat::from_rows(&[vec![2.0, 1.0], vec![1.0, 2.0]]);
        let (vals, vecs) = sym_eigen(&a);
        assert!((vals[0] - 3.0).abs() < 1e-12 && (vals[1] - 1.0).abs() < 1e-12);
        assert!((vecs[(0, 0)].abs() - 0.5f64.sqrt()).abs() < 1e-12);
    }
}
