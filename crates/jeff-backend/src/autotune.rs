//! Stage 17.4 — autotuning over a FIXED candidate set, machine-specific, every winner
//! verified bit-exact, results cached as "wisdom".
//!
//! This is NOT self-evolution: it searches a *predefined* set of equivalent implementations
//! and picks the empirically fastest one ON THIS CPU — it never invents algorithms. The
//! candidate set is an equivalence class (every member is bit-for-bit identical to the
//! oracle), so selecting the lowest-cost member is exactly the e-graph principle
//! (semantics-preserving rewrites → extract lowest cost) made empirical. (The full egg
//! equality-saturation backend lives in `jeff-recognizer`; here the equivalence is enforced
//! by the bit-exact gate before any config is cached.)

use crate::measure::Timer;
use crate::simd::{igemm_scalar, igemm_tiled};
use jeff_math::fmat::Rng;
use std::collections::HashMap;

/// A GEMM implementation choice (the fixed search space for the integer micro-kernel).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GemmChoice {
    /// The naive streaming loop (LLVM auto-vectorizes it well — see 17.2).
    Scalar,
    /// The register-tiled micro-kernel (retained as a candidate; usually loses here).
    Tiled,
}

type GemmFn = fn(&[i64], &[i64], usize, usize, usize) -> Vec<i64>;

fn impl_of(choice: GemmChoice) -> GemmFn {
    match choice {
        GemmChoice::Scalar => igemm_scalar,
        GemmChoice::Tiled => igemm_tiled,
    }
}

/// The full candidate set (the equivalence class to search/extract from).
pub const GEMM_CANDIDATES: [GemmChoice; 2] = [GemmChoice::Scalar, GemmChoice::Tiled];

/// Empirical GEMM autotuner with cached wisdom (keyed by problem size).
#[derive(Default)]
pub struct GemmTuner {
    wisdom: HashMap<usize, GemmChoice>,
    /// How many actual searches ran (a cache hit does not increment this).
    pub searches: u64,
}

impl GemmTuner {
    pub fn new() -> Self {
        Self::default()
    }

    /// A small deterministic probe input at size `n` (values kept small to avoid i64 overflow).
    fn probe(n: usize) -> (Vec<i64>, Vec<i64>) {
        let mut r = Rng::new(0x17_4A ^ n as u64);
        (
            (0..n * n).map(|_| (r.next_u64() % 7) as i64).collect(),
            (0..n * n).map(|_| (r.next_u64() % 7) as i64).collect(),
        )
    }

    /// Return the empirically fastest candidate for size `n`. On a miss, benchmark every
    /// candidate, **reject any that is not bit-exact vs the oracle**, keep the fastest
    /// verified one, and cache it. On a hit, reuse the cached wisdom (no re-search).
    pub fn best(&mut self, n: usize) -> GemmChoice {
        if let Some(&c) = self.wisdom.get(&n) {
            return c;
        }
        self.searches += 1;
        let (a, b) = Self::probe(n);
        let oracle = igemm_scalar(&a, &b, n, n, n);
        let mut best: Option<(GemmChoice, std::time::Duration)> = None;
        for &choice in &GEMM_CANDIDATES {
            let f = impl_of(choice);
            // bit-exact gate: a candidate that changes the answer is discarded (P0).
            if f(&a, &b, n, n, n) != oracle {
                continue;
            }
            let t = Timer::best_of(3, || {
                std::hint::black_box(f(&a, &b, n, n, n));
            });
            if best.is_none_or(|(_, bt)| t < bt) {
                best = Some((choice, t));
            }
        }
        let winner = best.expect("the scalar oracle is always a verified candidate").0;
        self.wisdom.insert(n, winner);
        winner
    }

    /// Verify a chosen config is bit-exact at `n` (used to re-check cached wisdom).
    pub fn verify(&self, choice: GemmChoice, n: usize) -> bool {
        let (a, b) = Self::probe(n);
        impl_of(choice)(&a, &b, n, n, n) == igemm_scalar(&a, &b, n, n, n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equivalent_candidates_are_bit_exact() {
        // egraph_rewrite_preserves_result: every candidate in the search space is a
        // semantics-preserving rewrite — bit-for-bit identical to the oracle.
        let mut r = Rng::new(0x17_4B);
        for n in [8usize, 17, 64] {
            let a: Vec<i64> = (0..n * n).map(|_| (r.next_u64() % 9) as i64 - 4).collect();
            let b: Vec<i64> = (0..n * n).map(|_| (r.next_u64() % 9) as i64 - 4).collect();
            let oracle = igemm_scalar(&a, &b, n, n, n);
            for &c in &GEMM_CANDIDATES {
                assert_eq!(impl_of(c)(&a, &b, n, n, n), oracle, "candidate {c:?} must preserve result");
            }
        }
    }

    #[test]
    fn autotuned_config_verified() {
        // The winning config is bit-exact (verified before it was ever cached).
        let mut tuner = GemmTuner::new();
        for n in [32usize, 96] {
            let winner = tuner.best(n);
            assert!(tuner.verify(winner, n), "autotuned winner must be bit-exact at n={n}");
        }
    }

    #[test]
    fn wisdom_cached_and_reused() {
        // Search is paid once per size; subsequent calls reuse cached wisdom.
        let mut tuner = GemmTuner::new();
        let c1 = tuner.best(64);
        assert_eq!(tuner.searches, 1);
        let c2 = tuner.best(64); // cache hit — no new search
        assert_eq!(tuner.searches, 1, "cached wisdom must be reused, not re-searched");
        assert_eq!(c1, c2);
        let _ = tuner.best(128); // a new size searches once more
        assert_eq!(tuner.searches, 2);
    }

    #[test]
    fn autotuner_picks_a_real_winner() {
        // On this CPU the naive loop auto-vectorizes best (17.2 finding); the tuner is free
        // to pick either, but it must pick a VERIFIED candidate from the fixed set.
        let mut tuner = GemmTuner::new();
        let w = tuner.best(128);
        assert!(GEMM_CANDIDATES.contains(&w));
        assert!(tuner.verify(w, 128));
    }
}
