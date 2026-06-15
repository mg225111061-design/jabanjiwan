//! Recurrence discovery and **certified exclusion** (Stage 15.1 algebraic family / 15.3
//! flagship absence certificate).
//!
//! A sequence `a_0, a_1, …` is *D-finite* (holonomic) of order `r`, degree `d` if some
//! nonzero operator `Σ_{i=0}^{r} p_i(n)·a_{n+i} = 0` annihilates it, with each `p_i` a
//! polynomial of degree ≤ `d` (the `d = 0` case is C-finite — a constant-coefficient linear
//! recurrence). Finding such an operator is a homogeneous linear system in the
//! `(r+1)(d+1)` unknown coefficients `c_{i,j}` (`p_i(n) = Σ_j c_{i,j} n^j`); row `n` is
//! `Σ_{i,j} c_{i,j}·n^j·a_{n+i} = 0` for `n = 0 … N−1−r`.
//!
//! Two exact, dual outcomes over `ℚ` (no floats — R33):
//! - **Discovery** ([`fit_recurrence`]): a nonzero nullspace vector *is* an annihilating
//!   operator (a candidate fold, to be verified on held-out terms — Tier B).
//! - **Exclusion** ([`excludes_recurrence`]): a *trivial* nullspace (full column rank, with
//!   at least as many equations as unknowns) is a **proof** that NO nonzero operator of
//!   order ≤ r, degree ≤ d annihilates the given `N` samples. This is the Stage-15 upgrade:
//!   defer becomes proof. Per the uncomputability boundary it is always relative to the
//!   class (D-finite) and parameters Θ = (r, d, N) — never an absolute "structureless".

use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::{One, Zero};

use crate::linsolve;

fn rat(n: &BigInt) -> BigRational {
    BigRational::from(n.clone())
}

/// The `(N−r) × (r+1)(d+1)` annihilator matrix over ℚ (rows = evaluation points).
pub fn annihilator_matrix(samples: &[BigInt], r: usize, d: usize) -> Vec<Vec<BigRational>> {
    let big_n = samples.len();
    if big_n <= r {
        return Vec::new();
    }
    let cols = (r + 1) * (d + 1);
    let mut rows = Vec::with_capacity(big_n - r);
    for n in 0..(big_n - r) {
        let mut row = vec![BigRational::zero(); cols];
        // powers n^0..n^d
        let mut npow = vec![BigRational::one(); d + 1];
        for j in 1..=d {
            npow[j] = npow[j - 1].clone() * BigRational::from(BigInt::from(n));
        }
        for i in 0..=r {
            let a = rat(&samples[n + i]);
            for j in 0..=d {
                row[i * (d + 1) + j] = npow[j].clone() * a.clone();
            }
        }
        rows.push(row);
    }
    rows
}

/// Surplus `N − r − (r+1)(d+1)` — the determinacy margin (Kauers: ≥ ~10 convincing). For an
/// exact exclusion proof only `surplus ≥ 0` is required, but it is reported for honesty.
pub fn surplus(num_samples: usize, r: usize, d: usize) -> isize {
    num_samples as isize - r as isize - ((r + 1) * (d + 1)) as isize
}

/// Discovery: a nonzero operator `[c_{0,0..d}, …, c_{r,0..d}]` annihilating the samples, or
/// `None` if the system has only the trivial solution. (A candidate — verify on held-out
/// terms before trusting it; the *exclusion* direction is the proven one.)
pub fn fit_recurrence(samples: &[BigInt], r: usize, d: usize) -> Option<Vec<BigRational>> {
    let m = annihilator_matrix(samples, r, d);
    if m.is_empty() {
        return None;
    }
    let ns = linsolve::nullspace(&m);
    ns.into_iter().next()
}

/// Exclusion proof: `true` iff NO nonzero order-≤r, degree-≤d operator annihilates the
/// samples — i.e. the system is determined (`surplus ≥ 0`) and has full column rank
/// (trivial nullspace). Exact over ℚ.
pub fn excludes_recurrence(samples: &[BigInt], r: usize, d: usize) -> bool {
    if surplus(samples.len(), r, d) < 0 {
        return false; // underdetermined — cannot exclude
    }
    let m = annihilator_matrix(samples, r, d);
    if m.is_empty() {
        return false;
    }
    linsolve::nullspace(&m).is_empty()
}

/// Verify a claimed annihilating operator exactly: every equation row evaluates to 0.
pub fn verify_annihilator(samples: &[BigInt], r: usize, d: usize, op: &[BigRational]) -> bool {
    let m = annihilator_matrix(samples, r, d);
    if m.is_empty() || op.len() != (r + 1) * (d + 1) || op.iter().all(|c| c.is_zero()) {
        return false;
    }
    m.iter().all(|row| {
        row.iter()
            .zip(op)
            .fold(BigRational::zero(), |acc, (x, c)| acc + x * c)
            .is_zero()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seq(v: &[i64]) -> Vec<BigInt> {
        v.iter().map(|&x| BigInt::from(x)).collect()
    }

    #[test]
    fn fibonacci_has_order2_constant_recurrence() {
        // a_{n+2} = a_{n+1} + a_n  ⇒ a C-finite (degree 0) order-2 operator exists.
        let fib = seq(&[0, 1, 1, 2, 3, 5, 8, 13, 21, 34, 55, 89, 144, 233, 377]);
        let op = fit_recurrence(&fib, 2, 0).expect("Fibonacci is C-finite order 2");
        assert!(verify_annihilator(&fib, 2, 0, &op));
        assert!(!excludes_recurrence(&fib, 2, 0), "a recurrence exists, cannot exclude");
        // but NO order-1 constant-coefficient recurrence (it is not geometric).
        assert!(excludes_recurrence(&fib, 1, 0), "no order-1 const-coeff recurrence");
    }

    #[test]
    fn factorial_is_d_finite_order1_degree1() {
        // n! : a_{n+1} = (n+1) a_n ⇒ order 1, degree 1 operator exists.
        let fact = seq(&[1, 1, 2, 6, 24, 120, 720, 5040, 40320, 362880, 3628800, 39916800]);
        assert!(fit_recurrence(&fact, 1, 1).is_some());
        assert!(!excludes_recurrence(&fact, 1, 1));
        // no constant-coefficient (degree 0) recurrence of order ≤ 2 fits n!.
        assert!(excludes_recurrence(&fact, 2, 0), "n! is not C-finite of order 2");
    }

    #[test]
    fn polynomial_is_c_finite() {
        // a_n = n^2 has zero third differences ⇒ C-finite order 3 (degree 0).
        let sq: Vec<BigInt> = (0..16i64).map(|n| BigInt::from(n * n)).collect();
        assert!(!excludes_recurrence(&sq, 3, 0), "n^2 is C-finite order 3");
        assert!(fit_recurrence(&sq, 3, 0).is_some());
    }

    #[test]
    fn structureless_sequence_excluded_exactly() {
        // a deterministic high-entropy integer sequence (xorshift) has no low-order
        // D-finite operator — the exclusion is exact (full column rank).
        let mut x = 0x1234_5678_9abc_def1u64;
        let s: Vec<BigInt> = (0..40)
            .map(|_| {
                x ^= x << 13;
                x ^= x >> 7;
                x ^= x << 17;
                BigInt::from((x % 1000) as i64)
            })
            .collect();
        // order ≤ 2, degree ≤ 1: cols = 6, rows = 38, surplus large ⇒ excluded.
        assert!(surplus(s.len(), 2, 1) >= 0);
        assert!(excludes_recurrence(&s, 2, 1), "high-entropy seq has no (2,1) operator");
        assert!(fit_recurrence(&s, 2, 1).is_none());
    }

    #[test]
    fn cannot_exclude_when_underdetermined() {
        // too few samples for the unknown count ⇒ no exclusion claim (honest).
        let short = seq(&[1, 2, 3, 4]);
        assert!(surplus(short.len(), 2, 2) < 0);
        assert!(!excludes_recurrence(&short, 2, 2));
    }
}
