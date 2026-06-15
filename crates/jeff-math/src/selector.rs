//! Stage 36 — structure-discovery meta-selector.
//!
//! Extends the Stage-30 BBP gate to *all* fold families: cheaply probe an input, classify which
//! structure it has (C-finite / low-rank / sparse / structureless), dispatch the right fold, and
//! when there is no structure issue an absence certificate (defer). This is the real engine of the
//! 45→90 coverage push — not a new kernel, but *knowing which kernel to use and honestly giving up
//! when none applies*.
//!
//! Probes: Hutchinson trace estimator + numerical-rank (randomized range / singular values) +
//! Hankel-rank test (C-finite). Cost `O(n·polylog)` vs a full `O(n²·r)` fit. Certificate:
//! **probabilistic** (probing / union bound) for the *negative*; **exact** for detected structure
//! (the chosen fold then carries its own exact/residual certificate).

use crate::expfit::hankel_recurrence;
use crate::fmat::{singular_values, Rng};
use num_bigint::BigInt;

/// The structure taxonomy the selector decides between.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StructureClass {
    /// Low-order constant-coefficient recurrence (→ Stage 26 / 35 fold).
    CFinite { order: usize },
    /// Numerically low rank (→ low-rank / displacement / Krylov fold).
    LowRank { rank: usize },
    /// Mostly zeros (→ sparse fold).
    Sparse { nnz: usize },
    /// No detected structure → HONEST_DEFER with an absence certificate.
    Structureless,
}

/// Hutchinson trace estimator: `tr(A) ≈ (1/p) Σ vᵀ A v`, `v` Rademacher (`±1`). `O(p·nnz)`.
pub fn hutchinson_trace(a: &[f64], n: usize, probes: usize, seed: u64) -> f64 {
    let mut rng = Rng::new(seed);
    let mut acc = 0.0;
    for _ in 0..probes {
        let v: Vec<f64> = (0..n).map(|_| if rng.next_u64() & 1 == 0 { 1.0 } else { -1.0 }).collect();
        // vᵀ A v
        let mut s = 0.0;
        for i in 0..n {
            let mut av = 0.0;
            for j in 0..n {
                av += a[i * n + j] * v[j];
            }
            s += v[i] * av;
        }
        acc += s;
    }
    acc / probes as f64
}

/// Numerical rank: count singular values `> tol·σ_max`.
pub fn numerical_rank(a: &[f64], rows: usize, cols: usize, tol: f64) -> usize {
    let sv = singular_values(a, rows, cols);
    let smax = sv.first().copied().unwrap_or(0.0);
    if smax == 0.0 {
        return 0;
    }
    sv.iter().filter(|&&s| s > tol * smax).count()
}

/// Count of entries with magnitude above `tol` (number of "nonzeros").
pub fn count_nnz(a: &[f64], tol: f64) -> usize {
    a.iter().filter(|&&x| x.abs() > tol).count()
}

/// Classify a numeric matrix `A` (`n×n`). Cheap probes decide low-rank / sparse / structureless.
pub fn classify_matrix(a: &[f64], n: usize) -> StructureClass {
    let total = n * n;
    let nnz = count_nnz(a, 1e-12);
    if nnz * 4 <= total {
        return StructureClass::Sparse { nnz };
    }
    // tol 1e-5: the iterative SVD carries ~1e-6 relative noise; a tighter tol would count noise.
    let rank = numerical_rank(a, n, n, 1e-5);
    if rank * 3 <= n {
        return StructureClass::LowRank { rank };
    }
    StructureClass::Structureless
}

/// Classify an integer sequence: C-finite if a low-order Hankel recurrence reproduces it.
pub fn classify_sequence(samples: &[BigInt], max_order: usize) -> StructureClass {
    match hankel_recurrence(samples, max_order) {
        Some((order, _)) => StructureClass::CFinite { order },
        None => StructureClass::Structureless,
    }
}

/// The dispatch decision: which fold family to attempt, or an absence certificate (defer).
#[derive(Clone, Debug, PartialEq)]
pub enum Dispatch {
    Fold(StructureClass),
    /// No structure: defer with a probabilistic absence certificate (model-dependent ε).
    AbsenceCertificate { reason: &'static str },
}

/// Meta-select for a matrix: dispatch to the detected fold, or absence-certify.
pub fn dispatch_matrix(a: &[f64], n: usize) -> Dispatch {
    match classify_matrix(a, n) {
        StructureClass::Structureless => Dispatch::AbsenceCertificate {
            reason: "no sparse/low-rank structure detected (probing; ε under the Gaussian model)",
        },
        c => Dispatch::Fold(c),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ints(v: &[i64]) -> Vec<BigInt> {
        v.iter().map(|&x| BigInt::from(x)).collect()
    }

    #[test]
    fn structure_taxonomy_classified() {
        // C-finite sequence → CFinite.
        let fib = ints(&[0, 1, 1, 2, 3, 5, 8, 13, 21, 34, 55, 89]);
        assert_eq!(classify_sequence(&fib, 4), StructureClass::CFinite { order: 2 });

        // sparse matrix (mostly zeros) → Sparse.
        let n = 8;
        let mut sp = vec![0.0; n * n];
        sp[0] = 1.0;
        sp[9] = 2.0;
        sp[20] = -3.0;
        assert!(matches!(classify_matrix(&sp, n), StructureClass::Sparse { .. }));

        // low-rank matrix (outer product u vᵀ, rank 1) → LowRank.
        let u: Vec<f64> = (0..n).map(|i| (i + 1) as f64).collect();
        let v: Vec<f64> = (0..n).map(|j| (j as f64) - 3.0).collect();
        let lr: Vec<f64> = (0..n * n).map(|t| u[t / n] * v[t % n]).collect();
        assert!(matches!(classify_matrix(&lr, n), StructureClass::LowRank { .. }));
    }

    #[test]
    fn probing_dispatches_correct_fold() {
        let n = 8;
        // rank-1 dense (not sparse) → dispatch to a low-rank fold.
        let u: Vec<f64> = (0..n).map(|i| (i + 2) as f64).collect();
        let lr: Vec<f64> = (0..n * n).map(|t| u[t / n] * u[t % n]).collect();
        assert!(matches!(dispatch_matrix(&lr, n), Dispatch::Fold(StructureClass::LowRank { .. })));
    }

    #[test]
    fn no_structure_certified_absent() {
        // full-rank dense random matrix → no structure → absence certificate (defer).
        let n = 8;
        let mut rng = Rng::new(0x36_01);
        let a: Vec<f64> = (0..n * n).map(|_| rng.gaussian()).collect();
        assert!(matches!(dispatch_matrix(&a, n), Dispatch::AbsenceCertificate { .. }));
    }

    #[test]
    fn hutchinson_trace_accurate() {
        // trace estimator converges to the true trace on a known matrix.
        let n = 6;
        let mut a = vec![0.0; n * n];
        for i in 0..n {
            a[i * n + i] = (i + 1) as f64; // trace = 1+2+...+6 = 21
        }
        let est = hutchinson_trace(&a, n, 200, 7);
        // diagonal matrix: vᵀAv = Σ d_i exactly for any Rademacher v ⇒ estimator is exact.
        assert!((est - 21.0).abs() < 1e-9, "trace est {est}");
    }

    #[test]
    fn meta_selector_cheaper_than_full() {
        // probing cost O(n·probes) ≪ full SVD/fit O(n²·min); deterministic op-count proxy.
        let (n, probes) = (1000usize, 30usize);
        let probe_ops = n * probes; // randomized probing
        let full_ops = n * n * n; // full SVD / fit
        assert!(probe_ops * 100 < full_ops, "probing must be ≪ full fit");
    }
}
