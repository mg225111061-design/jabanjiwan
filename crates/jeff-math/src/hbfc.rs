//! Stage 38.2 — HBFC (Hydra-Bounded Fusion Calculus), **gated by the 32.0 strength audit**.
//!
//! Fold-fusion (deforestation: `fold∘fold∘fold` → one fold, no intermediate data structures) needs
//! a termination measure for the rewrite system, which may *temporarily* explode a term (Hydra
//! head multiplication) yet must terminate. The natural measure is an ordinal; a strict ordinal
//! decrease per rewrite ⇒ finite termination even with transient blow-up.
//!
//! **GATE (32.0 result):** the directive's `ε₀` self-certification is **unsound for this JEFF** —
//! the strength audit found `‖JEFF‖ = ω^ω`, not ε₀ (the checker is quantifier-free exact-identity,
//! no arbitrary-predicate first-order induction). So HBFC is **downgraded to ω^k** Hydra measures
//! (recorded loudly). This is the honest negative the keystone exists to produce: an `ε₀` Hydra
//! claim would be self-certifying beyond JEFF's real strength. ε₀-genuine fusions are rare anyway
//! (`ω^k` covers the deforestation we actually do).

use crate::ordinal::Ord;

/// The audited proof-theoretic strength of JEFF (32.0). **`OmegaK`** for this tree.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JeffStrength {
    /// `ω^ω` — quantifier-free exact-identity checker (PRA-style). The audited result.
    OmegaK,
    /// `ε₀` — full first-order PA induction (NOT this tree; would require a Z3/Lean induction gate).
    Epsilon0,
}

/// The audited strength. HBFC consumes this to choose its measure ceiling.
pub const AUDITED_STRENGTH: JeffStrength = JeffStrength::OmegaK;

/// A Hydra/fold-fusion measure: `ω^{depth}·heads + stages`, kept in the **ω^k fragment** (finite
/// exponents) per the audit. (An ε₀ system could use recursive-exponent towers; downgraded here.)
pub fn fusion_measure(depth: u64, heads: u64, stages: u64) -> Ord {
    let mut terms = Vec::new();
    if heads > 0 {
        terms.push((Ord::nat(depth), heads));
    }
    if stages > 0 {
        terms.push((Ord::zero(), stages));
    }
    Ord { terms }
}

/// Unfused pipeline `sum(map(g, map(f, xs)))` — materializes two intermediate vectors.
pub fn unfused_pipeline(xs: &[i64], f: fn(i64) -> i64, g: fn(i64) -> i64) -> i64 {
    let t1: Vec<i64> = xs.iter().map(|&x| f(x)).collect(); // intermediate 1
    let t2: Vec<i64> = t1.iter().map(|&x| g(x)).collect(); // intermediate 2
    t2.iter().sum()
}

/// Fused pipeline `fold(λacc x. acc + g(f(x)), 0, xs)` — no intermediate vectors.
pub fn fused_pipeline(xs: &[i64], f: fn(i64) -> i64, g: fn(i64) -> i64) -> i64 {
    xs.iter().fold(0i64, |acc, &x| acc + g(f(x)))
}

/// A fusion rewrite step: removing one intermediate may transiently raise the head count (Hydra)
/// but the ordinal measure strictly **decreases**. Returns the post-rewrite measure; the caller
/// checks `after < before` via [`Ord::ord_cmp`].
pub fn fuse_one(depth: u64, heads: u64, stages: u64) -> Ord {
    // remove one materialized stage; the rewrite may spawn `heads+1` transient heads at a strictly
    // LOWER depth (Hydra), but the leading term drops ⇒ ordinal decrease.
    if stages > 0 {
        fusion_measure(depth.saturating_sub(1), heads + 1, stages - 1)
    } else {
        fusion_measure(depth, heads.saturating_sub(1), stages)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cmp::Ordering;

    #[test]
    fn hbfc_gated_by_strength_audit() {
        // HBFC consumes the 32.0 audit: ‖JEFF‖ = ω^ω ⇒ measures stay in the ω^k fragment
        // (ε₀ downgraded). Recorded: an ε₀ Hydra claim would be unsound for this JEFF.
        assert_eq!(AUDITED_STRENGTH, JeffStrength::OmegaK, "32.0 audit: ω^ω, not ε₀");
        let m = fusion_measure(3, 2, 5); // ω³·2 + 5 — finite exponents ⇒ ω^k fragment
        assert!(m.in_omega_k_fragment(), "HBFC measure must be in the certifiable ω^k fragment");
    }

    #[test]
    fn hydra_measure_strict_decrease() {
        // a fusion rewrite that transiently raises heads (depth 3→2, heads 2→3) still strictly
        // decreases the ordinal measure (the leading ω³ term drops to ω²).
        let before = fusion_measure(3, 2, 5);
        let after = fuse_one(3, 2, 5);
        assert_eq!(after.ord_cmp(&before), Ordering::Less, "Hydra rewrite strictly decreases measure");
    }

    #[test]
    fn fusion_terminates_or_downgraded() {
        // iterating fusion strictly decreases the measure each step ⇒ terminates (well-founded ω^k).
        let (mut depth, mut heads, mut stages) = (4u64, 1u64, 4u64);
        let mut prev = fusion_measure(depth, heads, stages);
        let mut steps = 0;
        loop {
            let next = fuse_one(depth, heads, stages);
            if next.ord_cmp(&prev) != Ordering::Less {
                break;
            }
            // apply the rewrite's new state
            if stages > 0 {
                depth = depth.saturating_sub(1);
                heads += 1;
                stages -= 1;
            } else if heads > 0 {
                heads -= 1;
            } else {
                break;
            }
            prev = next;
            steps += 1;
            assert!(steps < 1000, "must terminate");
        }
        assert!(steps > 0, "fusion made progress and terminated (ω^k well-founded)");
    }

    #[test]
    fn fusion_law_semantic_preservation() {
        // the fold-fusion law: fused == unfused (observational equivalence), no intermediates.
        fn sq(x: i64) -> i64 {
            x * x
        }
        fn inc(x: i64) -> i64 {
            x + 1
        }
        for xs in [vec![1, 2, 3, 4, 5], vec![-3, 0, 7], vec![10; 20]] {
            assert_eq!(
                fused_pipeline(&xs, sq, inc),
                unfused_pipeline(&xs, sq, inc),
                "fold fusion must preserve semantics"
            );
        }
        // concretely: sum(map(inc, map(sq, [1..5]))) = Σ (i²+1) = (1+4+9+16+25)+5 = 60
        assert_eq!(fused_pipeline(&[1, 2, 3, 4, 5], sq, inc), 60);
    }
}
