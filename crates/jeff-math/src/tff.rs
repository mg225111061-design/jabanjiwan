//! Stage 38.1 — TFF (Transfinite Fixpoint Folding): verified abstract interpretation.
//!
//! A complete lattice (here the **interval domain**) with `⊑`, `⊔`, widening `∇`, narrowing `△`,
//! and a monotone operator `F`. Naive Kleene iteration may not terminate (`[0,n]` only at the
//! limit, or `[0,+∞]` for an open loop); **widening** forces convergence to a post-fixpoint, then
//! **narrowing** recovers precision. The post-fixpoint `F(x) ⊑ x` is checked by exact lattice
//! inequalities, and widening termination by an ordinal measure (each bound goes finite→∞ at most
//! once — well-founded).
//!
//! **Honest:** the exact least fixpoint is uncomputable (Rice) — TFF certifies a **sound upper
//! bound only** (`interval-bound` certificate), never "computed exactly". New domain for JEFF
//! (verified abstract interpretation); incumbents (Astrée/IKOS) do AI but emit no machine-checked
//! certificate — that is the new part.

use crate::ordinal::Ord;

const NEG_INF: i64 = i64::MIN;
const POS_INF: i64 = i64::MAX;

/// An interval `[lo, hi]` (with ±∞ sentinels); `lo > hi` denotes `⊥` (empty / unreachable).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Interval {
    pub lo: i64,
    pub hi: i64,
}

impl Interval {
    pub fn bottom() -> Interval {
        Interval { lo: POS_INF, hi: NEG_INF }
    }
    pub fn new(lo: i64, hi: i64) -> Interval {
        Interval { lo, hi }
    }
    pub fn is_bottom(&self) -> bool {
        self.lo > self.hi
    }
    /// `self ⊑ other` (self is contained in other).
    pub fn leq(&self, o: &Interval) -> bool {
        self.is_bottom() || (o.lo <= self.lo && self.hi <= o.hi)
    }
    /// Join `⊔` (smallest interval containing both).
    pub fn join(&self, o: &Interval) -> Interval {
        if self.is_bottom() {
            return *o;
        }
        if o.is_bottom() {
            return *self;
        }
        Interval { lo: self.lo.min(o.lo), hi: self.hi.max(o.hi) }
    }
    /// Standard interval widening `∇`: a bound that grew is pushed to ±∞ (forces termination).
    pub fn widen(&self, o: &Interval) -> Interval {
        if self.is_bottom() {
            return *o;
        }
        if o.is_bottom() {
            return *self;
        }
        Interval {
            lo: if o.lo < self.lo { NEG_INF } else { self.lo },
            hi: if o.hi > self.hi { POS_INF } else { self.hi },
        }
    }
    /// Narrowing `△`: recover an ±∞ bound from the next iterate (precision recovery).
    pub fn narrow(&self, o: &Interval) -> Interval {
        if self.is_bottom() || o.is_bottom() {
            return Interval::bottom();
        }
        Interval {
            lo: if self.lo == NEG_INF { o.lo } else { self.lo },
            hi: if self.hi == POS_INF { o.hi } else { self.hi },
        }
    }
    /// Termination measure: number of *finite* bounds (each can go finite→∞ at most once under
    /// widening). Mapped to an ordinal (finite here; a relational domain would be `ω·k`).
    pub fn measure(&self) -> Ord {
        let finite = (self.lo != NEG_INF) as u64 + (self.hi != POS_INF) as u64;
        Ord::nat(finite)
    }
}

/// The loop functional for `i := 0; while i < n { i++ }` at the loop head:
/// `F(X) = {0} ⊔ ((X ∩ [-∞, n-1]) + 1)` = `[min(0, lo+1), max(0, min(hi+1, n))]`.
pub fn loop_step(x: &Interval, n: i64) -> Interval {
    let init = Interval::new(0, 0);
    if x.is_bottom() {
        return init;
    }
    let guarded_hi = x.hi.min(n - 1);
    if x.lo > guarded_hi {
        return init; // guard empty
    }
    let advanced = Interval::new(x.lo + 1, guarded_hi + 1);
    init.join(&advanced)
}

/// Analyze the loop with widening to a post-fixpoint, then one narrowing step. Returns
/// `(post_fixpoint_invariant, narrowed_invariant)`. The narrowed interval is the **sound upper
/// bound** on reachable `i` (the certificate); `loop_step(inv) ⊑ inv` holds for it.
pub fn analyze_loop(n: i64) -> (Interval, Interval) {
    // widening sequence
    let mut x = Interval::bottom();
    for _ in 0..64 {
        let next = x.widen(&loop_step(&x, n));
        if next == x {
            break;
        }
        x = next;
    }
    let post = x;
    // one narrowing step (can iterate; one suffices here)
    let mut y = post;
    for _ in 0..64 {
        let next = y.narrow(&loop_step(&y, n));
        if next == y {
            break;
        }
        y = next;
    }
    (post, y)
}

/// Post-fixpoint soundness check: `F(inv) ⊑ inv` (an exact lattice inequality).
pub fn is_post_fixpoint(inv: &Interval, n: i64) -> bool {
    loop_step(inv, n).leq(inv)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cmp::Ordering;

    #[test]
    fn widening_reaches_postfixpoint() {
        // widening converges (to [0,+∞]) and it IS a post-fixpoint.
        let (post, _) = analyze_loop(100);
        assert_eq!(post, Interval::new(0, POS_INF), "widening jumps the upper bound to +∞");
        assert!(is_post_fixpoint(&post, 100), "F(post) ⊑ post");
    }

    #[test]
    fn narrowing_recovers_precision() {
        // narrowing recovers the tight invariant [0, n].
        let n = 100;
        let (_, narrowed) = analyze_loop(n);
        assert_eq!(narrowed, Interval::new(0, n), "narrowing recovers [0,n]");
        assert!(is_post_fixpoint(&narrowed, n));
    }

    #[test]
    fn interval_bound_sound_upper() {
        // the certificate is a SOUND UPPER bound: every reachable i is inside it (we check the
        // concrete reachable set {0..n} ⊆ invariant). Not claimed exact (Rice) — sound-upper.
        let n = 50;
        let (_, inv) = analyze_loop(n);
        for i in 0..=n {
            assert!(inv.lo <= i && i <= inv.hi, "reachable i={i} must be in the sound bound {inv:?}");
        }
        // and it justifies bounds-check elimination for an array of size n+1 (i ≤ n).
        assert!(inv.hi <= n, "invariant proves i ≤ n");
    }

    #[test]
    fn lattice_ordinal_termination() {
        // widening strictly decreases the ordinal measure (finite-bound count) — well-founded ⇒
        // terminates. [0,0] (measure 2) → [0,+∞] (measure 1): strict ordinal decrease.
        let a = Interval::new(0, 0);
        let b = a.widen(&loop_step(&a, 100)); // [0,+∞]
        assert_eq!(b.measure().ord_cmp(&a.measure()), Ordering::Less, "measure strictly decreases");
    }

    #[test]
    fn lattice_laws() {
        let a = Interval::new(0, 5);
        let b = Interval::new(3, 10);
        assert!(a.leq(&a.join(&b)) && b.leq(&a.join(&b)), "join is an upper bound");
        assert!(Interval::bottom().leq(&a), "⊥ is least");
    }
}
