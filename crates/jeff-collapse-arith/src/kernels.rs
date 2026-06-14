//! Stage-3 Tier-S kernel collapsers (CLAUDE.md 10.2, B.6, 20.2).
//!
//! Three exact, proof-carrying linear-algebra kernels. Each runs the fast kernel,
//! checks its structural precondition, emits a certificate, and **verifies** it —
//! returning a verified collapse or an honest defer. Correctness rests on the
//! independent checker (jeff-verify), never on the kernel itself (P0).
//!
//!   * Woodbury / Sherman–Morrison — `(A+UCV)⁻¹` in O(n²k+k³); exact cert `=I`
//!     over ℚ. Precondition: rank `k ≪ n`, else demote to constant-factor-only.
//!   * Strassen — `A·B` in O(n^2.807); Freivalds cert over ℤ, `2^-r` false-accept.
//!   * Cholesky (LDLᵀ) — `A = L D Lᵀ` exact over ℚ; SPD via `D>0`, else **refused**
//!     (never a negative square root).
//!
//! These kernels operate on matrices directly (the surface language has no matrix
//! syntax yet — that is out of Tier-S scope, AR-5); they are the verified numeric
//! core that surface support would later call.

use jeff_cert::{
    BarrierTag, Boundary, Certificate, Collapsed, CollapseOutcome, Defer, Evidence, IrRef,
    Obligation,
};
use jeff_math::{freivalds_seeds, IntMatrix, RatMatrix};
use jeff_span::Span;
use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::Zero;

/// The result a kernel produced (alongside the gated outcome).
#[derive(Clone, Debug)]
pub enum KernelResult {
    Inverse(RatMatrix),
    Product(IntMatrix),
    Ldlt(RatMatrix, Vec<BigRational>),
}

/// A kernel collapse attempt: the gated outcome plus the computed result.
#[derive(Clone, Debug)]
pub struct KernelOutcome {
    pub outcome: CollapseOutcome,
    pub result: Option<KernelResult>,
}

fn node() -> IrRef {
    IrRef::new(1, Span::dummy())
}

fn defer(tag: BarrierTag, diag: &str) -> KernelOutcome {
    let mut d = Defer::new(tag, node());
    d.diagnostic.message = format!("{} — {}", d.diagnostic.message, diag);
    KernelOutcome {
        outcome: CollapseOutcome::Defer(d),
        result: None,
    }
}

fn collapsed(cert: Certificate, result: KernelResult) -> KernelOutcome {
    match jeff_verify::verify(cert) {
        Some(vc) => KernelOutcome {
            outcome: CollapseOutcome::Collapsed(Collapsed::new(node(), vc)),
            result: Some(result),
        },
        // R31: a kernel result that does not verify must fall back, never ship.
        None => defer(
            BarrierTag::ConstantFactorOnly,
            "kernel certificate failed verification (falling back)",
        ),
    }
}

/// Sherman–Morrison / Woodbury rank-`k` inverse update. `a` and its inverse `a_inv`
/// are given; the update is `U C V` (`u: n×k`, `c: k×k`, `v: k×n`). Collapses iff
/// `k ≪ n` (here `4k ≤ n`), else demotes to constant-factor-only (reported, R19).
pub fn woodbury_collapse(
    a: &RatMatrix,
    a_inv: &RatMatrix,
    u: &RatMatrix,
    c: &RatMatrix,
    v: &RatMatrix,
) -> KernelOutcome {
    let n = a.rows;
    let k = u.cols;
    if k * 4 > n {
        return defer(
            BarrierTag::ConstantFactorOnly,
            "low-rank update is not k<<n (rank-k ~ n); no asymptotic win over full re-inversion",
        );
    }
    let Some(updated) = RatMatrix::woodbury(a_inv, u, c, v) else {
        return defer(
            BarrierTag::ConstantFactorOnly,
            "Woodbury inner system singular; fall back to full inversion",
        );
    };
    // (A + U C V), then certify (A+UCV)·X̂ = I exactly.
    let aucv = a.add(&u.mul(c).mul(v));
    let cert = Certificate {
        collapser_id: "kernel/woodbury".into(),
        source: node(),
        collapsed: IrRef::new(2, Span::dummy()),
        obligation: Obligation::new(format!(
            "(A + U C V)·X̂ = I exactly over Q; rank k={k} << n={n}"
        )),
        evidence: Evidence::MatrixInverse {
            m: aucv,
            inv: updated.clone(),
        },
        boundaries: vec![Boundary::new(format!("structural: rank k={k}, dimension n={n}"))],
        fallback: node(),
    };
    collapsed(cert, KernelResult::Inverse(updated))
}

/// Strassen product with an exact Freivalds certificate (`rounds` rounds ⇒ false-
/// accept ≤ 2^-rounds). Seeds are recorded for deterministic replay (R11).
pub fn strassen_collapse(a: &IntMatrix, b: &IntMatrix, rounds: usize) -> KernelOutcome {
    if a.n != b.n {
        return defer(BarrierTag::ConstantFactorOnly, "non-conformant matrices");
    }
    let n = a.n;
    let c = a.strassen_mul(b);
    // deterministic salt from the data, so seeds (and thus the cert) are reproducible.
    let salt = fold_salt(a).wrapping_mul(31).wrapping_add(fold_salt(b));
    let seeds = freivalds_seeds(n, rounds.max(1), salt);
    let cert = Certificate {
        collapser_id: "kernel/strassen".into(),
        source: node(),
        collapsed: IrRef::new(2, Span::dummy()),
        obligation: Obligation::new(format!(
            "C = A·B verified by exact Freivalds over Z, {rounds} rounds (false-accept <= 2^-{rounds})"
        )),
        evidence: Evidence::FreivaldsProduct {
            a: a.data.clone(),
            b: b.data.clone(),
            c: c.data.clone(),
            dim: n,
            seeds,
        },
        boundaries: vec![Boundary::new("{0,1} test vectors; integral-domain Freivalds lemma")],
        fallback: node(),
    };
    collapsed(cert, KernelResult::Product(c))
}

fn fold_salt(m: &IntMatrix) -> u64 {
    let mut h = 1469598103934665603u64;
    for v in &m.data {
        // fold the low 64 bits of each entry (deterministic)
        let bits = (v % BigInt::from(u64::MAX)).to_string();
        for b in bits.bytes() {
            h ^= b as u64;
            h = h.wrapping_mul(1099511628211);
        }
    }
    h
}

/// Cholesky via sqrt-free LDLᵀ. SPD ⇒ exact cert `A = L D Lᵀ` with `D>0`. Non-SPD or
/// non-factorable ⇒ **refused** (no negative square root, R per the brief).
pub fn cholesky_collapse(a: &RatMatrix) -> KernelOutcome {
    if a.rows != a.cols {
        return defer(BarrierTag::ConstantFactorOnly, "Cholesky needs a square matrix");
    }
    // symmetry is required for SPD
    if a != &a.transpose() {
        return defer(
            BarrierTag::ConstantFactorOnly,
            "matrix is not symmetric; Cholesky/LDLᵀ refused",
        );
    }
    let Some((l, d)) = a.ldlt() else {
        return defer(
            BarrierTag::ConstantFactorOnly,
            "matrix has a zero pivot; not LDLᵀ-factorable",
        );
    };
    if !d.iter().all(|x| *x > BigRational::zero()) {
        return defer(
            BarrierTag::ConstantFactorOnly,
            "matrix is not positive-definite (some D<=0); refused (no negative sqrt)",
        );
    }
    let cert = Certificate {
        collapser_id: "kernel/cholesky-ldlt".into(),
        source: node(),
        collapsed: IrRef::new(2, Span::dummy()),
        obligation: Obligation::new("A = L·diag(D)·Lᵀ exactly over Q, and D>0 (SPD)"),
        evidence: Evidence::LdltSpd {
            a: a.clone(),
            l: l.clone(),
            d: d.clone(),
        },
        boundaries: vec![Boundary::new("SPD: every pivot D[i] > 0")],
        fallback: node(),
    };
    let _ = BigInt::zero();
    collapsed(cert, KernelResult::Ldlt(l, d))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn woodbury_collapses_when_low_rank() {
        let a = RatMatrix::from_i64(8, 8, &diag8());
        let a_inv = a.inverse().unwrap();
        let u = RatMatrix::from_i64(8, 1, &[1, 0, 0, 0, 0, 0, 0, 1]);
        let c = RatMatrix::from_i64(1, 1, &[1]);
        let v = RatMatrix::from_i64(1, 8, &[1, 0, 0, 0, 0, 0, 0, 1]);
        let out = woodbury_collapse(&a, &a_inv, &u, &c, &v);
        assert!(matches!(out.outcome, CollapseOutcome::Collapsed(_)), "k=1<<8 collapses");
        // result equals the direct inverse of A+uvᵀ
        let aucv = a.add(&u.mul(&c).mul(&v));
        let KernelResult::Inverse(x) = out.result.unwrap() else { panic!() };
        assert_eq!(x, aucv.inverse().unwrap());
    }

    #[test]
    fn woodbury_demotes_when_rank_near_n() {
        let a = RatMatrix::identity(4);
        let a_inv = a.inverse().unwrap();
        // k=3, n=4 → 4k>n → demote (constant-factor-only).
        let u = RatMatrix::from_i64(4, 3, &[1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0]);
        let c = RatMatrix::identity(3);
        let v = RatMatrix::from_i64(3, 4, &[1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0]);
        let out = woodbury_collapse(&a, &a_inv, &u, &c, &v);
        assert!(matches!(out.outcome, CollapseOutcome::Defer(_)), "k~n demotes");
    }

    #[test]
    fn strassen_collapses_with_freivalds_cert() {
        let a = IntMatrix::from_i64(3, &[1, 2, 3, 4, 5, 6, 7, 8, 9]);
        let b = IntMatrix::from_i64(3, &[2, 0, 1, 1, 3, 0, 0, 1, 4]);
        let out = strassen_collapse(&a, &b, 40);
        assert!(matches!(out.outcome, CollapseOutcome::Collapsed(_)));
        let KernelResult::Product(c) = out.result.unwrap() else { panic!() };
        assert_eq!(c, a.naive_mul(&b)); // result is the true product
    }

    #[test]
    fn cholesky_collapses_spd_refuses_non_spd() {
        let spd = RatMatrix::from_i64(3, 3, &[4, 2, 0, 2, 5, 2, 0, 2, 3]);
        let out = cholesky_collapse(&spd);
        assert!(matches!(out.outcome, CollapseOutcome::Collapsed(_)), "SPD collapses");
        // indefinite (not positive-definite) → refused
        let indef = RatMatrix::from_i64(2, 2, &[1, 2, 2, 1]);
        assert!(matches!(cholesky_collapse(&indef).outcome, CollapseOutcome::Defer(_)));
        // non-symmetric → refused
        let asym = RatMatrix::from_i64(2, 2, &[1, 2, 3, 4]);
        assert!(matches!(cholesky_collapse(&asym).outcome, CollapseOutcome::Defer(_)));
    }

    fn diag8() -> Vec<i64> {
        let mut d = vec![0i64; 64];
        for i in 0..8 {
            d[i * 8 + i] = (i as i64) + 2;
        }
        d
    }
}
