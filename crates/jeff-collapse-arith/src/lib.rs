//! Layer 1 — arithmetic folder. Stage 0 delivers the **Faulhaber** power-sum
//! collapse with a real, verifier-checked certificate.
//!
//! Authority: CLAUDE.md 10.2 (Layer 1), PART 11 example 1, APPENDIX F.1, E.7
//! (collapser skeleton), 20.1 (new-collapser checklist).
//!
//! Method (AR-4: naive-correct reference → derive → *prove*):
//!   1. The summand `P(i)` is converted to an exact `UniPoly` in the binder.
//!   2. The closed form `S(m) = Σ_{i=lo}^{m} P(i)` is a polynomial of degree
//!      `deg P + 1`. We sample the *naive* sum (the evaluator) at `deg P + 2`
//!      points — including `lo-1` (empty sum = 0) — and interpolate exactly.
//!   3. We emit `PolynomialIdentity( S(m) - S(m-1) - P(m) ≡ 0 )` and check the
//!      base `S(lo-1) = 0`. The identity is discharged by `jeff-verify` (exact
//!      coefficient-zero, F.1). If it does not verify, we **defer** (R31/R1).
//!
//! Nothing here trusts the construction: the polynomial identity is independently
//! checked, and a wrong closed form fails to verify (DR1/DR7).

use jeff_cert::{
    Boundary, Certificate, Collapsed, CollapseOutcome, Defer, Evidence, IrRef, Obligation,
};
use jeff_cert::BarrierTag;
use jeff_core_ir::{eval, BinOp, CoreDomain, CoreExpr, CoreExprKind, CoreFn, RedKind};
use jeff_math::{Poly, UniPoly};
use jeff_recognizer::{Collapser, DispatchTag, Recognized};
use num_bigint::BigInt;
use num_integer::Integer;
use num_rational::BigRational;
use num_traits::{One, ToPrimitive, Zero};
use std::collections::BTreeMap;

/// The Faulhaber-like power-sum collapser (PART 19.3).
pub struct Faulhaber;

/// Output of [`collapse`]: the gated [`CollapseOutcome`] (verified cert or defer)
/// plus the closed-form residual to lower/execute when collapsed. Keeping the
/// residual `CoreExpr` here (DR6) conveys the closed form the schema's `IrRef`
/// refers to, without a global node arena at this stage.
pub struct ArithCollapse {
    pub outcome: CollapseOutcome,
    pub residual: Option<CoreExpr>,
}

impl Collapser for Faulhaber {
    const ID: jeff_recognizer::CollapserId = "arith/faulhaber";
    fn entry(&self, r: &Recognized) -> bool {
        matches!(r.tag, DispatchTag::AffineTripCount)
    }
    fn try_collapse(&self, f: &CoreFn) -> CollapseOutcome {
        collapse(f).outcome
    }
}

const SOURCE_NODE: u32 = 1;
const COLLAPSED_NODE: u32 = 2;

/// Attempt the Faulhaber collapse of `f`, returning the outcome and (if collapsed)
/// the closed-form residual.
pub fn collapse(f: &CoreFn) -> ArithCollapse {
    let span = f.body.span;
    let src = IrRef::new(SOURCE_NODE, span);
    let defer = |tag: BarrierTag| ArithCollapse {
        outcome: CollapseOutcome::Defer(Defer::new(tag, src)),
        residual: None,
    };

    // Must be: Sum over a range, with a polynomial summand in the binder.
    let CoreExprKind::Reduction {
        kind: RedKind::Sum,
        binder,
        domain: CoreDomain::Range { lo, hi, inclusive },
        body,
    } = &f.body.kind
    else {
        return defer(BarrierTag::ConstantFactorOnly);
    };

    // Summand must be a polynomial in the binder alone.
    let Some(p) = to_unipoly(body, binder) else {
        // body depends on a parameter or is non-polynomial → not a Faulhaber sum
        return defer(BarrierTag::ConstantFactorOnly);
    };

    // lo must be a constant; hi must be a single parameter variable.
    let Some(lo_val) = const_int(lo) else {
        return defer(BarrierTag::ConstantFactorOnly);
    };
    let CoreExprKind::Var(param) = &hi.kind else {
        return defer(BarrierTag::ConstantFactorOnly);
    };

    let deg = p.degree().unwrap_or(0);
    let n_points = deg + 2; // closed form has degree deg+1
    let lo_minus_1 = &lo_val - BigInt::one();

    // Sample the naive sum at param = lo-1, lo, ... (includes the empty-sum base).
    let mut points: Vec<(BigRational, BigRational)> = Vec::with_capacity(n_points);
    for k in 0..n_points {
        let m = &lo_minus_1 + BigInt::from(k as u64);
        let mut env: BTreeMap<String, BigInt> = BTreeMap::new();
        env.insert(param.clone(), m.clone());
        // Evaluate Σ_{i=lo}^{m} P(i) via the original reduction (naive ground truth).
        let Ok(y) = eval(&f.body, &env) else {
            return defer(BarrierTag::ConstantFactorOnly);
        };
        points.push((BigRational::from(m), BigRational::from(y)));
    }
    let s = Poly_interpolate(&points);

    // Boundary: empty sum at param = lo-1 must be 0 (base case). Exact check.
    let base_x = BigRational::from(lo_minus_1.clone());
    if !s.eval(&base_x).is_zero() {
        return defer(BarrierTag::ConstantFactorOnly);
    }

    // Build the difference-identity certificate: S(param) - S(param-1) - P(param) ≡ 0.
    let s_mv = s.to_multivar(param);
    let s_shift_mv = s.shift(-1).to_multivar(param);
    let p_mv = p.to_multivar(param);
    let diff: Poly = s_mv.sub(&s_shift_mv).sub(&p_mv);

    let inclusive_note = if *inclusive { "inclusive" } else { "exclusive" };
    let cert = Certificate {
        collapser_id: Faulhaber::ID.into(),
        source: src,
        collapsed: IrRef::new(COLLAPSED_NODE, span),
        obligation: Obligation::new(format!(
            "forall {param}. S({param}) - S({param}-1) == summand({param}); \
             S({lo_minus_1})=0 (empty sum); range {lo_val}..{param} {inclusive_note}"
        )),
        evidence: Evidence::PolynomialIdentity { poly: diff },
        boundaries: vec![Boundary::new(format!("S({lo_minus_1}) = 0 (empty sum)"))],
        fallback: src,
    };

    match jeff_verify::verify(cert) {
        Some(vc) => {
            let residual = unipoly_to_core(&s, param, span);
            ArithCollapse {
                outcome: CollapseOutcome::Collapsed(Collapsed::new(
                    IrRef::new(COLLAPSED_NODE, span),
                    vc,
                )),
                residual: Some(residual),
            }
        }
        None => defer(BarrierTag::ConstantFactorOnly), // R31: not verified → fallback
    }
}

/// Lagrange interpolation wrapper (name kept explicit for readability).
#[allow(non_snake_case)]
fn Poly_interpolate(points: &[(BigRational, BigRational)]) -> UniPoly {
    UniPoly::interpolate(points)
}

/// Convert a core expression to an exact univariate polynomial in `binder`, or
/// `None` if it is not a polynomial in the binder alone (e.g. it mentions a
/// parameter, or uses a non-polynomial op).
pub fn to_unipoly(e: &CoreExpr, binder: &str) -> Option<UniPoly> {
    match &e.kind {
        CoreExprKind::Int(v) => Some(UniPoly::constant(BigRational::from(v.clone()))),
        CoreExprKind::Var(x) if x == binder => Some(UniPoly::x()),
        CoreExprKind::Var(_) => None,
        CoreExprKind::Neg(a) => Some(to_unipoly(a, binder)?.neg()),
        CoreExprKind::Bin(op, a, b) => {
            let la = to_unipoly(a, binder)?;
            match op {
                BinOp::Add => Some(la.add(&to_unipoly(b, binder)?)),
                BinOp::Sub => Some(la.sub(&to_unipoly(b, binder)?)),
                BinOp::Mul => Some(la.mul(&to_unipoly(b, binder)?)),
                BinOp::Pow => {
                    if let CoreExprKind::Int(e) = &b.kind {
                        let exp = e.to_u32()?;
                        Some(la.pow(exp))
                    } else {
                        None
                    }
                }
                _ => None,
            }
        }
        CoreExprKind::Reduction { .. } => None,
    }
}

fn const_int(e: &CoreExpr) -> Option<BigInt> {
    let env: BTreeMap<String, BigInt> = BTreeMap::new();
    eval(e, &env).ok()
}

/// Render a rational-coefficient polynomial `S(param)` as an exact integer core
/// expression `N(param) / D` where `D` clears all denominators. `N(param)/D` is an
/// exact integer for every integer `param` because `S` is integer-valued (it
/// interpolates integer sums).
fn unipoly_to_core(s: &UniPoly, param: &str, span: jeff_span::Span) -> CoreExpr {
    let mk = |k: CoreExprKind| CoreExpr { kind: k, span };
    let int = |v: BigInt| CoreExpr {
        kind: CoreExprKind::Int(v),
        span,
    };
    // common denominator D
    let mut d = BigInt::one();
    for c in &s.coeffs {
        d = d.lcm(c.denom());
    }
    // numerator: Σ_k (c_k * D) param^k  (integer coeffs)
    let mut terms: Vec<CoreExpr> = Vec::new();
    for (k, c) in s.coeffs.iter().enumerate() {
        // n_k = numer(c) * (D / denom(c))
        let nk = c.numer() * (&d / c.denom());
        if nk.is_zero() {
            continue;
        }
        let monomial = if k == 0 {
            int(nk)
        } else {
            let powk = if k == 1 {
                mk(CoreExprKind::Var(param.to_string()))
            } else {
                mk(CoreExprKind::Bin(
                    BinOp::Pow,
                    Box::new(mk(CoreExprKind::Var(param.to_string()))),
                    Box::new(int(BigInt::from(k as u64))),
                ))
            };
            if nk.is_one() {
                powk
            } else {
                mk(CoreExprKind::Bin(BinOp::Mul, Box::new(int(nk)), Box::new(powk)))
            }
        };
        terms.push(monomial);
    }
    let numerator = match terms.into_iter().reduce(|acc, t| {
        mk(CoreExprKind::Bin(BinOp::Add, Box::new(acc), Box::new(t)))
    }) {
        Some(e) => e,
        None => int(BigInt::zero()),
    };
    if d.is_one() {
        numerator
    } else {
        // exact integer division (numerator is divisible by D for integer param)
        mk(CoreExprKind::Bin(
            BinOp::Div,
            Box::new(numerator),
            Box::new(int(d)),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jeff_core_ir::lower;
    use jeff_syntax::parse;

    fn fn_of(src: &str) -> CoreFn {
        let p = parse(src, 0).unwrap();
        lower(&p).unwrap().funcs.into_iter().next().unwrap()
    }

    fn eval_residual(r: &CoreExpr, param: &str, n: i64) -> BigInt {
        let mut env = BTreeMap::new();
        env.insert(param.to_string(), BigInt::from(n));
        eval(r, &env).unwrap()
    }

    #[test]
    fn collapses_triangular_with_verified_certificate() {
        let f = fn_of("total fn s(n: nat) -> nat: sum i in 0..=n: i\n");
        let ac = collapse(&f);
        assert!(matches!(ac.outcome, CollapseOutcome::Collapsed(_)));
        let r = ac.residual.unwrap();
        // residual must equal the naive sum for many n (DR7).
        for n in 0..50 {
            let naive: i64 = (0..=n).sum();
            assert_eq!(eval_residual(&r, "n", n), BigInt::from(naive), "n={n}");
        }
    }

    #[test]
    fn collapses_sum_of_squares() {
        let f = fn_of("total fn s2(n: nat) -> nat: sum i in 0..=n: i*i\n");
        let ac = collapse(&f);
        assert!(matches!(ac.outcome, CollapseOutcome::Collapsed(_)));
        let r = ac.residual.unwrap();
        for n in 0..40 {
            let naive: i64 = (0..=n).map(|i| i * i).sum();
            assert_eq!(eval_residual(&r, "n", n), BigInt::from(naive), "n={n}");
        }
    }

    #[test]
    fn collapses_sum_of_cubes() {
        let f = fn_of("total fn s3(n: nat) -> nat: sum i in 0..=n: i**3\n");
        let ac = collapse(&f);
        assert!(matches!(ac.outcome, CollapseOutcome::Collapsed(_)));
        let r = ac.residual.unwrap();
        for n in 0..30 {
            let naive: i64 = (0..=n).map(|i| i * i * i).sum();
            assert_eq!(eval_residual(&r, "n", n), BigInt::from(naive), "n={n}");
        }
    }

    #[test]
    fn defers_when_summand_has_parameter() {
        // sum i in 0..=n: a*i  — body mentions parameter `a` (not a pure power sum).
        let f = fn_of("total fn s(n: nat, a: nat) -> nat: sum i in 0..=n: a*i\n");
        let ac = collapse(&f);
        assert!(matches!(ac.outcome, CollapseOutcome::Defer(_)));
        assert!(ac.residual.is_none());
    }
}
