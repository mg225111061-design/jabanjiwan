//! Stage-6A Batch-1 sparse / low-rank recovery kernels (CLAUDE.md PART B Batch 1).
//!
//! Each kernel exploits a *hidden structure* (sparsity / low rank) detected at runtime.
//! The discipline (PART A): naive-correct oracle first; structure measured, not assumed;
//! the certificate is a **measured residual recomputed against the exact oracle**, so
//! correctness never depends on a probabilistic bound; structureless input → defer.
//!
//! These are the kernels; the certificate *checkers* (the independent re-derivation)
//! live in `jeff-verify`, and the precondition/defer dispatch lives in the collapser.

// Index-based loops read more clearly than iterators for these
// matrix / recurrence kernels; allow it module-wide (numeric code).
#![allow(clippy::needless_range_loop)]

use crate::fmat::{solve_dense, FMat, Rng};

// ---- 1.1 Compressed sensing: ℓ₁ / greedy sparse recovery (Candès–Tao 2005) ----

/// Recover a k-sparse `x` from `y = Φx` by Orthogonal Matching Pursuit (a greedy
/// surrogate for ℓ₁ basis pursuit; exact under RIP, Candès 2008 δ_2k<√2−1). Returns
/// the dense length-`cols` vector with ≤k nonzeros. The *certificate* (checked
/// elsewhere) is `‖Φx−y‖₂ ≤ ε ∧ ‖x‖₀ ≤ k` — independent of how `x` was found.
pub fn omp(phi: &FMat, y: &[f64], k: usize) -> Vec<f64> {
    let m = phi.rows;
    let n = phi.cols;
    debug_assert_eq!(y.len(), m);
    let mut residual = y.to_vec();
    let mut support: Vec<usize> = Vec::new();
    let mut x = vec![0.0; n];

    for _ in 0..k.min(n) {
        // pick the column most correlated with the current residual
        let mut best_j = usize::MAX;
        let mut best_c = 0.0;
        for j in 0..n {
            if support.contains(&j) {
                continue;
            }
            let mut c = 0.0;
            for i in 0..m {
                c += phi.get(i, j) * residual[i];
            }
            if c.abs() > best_c {
                best_c = c.abs();
                best_j = j;
            }
        }
        if best_j == usize::MAX || best_c < 1e-12 {
            break;
        }
        support.push(best_j);

        // least squares on the selected support via normal equations GᵀG a = Gᵀy
        let s = support.len();
        let mut g = FMat::zeros(m, s);
        for (col, &j) in support.iter().enumerate() {
            for i in 0..m {
                g.set(i, col, phi.get(i, j));
            }
        }
        let gt = g.transpose();
        let gram = gt.matmul(&g);
        let rhs: Vec<f64> = (0..s)
            .map(|col| (0..m).map(|i| g.get(i, col) * y[i]).sum())
            .collect();
        let Some(coef) = solve_dense(&gram, &rhs) else {
            break;
        };
        // update x and residual
        for v in x.iter_mut() {
            *v = 0.0;
        }
        for (col, &j) in support.iter().enumerate() {
            x[j] = coef[col];
        }
        for i in 0..m {
            let mut ax = 0.0;
            for (col, &j) in support.iter().enumerate() {
                ax += phi.get(i, j) * coef[col];
            }
            residual[i] = y[i] - ax;
        }
    }
    x
}

/// `Φx` (the oracle used by the checker to recompute the residual).
pub fn apply(phi: &FMat, x: &[f64]) -> Vec<f64> {
    (0..phi.rows)
        .map(|i| (0..phi.cols).map(|j| phi.get(i, j) * x[j]).sum())
        .collect()
}

/// Count of nonzeros above a magnitude floor (the ℓ₀ "sparsity" used in the cert).
pub fn nnz(x: &[f64], floor: f64) -> usize {
    x.iter().filter(|v| v.abs() > floor).count()
}

// ---- 1.2 Sparse FFT (HIKP 2012): recover a k-sparse spectrum ----

/// Naive DFT of a real signal → `(re, im)` per frequency (the O(n²) oracle; the
/// sublinear HIKP binning is a Stage-6B promotion concern — the *certificate*
/// (residual vs this oracle) is identical regardless of how the spectrum is found).
pub fn dft_real(x: &[f64]) -> (Vec<f64>, Vec<f64>) {
    let n = x.len();
    let mut re = vec![0.0; n];
    let mut im = vec![0.0; n];
    let two_pi = std::f64::consts::TAU;
    for (f, (rf, imf)) in re.iter_mut().zip(im.iter_mut()).enumerate() {
        for (t, &xt) in x.iter().enumerate() {
            let ang = -two_pi * (f as f64) * (t as f64) / (n as f64);
            *rf += xt * ang.cos();
            *imf += xt * ang.sin();
        }
    }
    (re, im)
}

/// Reconstruct a real signal from a sparse spectrum support `(freq, re, im)` by the
/// inverse DFT (the checker's oracle). Returns the real part of length `n`.
pub fn idft_sparse(support: &[(usize, f64, f64)], n: usize) -> Vec<f64> {
    let two_pi = std::f64::consts::TAU;
    let mut x = vec![0.0; n];
    for (t, xt) in x.iter_mut().enumerate() {
        let mut acc = 0.0;
        for &(f, re, im) in support {
            let ang = two_pi * (f as f64) * (t as f64) / (n as f64);
            // Re[(re+i·im)·e^{iθ}] = re·cosθ − im·sinθ
            acc += re * ang.cos() - im * ang.sin();
        }
        *xt = acc / n as f64;
    }
    x
}

/// Recover the `k` heaviest spectral bins of a real signal (support = the k largest
/// `|X[f]|`). Returns `(freq, re, im)` triples.
pub fn sparse_fft(x: &[f64], k: usize) -> Vec<(usize, f64, f64)> {
    let (re, im) = dft_real(x);
    let n = x.len();
    let mut idx: Vec<usize> = (0..n).collect();
    idx.sort_by(|&a, &b| {
        let ma = re[b] * re[b] + im[b] * im[b];
        let mb = re[a] * re[a] + im[a] * im[a];
        ma.partial_cmp(&mb).unwrap_or(std::cmp::Ordering::Equal)
    });
    idx.into_iter()
        .take(k.min(n))
        .map(|f| (f, re[f], im[f]))
        .collect()
}

/// ℓ₂ norm of a real vector.
pub fn l2(x: &[f64]) -> f64 {
    x.iter().map(|v| v * v).sum::<f64>().sqrt()
}

// ---- 1.3 Matrix completion: nuclear-norm surrogate via soft-impute (Candès–Recht) ----

/// Complete a partially-observed `rows×cols` matrix to rank ≤ `r` by alternating
/// least squares (ALS): fix `V`, solve each row of `U` from its observed entries; fix
/// `U`, solve each column of `V`; repeat. Returns factors `(U, V)` with `U: rows×r`,
/// `V: cols×r`, so the completion is `U·Vᵀ`. Deterministic (fixed seed, R11). A tiny
/// ridge keeps the r×r solves stable. The certificate (checked elsewhere) is
/// `‖UVᵀ−M‖_Ω ≤ ε ∧ rank ≤ r`.
pub fn complete(
    observed: &[(usize, usize, f64)],
    rows: usize,
    cols: usize,
    r: usize,
    iters: usize,
) -> (FMat, FMat) {
    let lambda = 1e-8;
    // group observations by row and by column
    let mut by_row: Vec<Vec<(usize, f64)>> = vec![Vec::new(); rows];
    let mut by_col: Vec<Vec<(usize, f64)>> = vec![Vec::new(); cols];
    for &(i, j, v) in observed {
        by_row[i].push((j, v));
        by_col[j].push((i, v));
    }
    let mut rng = Rng::new(0xC0FFEE);
    let mut u = FMat::zeros(rows, r);
    let mut v = FMat::zeros(cols, r);
    for i in 0..rows {
        for l in 0..r {
            u.set(i, l, rng.gaussian() * 0.1);
        }
    }
    for j in 0..cols {
        for l in 0..r {
            v.set(j, l, rng.gaussian() * 0.1);
        }
    }

    // solve (Σ wₐwₐᵀ + λI) z = Σ valₐ wₐ for the r-vector z (one factor row/col)
    let solve_factor = |obs: &[(usize, f64)], other: &FMat| -> Vec<f64> {
        let mut a = FMat::zeros(r, r);
        let mut b = vec![0.0; r];
        for &(idx, val) in obs {
            for p in 0..r {
                let wp = other.get(idx, p);
                b[p] += val * wp;
                for q in 0..r {
                    a.set(p, q, a.get(p, q) + wp * other.get(idx, q));
                }
            }
        }
        for p in 0..r {
            a.set(p, p, a.get(p, p) + lambda);
        }
        solve_dense(&a, &b).unwrap_or_else(|| vec![0.0; r])
    };

    for _ in 0..iters {
        for i in 0..rows {
            let zi = solve_factor(&by_row[i], &v);
            for l in 0..r {
                u.set(i, l, zi[l]);
            }
        }
        for j in 0..cols {
            let zj = solve_factor(&by_col[j], &u);
            for l in 0..r {
                v.set(j, l, zj[l]);
            }
        }
    }
    (u, v)
}

/// `(U·Vᵀ)_{ij}` (the oracle the checker uses on observed entries).
pub fn factor_entry(u: &FMat, v: &FMat, i: usize, j: usize) -> f64 {
    (0..u.cols).map(|l| u.get(i, l) * v.get(j, l)).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn omp_recovers_exact_sparse_signal() {
        // Φ random 12×24, plant a 3-sparse x, y=Φx; OMP must recover it (residual≈0).
        let m = 12;
        let n = 24;
        let phi = crate::fmat::gaussian_matrix(m, n, 0xABCDEF);
        let mut x = vec![0.0; n];
        x[2] = 3.0;
        x[7] = -1.5;
        x[19] = 2.0;
        let y = apply(&phi, &x);
        let xr = omp(&phi, &y, 3);
        let resid = l2(&apply(&phi, &xr).iter().zip(&y).map(|(a, b)| a - b).collect::<Vec<_>>());
        assert!(resid < 1e-6, "OMP residual {resid} too large");
        assert!(nnz(&xr, 1e-6) <= 3);
    }

    #[test]
    fn omp_dense_signal_leaves_large_residual() {
        // A dense (non-sparse) target: k=3 OMP cannot explain it → residual stays up.
        let m = 8;
        let n = 16;
        let phi = crate::fmat::gaussian_matrix(m, n, 0x1111);
        let dense: Vec<f64> = (0..n).map(|i| (i as f64 * 0.7).sin() + 0.5).collect();
        let y = apply(&phi, &dense);
        let xr = omp(&phi, &y, 3);
        let resid = l2(&apply(&phi, &xr).iter().zip(&y).map(|(a, b)| a - b).collect::<Vec<_>>());
        assert!(resid > 1e-3, "dense signal should NOT be 3-sparse recoverable");
    }

    #[test]
    fn sparse_fft_recovers_few_tones() {
        // x = cos(2π·3 t/n) + 0.5 cos(2π·7 t/n): exactly 4 nonzero bins (±3, ±7).
        let n = 32;
        let two_pi = std::f64::consts::TAU;
        let x: Vec<f64> = (0..n)
            .map(|t| {
                (two_pi * 3.0 * t as f64 / n as f64).cos()
                    + 0.5 * (two_pi * 7.0 * t as f64 / n as f64).cos()
            })
            .collect();
        let support = sparse_fft(&x, 4);
        let recon = idft_sparse(&support, n);
        let diff: Vec<f64> = x.iter().zip(&recon).map(|(a, b)| a - b).collect();
        assert!(l2(&diff) < 1e-6 * l2(&x), "4-sparse reconstruction must match");
    }

    #[test]
    fn matrix_completion_recovers_low_rank() {
        // rank-1 M = u vᵀ, observe 70% of entries, complete to rank 1.
        let rows = 10;
        let cols = 10;
        let u: Vec<f64> = (0..rows).map(|i| 1.0 + i as f64 * 0.1).collect();
        let v: Vec<f64> = (0..cols).map(|j| 2.0 - j as f64 * 0.05).collect();
        let mut observed = Vec::new();
        for i in 0..rows {
            for j in 0..cols {
                if (i * 7 + j * 3) % 10 < 7 {
                    observed.push((i, j, u[i] * v[j]));
                }
            }
        }
        let (uu, vv) = complete(&observed, rows, cols, 1, 30);
        // check on a HELD-OUT entry not in the observed set
        let mut err = 0.0;
        let mut cnt = 0;
        for i in 0..rows {
            for j in 0..cols {
                if (i * 7 + j * 3) % 10 >= 7 {
                    let got = factor_entry(&uu, &vv, i, j);
                    err += (got - u[i] * v[j]).powi(2);
                    cnt += 1;
                }
            }
        }
        let rmse = (err / cnt as f64).sqrt();
        assert!(rmse < 1e-3, "held-out RMSE {rmse} too large for rank-1 completion");
    }
}
