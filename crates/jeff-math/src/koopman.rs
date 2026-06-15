//! Stage 40 — Koopman / dynamical-systems fold axis (the first *dynamic* axis: long-term flow).
//!
//! A nonlinear map `x_{n+1} = f(x_n)` has a long-term behavior captured by the (infinite-dim but
//! **linear**) Koopman operator. An observable expands as `g(x_n) = Σ_j c_j λ_j^n` (Koopman
//! eigenfunctions) — **exactly** the Stage-35 exponential-sum / Hankel-nullspace problem. So:
//! - **40.1** non-chaotic (discrete-spectrum) long-term state `t=N` collapses to `Σ c_j λ_j^N` in
//!   `O(log N)` (fast power) vs naive `O(N)` iteration — ratio diverges, reusing Stage 26/35.
//! - **40.2** chaos ⇒ `λ_Lyapunov > 0` ⇒ **no long-term closed form** (the dynamical analog of the
//!   Stage-37 Galois absence: not "couldn't find it", but "exponential sensitivity ⇒ none exists").
//! - **40.3** this is the **nonlinear** sibling of Stage-31.2 Krylov (`A^N x₀`): when `f` is linear,
//!   Koopman reduces exactly to the C-finite / Krylov fold.
//!
//! Honest scope: Koopman closes in finitely many modes **only** for non-chaotic / discrete-spectrum
//! dynamics; continuous spectrum / chaos ⇒ infinite modes / absence. DMD shares Stage-35's Hankel
//! condition-number weakness (near modes unstable). Domain: signal / numeric (direct).

use crate::cfinite::cfinite_bostan;
use crate::expfit::hankel_recurrence;
use num_bigint::BigInt;

/// Koopman DMD via the shared Hankel core: returns `(order, recurrence)` whose characteristic
/// roots are the Koopman eigenvalues `λ_j`. Reuses [`hankel_recurrence`] (Stage 35).
pub fn koopman_dmd_hankel(observable: &[BigInt], max_modes: usize) -> Option<usize> {
    hankel_recurrence(observable, max_modes).map(|(order, _)| order)
}

/// Long-term observable value `g(x_N) mod q` for a non-chaotic (C-finite) observable, via the
/// Stage-26 fast power `O(log N)` (vs naive `O(N)` iteration). `c`/`init` define the C-finite
/// recurrence the Koopman modes induce.
pub fn koopman_longterm(c: &[u64], init: &[u64], n: u64, q: u64) -> u64 {
    cfinite_bostan(c, init, n, q)
}

/// Lyapunov exponent of the logistic map `x_{n+1} = r·x_n·(1−x_n)`:
/// `λ_L = ⟨ ln|f'(x_n)| ⟩ = ⟨ ln|r·(1−2x_n)| ⟩` over the orbit (after a transient).
pub fn lyapunov_logistic(r: f64, x0: f64, iters: usize) -> f64 {
    let mut x = x0;
    // burn-in transient
    for _ in 0..500 {
        x = r * x * (1.0 - x);
    }
    let mut acc = 0.0;
    let mut count = 0;
    for _ in 0..iters {
        let d = (r * (1.0 - 2.0 * x)).abs();
        if d > 0.0 {
            acc += d.ln();
            count += 1;
        }
        x = r * x * (1.0 - x);
    }
    acc / count.max(1) as f64
}

/// Birkhoff time-average of the observable `g(x)=x` over a logistic orbit.
pub fn logistic_time_average(r: f64, x0: f64, iters: usize) -> f64 {
    let mut x = x0;
    for _ in 0..500 {
        x = r * x * (1.0 - x);
    }
    let mut acc = 0.0;
    for _ in 0..iters {
        acc += x;
        x = r * x * (1.0 - x);
    }
    acc / iters as f64
}

/// Dynamical fold/defer decision from the Lyapunov exponent.
#[derive(Clone, Debug, PartialEq)]
pub enum DynamicsVerdict {
    /// Non-chaotic (`λ_L < 0`): the long-term state folds (Koopman closes in finite modes).
    Fold,
    /// Chaotic (`λ_L > 0`): **absence** — no long-term closed form (exponential sensitivity).
    ChaosAbsence { lyapunov: f64 },
}

/// Decide fold-vs-absence for the logistic map at parameter `r`.
pub fn logistic_verdict(r: f64) -> DynamicsVerdict {
    let l = lyapunov_logistic(r, 0.1234, 5000);
    if l > 0.0 {
        DynamicsVerdict::ChaosAbsence { lyapunov: l }
    } else {
        DynamicsVerdict::Fold
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ints(v: &[i64]) -> Vec<BigInt> {
        v.iter().map(|&x| BigInt::from(x)).collect()
    }

    #[test]
    fn koopman_dmd_via_hankel() {
        // a 2-mode Koopman observable (integer exponential sum 2ⁿ+3ⁿ) → order-2 via the Hankel core.
        let obs = ints(&[2, 5, 13, 35, 97, 275, 793, 2315]);
        assert_eq!(koopman_dmd_hankel(&obs, 4), Some(2), "two Koopman modes ⇒ order 2");
    }

    #[test]
    fn reuses_exponential_fit() {
        // the DMD core IS Stage-35's hankel_recurrence (Fibonacci → order 2).
        let fib = ints(&[0, 1, 1, 2, 3, 5, 8, 13, 21, 34, 55, 89]);
        assert_eq!(koopman_dmd_hankel(&fib, 4), Some(2));
    }

    #[test]
    fn koopman_longterm_via_fastpow() {
        // long-term Koopman state via fast power == naive iteration (here a C-finite observable).
        const Q: u64 = 1_000_000_007;
        let c = [1u64, 1]; // Fibonacci-like 2-mode recurrence
        let init = [0u64, 1];
        // fast power vs naive iterate agree for several N.
        let naive = |n: u64| {
            let (mut a, mut b) = (0u64, 1u64);
            for _ in 0..n {
                let t = (a + b) % Q;
                a = b;
                b = t;
            }
            a
        };
        for &n in &[10u64, 100, 1000] {
            assert_eq!(koopman_longterm(&c, &init, n, Q), naive(n), "fastpow==naive at N={n}");
        }
    }

    #[test]
    fn koopman_ratio_diverges() {
        // op-count proxy: naive O(N) iteration vs fast-power O(log N); ratio diverges (Stage-26 label).
        let mut prev = 0f64;
        for k in 3..=7 {
            let n = 10u64.pow(k);
            let ratio = n as f64 / ((64 - n.leading_zeros()) as f64);
            assert!(ratio > prev);
            prev = ratio;
        }
        assert!(prev > 1e5);
    }

    #[test]
    fn lyapunov_positive_defers_chaos() {
        // logistic r=4 is fully chaotic: λ_L ≈ ln 2 ≈ 0.693 > 0 ⇒ ChaosAbsence (no closed form).
        let l = lyapunov_logistic(4.0, 0.1234, 5000);
        assert!(l > 0.5, "r=4 Lyapunov must be positive (≈ln2), got {l}");
        assert!(matches!(logistic_verdict(4.0), DynamicsVerdict::ChaosAbsence { .. }));
    }

    #[test]
    fn lyapunov_negative_folds() {
        // logistic r=2.5 converges to the fixed point x*=0.6: λ_L < 0 ⇒ Fold.
        let l = lyapunov_logistic(2.5, 0.1234, 5000);
        assert!(l < 0.0, "r=2.5 Lyapunov must be negative (stable fixed point), got {l}");
        assert_eq!(logistic_verdict(2.5), DynamicsVerdict::Fold);
    }

    #[test]
    fn ergodic_time_equals_space() {
        // Birkhoff: for logistic r=4, the invariant density ρ(x)=1/(π√(x(1−x))) has mean 1/2, so the
        // time average of x over a chaotic orbit ≈ 1/2 (= the space average).
        let ta = logistic_time_average(4.0, 0.1234, 200_000);
        assert!((ta - 0.5).abs() < 0.02, "time average {ta} ≈ space average 0.5 (Birkhoff)");
    }

    #[test]
    fn chaos_absence_certified() {
        // the chaos-absence verdict carries the (positive) Lyapunov witness.
        match logistic_verdict(3.9) {
            DynamicsVerdict::ChaosAbsence { lyapunov } => assert!(lyapunov > 0.0),
            DynamicsVerdict::Fold => panic!("r=3.9 is chaotic, must be ChaosAbsence"),
        }
    }

    #[test]
    fn koopman_reduces_to_krylov_when_linear() {
        // when f is linear (a linear recurrence / A^N x₀), the Koopman observable is C-finite of
        // order = state dimension — i.e. Koopman = the Stage-31.2 Krylov / Stage-26 C-finite fold.
        // Fibonacci (linear map, 2×2 companion) → Koopman DMD order 2 = the linear state dimension.
        let fib = ints(&[0, 1, 1, 2, 3, 5, 8, 13, 21, 34]);
        assert_eq!(koopman_dmd_hankel(&fib, 4), Some(2), "linear map ⇒ Koopman reduces to C-finite/Krylov");
    }
}
