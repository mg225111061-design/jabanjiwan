//! Stage-6A Batch-2: planted / spiked detection (CLAUDE.md PART B Batch 2).
//!
//! Each kernel has a *proven detectability threshold* that JEFF uses as a defer
//! boundary: structure above threshold → collapse with a certificate; below → defer.
//! Several certificates are `threshold-conditional`; planted-clique's is `exact` (the
//! recovered set is literally checked to be a clique). The spiked-tensor kernel teaches
//! the **3-way** trichotomy: detectable / in-the-gap (open) / undetectable.

// Index-based loops read clearly for these spectral/matrix kernels.
#![allow(clippy::needless_range_loop)]

use crate::fmat::{jacobi_eig, FMat};

// ---- shared helpers ----

/// Build a symmetric `FMat` from a flat row-major `n×n` adjacency (`u8` 0/1).
pub fn adjacency_fmat(adj: &[u8], n: usize) -> FMat {
    let mut m = FMat::zeros(n, n);
    for i in 0..n {
        for j in 0..n {
            m.set(i, j, adj[i * n + j] as f64);
        }
    }
    m
}

/// Top two eigenvalues of a symmetric flat `p×p` matrix (descending).
pub fn top_two_eigenvalues(sym: &[f64], p: usize) -> (f64, f64) {
    let m = FMat::from_data(p, p, sym.to_vec());
    let (ev, _) = jacobi_eig(&m);
    let l1 = ev.first().copied().unwrap_or(0.0);
    let l2 = ev.get(1).copied().unwrap_or(0.0);
    (l1, l2)
}

// ---- 2.1 BBP spiked-matrix detection ----

/// The Baik–Ben Arous–Péché bulk edge `(1+√γ)²` plus a Tracy–Widom margin, `γ=p/n`.
pub fn bbp_threshold(p: usize, n: usize) -> f64 {
    let gamma = p as f64 / n as f64;
    let edge = (1.0 + gamma.sqrt()).powi(2);
    edge + 2.0 * (n as f64).powf(-2.0 / 3.0) // c·n^{-2/3} margin
}

/// Sample covariance `(1/n) Σ xₖ xₖᵀ` from `n` samples × `p` features (row-major data).
pub fn sample_covariance(data: &[f64], n: usize, p: usize) -> Vec<f64> {
    let mut cov = vec![0.0; p * p];
    for k in 0..n {
        for i in 0..p {
            let xi = data[k * p + i];
            for j in 0..p {
                cov[i * p + j] += xi * data[k * p + j];
            }
        }
    }
    for c in cov.iter_mut() {
        *c /= n as f64;
    }
    cov
}

// ---- 2.2 Planted clique: spectral candidate + exact clique check ----

/// Spectral planted-clique recovery: top-k coordinates of the 2nd adjacency
/// eigenvector, cleaned up to a clique. Returns candidate vertices (a clique if the
/// planted size `k > c√n`). The *certificate* is the exact clique check, not this.
pub fn planted_clique(adj: &[u8], n: usize, k: usize) -> Vec<usize> {
    let m = adjacency_fmat(adj, n);
    let (_, vecs) = jacobi_eig(&m);
    // second eigenvector = column 1 (carries the clique signal over the all-ones bulk)
    let col = 1.min(n.saturating_sub(1));
    let mut idx: Vec<usize> = (0..n).collect();
    idx.sort_by(|&a, &b| {
        vecs.get(b, col)
            .abs()
            .partial_cmp(&vecs.get(a, col).abs())
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    // candidate window: top ~2k by eigenvector magnitude, then keep vertices adjacent
    // to ≥ 3k/4 of it (AKS cleanup — the planted clique lands in this set w.h.p.).
    let window: Vec<usize> = idx.into_iter().take((2 * k).min(n)).collect();
    let thresh = (3 * k) / 4;
    let mut c: Vec<usize> = (0..n)
        .filter(|&v| window.iter().filter(|&&u| u != v && adj[v * n + u] == 1).count() >= thresh)
        .collect();
    // peel: repeatedly drop the lowest within-C-degree vertex until C is a clique. This
    // GUARANTEES a clique output (so the exact certificate passes), and a planted clique
    // — whose vertices are mutually adjacent (max within-C degree) — survives the peel.
    while !is_clique(adj, n, &c) && c.len() > 1 {
        let worst = c
            .iter()
            .copied()
            .min_by_key(|&v| c.iter().filter(|&&u| u != v && adj[v * n + u] == 1).count())
            .unwrap();
        c.retain(|&v| v != worst);
    }
    c
}

/// Exact certificate check: is `set` a clique (all pairs adjacent, no self-pairs)?
pub fn is_clique(adj: &[u8], n: usize, set: &[usize]) -> bool {
    for a in 0..set.len() {
        for b in (a + 1)..set.len() {
            let (u, v) = (set[a], set[b]);
            if u >= n || v >= n || adj[u * n + v] != 1 {
                return false;
            }
        }
    }
    true
}

// ---- 2.3 Stochastic block model: spectral detection ----

/// Kesten–Stigum spectral threshold proxy `√(mean degree)` (bulk edge of the centered
/// adjacency). Detection requires the 2nd eigenvalue to exceed this.
pub fn sbm_threshold(adj: &[u8], n: usize) -> f64 {
    let edges: usize = adj.iter().map(|&x| x as usize).sum();
    let mean_deg = edges as f64 / n as f64;
    mean_deg.sqrt()
}

/// Second eigenvalue of the adjacency (the community-detection signal) and the sign
/// pattern of its eigenvector (the recovered 2-partition).
pub fn sbm_detect(adj: &[u8], n: usize) -> (f64, Vec<bool>) {
    let m = adjacency_fmat(adj, n);
    let (ev, vecs) = jacobi_eig(&m);
    let l2 = ev.get(1).copied().unwrap_or(0.0);
    let col = 1.min(n.saturating_sub(1));
    let part: Vec<bool> = (0..n).map(|i| vecs.get(i, col) >= 0.0).collect();
    (l2, part)
}

// ---- 2.4 Spiked tensor (3-way trichotomy) ----

/// Detectability regimes for an order-3 spike of strength `beta` in dimension `p`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TensorRegime {
    /// β ≳ p^{3/4}: efficient recovery possible.
    Efficient,
    /// √p ≲ β ≲ p^{3/4}: signal exists but no known efficient algorithm (open).
    StatCompGap,
    /// β ≲ √p: information-theoretically undetectable.
    Undetectable,
}

/// Classify by comparing the unfolded top singular value `sigma` to the two thresholds.
pub fn tensor_regime(sigma: f64, p: usize) -> TensorRegime {
    let pf = p as f64;
    let efficient = pf.powf(0.75);
    let it = pf.sqrt();
    if sigma >= efficient {
        TensorRegime::Efficient
    } else if sigma >= it {
        TensorRegime::StatCompGap
    } else {
        TensorRegime::Undetectable
    }
}

/// Unfold an order-3 tensor `T[i,j,k]` (flat `p³`, row-major) to a `p × p²` matrix and
/// return `(top singular value, top left singular vector)` via `MMᵀ`'s top eigenpair.
pub fn tensor_unfold_top(tensor: &[f64], p: usize) -> (f64, Vec<f64>) {
    // M (p × p²); MMᵀ (p × p) = Σ over (j,k) of outer products of mode-1 fibers.
    let mut mmt = vec![0.0; p * p];
    for i1 in 0..p {
        for i2 in 0..p {
            let mut acc = 0.0;
            for j in 0..p {
                for k in 0..p {
                    acc += tensor[(i1 * p + j) * p + k] * tensor[(i2 * p + j) * p + k];
                }
            }
            mmt[i1 * p + i2] = acc;
        }
    }
    let m = FMat::from_data(p, p, mmt);
    let (ev, vecs) = jacobi_eig(&m);
    let sigma = ev.first().copied().unwrap_or(0.0).max(0.0).sqrt();
    let v: Vec<f64> = (0..p).map(|i| vecs.get(i, 0)).collect();
    (sigma, v)
}

/// Residual `‖T − β·v⊗v⊗v‖_F` (the exploit-case certificate quantity).
pub fn rank1_tensor_residual(tensor: &[f64], v: &[f64], beta: f64, p: usize) -> f64 {
    let mut acc = 0.0;
    for i in 0..p {
        for j in 0..p {
            for k in 0..p {
                let approx = beta * v[i] * v[j] * v[k];
                let d = tensor[(i * p + j) * p + k] - approx;
                acc += d * d;
            }
        }
    }
    acc.sqrt()
}

// ---- 2.5 Sparse PCA: diagonal thresholding ----

/// Quadratic form `vᵀ Σ̂ v` for a flat `p×p` covariance and vector `v`.
pub fn quad_form(cov: &[f64], v: &[f64], p: usize) -> f64 {
    let mut acc = 0.0;
    for i in 0..p {
        for j in 0..p {
            acc += v[i] * cov[i * p + j] * v[j];
        }
    }
    acc
}

/// Diagonal-thresholding sparse PCA: pick the `k` highest-variance coordinates, take the
/// top eigenvector of that `k×k` principal submatrix, embed back. Returns a unit `v`
/// supported on ≤k coordinates.
pub fn sparse_pca(cov: &[f64], p: usize, k: usize) -> Vec<f64> {
    let mut diag: Vec<(f64, usize)> = (0..p).map(|i| (cov[i * p + i], i)).collect();
    diag.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    let support: Vec<usize> = diag.into_iter().take(k).map(|(_, i)| i).collect();
    // principal submatrix
    let kk = support.len();
    let mut sub = FMat::zeros(kk, kk);
    for (a, &i) in support.iter().enumerate() {
        for (b, &j) in support.iter().enumerate() {
            sub.set(a, b, cov[i * p + j]);
        }
    }
    let (_, vecs) = jacobi_eig(&sub);
    let mut v = vec![0.0; p];
    for (a, &i) in support.iter().enumerate() {
        v[i] = vecs.get(a, 0);
    }
    v
}

// ---- 2.6 Spectral refutation of random 2-XOR (honest negative, used positively) ----

/// Build the signed adjacency of a 2-XOR system: edge `(i,j)` with sign `(−1)^{b}`.
/// Returns a flat symmetric `n×n` matrix.
pub fn signed_adjacency(constraints: &[(usize, usize, u8)], n: usize) -> Vec<f64> {
    let mut a = vec![0.0; n * n];
    for &(i, j, b) in constraints {
        let s = if b == 0 { 1.0 } else { -1.0 };
        a[i * n + j] += s;
        a[j * n + i] += s;
    }
    a
}

/// Spectral upper bound on the number of simultaneously satisfiable 2-XOR constraints:
/// `max_x (sat − unsat) = max xᵀA_σx ≤ λ_max·n`, so `max sat ≤ (m + λ_max·n)/2`. If this
/// is `< m` the system is **unsatisfiable** — a checkable spectral witness.
pub fn xor_spectral_max_sat(signed_adj: &[f64], n: usize, m: usize) -> f64 {
    let (ev, _) = jacobi_eig(&FMat::from_data(n, n, signed_adj.to_vec()));
    let lambda_max = ev.first().copied().unwrap_or(0.0);
    (m as f64 + lambda_max * n as f64) / 2.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fmat::Rng;

    #[test]
    fn bbp_detects_spike_above_threshold() {
        // p features, n samples, plant a strong rank-1 spike in the covariance.
        let (n, p) = (200usize, 10usize);
        let mut rng = Rng::new(0xB0B);
        let spike: Vec<f64> = {
            let mut v: Vec<f64> = (0..p).map(|_| rng.gaussian()).collect();
            let nrm = (v.iter().map(|x| x * x).sum::<f64>()).sqrt();
            v.iter_mut().for_each(|x| *x /= nrm);
            v
        };
        let beta = 8.0_f64;
        let mut data = vec![0.0; n * p];
        for k in 0..n {
            let g = rng.gaussian();
            for i in 0..p {
                data[k * p + i] = rng.gaussian() + beta.sqrt() * g * spike[i];
            }
        }
        let cov = sample_covariance(&data, n, p);
        let (l1, l2) = top_two_eigenvalues(&cov, p);
        assert!(l1 > bbp_threshold(p, n), "spike eigenvalue {l1} should detach");
        assert!(l1 - l2 > 1.0, "gap should be visible");
    }

    #[test]
    fn bbp_no_spike_sticks_to_bulk() {
        let (n, p) = (400usize, 8usize);
        let mut rng = Rng::new(0x1234);
        let data: Vec<f64> = (0..n * p).map(|_| rng.gaussian()).collect();
        let cov = sample_covariance(&data, n, p);
        let (l1, _) = top_two_eigenvalues(&cov, p);
        // pure noise: top eigenvalue near the bulk edge, below the margin'd threshold
        assert!(l1 < bbp_threshold(p, n) + 0.6, "pure noise λ_max {l1}");
    }

    #[test]
    fn planted_clique_recovered_and_verified() {
        // n=50 G(n,1/2) with a planted clique of size 20 (≫ √50 ≈ 7.1, comfortable).
        let n = 50;
        let k = 20;
        let mut rng = Rng::new(0xC11);
        let mut adj = vec![0u8; n * n];
        for i in 0..n {
            for j in (i + 1)..n {
                let e = if rng.gaussian() > 0.0 { 1 } else { 0 };
                adj[i * n + j] = e;
                adj[j * n + i] = e;
            }
        }
        let clique: Vec<usize> = (0..k).collect();
        for a in 0..k {
            for b in (a + 1)..k {
                adj[clique[a] * n + clique[b]] = 1;
                adj[clique[b] * n + clique[a]] = 1;
            }
        }
        let found = planted_clique(&adj, n, k);
        assert!(is_clique(&adj, n, &found), "recovered set must be a clique");
        assert!(found.len() >= k, "recovered clique {} should be ≥ planted {k}", found.len());
    }

    #[test]
    fn is_clique_rejects_non_clique() {
        let n = 3;
        // triangle missing edge (0,2)
        let adj = vec![0, 1, 0, 1, 0, 1, 0, 1, 0];
        assert!(!is_clique(&adj, n, &[0, 1, 2]));
        assert!(is_clique(&adj, n, &[0, 1]));
    }

    #[test]
    fn tensor_trichotomy_classifies() {
        let p = 16;
        // p^{3/4}=8, √p=4
        assert_eq!(tensor_regime(10.0, p), TensorRegime::Efficient);
        assert_eq!(tensor_regime(5.0, p), TensorRegime::StatCompGap);
        assert_eq!(tensor_regime(2.0, p), TensorRegime::Undetectable);
    }

    #[test]
    fn spiked_tensor_recovers_strong_spike() {
        let p = 6;
        let mut rng = Rng::new(0x7);
        let mut v: Vec<f64> = (0..p).map(|_| rng.gaussian()).collect();
        let nrm = (v.iter().map(|x| x * x).sum::<f64>()).sqrt();
        v.iter_mut().for_each(|x| *x /= nrm);
        let beta = 40.0;
        let mut t = vec![0.0; p * p * p];
        for i in 0..p {
            for j in 0..p {
                for k in 0..p {
                    t[(i * p + j) * p + k] = beta * v[i] * v[j] * v[k] + 0.01 * rng.gaussian();
                }
            }
        }
        let (sigma, vhat) = tensor_unfold_top(&t, p);
        assert!(sigma > p as f64, "strong spike σ={sigma}");
        // residual with recovered v and β≈σ should be small
        let r = rank1_tensor_residual(&t, &vhat, sigma, p);
        // align sign
        let r2 = rank1_tensor_residual(&t, &vhat.iter().map(|x| -x).collect::<Vec<_>>(), sigma, p);
        assert!(r.min(r2) < beta, "residual {} should be << signal", r.min(r2));
    }

    #[test]
    fn sparse_pca_finds_planted_support() {
        // covariance = I + β e_S e_Sᵀ with support {1,4,7}
        let p = 10;
        let support = [1usize, 4, 7];
        let mut u = vec![0.0; p];
        for &i in &support {
            u[i] = 1.0 / (support.len() as f64).sqrt();
        }
        let beta = 20.0;
        let mut cov = vec![0.0; p * p];
        for i in 0..p {
            cov[i * p + i] = 1.0;
        }
        for i in 0..p {
            for j in 0..p {
                cov[i * p + j] += beta * u[i] * u[j];
            }
        }
        let v = sparse_pca(&cov, p, 3);
        let nz: Vec<usize> = (0..p).filter(|&i| v[i].abs() > 1e-9).collect();
        assert_eq!(nz.len(), 3);
        assert!(quad_form(&cov, &v, p) > 5.0, "support should capture the spike");
    }

    #[test]
    fn xor_refutation_certifies_random_dense_unsat() {
        // random dense 2-XOR (m ≫ n) is w.h.p. unsatisfiable; the spectral bound bites:
        // λ_max·n < m ⇒ max satisfiable < m ⇒ certifiably UNSAT (the honest negative).
        let n = 20;
        let mut rng = Rng::new(0xFEED);
        let m = 400;
        let mut constraints = Vec::new();
        for _ in 0..m {
            let i = (rng.next_u64() as usize) % n;
            let mut j = (rng.next_u64() as usize) % n;
            if j == i {
                j = (j + 1) % n;
            }
            let b = (rng.next_u64() & 1) as u8;
            constraints.push((i, j, b));
        }
        let sa = signed_adjacency(&constraints, n);
        let max_sat = xor_spectral_max_sat(&sa, n, m);
        assert!(max_sat < m as f64, "spectral bound {max_sat} < m={m} ⇒ UNSAT witness");
    }
}
