//! Floating-point linear algebra for the Tier-A numeric kernels (CLAUDE.md 10.2,
//! APPENDIX I.3). This is the first place exact arithmetic is impossible, so every
//! result here is *approximate* and must be paired with an explicit tolerance and a
//! measured residual (the certificate). Nothing in this module claims exact equality.
//!
//! Determinism (R11): the Gaussian source is a seeded splitmix64 stream, so the same
//! inputs+seed reproduce the same projection (and thus the same residual).
//!
//! Float-precision honesty (the Tier-A brief, point 6): residuals are computed in
//! f64; a tolerance must dominate the f64 round-off floor (~ machine_eps · n · scale)
//! or "within tol" would be measuring noise. Callers choose tol accordingly and the
//! tolerance is part of the contract, not a hidden constant.

/// Row-major dense `f64` matrix.
#[derive(Clone, Debug, PartialEq)]
pub struct FMat {
    pub rows: usize,
    pub cols: usize,
    pub data: Vec<f64>,
}

impl FMat {
    pub fn zeros(rows: usize, cols: usize) -> Self {
        FMat {
            rows,
            cols,
            data: vec![0.0; rows * cols],
        }
    }
    pub fn from_data(rows: usize, cols: usize, data: Vec<f64>) -> Self {
        assert_eq!(data.len(), rows * cols);
        FMat { rows, cols, data }
    }
    pub fn get(&self, i: usize, j: usize) -> f64 {
        self.data[i * self.cols + j]
    }
    pub fn set(&mut self, i: usize, j: usize, v: f64) {
        self.data[i * self.cols + j] = v;
    }

    pub fn transpose(&self) -> FMat {
        let mut t = FMat::zeros(self.cols, self.rows);
        for i in 0..self.rows {
            for j in 0..self.cols {
                t.set(j, i, self.get(i, j));
            }
        }
        t
    }

    pub fn matmul(&self, b: &FMat) -> FMat {
        assert_eq!(self.cols, b.rows, "matmul dim mismatch");
        let mut c = FMat::zeros(self.rows, b.cols);
        for i in 0..self.rows {
            for k in 0..self.cols {
                let a = self.get(i, k);
                if a == 0.0 {
                    continue;
                }
                for j in 0..b.cols {
                    c.data[i * b.cols + j] += a * b.get(k, j);
                }
            }
        }
        c
    }

    pub fn sub(&self, b: &FMat) -> FMat {
        assert_eq!((self.rows, self.cols), (b.rows, b.cols));
        FMat {
            rows: self.rows,
            cols: self.cols,
            data: self.data.iter().zip(&b.data).map(|(x, y)| x - y).collect(),
        }
    }

    /// Frobenius norm.
    pub fn frob_norm(&self) -> f64 {
        self.data.iter().map(|x| x * x).sum::<f64>().sqrt()
    }

    /// Column `j` as a vector.
    fn col(&self, j: usize) -> Vec<f64> {
        (0..self.rows).map(|i| self.get(i, j)).collect()
    }

    pub fn identity(n: usize) -> FMat {
        let mut m = FMat::zeros(n, n);
        for i in 0..n {
            m.set(i, i, 1.0);
        }
        m
    }

    pub fn add(&self, b: &FMat) -> FMat {
        FMat {
            rows: self.rows,
            cols: self.cols,
            data: self.data.iter().zip(&b.data).map(|(x, y)| x + y).collect(),
        }
    }
    pub fn scale(&self, s: f64) -> FMat {
        FMat {
            rows: self.rows,
            cols: self.cols,
            data: self.data.iter().map(|x| x * s).collect(),
        }
    }

    /// f64 inverse via Gauss–Jordan with partial pivoting. `None` if (near-)singular.
    pub fn inverse(&self) -> Option<FMat> {
        let n = self.rows;
        if n != self.cols {
            return None;
        }
        let mut a = self.data.clone();
        let mut inv = FMat::identity(n).data;
        let idx = |i: usize, j: usize| i * n + j;
        for col in 0..n {
            // partial pivot: largest |a[r][col]|
            let mut piv = col;
            let mut best = a[idx(col, col)].abs();
            for r in (col + 1)..n {
                let v = a[idx(r, col)].abs();
                if v > best {
                    best = v;
                    piv = r;
                }
            }
            if best < 1e-300 {
                return None;
            }
            if piv != col {
                for k in 0..n {
                    a.swap(idx(col, k), idx(piv, k));
                    inv.swap(idx(col, k), idx(piv, k));
                }
            }
            let p = a[idx(col, col)];
            for k in 0..n {
                a[idx(col, k)] /= p;
                inv[idx(col, k)] /= p;
            }
            for r in 0..n {
                if r == col {
                    continue;
                }
                let f = a[idx(r, col)];
                if f == 0.0 {
                    continue;
                }
                for k in 0..n {
                    a[idx(r, k)] -= f * a[idx(col, k)];
                    inv[idx(r, k)] -= f * inv[idx(col, k)];
                }
            }
        }
        Some(FMat { rows: n, cols: n, data: inv })
    }

    pub fn is_symmetric(&self, tol: f64) -> bool {
        if self.rows != self.cols {
            return false;
        }
        let n = self.rows;
        for i in 0..n {
            for j in (i + 1)..n {
                if (self.get(i, j) - self.get(j, i)).abs() > tol {
                    return false;
                }
            }
        }
        true
    }

    /// LDLᵀ diagonal `D` (sqrt-free Cholesky), or `None` on a near-zero pivot.
    /// `A` is PSD iff all `D[i] ≥ −tol`; positive-definite iff all `D[i] > tol`.
    pub fn ldlt_diag(&self) -> Option<Vec<f64>> {
        let n = self.rows;
        if n != self.cols {
            return None;
        }
        let mut l = FMat::identity(n);
        let mut d = vec![0.0; n];
        for j in 0..n {
            let mut dj = self.get(j, j);
            for (k, &dk) in d.iter().enumerate().take(j) {
                dj -= l.get(j, k) * l.get(j, k) * dk;
            }
            if dj.abs() < 1e-300 {
                return None;
            }
            d[j] = dj;
            for i in (j + 1)..n {
                let mut s = self.get(i, j);
                for (k, &dk) in d.iter().enumerate().take(j) {
                    s -= l.get(i, k) * l.get(j, k) * dk;
                }
                l.set(i, j, s / dj);
            }
        }
        Some(d)
    }
}

/// Symmetric eigendecomposition via the cyclic Jacobi algorithm. Returns
/// `(eigenvalues, eigenvectors)` sorted by **descending** eigenvalue, eigenvectors as
/// the columns of the returned matrix. Robust for small symmetric matrices; used by the
/// spectral kernels (BBP spike, sparse PCA, spectral clustering eigengaps, …). Assumes
/// `a` is symmetric (callers symmetrize first).
pub fn jacobi_eig(a: &FMat) -> (Vec<f64>, FMat) {
    let n = a.rows;
    let mut m = a.clone();
    let mut v = FMat::identity(n);
    if n == 0 {
        return (vec![], v);
    }
    for _ in 0..100 {
        // largest off-diagonal magnitude
        let mut off = 0.0;
        let (mut p, mut q) = (0usize, 1usize.min(n - 1));
        for i in 0..n {
            for j in (i + 1)..n {
                if m.get(i, j).abs() > off {
                    off = m.get(i, j).abs();
                    p = i;
                    q = j;
                }
            }
        }
        if off < 1e-14 || n == 1 {
            break;
        }
        let app = m.get(p, p);
        let aqq = m.get(q, q);
        let apq = m.get(p, q);
        let theta = 0.5 * (aqq - app) / apq;
        let t = theta.signum() / (theta.abs() + (theta * theta + 1.0).sqrt());
        let c = 1.0 / (t * t + 1.0).sqrt();
        let s = t * c;
        // apply rotation J^T M J
        for i in 0..n {
            let mip = m.get(i, p);
            let miq = m.get(i, q);
            m.set(i, p, c * mip - s * miq);
            m.set(i, q, s * mip + c * miq);
        }
        for j in 0..n {
            let mpj = m.get(p, j);
            let mqj = m.get(q, j);
            m.set(p, j, c * mpj - s * mqj);
            m.set(q, j, s * mpj + c * mqj);
        }
        // accumulate eigenvectors
        for i in 0..n {
            let vip = v.get(i, p);
            let viq = v.get(i, q);
            v.set(i, p, c * vip - s * viq);
            v.set(i, q, s * vip + c * viq);
        }
    }
    let mut pairs: Vec<(f64, usize)> = (0..n).map(|i| (m.get(i, i), i)).collect();
    pairs.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    let eigvals: Vec<f64> = pairs.iter().map(|&(e, _)| e).collect();
    let mut eigvecs = FMat::zeros(n, n);
    for (newcol, &(_, oldcol)) in pairs.iter().enumerate() {
        for i in 0..n {
            eigvecs.set(i, newcol, v.get(i, oldcol));
        }
    }
    (eigvals, eigvecs)
}

/// Solve a dense `A x = b` in f64 via Gauss elimination with partial pivoting.
/// `None` if (near-)singular. (Used for the Lyapunov Kronecker system.)
pub fn solve_dense(a: &FMat, b: &[f64]) -> Option<Vec<f64>> {
    let n = a.rows;
    if a.cols != n || b.len() != n {
        return None;
    }
    let mut m = a.data.clone();
    let mut rhs = b.to_vec();
    let idx = |i: usize, j: usize| i * n + j;
    for col in 0..n {
        let mut piv = col;
        let mut best = m[idx(col, col)].abs();
        for r in (col + 1)..n {
            let v = m[idx(r, col)].abs();
            if v > best {
                best = v;
                piv = r;
            }
        }
        if best < 1e-300 {
            return None;
        }
        if piv != col {
            for k in 0..n {
                m.swap(idx(col, k), idx(piv, k));
            }
            rhs.swap(col, piv);
        }
        let p = m[idx(col, col)];
        for r in (col + 1)..n {
            let f = m[idx(r, col)] / p;
            if f == 0.0 {
                continue;
            }
            for k in col..n {
                m[idx(r, k)] -= f * m[idx(col, k)];
            }
            rhs[r] -= f * rhs[col];
        }
    }
    // back-substitution
    let mut x = vec![0.0; n];
    for i in (0..n).rev() {
        let mut s = rhs[i];
        for j in (i + 1)..n {
            s -= m[idx(i, j)] * x[j];
        }
        x[i] = s / m[idx(i, i)];
    }
    Some(x)
}

/// Deterministic splitmix64 PRNG with a standard-normal generator (Box–Muller).
pub struct Rng {
    state: u64,
}

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng { state: seed }
    }
    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    fn next_unit(&mut self) -> f64 {
        // uniform in (0,1)
        ((self.next_u64() >> 11) as f64 + 0.5) / (1u64 << 53) as f64
    }
    pub fn gaussian(&mut self) -> f64 {
        let u1 = self.next_unit();
        let u2 = self.next_unit();
        (-2.0 * u1.ln()).sqrt() * (std::f64::consts::TAU * u2).cos()
    }
}

/// An `rows × cols` standard Gaussian matrix from `seed` (deterministic, R11).
pub fn gaussian_matrix(rows: usize, cols: usize, seed: u64) -> FMat {
    let mut rng = Rng::new(seed);
    let mut m = FMat::zeros(rows, cols);
    for x in m.data.iter_mut() {
        *x = rng.gaussian();
    }
    m
}

/// Thin QR via modified Gram–Schmidt: returns `Q` with orthonormal columns spanning
/// the column space of `a` (columns whose norm collapses below `tol` are dropped).
pub fn qr_q(a: &FMat) -> FMat {
    let m = a.rows;
    let n = a.cols;
    let mut qcols: Vec<Vec<f64>> = Vec::new();
    for j in 0..n {
        let mut v = a.col(j);
        // subtract projections onto previous q's
        for q in &qcols {
            let dot: f64 = v.iter().zip(q).map(|(x, y)| x * y).sum();
            for (vi, qi) in v.iter_mut().zip(q) {
                *vi -= dot * qi;
            }
        }
        let norm = v.iter().map(|x| x * x).sum::<f64>().sqrt();
        if norm > 1e-12 {
            for vi in v.iter_mut() {
                *vi /= norm;
            }
            qcols.push(v);
        }
    }
    let k = qcols.len();
    let mut q = FMat::zeros(m, k);
    for (j, qc) in qcols.iter().enumerate() {
        for (i, &val) in qc.iter().enumerate() {
            q.set(i, j, val);
        }
    }
    q
}

/// Randomized range finder (Halko–Martinsson–Tropp): `Q` whose columns approximate
/// the dominant rank-`k` range of `a`, using `k+p` Gaussian samples. Deterministic
/// (seeded). HMT 2011 Thm 10.5 (Frobenius expectation):
/// `E‖A − QQᵀA‖_F ≤ (1 + k/(p−1))^{1/2} (Σ_{j>k} σ_j²)^{1/2}` (p ≥ 2). We do NOT rely
/// on this bound for correctness — the residual is measured (see `low_rank_approx`).
pub fn randomized_range(a: &FMat, k: usize, p: usize, seed: u64) -> FMat {
    let samples = (k + p).min(a.cols);
    let omega = gaussian_matrix(a.cols, samples, seed);
    let y = a.matmul(&omega);
    qr_q(&y)
}

/// Rank-`k` approximation `Â = Q (Qᵀ A)` and its **measured** Frobenius residual
/// `‖A − Â‖_F`. The residual is the certificate quantity — exact-as-f64-allows.
pub fn low_rank_approx(a: &FMat, q: &FMat) -> (FMat, f64) {
    let qt_a = q.transpose().matmul(a);
    let approx = q.matmul(&qt_a);
    let residual = a.sub(&approx).frob_norm();
    (approx, residual)
}

/// Sparse symmetric matrix in coordinate form, for Krylov (CG).
#[derive(Clone, Debug)]
pub struct Sparse {
    pub n: usize,
    pub entries: Vec<(usize, usize, f64)>,
}

impl Sparse {
    pub fn new(n: usize) -> Self {
        Sparse {
            n,
            entries: Vec::new(),
        }
    }
    pub fn push(&mut self, i: usize, j: usize, v: f64) {
        self.entries.push((i, j, v));
    }
    pub fn nnz(&self) -> usize {
        self.entries.len()
    }
    pub fn matvec(&self, x: &[f64]) -> Vec<f64> {
        let mut y = vec![0.0; self.n];
        for &(i, j, v) in &self.entries {
            y[i] += v * x[j];
        }
        y
    }
}

/// L2 residual `‖Ax − b‖₂`.
pub fn lin_residual(a: &Sparse, x: &[f64], b: &[f64]) -> f64 {
    let ax = a.matvec(x);
    ax.iter()
        .zip(b)
        .map(|(p, q)| (p - q) * (p - q))
        .sum::<f64>()
        .sqrt()
}

/// Conjugate gradients for SPD `A x = b`. Returns `(x, residual)` after at most
/// `max_iter` iterations; the caller checks `residual ≤ tol` and otherwise falls
/// back (P0). Krylov/CG: the residual `‖Ax−b‖` is the deterministic certificate.
pub fn cg_solve(a: &Sparse, b: &[f64], max_iter: usize) -> (Vec<f64>, f64) {
    let n = a.n;
    let mut x = vec![0.0; n];
    let mut r: Vec<f64> = b.to_vec(); // r = b - A*0
    let mut p = r.clone();
    let mut rs_old: f64 = r.iter().map(|v| v * v).sum();
    for _ in 0..max_iter {
        if rs_old.sqrt() <= 1e-300 {
            break;
        }
        let ap = a.matvec(&p);
        let pap: f64 = p.iter().zip(&ap).map(|(x, y)| x * y).sum();
        if pap.abs() <= 1e-300 {
            break;
        }
        let alpha = rs_old / pap;
        for i in 0..n {
            x[i] += alpha * p[i];
            r[i] -= alpha * ap[i];
        }
        let rs_new: f64 = r.iter().map(|v| v * v).sum();
        let beta = rs_new / rs_old;
        for i in 0..n {
            p[i] = r[i] + beta * p[i];
        }
        rs_old = rs_new;
    }
    let residual = lin_residual(a, &x, b);
    (x, residual)
}

/// Singular values of a row-major `rows×cols` matrix, **descending**, via the eigenvalues
/// of the smaller Gram matrix (`AᵀA` or `AAᵀ`). Floats: exact up to the Jacobi tolerance.
pub fn singular_values(data: &[f64], rows: usize, cols: usize) -> Vec<f64> {
    assert_eq!(data.len(), rows * cols, "data length must be rows*cols");
    let (g, dim) = if cols <= rows {
        let mut g = vec![0.0; cols * cols];
        for i in 0..cols {
            for j in 0..cols {
                let mut s = 0.0;
                for k in 0..rows {
                    s += data[k * cols + i] * data[k * cols + j];
                }
                g[i * cols + j] = s;
            }
        }
        (g, cols)
    } else {
        let mut g = vec![0.0; rows * rows];
        for i in 0..rows {
            for j in 0..rows {
                let mut s = 0.0;
                for k in 0..cols {
                    s += data[i * cols + k] * data[j * cols + k];
                }
                g[i * rows + j] = s;
            }
        }
        (g, rows)
    };
    let (eig, _) = jacobi_eig(&FMat::from_data(dim, dim, g));
    eig.iter().map(|&l| l.max(0.0).sqrt()).collect()
}

/// `true` iff the matrix has **no** rank-≤`r` approximation within spectral error `eps` —
/// i.e. σ_{r+1} > eps (Stage 15.3 #2; best rank-r Frobenius error ≥ σ_{r+1}). A tolerance
/// statement, labeled (low-rank, (r, eps)).
pub fn excludes_rank(data: &[f64], rows: usize, cols: usize, r: usize, eps: f64) -> bool {
    let sv = singular_values(data, rows, cols);
    sv.get(r).map(|&s| s > eps).unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build A = U diag(sv) Uᵀ for a random orthonormal-ish U, with chosen singular
    /// values — so we know the spectrum without computing an SVD (the oracle).
    fn matrix_with_spectrum(n: usize, sv: &[f64], seed: u64) -> FMat {
        // random Gaussian then QR to get an orthonormal U (n×n)
        let g = gaussian_matrix(n, n, seed);
        let u = qr_q(&g); // n×n orthonormal columns (full rank w.h.p.)
        let mut s = FMat::zeros(n, n);
        for (i, &val) in sv.iter().enumerate().take(n) {
            s.set(i, i, val);
        }
        u.matmul(&s).matmul(&u.transpose())
    }

    #[test]
    fn rsvd_small_residual_for_fast_decay() {
        // sharp decay: rank ~3, rest tiny → randomized range should capture it.
        let mut sv = vec![1e-7; 16];
        sv[0] = 10.0;
        sv[1] = 5.0;
        sv[2] = 2.0;
        let a = matrix_with_spectrum(16, &sv, 42);
        let q = randomized_range(&a, 3, 5, 7);
        let (_approx, residual) = low_rank_approx(&a, &q);
        assert!(residual < 1e-3, "fast-decay residual should be small, got {residual}");
    }

    #[test]
    fn rsvd_large_residual_for_flat_spectrum() {
        // flat spectrum (white-noise-like) of dim 16: a rank-8 range leaves ~8 unit
        // singular values uncaptured ⇒ residual ≈ sqrt(8) ≫ 1 ⇒ not low-rank.
        let sv = vec![1.0; 16];
        let a = matrix_with_spectrum(16, &sv, 1);
        let q = randomized_range(&a, 3, 5, 7); // 8 samples of a 16-dim space
        let (_approx, residual) = low_rank_approx(&a, &q);
        assert!(residual > 1.5, "flat spectrum has no low-rank approx, got {residual}");
    }

    #[test]
    fn cg_solves_spd_system() {
        // A = [[4,1],[1,3]] SPD, b = [1,2]; solution residual must be tiny.
        let mut a = Sparse::new(2);
        a.push(0, 0, 4.0);
        a.push(0, 1, 1.0);
        a.push(1, 0, 1.0);
        a.push(1, 1, 3.0);
        let b = [1.0, 2.0];
        let (x, residual) = cg_solve(&a, &b, 100);
        assert!(residual < 1e-9, "CG residual {residual}");
        // sanity: A x ≈ b
        let ax = a.matvec(&x);
        assert!((ax[0] - 1.0).abs() < 1e-9 && (ax[1] - 2.0).abs() < 1e-9);
    }
}
