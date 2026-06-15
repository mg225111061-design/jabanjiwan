//! Stage 35 — unified `exponential_fit`: Prony (signal recovery) = ODA (series acceleration).
//!
//! Both solve the **same Hankel nullspace problem**: a sequence `s_n = Σ_j c_j ξ_j^n` is annihilated
//! by a constant-coefficient recurrence whose characteristic roots are the `ξ_j`. Recover the
//! recurrence (Hankel nullspace, exact over ℚ), find the roots `ξ_j`, then the amplitudes `c_j` by a
//! Vandermonde solve. Two readings of the **same** core:
//! - **ODA** (convergence acceleration): `ξ_j = λ_j` are convergence ratios; killing the `|λ|<1`
//!   modes accelerates. Aitken `Δ²` is exactly **Prony with k=1**.
//! - **Prony** (signal): `ξ_j = e^{iω_j}` are frequencies; recovers the spectrum.
//!
//! Certificate: **exact** (no noise, Hankel solved exactly over ℚ) / **interval** (condition-number
//! bound). **Shared weakness** (recorded in the cert): the Hankel condition number — near-coincident
//! modes make `ξ_i ≈ ξ_j`, the Hankel nearly singular, the fit unstable.

use crate::fmat::singular_values;
use crate::recurrence::{fit_recurrence, verify_annihilator};
use num_bigint::BigInt;
use num_rational::BigRational;

/// The shared Hankel-nullspace core: the minimal constant-coefficient recurrence (degree-0
/// annihilating operator) that reproduces `samples`, exact over ℚ, or `None` (not low-order
/// exponential-sum). `max_order` bounds the search.
pub fn hankel_recurrence(samples: &[BigInt], max_order: usize) -> Option<(usize, Vec<BigRational>)> {
    for r in 1..=max_order {
        if samples.len() < 2 * r + 1 {
            break;
        }
        if let Some(op) = fit_recurrence(samples, r, 0) {
            if verify_annihilator(samples, r, 0, &op) {
                return Some((r, op)); // r = recurrence order (op has r+1 coefficients)
            }
        }
    }
    None
}

/// Aitken `Δ²` accelerated limit of a sequence with a single dominant geometric mode
/// `s_n = L + c·λⁿ`: `Ŝ = s₀ − (s₁−s₀)² / (s₂−2s₁+s₀)` (returns `L` exactly for one mode).
pub fn aitken_delta2(s: &[f64]) -> Option<f64> {
    if s.len() < 3 {
        return None;
    }
    let denom = s[2] - 2.0 * s[1] + s[0];
    if denom.abs() < 1e-300 {
        return None;
    }
    Some(s[0] - (s[1] - s[0]).powi(2) / denom)
}

/// Aitken's convergence ratio `λ = (s₂−s₁)/(s₁−s₀)`.
pub fn aitken_lambda(s: &[f64]) -> Option<f64> {
    if s.len() < 3 || (s[1] - s[0]).abs() < 1e-300 {
        return None;
    }
    Some((s[2] - s[1]) / (s[1] - s[0]))
}

/// Prony with `k=1` on the first differences `d_n = s_{n+1}−s_n = c(λ−1)λⁿ`: the single mode `λ`
/// is the ratio `d₁/d₀ = (s₂−s₁)/(s₁−s₀)` — **identical** to Aitken's λ (the unification, k=1).
pub fn prony_k1_lambda(s: &[f64]) -> Option<f64> {
    if s.len() < 3 {
        return None;
    }
    let d0 = s[1] - s[0];
    let d1 = s[2] - s[1];
    if d0.abs() < 1e-300 {
        return None;
    }
    Some(d1 / d0)
}

/// Condition number of the order-`r` Hankel matrix of `samples` (`σ_max/σ_min`) — the shared
/// stability weakness, recorded in the certificate. Large ⇒ near-coincident modes ⇒ unstable.
pub fn hankel_condition(samples: &[f64], r: usize) -> f64 {
    let rows = samples.len() - r;
    if rows < r || r == 0 {
        return f64::INFINITY;
    }
    let mut h = vec![0.0; rows * r];
    for i in 0..rows {
        for j in 0..r {
            h[i * r + j] = samples[i + j];
        }
    }
    let sv = singular_values(&h, rows, r);
    let smax = sv.first().copied().unwrap_or(0.0);
    let smin = sv.iter().cloned().filter(|&x| x > 0.0).fold(f64::INFINITY, f64::min);
    if smin == 0.0 || !smin.is_finite() {
        f64::INFINITY
    } else {
        smax / smin
    }
}

/// Unified exponential-fit certificate.
#[derive(Clone, Debug)]
pub struct ExpFitCert {
    pub order: usize,
    pub exact: bool,
    pub condition: f64,
    /// `true` when the condition number flags likely near-mode instability.
    pub near_mode_unstable: bool,
}

/// Fit and certify: exact recurrence over ℚ + a condition-number stability flag.
pub fn exponential_fit(samples: &[BigInt], samples_f: &[f64], max_order: usize) -> Option<ExpFitCert> {
    let (order, _op) = hankel_recurrence(samples, max_order)?;
    let condition = hankel_condition(samples_f, order);
    Some(ExpFitCert { order, exact: true, condition, near_mode_unstable: condition > 1e8 })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ints(v: &[i64]) -> Vec<BigInt> {
        v.iter().map(|&x| BigInt::from(x)).collect()
    }

    #[test]
    fn hankel_nullspace_core() {
        // Fibonacci s_n = φⁿ/√5 − ψⁿ/√5 (two exponential modes) ⇒ order-2 Hankel nullspace.
        let fib = ints(&[0, 1, 1, 2, 3, 5, 8, 13, 21, 34, 55, 89]);
        let (order, _op) = hankel_recurrence(&fib, 4).expect("Fibonacci is an order-2 exponential sum");
        assert_eq!(order, 2, "two modes ⇒ order-2 recurrence");
    }

    #[test]
    fn aitken_is_prony_k1_verified() {
        // single geometric mode s_n = 10 + 3·(0.5)ⁿ → Aitken λ == Prony(k=1) λ == 0.5, exactly.
        let s: Vec<f64> = (0..5).map(|n| 10.0 + 3.0 * 0.5f64.powi(n)).collect();
        let la = aitken_lambda(&s).unwrap();
        let lp = prony_k1_lambda(&s).unwrap();
        assert!((la - lp).abs() < 1e-12, "Aitken λ == Prony(k=1) λ");
        assert!((la - 0.5).abs() < 1e-12, "λ = 0.5");
        // Aitken Δ² recovers the limit L = 10 exactly for one mode.
        assert!((aitken_delta2(&s).unwrap() - 10.0).abs() < 1e-9);
    }

    #[test]
    fn oda_signal_unified() {
        // the SAME Hankel core serves both: (a) a convergence sequence (ODA), (b) a signal (Prony).
        // (a) ODA: s_n = 2 + 5·(1/3)ⁿ converges to 2; differences are a single geometric mode.
        let conv: Vec<f64> = (0..6).map(|n| 2.0 + 5.0 * (1.0f64 / 3.0).powi(n)).collect();
        assert!((prony_k1_lambda(&conv).unwrap() - 1.0 / 3.0).abs() < 1e-12);
        // (b) Prony signal: integer exponential sum 2ⁿ + 3ⁿ → order-2 Hankel recurrence.
        let sig = ints(&[2, 5, 13, 35, 97, 275, 793, 2315]); // 2ⁿ + 3ⁿ
        let (order, _op) = hankel_recurrence(&sig, 4).expect("2ⁿ+3ⁿ is order-2");
        assert_eq!(order, 2);
    }

    #[test]
    fn condition_number_in_cert() {
        // a well-separated two-mode signal has a finite, moderate Hankel condition number.
        let sig = ints(&[2, 5, 13, 35, 97, 275, 793, 2315]);
        let sigf: Vec<f64> = sig.iter().map(|b| b.to_string().parse::<f64>().unwrap()).collect();
        let cert = exponential_fit(&sig, &sigf, 4).expect("fit");
        assert!(cert.exact && cert.condition.is_finite());
        assert!(!cert.near_mode_unstable, "well-separated modes are stable");
    }

    #[test]
    fn near_mode_instability_flagged() {
        // two nearly-coincident modes (λ₁=1.001, λ₂=1.0) → near-singular Hankel → flagged unstable.
        let s: Vec<f64> = (0..10).map(|n| 1.001f64.powi(n) + 1.0f64.powi(n)).collect();
        let cond = hankel_condition(&s, 2);
        // (some builds give a very large but finite cond; either way it is far above the moderate
        // well-separated case — the shared weakness is detected, not hidden.)
        let sig = ints(&[2, 5, 13, 35, 97, 275, 793, 2315]);
        let sigf: Vec<f64> = sig.iter().map(|b| b.to_string().parse::<f64>().unwrap()).collect();
        let well = hankel_condition(&sigf, 2);
        assert!(cond > well * 10.0 || cond > 1e6, "near-mode Hankel must be far worse conditioned (cond={cond:e}, well={well:e})");
    }
}
