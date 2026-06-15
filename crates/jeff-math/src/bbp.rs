//! Stage 30 — BBP / Tracy–Widom phase transition as a FOLD SOUNDNESS GATE.
//!
//! Port of reference kernel #4 (`kernels/k4_bbp_gate.py`). **A meta-gate, not a collapse.** It
//! answers "is there statistically-significant low-rank structure in this matrix, or is it noise?"
//! — deciding whether to *attempt* a low-rank fold, and issuing a **quantitative ε-level absence
//! certificate** when there is none. This is the quantitative completion of HONEST_DEFER and the
//! precondition detector for the hidden-structure hunt.
//!
//! Random-matrix theory: noise singular values fill the Marchenko–Pastur bulk with top edge
//! `1+√γ` (γ = m/n); BBP says a rank-1 spike separates from the bulk iff its strength
//! `θ > γ^{1/4}`; Tracy–Widom gives the null fluctuation scale `~ n^{-2/3}`.
//!
//! **Certificate honesty:** PROBABILISTIC with explicit ε — never exact. `P[top sv > threshold |
//! pure noise] = ε`. The ε is **valid only under the iid-Gaussian noise model**; under
//! heavy-tailed / correlated / non-Gaussian noise it is **void** (carried in the certificate).

use crate::fmat::Rng;

/// Marchenko–Pastur top edge for the singular values of an `n×m` matrix with unit-variance
/// entries scaled by `1/√n`. `γ = m/n` (assume γ ≤ 1). Edge `= 1 + √γ`.
pub fn mp_top_edge(gamma: f64) -> f64 {
    1.0 + gamma.sqrt()
}

/// BBP detectability threshold for the spike strength `θ`: a rank-1 spike separates from the bulk
/// iff `θ > γ^{1/4}`.
pub fn bbp_threshold(gamma: f64) -> f64 {
    gamma.powf(0.25)
}

/// Tracy–Widom null fluctuation scale `~ n^{-2/3}`.
pub fn tw_scale(n: usize) -> f64 {
    (n as f64).powf(-2.0 / 3.0)
}

/// Top singular value of `A` (`n×m`, row-major) via power iteration on `AᵀA`. `O(nnz)` per iter.
/// `seed` fixes the random start for determinism (R11).
pub fn top_singular_value(a: &[f64], n: usize, m: usize, iters: usize, seed: u64) -> f64 {
    debug_assert_eq!(a.len(), n * m);
    let mut rng = Rng::new(seed);
    let mut v: Vec<f64> = (0..m).map(|_| rng.gaussian()).collect();
    let norm = |x: &[f64]| x.iter().map(|t| t * t).sum::<f64>().sqrt();
    let nv0 = norm(&v);
    if nv0 == 0.0 {
        return 0.0;
    }
    for x in v.iter_mut() {
        *x /= nv0;
    }
    let mut s_old = 0.0;
    let mut u = vec![0.0; n];
    for _ in 0..iters {
        // u = A v
        for i in 0..n {
            let row = &a[i * m..i * m + m];
            u[i] = row.iter().zip(&v).map(|(x, y)| x * y).sum();
        }
        // v = Aᵀ u
        for (j, vj) in v.iter_mut().enumerate() {
            let mut acc = 0.0;
            for i in 0..n {
                acc += a[i * m + j] * u[i];
            }
            *vj = acc;
        }
        let nv = norm(&v);
        if nv == 0.0 {
            return 0.0;
        }
        for x in v.iter_mut() {
            *x /= nv;
        }
        let s = nv.sqrt();
        if (s - s_old).abs() < 1e-10 * s {
            break;
        }
        s_old = s;
    }
    // ‖A v‖ for the converged right singular vector
    let mut av = vec![0.0; n];
    for i in 0..n {
        let row = &a[i * m..i * m + m];
        av[i] = row.iter().zip(&v).map(|(x, y)| x * y).sum();
    }
    norm(&av)
}

/// Generate a pure-noise `n×m` matrix, entries `N(0, σ²)` scaled by `1/√n`.
fn noise_matrix(n: usize, m: usize, sigma: f64, rng: &mut Rng) -> Vec<f64> {
    let scale = sigma / (n as f64).sqrt();
    (0..n * m).map(|_| rng.gaussian() * scale).collect()
}

/// Monte-Carlo calibrate the decision threshold: the `(1−ε)` quantile of the top singular value of
/// a PURE-NOISE `n×m` matrix (entries `N(0,σ²)`, scaled by `1/√n`). Deterministic given `seed`.
pub fn calibrate_threshold(n: usize, m: usize, sigma: f64, eps: f64, trials: usize, seed: u64) -> f64 {
    let mut rng = Rng::new(seed);
    let mut tops: Vec<f64> = Vec::with_capacity(trials);
    for _ in 0..trials {
        let a = noise_matrix(n, m, sigma, &mut rng);
        tops.push(top_singular_value(&a, n, m, 100, seed ^ 0x9E37_79B9));
    }
    tops.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let idx = (((trials as f64) * (1.0 - eps)).ceil() as usize).min(trials).saturating_sub(1);
    tops[idx]
}

/// The gate decision + its certificate semantics.
#[derive(Clone, Debug, PartialEq)]
pub enum GateOutcome {
    /// Significant low-rank structure: attempt the fold.
    Fold { top_sv: f64, threshold: f64, separated_from_bulk: bool },
    /// No significant structure: an ε-level absence certificate (Gaussian-noise model only).
    Defer { top_sv: f64, threshold: f64, eps: f64, model_note: &'static str },
}

impl GateOutcome {
    pub fn is_fold(&self) -> bool {
        matches!(self, GateOutcome::Fold { .. })
    }
}

/// The ε-validity caveat carried by every absence certificate.
pub const EPS_MODEL_NOTE: &str =
    "ε valid only under iid-Gaussian noise; void under heavy-tailed / correlated / non-Gaussian noise";

/// The soundness gate: normalize `A` by `1/√n`, take its top singular value, and decide FOLD vs
/// HONEST_DEFER against the calibrated `threshold`. Reports separation from the MP bulk edge.
#[allow(clippy::too_many_arguments)] // the RMT decision needs (matrix, n, m, σ, threshold, γ, ε, seed)
pub fn soundness_gate(
    a: &[f64],
    n: usize,
    m: usize,
    sigma: f64,
    threshold: f64,
    gamma: f64,
    eps: f64,
    seed: u64,
) -> GateOutcome {
    let scale = 1.0 / (n as f64).sqrt();
    let an: Vec<f64> = a.iter().map(|x| x * scale).collect();
    let s_top = top_singular_value(&an, n, m, 100, seed);
    let edge = mp_top_edge(gamma) * sigma;
    if s_top > threshold {
        GateOutcome::Fold { top_sv: s_top, threshold, separated_from_bulk: s_top > edge }
    } else {
        GateOutcome::Defer { top_sv: s_top, threshold, eps, model_note: EPS_MODEL_NOTE }
    }
}

/// Helper for tests/benches: a noise + rank-`r` signal matrix (signal already in normalized scale,
/// strength `theta`), entries `N(0,1)` noise. Returns the raw (un-`1/√n`-scaled) matrix.
pub fn noise_plus_spike(n: usize, m: usize, theta: f64, rank: usize, seed: u64) -> Vec<f64> {
    let mut rng = Rng::new(seed);
    // raw noise N(0,1); the gate divides by √n internally.
    let mut a: Vec<f64> = (0..n * m).map(|_| rng.gaussian()).collect();
    if theta > 0.0 && rank > 0 {
        let sqn = (n as f64).sqrt();
        for _ in 0..rank {
            // unit u (n), unit v (m); add theta·√n·(u vᵀ) so that after the gate's 1/√n it is theta·(u vᵀ).
            let mut u: Vec<f64> = (0..n).map(|_| rng.gaussian()).collect();
            let mut vv: Vec<f64> = (0..m).map(|_| rng.gaussian()).collect();
            let nu = u.iter().map(|t| t * t).sum::<f64>().sqrt();
            let nv = vv.iter().map(|t| t * t).sum::<f64>().sqrt();
            for x in u.iter_mut() {
                *x /= nu;
            }
            for x in vv.iter_mut() {
                *x /= nv;
            }
            for i in 0..n {
                for j in 0..m {
                    a[i * m + j] += theta * sqn * u[i] * vv[j];
                }
            }
        }
    }
    a
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mp_edge_reproduced() {
        // n=300, m=200, γ=2/3 → MP top edge ≈ 1.8165 (reference).
        let g = 200.0 / 300.0;
        assert!((mp_top_edge(g) - 1.8165).abs() < 1e-3, "edge={}", mp_top_edge(g));
    }

    #[test]
    fn bbp_threshold_reproduced() {
        // θ* = γ^{1/4} ≈ 0.9036.
        let g = 200.0 / 300.0;
        assert!((bbp_threshold(g) - 0.9036).abs() < 1e-3, "θ*={}", bbp_threshold(g));
    }

    #[test]
    fn tw_scale_order_correct() {
        // n^{-2/3} ≈ 0.0223 for n=300.
        assert!((tw_scale(300) - 0.0223).abs() < 1e-3, "tw={}", tw_scale(300));
    }

    #[test]
    fn top_singular_value_matches_known() {
        // diagonal matrix diag(3,2) → top sv = 3 (power iteration).
        let a = vec![3.0, 0.0, 0.0, 2.0];
        let s = top_singular_value(&a, 2, 2, 100, 1);
        assert!((s - 3.0).abs() < 1e-6, "top sv = {s}");
    }

    #[test]
    fn pure_noise_defers() {
        let (n, m, sigma, eps) = (100usize, 60usize, 1.0, 0.05);
        let thr = calibrate_threshold(n, m, sigma, eps, 200, 7);
        let mut rng = Rng::new(999);
        let a: Vec<f64> = (0..n * m).map(|_| rng.gaussian()).collect();
        let r = soundness_gate(&a, n, m, sigma, thr, m as f64 / n as f64, eps, 5);
        assert!(!r.is_fold(), "pure noise must DEFER: {r:?}");
    }

    #[test]
    fn noise_plus_rank3_folds() {
        let (n, m, sigma, eps) = (100usize, 60usize, 1.0, 0.05);
        let thr = calibrate_threshold(n, m, sigma, eps, 200, 7);
        // strong rank-3 structure (theta = 2.5) well above θ*.
        let a = noise_plus_spike(n, m, 2.5, 3, 123);
        let r = soundness_gate(&a, n, m, sigma, thr, m as f64 / n as f64, eps, 5);
        assert!(r.is_fold(), "strong rank-3 must FOLD: {r:?}");
    }

    #[test]
    fn bbp_transition_sharp_at_threshold() {
        // detection probability rises sharply through θ*: low well below, high well above.
        let (n, m, sigma, eps) = (80usize, 60usize, 1.0, 0.05);
        let g = m as f64 / n as f64;
        let thr = calibrate_threshold(n, m, sigma, eps, 150, 11);
        let theta_star = bbp_threshold(g);
        let detect_rate = |theta: f64, trials: usize, seed0: u64| -> f64 {
            let mut det = 0;
            for t in 0..trials {
                let a = noise_plus_spike(n, m, theta, 1, seed0 + t as u64);
                if soundness_gate(&a, n, m, sigma, thr, g, eps, 5).is_fold() {
                    det += 1;
                }
            }
            det as f64 / trials as f64
        };
        let below = detect_rate(theta_star * 0.4, 60, 1000);
        let above = detect_rate(theta_star * 2.0, 60, 2000);
        assert!(below < 0.3, "below θ* detection should be low, got {below}");
        assert!(above > 0.7, "above θ* detection should be high, got {above}");
        assert!(above - below > 0.4, "transition must be sharp: below={below} above={above}");
    }

    #[test]
    fn fp_rate_honors_eps() {
        // on pure-noise trials the FOLD (false-positive) fraction is near ε (same order; the
        // reference measured 0.018 vs ε=0.01 with finite calibration). Probabilistic, Gaussian.
        let (n, m, sigma, eps) = (80usize, 60usize, 1.0, 0.05);
        let thr = calibrate_threshold(n, m, sigma, eps, 200, 21);
        let mut fp = 0;
        let trials = 300;
        for t in 0..trials {
            let mut rng = Rng::new(50_000 + t as u64);
            let a: Vec<f64> = (0..n * m).map(|_| rng.gaussian()).collect();
            if soundness_gate(&a, n, m, sigma, thr, m as f64 / n as f64, eps, 5).is_fold() {
                fp += 1;
            }
        }
        let rate = fp as f64 / trials as f64;
        // same order as ε (calibrated at 0.05): allow a generous band for finite-sample noise.
        assert!(rate <= 3.0 * eps, "FP rate {rate} should be near ε={eps}");
    }
}
