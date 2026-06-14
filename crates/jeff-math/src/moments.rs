//! Stage-6A Batch-3: latent-variable / moment methods (CLAUDE.md PART B Batch 3).
//!
//! These replace non-convex EM / Baum–Welch local search with provable spectral/moment
//! methods. The shared structure is a **spectral gap** (a moment matrix or contracted
//! tensor has a clean rank), which is the runtime precondition; the certificate is the
//! moment-reconstruction residual against the exact oracle. All certs are
//! `eps-approximate`. Reuses `jacobi_eig` and the Prony recurrence machinery.

#![allow(clippy::needless_range_loop)]

use crate::fmat::{jacobi_eig, FMat};

// ---- shared: order-3 tensor reconstruction residual ----

/// `‖T − Σ_i λ_i a_i⊗a_i⊗a_i‖_F` for factors `a` (row-major `r×p`) — the cert quantity.
pub fn tensor_decomp_residual(tensor: &[f64], lambdas: &[f64], a: &[f64], r: usize, p: usize) -> f64 {
    let mut acc = 0.0;
    for i in 0..p {
        for j in 0..p {
            for k in 0..p {
                let mut approx = 0.0;
                for c in 0..r {
                    approx += lambdas[c] * a[c * p + i] * a[c * p + j] * a[c * p + k];
                }
                let d = tensor[(i * p + j) * p + k] - approx;
                acc += d * d;
            }
        }
    }
    acc.sqrt()
}

// ---- 3.1 Jennrich / orthogonal symmetric tensor decomposition ----

/// Contract an order-3 tensor along `theta`: `M[a,b] = Σ_k θ_k T[a,b,k]`.
fn contract_mode3(tensor: &[f64], theta: &[f64], p: usize) -> FMat {
    let mut m = FMat::zeros(p, p);
    for a in 0..p {
        for b in 0..p {
            let mut acc = 0.0;
            for k in 0..p {
                acc += theta[k] * tensor[(a * p + b) * p + k];
            }
            m.set(a, b, acc);
        }
    }
    m
}

/// Decompose an orthogonally-decomposable symmetric tensor `T = Σ_i λ_i a_i⊗³` via
/// Jennrich: contract with a generic `θ`, whose eigenvectors are the `a_i`. Returns the
/// top-`r` `(λ_i, a_i)` and the eigenvalue gap of the contraction (the detection signal).
pub fn jennrich_decompose(tensor: &[f64], p: usize, r: usize) -> (Vec<f64>, Vec<f64>, f64) {
    // a fixed pseudo-random θ (deterministic, R11)
    let theta: Vec<f64> = (0..p).map(|k| (k as f64 * 1.3 + 0.7).sin()).collect();
    let m = contract_mode3(tensor, &theta, p);
    let (eigvals, eigvecs) = jacobi_eig(&m);
    // gap between the r-th and (r+1)-th |eigenvalue| of the contraction
    let mut mags: Vec<f64> = eigvals.iter().map(|x| x.abs()).collect();
    mags.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
    let gap = if mags.len() > r && mags[r] > 1e-12 {
        mags[r - 1] / mags[r]
    } else {
        f64::INFINITY
    };
    // factors = top-r eigenvectors (columns); λ_i = T(a_i,a_i,a_i)
    let mut lambdas = Vec::with_capacity(r);
    let mut factors = Vec::with_capacity(r * p);
    for c in 0..r.min(p) {
        let ai: Vec<f64> = (0..p).map(|i| eigvecs.get(i, c)).collect();
        let mut lam = 0.0;
        for i in 0..p {
            for j in 0..p {
                for k in 0..p {
                    lam += tensor[(i * p + j) * p + k] * ai[i] * ai[j] * ai[k];
                }
            }
        }
        lambdas.push(lam);
        factors.extend_from_slice(&ai);
    }
    (lambdas, factors, gap)
}

// ---- 3.2 Spectral HMM: the bigram moment matrix has rank = #hidden states ----

/// Singular values (descending) of a flat `rows×cols` matrix via `jacobi_eig(BᵀB)`.
pub fn singular_values(b: &[f64], rows: usize, cols: usize) -> Vec<f64> {
    let bm = FMat::from_data(rows, cols, b.to_vec());
    let btb = bm.transpose().matmul(&bm);
    let (ev, _) = jacobi_eig(&btb);
    ev.iter().map(|&e| e.max(0.0).sqrt()).collect()
}

/// Rank-`m` reconstruction residual `‖B − B_m‖_F` and the singular gap `σ_m/σ_{m+1}`.
/// (The HMM observable-operator structure manifests as a rank-`m` bigram matrix; the
/// full sequence-prediction OOM is tracked for promotion, R24.)
pub fn bigram_rank_residual(b: &[f64], rows: usize, cols: usize, m: usize) -> (f64, f64) {
    use crate::fmat::{low_rank_approx, randomized_range};
    let bm = FMat::from_data(rows, cols, b.to_vec());
    let q = randomized_range(&bm, m, 2, 0xB16);
    let (_, resid) = low_rank_approx(&bm, &q);
    let sv = singular_values(b, rows, cols);
    let gap = if sv.len() > m && sv[m] > 1e-12 {
        sv[m - 1] / sv[m]
    } else {
        f64::INFINITY
    };
    (resid, gap)
}

// ---- 3.3 Anandkumar whitened tensor decomposition (mixtures / topics) ----

/// Recover `(w_i, μ_i)` from second/third moments `M2 = Σ w_i μ_iμ_iᵀ` (flat `p×p`) and
/// `M3 = Σ w_i μ_i⊗³` (flat `p³`) by whitening `M2`, decomposing the orthogonalized
/// `M3` (Jennrich), then un-whitening. Returns `(weights, means row-major k×p)`.
pub fn whiten_decompose(m2: &[f64], m3: &[f64], p: usize, k: usize) -> (Vec<f64>, Vec<f64>) {
    let m2m = FMat::from_data(p, p, m2.to_vec());
    let (eigvals, eigvecs) = jacobi_eig(&m2m);
    // whitening W = U_k Λ_k^{-1/2} (p×k); un-whitener Uk Λ^{1/2}
    let mut w = FMat::zeros(p, k);
    let mut unwhite = FMat::zeros(p, k);
    for c in 0..k {
        let lam = eigvals[c].max(1e-12);
        for i in 0..p {
            w.set(i, c, eigvecs.get(i, c) / lam.sqrt());
            unwhite.set(i, c, eigvecs.get(i, c) * lam.sqrt());
        }
    }
    // whitened tensor T̃[a,b,c] = Σ_{ijk} M3[i,j,k] W[i,a]W[j,b]W[k,c]  (k×k×k)
    let mut tt = vec![0.0; k * k * k];
    for a in 0..k {
        for b in 0..k {
            for cc in 0..k {
                let mut acc = 0.0;
                for i in 0..p {
                    let wia = w.get(i, a);
                    if wia == 0.0 {
                        continue;
                    }
                    for j in 0..p {
                        let wjb = w.get(j, b);
                        for kk in 0..p {
                            acc += m3[(i * p + j) * p + kk] * wia * wjb * w.get(kk, cc);
                        }
                    }
                }
                tt[(a * k + b) * k + cc] = acc;
            }
        }
    }
    let (lams, vfac, _) = jennrich_decompose(&tt, k, k);
    // un-whiten: μ_i = λ̃_i · (Uk Λ^{1/2}) ṽ_i ; w_i = 1/λ̃_i²
    let mut weights = Vec::with_capacity(k);
    let mut means = Vec::with_capacity(k * p);
    for c in 0..k {
        let lt = lams[c];
        let wi = if lt.abs() > 1e-12 { 1.0 / (lt * lt) } else { 0.0 };
        weights.push(wi);
        let vc = &vfac[c * k..c * k + k];
        for i in 0..p {
            let mut mu = 0.0;
            for a in 0..k {
                mu += unwhite.get(i, a) * vc[a];
            }
            means.push(lt * mu);
        }
    }
    (weights, means)
}

/// `‖M2 − Σ w_i μ_iμ_iᵀ‖_F` (flat `p×p`).
pub fn moment2_residual(m2: &[f64], weights: &[f64], means: &[f64], k: usize, p: usize) -> f64 {
    let mut acc = 0.0;
    for i in 0..p {
        for j in 0..p {
            let mut approx = 0.0;
            for c in 0..k {
                approx += weights[c] * means[c * p + i] * means[c * p + j];
            }
            let d = m2[i * p + j] - approx;
            acc += d * d;
        }
    }
    acc.sqrt()
}

// ---- 3.4 Method-of-moments: mixture of point masses via Prony on the moment sequence ----

/// Recover `k` component locations and weights from the moment sequence
/// `m_t = Σ_j w_j x_j^t` (a mixture of `k` point masses) — this is exactly Prony, since
/// the moments satisfy a degree-`k` recurrence. Returns `(weights, locations)` or `None`.
pub fn moment_mixture(moments: &[f64], k: usize) -> Option<(Vec<f64>, Vec<f64>)> {
    use crate::complex::poly_roots;
    use crate::complex::Complex;
    use crate::prony::prony_fit;
    let (a, res) = prony_fit(moments, k)?;
    if res > 1e-6 {
        return None; // not a clean k-mixture
    }
    // locations = real roots of Σ a_i x^i
    let coeffs: Vec<Complex> = a.iter().map(|&c| Complex::new(c, 0.0)).collect();
    let roots = poly_roots(&coeffs);
    let locations: Vec<f64> = roots.iter().map(|z| z.re).collect();
    // weights solve the Vandermonde system Σ_j w_j x_j^t = m_t (use t=0..k-1)
    let mut v = FMat::zeros(k, k);
    for t in 0..k {
        for (j, &x) in locations.iter().enumerate().take(k) {
            v.set(t, j, x.powi(t as i32));
        }
    }
    let rhs: Vec<f64> = (0..k).map(|t| moments[t]).collect();
    let weights = crate::fmat::solve_dense(&v, &rhs)?;
    Some((weights, locations))
}

// ---- 3.5 FastICA: a high-kurtosis (non-Gaussian) projection direction ----

/// Standardize a projection to unit variance and return its excess kurtosis
/// `E[s⁴]−3` (0 for a Gaussian). The certificate quantity for ICA.
pub fn excess_kurtosis(data: &[f64], n: usize, p: usize, direction: &[f64]) -> f64 {
    let s: Vec<f64> = (0..n)
        .map(|k| (0..p).map(|j| data[k * p + j] * direction[j]).sum())
        .collect();
    let mean = s.iter().sum::<f64>() / n as f64;
    let var = s.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / n as f64;
    if var < 1e-12 {
        return 0.0;
    }
    let sd = var.sqrt();
    let m4 = s.iter().map(|x| ((x - mean) / sd).powi(4)).sum::<f64>() / n as f64;
    m4 - 3.0
}

/// FastICA (kurtosis contrast): whiten the data, fixed-point iterate to a direction of
/// maximal |kurtosis|, return that direction **in original coordinates** (so a checker
/// can recompute the kurtosis from the raw data). `None` if whitening is degenerate.
pub fn fastica_direction(data: &[f64], n: usize, p: usize) -> Option<Vec<f64>> {
    // center
    let mut mean = vec![0.0; p];
    for k in 0..n {
        for j in 0..p {
            mean[j] += data[k * p + j];
        }
    }
    for m in mean.iter_mut() {
        *m /= n as f64;
    }
    // covariance + whitening W = D^{-1/2} Uᵀ
    let mut cov = FMat::zeros(p, p);
    for k in 0..n {
        for i in 0..p {
            let xi = data[k * p + i] - mean[i];
            for j in 0..p {
                cov.set(i, j, cov.get(i, j) + xi * (data[k * p + j] - mean[j]));
            }
        }
    }
    for i in 0..p {
        for j in 0..p {
            cov.set(i, j, cov.get(i, j) / n as f64);
        }
    }
    let (eigvals, eigvecs) = jacobi_eig(&cov);
    if eigvals.iter().any(|&e| e < 1e-9) {
        return None;
    }
    // whitened data Z = (X-μ) U D^{-1/2}  (n×p)
    let mut z = vec![0.0; n * p];
    for k in 0..n {
        for a in 0..p {
            let mut acc = 0.0;
            for i in 0..p {
                acc += (data[k * p + i] - mean[i]) * eigvecs.get(i, a);
            }
            z[k * p + a] = acc / eigvals[a].sqrt();
        }
    }
    // fixed-point on w (kurtosis: g(u)=u³)
    let mut w = vec![0.0; p];
    w[0] = 1.0;
    for _ in 0..100 {
        let mut wnew = vec![0.0; p];
        for k in 0..n {
            let u: f64 = (0..p).map(|j| z[k * p + j] * w[j]).sum();
            let u3 = u * u * u;
            for j in 0..p {
                wnew[j] += z[k * p + j] * u3;
            }
        }
        for j in 0..p {
            wnew[j] = wnew[j] / n as f64 - 3.0 * w[j];
        }
        let nrm = wnew.iter().map(|x| x * x).sum::<f64>().sqrt();
        if nrm < 1e-12 {
            break;
        }
        for x in wnew.iter_mut() {
            *x /= nrm;
        }
        let dot: f64 = wnew.iter().zip(&w).map(|(a, b)| a * b).sum();
        w = wnew;
        if (dot.abs() - 1.0).abs() < 1e-10 {
            break;
        }
    }
    // map back to original coords: direction a s.t. s = (X-μ)·a equals Z·w
    // Z = (X-μ) U D^{-1/2} ⇒ a = U D^{-1/2} w
    let mut dir = vec![0.0; p];
    for i in 0..p {
        let mut acc = 0.0;
        for a in 0..p {
            acc += eigvecs.get(i, a) * w[a] / eigvals[a].sqrt();
        }
        dir[i] = acc;
    }
    Some(dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fmat::Rng;

    fn unit(mut v: Vec<f64>) -> Vec<f64> {
        let n = v.iter().map(|x| x * x).sum::<f64>().sqrt();
        v.iter_mut().for_each(|x| *x /= n);
        v
    }

    #[test]
    fn jennrich_recovers_orthogonal_tensor() {
        // T = 3·e0⊗³ + 2·e1⊗³ + 1·e2⊗³ (orthonormal factors).
        let p = 3;
        let lam = [3.0, 2.0, 1.0];
        let mut t = vec![0.0; p * p * p];
        for c in 0..3 {
            for i in 0..p {
                for j in 0..p {
                    for k in 0..p {
                        let e = |x: usize| if x == c { 1.0 } else { 0.0 };
                        t[(i * p + j) * p + k] += lam[c] * e(i) * e(j) * e(k);
                    }
                }
            }
        }
        let (lams, fac, gap) = jennrich_decompose(&t, p, 3);
        assert!(gap > 1.5, "distinct eigenvalues ⇒ gap");
        let r = tensor_decomp_residual(&t, &lams, &fac, 3, p);
        assert!(r < 1e-8, "reconstruction residual {r}");
    }

    #[test]
    fn bigram_low_rank_detected() {
        // rank-2 bigram matrix → tiny residual + large gap; full-rank → no gap.
        let rows = 6;
        let cols = 6;
        let mut rng = Rng::new(0x9);
        // rank-2: B = u1 v1ᵀ + u2 v2ᵀ
        let u1: Vec<f64> = (0..rows).map(|_| rng.gaussian()).collect();
        let v1: Vec<f64> = (0..cols).map(|_| rng.gaussian()).collect();
        let u2: Vec<f64> = (0..rows).map(|_| rng.gaussian()).collect();
        let v2: Vec<f64> = (0..cols).map(|_| rng.gaussian()).collect();
        let mut b = vec![0.0; rows * cols];
        for i in 0..rows {
            for j in 0..cols {
                b[i * cols + j] = u1[i] * v1[j] + u2[i] * v2[j];
            }
        }
        let (resid, gap) = bigram_rank_residual(&b, rows, cols, 2);
        assert!(resid < 1e-6, "rank-2 residual {resid}");
        assert!(gap > 10.0, "rank gap {gap}");
    }

    #[test]
    fn whiten_decompose_recovers_mixture_moments() {
        // two components μ1,μ2 (non-orthogonal) with weights; check M2 reconstruction.
        let p = 3;
        let k = 2;
        let mu1 = unit(vec![1.0, 0.5, 0.0]);
        let mu2 = unit(vec![0.2, 1.0, 0.3]);
        let w = [0.6_f64, 0.4];
        let means = [mu1.clone(), mu2.clone()];
        let mut m2 = vec![0.0; p * p];
        let mut m3 = vec![0.0; p * p * p];
        for c in 0..k {
            for i in 0..p {
                for j in 0..p {
                    m2[i * p + j] += w[c] * means[c][i] * means[c][j];
                    for kk in 0..p {
                        m3[(i * p + j) * p + kk] += w[c] * means[c][i] * means[c][j] * means[c][kk];
                    }
                }
            }
        }
        let (wr, mr) = whiten_decompose(&m2, &m3, p, k);
        let r = moment2_residual(&m2, &wr, &mr, k, p);
        assert!(r < 1e-6, "M2 reconstruction residual {r}");
    }

    #[test]
    fn moment_mixture_recovers_point_masses() {
        // mixture of point masses at x=2, x=5 with weights 0.7, 0.3.
        let locs = [2.0_f64, 5.0];
        let w = [0.7_f64, 0.3];
        let moments: Vec<f64> = (0..6)
            .map(|t| w[0] * locs[0].powi(t) + w[1] * locs[1].powi(t))
            .collect();
        let (wr, lr) = moment_mixture(&moments, 2).expect("recover");
        let mut pairs: Vec<(f64, f64)> = lr.into_iter().zip(wr).collect();
        pairs.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        assert!((pairs[0].0 - 2.0).abs() < 1e-6 && (pairs[0].1 - 0.7).abs() < 1e-6);
        assert!((pairs[1].0 - 5.0).abs() < 1e-6 && (pairs[1].1 - 0.3).abs() < 1e-6);
    }

    #[test]
    fn fastica_finds_nongaussian_defers_gaussian() {
        let n = 2000;
        let p = 2;
        let mut rng = Rng::new(0x1CA);
        // non-Gaussian source: uniform-ish (low kurtosis) mixed; use a bimodal source
        let mut data = vec![0.0; n * p];
        for k in 0..n {
            // s1 bimodal (±1 + noise) → strongly non-Gaussian; s2 Gaussian
            let s1 = if rng.next_u64() & 1 == 0 { 1.0 } else { -1.0 } + 0.05 * rng.gaussian();
            let s2 = rng.gaussian();
            // mix
            data[k * p] = 0.8 * s1 + 0.6 * s2;
            data[k * p + 1] = 0.6 * s1 - 0.8 * s2;
        }
        let dir = fastica_direction(&data, n, p).expect("ica");
        let ek = excess_kurtosis(&data, n, p, &dir);
        assert!(ek.abs() > 0.5, "should find a non-Gaussian direction, ek={ek}");

        // purely Gaussian data → no non-Gaussian direction
        let gdata: Vec<f64> = (0..n * p).map(|_| rng.gaussian()).collect();
        let gdir = fastica_direction(&gdata, n, p).expect("ica");
        let gek = excess_kurtosis(&gdata, n, p, &gdir);
        assert!(gek.abs() < 0.5, "Gaussian data has ~0 excess kurtosis, got {gek}");
    }
}
