//! Entropic optimal transport (Sinkhorn). CLAUDE.md 10.2 (`sinkhorn_ot`), APPENDIX I.3.
//!
//! Solves the **entropy-regularized** OT problem
//! `min_P ⟨C,P⟩ + ε·H(P)` s.t. `P𝟙 = a`, `Pᵀ𝟙 = b`. The optimal plan has the Gibbs
//! form `P_ij = exp((f_i + g_j − C_ij)/ε)`; Sinkhorn alternately fits the dual
//! potentials `f, g`. **This is NOT exact (ε→0) Wasserstein** — the certificate
//! proves marginal feasibility of the regularized plan at the stated ε, nothing more.
//!
//! Numerical trap (handled): naive Sinkhorn forms `K=exp(−C/ε)` and underflows to 0
//! (→ NaN) for small ε. We use the **log-domain** update with `logsumexp`, which is
//! stable. (A NaN plan would be rejected by the checker anyway, but we prevent it.)

/// `log Σ_i exp(x_i)`, computed stably (subtract the max).
pub fn logsumexp(xs: &[f64]) -> f64 {
    let m = xs.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    if m == f64::NEG_INFINITY {
        return f64::NEG_INFINITY;
    }
    m + xs.iter().map(|x| (x - m).exp()).sum::<f64>().ln()
}

/// Reconstruct the Gibbs-form plan `P_ij = exp((f_i + g_j − C_ij)/ε)` (row-major).
pub fn plan_from_potentials(cost: &[f64], f: &[f64], g: &[f64], eps: f64, m: usize, n: usize) -> Vec<f64> {
    let mut p = vec![0.0; m * n];
    for i in 0..m {
        for j in 0..n {
            p[i * n + j] = ((f[i] + g[j] - cost[i * n + j]) / eps).exp();
        }
    }
    p
}

/// `‖P𝟙 − a‖₁ + ‖Pᵀ𝟙 − b‖₁` — the Sinkhorn marginal residual (the certificate).
pub fn marginal_residual(plan: &[f64], a: &[f64], b: &[f64], m: usize, n: usize) -> f64 {
    let mut res = 0.0;
    for i in 0..m {
        let row: f64 = (0..n).map(|j| plan[i * n + j]).sum();
        res += (row - a[i]).abs();
    }
    for j in 0..n {
        let col: f64 = (0..m).map(|i| plan[i * n + j]).sum();
        res += (col - b[j]).abs();
    }
    res
}

/// Log-domain Sinkhorn. Returns the dual potentials `(f, g)`. Requires positive
/// marginals (entries `> 0`). Deterministic (R11).
pub fn sinkhorn_log(
    cost: &[f64],
    a: &[f64],
    b: &[f64],
    eps: f64,
    max_iter: usize,
) -> (Vec<f64>, Vec<f64>) {
    let m = a.len();
    let n = b.len();
    let mut f = vec![0.0; m];
    let mut g = vec![0.0; n];
    let mut scratch = vec![0.0; m.max(n)];
    for _ in 0..max_iter {
        // f_i = ε(ln a_i − logsumexp_j((g_j − C_ij)/ε))
        for i in 0..m {
            for j in 0..n {
                scratch[j] = (g[j] - cost[i * n + j]) / eps;
            }
            f[i] = eps * (a[i].ln() - logsumexp(&scratch[..n]));
        }
        // g_j = ε(ln b_j − logsumexp_i((f_i − C_ij)/ε))
        for j in 0..n {
            for i in 0..m {
                scratch[i] = (f[i] - cost[i * n + j]) / eps;
            }
            g[j] = eps * (b[j].ln() - logsumexp(&scratch[..m]));
        }
    }
    (f, g)
}

/// Standard-domain Sinkhorn (matrix scaling) — the naive-correct oracle (AR-4) for
/// moderate ε where `exp(−C/ε)` does not underflow. Returns the plan directly.
pub fn sinkhorn_standard(cost: &[f64], a: &[f64], b: &[f64], eps: f64, max_iter: usize) -> Vec<f64> {
    let m = a.len();
    let n = b.len();
    let k: Vec<f64> = cost.iter().map(|c| (-c / eps).exp()).collect();
    let mut u = vec![1.0; m];
    let mut v = vec![1.0; n];
    for _ in 0..max_iter {
        // u = a ./ (K v)
        for i in 0..m {
            let kv: f64 = (0..n).map(|j| k[i * n + j] * v[j]).sum();
            u[i] = a[i] / kv;
        }
        // v = b ./ (Kᵀ u)
        for j in 0..n {
            let ku: f64 = (0..m).map(|i| k[i * n + j] * u[i]).sum();
            v[j] = b[j] / ku;
        }
    }
    let mut p = vec![0.0; m * n];
    for i in 0..m {
        for j in 0..n {
            p[i * n + j] = u[i] * k[i * n + j] * v[j];
        }
    }
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sinkhorn_zero_cost_gives_product_coupling() {
        // C = 0 ⇒ entropic OT optimum is the independent coupling P_ij = a_i b_j.
        let m = 3;
        let n = 3;
        let cost = vec![0.0; m * n];
        let a = vec![1.0 / 3.0; m];
        let b = vec![1.0 / 3.0; n];
        let (f, g) = sinkhorn_log(&cost, &a, &b, 0.1, 200);
        let p = plan_from_potentials(&cost, &f, &g, 0.1, m, n);
        for i in 0..m {
            for j in 0..n {
                assert!((p[i * n + j] - a[i] * b[j]).abs() < 1e-6);
            }
        }
        assert!(marginal_residual(&p, &a, &b, m, n) < 1e-6);
    }

    #[test]
    fn log_domain_matches_standard_for_moderate_eps() {
        // log-domain Sinkhorn agrees with the standard-domain oracle (AR-4).
        let m = 3;
        let n = 2;
        let cost = vec![0.0, 2.0, 1.0, 0.5, 3.0, 1.0];
        let a = vec![0.2, 0.5, 0.3];
        let b = vec![0.6, 0.4];
        let eps = 0.5;
        let (f, g) = sinkhorn_log(&cost, &a, &b, eps, 500);
        let p_log = plan_from_potentials(&cost, &f, &g, eps, m, n);
        let p_std = sinkhorn_standard(&cost, &a, &b, eps, 500);
        for (x, y) in p_log.iter().zip(&p_std) {
            assert!((x - y).abs() < 1e-6, "log {x} vs std {y}");
        }
    }

    #[test]
    fn log_domain_stable_at_small_eps() {
        // small ε underflows naive K=exp(−C/ε); log-domain stays finite (no NaN).
        let m = 2;
        let n = 2;
        let cost = vec![0.0, 5.0, 5.0, 0.0];
        let a = vec![0.5, 0.5];
        let b = vec![0.5, 0.5];
        let (f, g) = sinkhorn_log(&cost, &a, &b, 0.01, 300);
        let p = plan_from_potentials(&cost, &f, &g, 0.01, m, n);
        assert!(p.iter().all(|x| x.is_finite()), "log-domain plan must be finite");
        assert!(marginal_residual(&p, &a, &b, m, n) < 1e-4);
    }
}
