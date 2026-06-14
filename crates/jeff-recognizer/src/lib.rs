//! Layer 0 — the recognizer: drive work to a canonical form, extract an asymptotic
//! cost, and emit a dispatch tag (CLAUDE.md PART 6.3, 10.1). Also hosts the
//! [`Collapser`] trait (PART 6.4) since it references [`Recognized`].
//!
//! Stage-0 scope (AR-5): direct, principled pattern recognition for the fixtures
//! the thin slice needs (power sums → `AffineTripCount`). The constitution's design
//! (D5) is egglog equality saturation; that is the **Stage-2** plan and is *not*
//! faked here — unrecognised work returns `DispatchTag::None`, never a wrong tag.

use jeff_cert::CollapseOutcome;
use jeff_core_ir::{BinOp, CoreExpr, CoreExprKind, CoreFn, CoreDomain, RedKind};

pub use jeff_cert::CollapserId;

/// Dispatch tags (PART 6.3 / glossary).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DispatchTag {
    Holonomic,
    LinearStateTransition,
    Convolution,
    Gf2Affine,
    PlanarCsp,
    BoundedTreewidth,
    AffineTripCount,
    None,
}

/// Asymptotic cost lattice (PART 6.3 / 10.1): Const ≪ Log ≪ Sublinear ≪ Linear ≪
/// Superlinear(deg).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AsymptoticCost {
    Const,
    Log,
    Sublinear,
    Linear,
    Superlinear(u32),
}

/// A saturation/recognition budget (PART 6.3). Carried for API stability; the
/// Stage-0 direct recognizer is bounded by construction, so it is currently advisory.
#[derive(Clone, Copy, Debug)]
pub struct SaturationBudget {
    pub max_nodes: usize,
    pub max_millis: u64,
}

impl Default for SaturationBudget {
    fn default() -> Self {
        SaturationBudget {
            max_nodes: 100_000,
            max_millis: 1_000,
        }
    }
}

/// Recognition result: the dispatch tag and the *candidate* (post-collapse) cost.
#[derive(Clone, Copy, Debug)]
pub struct Recognized {
    pub tag: DispatchTag,
    /// Cost of the original work (e.g. a Θ(N) loop).
    pub original_cost: AsymptoticCost,
    /// Cost of the candidate collapsed form (e.g. O(1) closed form).
    pub candidate_cost: AsymptoticCost,
}

/// Recognize a function body (PART 10.1). Pure pattern dispatch for Stage 0.
pub fn recognize(f: &CoreFn, _budget: SaturationBudget) -> Recognized {
    match &f.body.kind {
        CoreExprKind::Reduction {
            kind: RedKind::Sum,
            binder,
            domain: CoreDomain::Range { .. },
            body,
        } => {
            if is_polynomial_in(body, binder) {
                // Σ of a polynomial over an affine range → closed quasi-polynomial.
                Recognized {
                    tag: DispatchTag::AffineTripCount,
                    original_cost: AsymptoticCost::Linear,
                    candidate_cost: AsymptoticCost::Const,
                }
            } else if mentions_call(body) {
                // Σ of a hypergeometric summand (binomial / factorial) → holonomic
                // (Gosper / Zeilberger creative telescoping).
                Recognized {
                    tag: DispatchTag::Holonomic,
                    original_cost: AsymptoticCost::Linear,
                    candidate_cost: AsymptoticCost::Sublinear,
                }
            } else if is_geometric_in(body, binder) {
                // Σ base^i → linear state transition (eigen / Bostan–Mori). The
                // collapser for this is Stage 1/3; recognise it honestly here.
                Recognized {
                    tag: DispatchTag::LinearStateTransition,
                    original_cost: AsymptoticCost::Linear,
                    candidate_cost: AsymptoticCost::Log,
                }
            } else {
                none()
            }
        }
        CoreExprKind::Reduction {
            kind: RedKind::Count,
            ..
        } => Recognized {
            // affine counting candidate (Barvinok, Stage 4)
            tag: DispatchTag::AffineTripCount,
            original_cost: AsymptoticCost::Linear,
            candidate_cost: AsymptoticCost::Const,
        },
        _ => none(),
    }
}

fn none() -> Recognized {
    Recognized {
        tag: DispatchTag::None,
        original_cost: AsymptoticCost::Linear,
        candidate_cost: AsymptoticCost::Linear,
    }
}

/// Is `e` a polynomial in `binder` with only constant (integer-literal) and
/// parameter-free structure? Allows `+ - *`, `**` with a constant exponent, the
/// binder, and integer literals. Other variables (parameters in the body) make it
/// *not* a pure power-sum for the Stage-0 Faulhaber path (conservative — DR8).
pub fn is_polynomial_in(e: &CoreExpr, binder: &str) -> bool {
    match &e.kind {
        CoreExprKind::Int(_) => true,
        CoreExprKind::Var(v) => v == binder, // only the binder; params disqualify
        CoreExprKind::Neg(a) => is_polynomial_in(a, binder),
        CoreExprKind::Bin(op, a, b) => match op {
            BinOp::Add | BinOp::Sub | BinOp::Mul => {
                is_polynomial_in(a, binder) && is_polynomial_in(b, binder)
            }
            BinOp::Pow => {
                // base polynomial, exponent a non-negative integer literal
                is_polynomial_in(a, binder) && matches!(&b.kind, CoreExprKind::Int(_))
            }
            _ => false,
        },
        CoreExprKind::Call(..) => false, // a binomial/factorial call is not a polynomial
        CoreExprKind::Reduction { .. } => false,
    }
}

/// Does `e` contain a builtin call (binomial / factorial) — i.e. is it a
/// hypergeometric summand candidate for the holonomic collapser?
fn mentions_call(e: &CoreExpr) -> bool {
    match &e.kind {
        CoreExprKind::Call(..) => true,
        CoreExprKind::Neg(a) => mentions_call(a),
        CoreExprKind::Bin(_, a, b) => mentions_call(a) || mentions_call(b),
        CoreExprKind::Reduction { body, .. } => mentions_call(body),
        _ => false,
    }
}

/// Is `e` of the form `base ** binder` (binder in the exponent) — a geometric term?
fn is_geometric_in(e: &CoreExpr, binder: &str) -> bool {
    fn mentions(e: &CoreExpr, binder: &str) -> bool {
        match &e.kind {
            CoreExprKind::Var(v) => v == binder,
            CoreExprKind::Int(_) => false,
            CoreExprKind::Neg(a) => mentions(a, binder),
            CoreExprKind::Bin(_, a, b) => mentions(a, binder) || mentions(b, binder),
            CoreExprKind::Call(_, args) => args.iter().any(|a| mentions(a, binder)),
            CoreExprKind::Reduction { body, .. } => mentions(body, binder),
        }
    }
    fn check(e: &CoreExpr, binder: &str) -> bool {
        match &e.kind {
            CoreExprKind::Bin(BinOp::Pow, _base, exp) => mentions(exp, binder),
            CoreExprKind::Bin(BinOp::Mul, a, b) => check(a, binder) || check(b, binder),
            _ => false,
        }
    }
    check(e, binder)
}

/// A collapser (PART 6.4). `entry` gates on the dispatch tag; `try_collapse` always
/// returns either a *verified* `Collapsed` or an honest `Defer` (R32: no half
/// collapse; R38: never panics).
pub trait Collapser {
    const ID: CollapserId;
    fn entry(&self, r: &Recognized) -> bool;
    fn try_collapse(&self, f: &CoreFn) -> CollapseOutcome;
}

#[cfg(test)]
mod tests {
    use super::*;
    use jeff_core_ir::lower;
    use jeff_syntax::parse;

    fn recognize_src(src: &str) -> Recognized {
        let p = parse(src, 0).unwrap();
        let ir = lower(&p).unwrap();
        recognize(&ir.funcs[0], SaturationBudget::default())
    }

    #[test]
    fn power_sum_is_affine_trip_count() {
        let r = recognize_src("total fn s(n: nat) -> nat: sum i in 0..=n: i\n");
        assert_eq!(r.tag, DispatchTag::AffineTripCount);
        assert_eq!(r.candidate_cost, AsymptoticCost::Const);

        let r2 = recognize_src("total fn s(n: nat) -> nat: sum i in 0..=n: i*i\n");
        assert_eq!(r2.tag, DispatchTag::AffineTripCount);
    }

    #[test]
    fn geometric_is_linear_state_transition() {
        let r = recognize_src("total fn s(n: nat) -> nat: sum i in 0..n: 2**i\n");
        assert_eq!(r.tag, DispatchTag::LinearStateTransition);
    }
}
