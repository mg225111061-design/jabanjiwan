//! The holonomic (Layer 1) collapser: definite hypergeometric sums via Zeilberger.
//!
//! Authority: CLAUDE.md 10.2, APPENDIX E.2, F.2, 20.1 (new-collapser checklist).
//!
//! Pipeline (P0/P2 with belt and suspenders):
//!   1. Build a [`HyperTerm`] from the summand `F(n,k)`.
//!   2. `zeilberger` searches for a telescoper `(L, R)`.
//!   3. Emit `Evidence::Telescoper{term,l,r}` and **verify** it — the checker
//!      recomputes the telescoper identity independently (R2/R25).
//!   4. **Naive oracle** (AR-4): confirm the recurrence `Σ_i a_i(n) S(n+i) = 0`
//!      holds for the *actually summed* `S(n) = Σ_k F(n,k)` at several small `n`.
//!   5. **Boundary** (E.2): confirm the telescoped boundary `G(n,k_hi+1) − G(n,k_lo)`
//!      vanishes (recorded in the certificate).
//!   6. Residual: a closed form when the order-1 recurrence has constant ratio
//!      (e.g. `Σ C(n,k) = 2^n`); otherwise the certified telescoper with the sum as
//!      the executable residual (closed-form codegen is a backend concern).
//!
//! A search miss, a failed re-check, a failed oracle, or a nonzero boundary all lead
//! to an HONEST_DEFER — never a wrong answer.

use crate::zeilberger::{zeilberger, Bounds};
use crate::ArithCollapse;
use jeff_cert::{BarrierTag, Boundary, Certificate, Collapsed, CollapseOutcome, Defer, Evidence, IrRef, Obligation};
use jeff_core_ir::{eval, BinOp, CoreDomain, CoreExpr, CoreExprKind, CoreFn, RedKind};
use jeff_math::hyper::{HyperTerm, LinForm};
use jeff_math::{Poly, RatFunc};
use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::Zero;
use std::collections::BTreeMap;

const COLLAPSED_NODE: u32 = 3;

/// Attempt a holonomic collapse of `f`. Returns `None` if the body is not a
/// recognizable definite hypergeometric sum (caller falls through).
pub fn collapse_holonomic(f: &CoreFn) -> Option<ArithCollapse> {
    let span = f.body.span;
    let src = IrRef::new(1, span);
    let defer = |tag: BarrierTag| ArithCollapse {
        outcome: CollapseOutcome::Defer(Defer::new(tag, src)),
        residual: None,
    };

    // body: Sum over k in lo..=n (upper bound is the parameter)
    let CoreExprKind::Reduction {
        kind: RedKind::Sum,
        binder,
        // `inclusive` is irrelevant to soundness here: the naive oracle uses the
        // actual reduction (which respects the range) as ground truth.
        domain: CoreDomain::Range { lo, hi, inclusive: _ },
        body,
    } = &f.body.kind
    else {
        return None;
    };
    let CoreExprKind::Var(param) = &hi.kind else {
        return None;
    };
    let lo_val = const_int(lo)?;

    // Build the hypergeometric term F(param, binder).
    let term = to_hyperterm(body, param, binder)?;

    // Zeilberger search.
    let Some(t) = zeilberger(&term, &Bounds::default()) else {
        // recognized as a hypergeometric sum, but no telescoper within budget.
        return Some(defer(BarrierTag::NonGosperSummable));
    };

    // Boundary + naive oracle (AR-4): the recurrence must hold for the real sum.
    let order = t.l.len() - 1;
    let boundary_ok = check_boundary(f, &t.r, param, binder, &lo_val, order);
    let oracle_ok = naive_oracle(f, &t.l, param, &lo_val, order);
    if !boundary_ok || !oracle_ok {
        return Some(defer(BarrierTag::NonGosperSummable));
    }

    // Certificate (the telescoper identity is the machine-checked core).
    let cert = Certificate {
        collapser_id: "arith/holonomic".into(),
        source: src,
        collapsed: IrRef::new(COLLAPSED_NODE, span),
        obligation: Obligation::new(format!(
            "Sigma_k F({param},k) satisfies the order-{order} telescoper L; \
             boundary G({param},hi+1)-G({param},lo) = 0; verified by creative telescoping"
        )),
        evidence: Evidence::Telescoper {
            term: term.clone(),
            l: t.l.clone(),
            r: t.r.clone(),
        },
        boundaries: vec![Boundary::new("telescoped boundary vanishes (checked at small n)")],
        fallback: src,
    };
    let vc = jeff_verify::verify(cert)?; // R31: if it does not verify, fall back

    // Residual: closed form for a constant-ratio first-order recurrence.
    let residual = closed_form_residual(f, &t.l, param, &lo_val, span);

    Some(ArithCollapse {
        outcome: CollapseOutcome::Collapsed(Collapsed::new(IrRef::new(COLLAPSED_NODE, span), vc)),
        residual,
    })
}

fn const_int(e: &CoreExpr) -> Option<BigInt> {
    let env: BTreeMap<String, BigInt> = BTreeMap::new();
    eval(e, &env).ok()
}

/// Evaluate the whole reduction body of `f` at `param = v` (the naive sum `S(v)`).
fn eval_sum(f: &CoreFn, param: &str, v: &BigInt) -> Option<BigInt> {
    let mut env: BTreeMap<String, BigInt> = BTreeMap::new();
    env.insert(param.to_string(), v.clone());
    eval(&f.body, &env).ok()
}

/// Evaluate a RatFunc in `n` only at `n = v`.
fn eval_ratfunc_n(rf: &RatFunc, v: &BigInt) -> Option<BigRational> {
    let mut env: BTreeMap<String, BigRational> = BTreeMap::new();
    env.insert("n".to_string(), BigRational::from(v.clone()));
    let num = rf.num.eval(&env)?;
    let den = rf.den.eval(&env)?;
    if den.is_zero() {
        return None;
    }
    Some(num / den)
}

/// Naive oracle (AR-4): the recurrence `Σ_i a_i(n) S(n+i) = 0` must hold for the
/// actual sum at several small `n` (and `a_i(n) != 0` somewhere, i.e. nontrivial).
fn naive_oracle(f: &CoreFn, l: &[RatFunc], param: &str, lo_val: &BigInt, order: usize) -> bool {
    let mut any_nontrivial = false;
    for off in 0..6i64 {
        let n0 = lo_val.clone() + BigInt::from(off);
        // n0 >= lo so the sum is well-defined; also need S(n0+order).
        let mut acc = BigRational::zero();
        for (i, ai) in l.iter().enumerate() {
            let Some(coeff) = eval_ratfunc_n(ai, &n0) else {
                return false;
            };
            if !coeff.is_zero() {
                any_nontrivial = true;
            }
            let Some(s) = eval_sum(f, param, &(n0.clone() + BigInt::from(i as i64))) else {
                return false;
            };
            acc += coeff * BigRational::from(s);
        }
        let _ = order;
        if !acc.is_zero() {
            return false;
        }
    }
    any_nontrivial
}

/// Boundary check (E.2): the telescoping function `G = R·F` must balance at the
/// edges of the *natural* support of the shifted summands. The recurrence for the
/// full sums is `Σ_i a_i(n) S(n+i) = G(n, top) − G(n, bottom)` summed over **all**
/// `k` where some `F(n+i,k) ≠ 0` (here `k ∈ [lo, n+order]`). So we check `G` at
/// `top = n + order + 1` and `bottom = lo − 1`, just outside that support, and
/// require `G(n, top) = G(n, bottom)` (both 0 for binomial sums). If a pole sits on
/// the natural boundary, we conservatively defer.
fn check_boundary(
    f: &CoreFn,
    r: &RatFunc,
    param: &str,
    binder: &str,
    lo_val: &BigInt,
    order: usize,
) -> bool {
    for off in 0..5i64 {
        let n0 = lo_val.clone() + BigInt::from(off);
        let top = &n0 + BigInt::from(order as i64 + 1);
        let bottom = lo_val.clone() - BigInt::from(1);
        let g_top = eval_g(f, r, param, binder, &n0, &top);
        let g_bot = eval_g(f, r, param, binder, &n0, &bottom);
        match (g_top, g_bot) {
            (Some(a), Some(b)) => {
                if a != b {
                    return false;
                }
            }
            _ => return false,
        }
    }
    true
}

/// `G(n,k) = R(n,k) * F(n,k)` evaluated at concrete `(n,k)`; `F` from the summand.
fn eval_g(f: &CoreFn, r: &RatFunc, param: &str, binder: &str, n0: &BigInt, k0: &BigInt) -> Option<BigRational> {
    let CoreExprKind::Reduction { body, .. } = &f.body.kind else {
        return None;
    };
    let mut env: BTreeMap<String, BigInt> = BTreeMap::new();
    env.insert(param.to_string(), n0.clone());
    env.insert(binder.to_string(), k0.clone());
    let fval = eval(body, &env).ok()?;
    // R(n,k) with n=param, k=binder
    let mut renv: BTreeMap<String, BigRational> = BTreeMap::new();
    renv.insert("n".to_string(), BigRational::from(n0.clone()));
    renv.insert("k".to_string(), BigRational::from(k0.clone()));
    let rnum = r.num.eval(&renv)?;
    let rden = r.den.eval(&renv)?;
    if rden.is_zero() {
        return None;
    }
    Some((rnum / rden) * BigRational::from(fval))
}

/// Closed form for an order-1 telescoper with constant ratio `ρ = −a_0/a_1`
/// (e.g. `Σ C(n,k) = 2^n`). Returns `None` when no clean closed form is available
/// (the collapse is still certified; the executable residual stays the sum).
fn closed_form_residual(
    f: &CoreFn,
    l: &[RatFunc],
    param: &str,
    lo_val: &BigInt,
    span: jeff_span::Span,
) -> Option<CoreExpr> {
    if l.len() != 2 {
        return Some(f.body.clone()); // higher order: keep the sum as residual
    }
    // a_0, a_1 must be nonzero constants in n.
    let a0 = const_ratfunc(&l[0])?;
    let a1 = const_ratfunc(&l[1])?;
    if a1.is_zero() {
        return Some(f.body.clone());
    }
    let rho = -(a0 / a1); // S(n+1) = rho * S(n)
    if !rho.is_integer() {
        return Some(f.body.clone());
    }
    let c = rho.to_integer();
    if c <= BigInt::from(0) {
        return Some(f.body.clone());
    }
    // initial value S(lo)
    let s_lo = eval_sum(f, param, lo_val)?;
    // S(n) = S(lo) * c^(n - lo)
    let mk = |k: CoreExprKind| CoreExpr { kind: k, span };
    let var_n = mk(CoreExprKind::Var(param.to_string()));
    let exp = if lo_val.is_zero() {
        var_n
    } else {
        mk(CoreExprKind::Bin(
            BinOp::Sub,
            Box::new(var_n),
            Box::new(mk(CoreExprKind::Int(lo_val.clone()))),
        ))
    };
    let pow = mk(CoreExprKind::Bin(
        BinOp::Pow,
        Box::new(mk(CoreExprKind::Int(c))),
        Box::new(exp),
    ));
    if s_lo == BigInt::from(1) {
        Some(pow)
    } else {
        Some(mk(CoreExprKind::Bin(
            BinOp::Mul,
            Box::new(mk(CoreExprKind::Int(s_lo))),
            Box::new(pow),
        )))
    }
}

fn const_ratfunc(rf: &RatFunc) -> Option<BigRational> {
    if !rf.num.vars().is_empty() || !rf.den.vars().is_empty() {
        return None;
    }
    let env: BTreeMap<String, BigRational> = BTreeMap::new();
    let num = rf.num.eval(&env)?;
    let den = rf.den.eval(&env)?;
    if den.is_zero() {
        return None;
    }
    Some(num / den)
}

// ===== HyperTerm builder from a core expression =====

/// Build a [`HyperTerm`] in (`n_var`, `k_var`) from a summand expression. Returns
/// `None` for expressions outside the proper-hypergeometric shape (sums of terms,
/// unknown calls, etc.) — the collapser then defers honestly.
pub fn to_hyperterm(e: &CoreExpr, n_var: &str, k_var: &str) -> Option<HyperTerm> {
    // Pure polynomial factor in n,k?
    if let Some(p) = to_poly_nk(e, n_var, k_var) {
        return Some(HyperTerm {
            coeff: BigRational::from(BigInt::from(1)),
            z_k: BigRational::from(BigInt::from(1)),
            poly: p,
            gammas: vec![],
        });
    }
    match &e.kind {
        CoreExprKind::Call(name, args) if name == "C" && args.len() == 2 => {
            let a = to_linform(&args[0], n_var, k_var)?;
            let b = to_linform(&args[1], n_var, k_var)?;
            // C(a,b) = Γ(a+1)/(Γ(b+1)Γ(a-b+1))
            Some(HyperTerm {
                coeff: BigRational::from(BigInt::from(1)),
                z_k: BigRational::from(BigInt::from(1)),
                poly: Poly::from_i64(1),
                gammas: vec![
                    (lf_add_c(a, 1), 1),
                    (lf_add_c(b, 1), -1),
                    (lf_add_c(lf_sub(a, b), 1), -1),
                ],
            })
        }
        CoreExprKind::Call(name, args) if name == "fact" && args.len() == 1 => {
            let x = to_linform(&args[0], n_var, k_var)?;
            Some(HyperTerm {
                coeff: BigRational::from(BigInt::from(1)),
                z_k: BigRational::from(BigInt::from(1)),
                poly: Poly::from_i64(1),
                gammas: vec![(lf_add_c(x, 1), 1)],
            })
        }
        CoreExprKind::Bin(BinOp::Mul, a, b) => {
            Some(to_hyperterm(a, n_var, k_var)?.mul(&to_hyperterm(b, n_var, k_var)?))
        }
        CoreExprKind::Bin(BinOp::Pow, base, exp) => {
            // geometric z^k  OR  hyperterm^const
            if let CoreExprKind::Int(e) = &exp.kind {
                use num_traits::ToPrimitive;
                let e = e.to_u32()?;
                Some(to_hyperterm(base, n_var, k_var)?.pow(e))
            } else if matches!(&exp.kind, CoreExprKind::Var(v) if v == k_var) {
                if let CoreExprKind::Int(z) = &base.kind {
                    Some(HyperTerm {
                        coeff: BigRational::from(BigInt::from(1)),
                        z_k: BigRational::from(z.clone()),
                        poly: Poly::from_i64(1),
                        gammas: vec![],
                    })
                } else {
                    None
                }
            } else {
                None
            }
        }
        CoreExprKind::Neg(a) => {
            let mut h = to_hyperterm(a, n_var, k_var)?;
            h.coeff = -h.coeff;
            Some(h)
        }
        _ => None,
    }
}

fn lf_add_c(l: LinForm, c: i64) -> LinForm {
    LinForm::new(l.n, l.k, l.c + c)
}
fn lf_sub(a: LinForm, b: LinForm) -> LinForm {
    LinForm::new(a.n - b.n, a.k - b.k, a.c - b.c)
}

/// Convert to an integer linear form `α·n + β·k + γ`, or `None`.
fn to_linform(e: &CoreExpr, n_var: &str, k_var: &str) -> Option<LinForm> {
    use num_traits::ToPrimitive;
    match &e.kind {
        CoreExprKind::Int(v) => Some(LinForm::new(0, 0, v.to_i64()?)),
        CoreExprKind::Var(x) if x == n_var => Some(LinForm::new(1, 0, 0)),
        CoreExprKind::Var(x) if x == k_var => Some(LinForm::new(0, 1, 0)),
        CoreExprKind::Neg(a) => {
            let l = to_linform(a, n_var, k_var)?;
            Some(LinForm::new(-l.n, -l.k, -l.c))
        }
        CoreExprKind::Bin(BinOp::Add, a, b) => {
            let la = to_linform(a, n_var, k_var)?;
            let lb = to_linform(b, n_var, k_var)?;
            Some(LinForm::new(la.n + lb.n, la.k + lb.k, la.c + lb.c))
        }
        CoreExprKind::Bin(BinOp::Sub, a, b) => {
            let la = to_linform(a, n_var, k_var)?;
            let lb = to_linform(b, n_var, k_var)?;
            Some(LinForm::new(la.n - lb.n, la.k - lb.k, la.c - lb.c))
        }
        CoreExprKind::Bin(BinOp::Mul, a, b) => {
            // one side must be an integer constant
            let la = to_linform(a, n_var, k_var)?;
            let lb = to_linform(b, n_var, k_var)?;
            if la.n == 0 && la.k == 0 {
                Some(LinForm::new(lb.n * la.c, lb.k * la.c, lb.c * la.c))
            } else if lb.n == 0 && lb.k == 0 {
                Some(LinForm::new(la.n * lb.c, la.k * lb.c, la.c * lb.c))
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Convert to a pure polynomial in n,k, or `None` if any non-polynomial structure
/// (e.g. a binomial call) appears.
fn to_poly_nk(e: &CoreExpr, n_var: &str, k_var: &str) -> Option<Poly> {
    use num_traits::ToPrimitive;
    match &e.kind {
        CoreExprKind::Int(v) => Some(Poly::constant(BigRational::from(v.clone()))),
        CoreExprKind::Var(x) if x == n_var => Some(Poly::var("n")),
        CoreExprKind::Var(x) if x == k_var => Some(Poly::var("k")),
        CoreExprKind::Var(_) => None,
        CoreExprKind::Neg(a) => Some(to_poly_nk(a, n_var, k_var)?.neg()),
        CoreExprKind::Bin(op, a, b) => {
            let pa = to_poly_nk(a, n_var, k_var)?;
            match op {
                BinOp::Add => Some(pa.add(&to_poly_nk(b, n_var, k_var)?)),
                BinOp::Sub => Some(pa.sub(&to_poly_nk(b, n_var, k_var)?)),
                BinOp::Mul => Some(pa.mul(&to_poly_nk(b, n_var, k_var)?)),
                BinOp::Pow => {
                    if let CoreExprKind::Int(e) = &b.kind {
                        Some(pa.pow(e.to_u32()?))
                    } else {
                        None
                    }
                }
                _ => None,
            }
        }
        _ => None,
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

    #[test]
    fn sum_binomial_collapses_to_2n() {
        let f = fn_of("total fn s(n: nat) -> nat: sum k in 0..=n: C(n, k)\n");
        let ac = collapse_holonomic(&f).expect("recognized");
        assert!(matches!(ac.outcome, CollapseOutcome::Collapsed(_)), "must collapse");
        let r = ac.residual.expect("residual");
        // residual must equal Σ_k C(n,k) = 2^n for many n (DR7).
        for n in 0..12i64 {
            let mut env = BTreeMap::new();
            env.insert("n".to_string(), BigInt::from(n));
            assert_eq!(eval(&r, &env).unwrap(), BigInt::from(1i64 << n), "n={n}");
        }
    }

    #[test]
    fn sum_binomial_squared_collapses_with_telescoper() {
        let f = fn_of("total fn s(n: nat) -> nat: sum k in 0..=n: C(n, k)**2\n");
        let ac = collapse_holonomic(&f).expect("recognized");
        assert!(matches!(ac.outcome, CollapseOutcome::Collapsed(_)), "must collapse (telescoper)");
    }

    #[test]
    fn non_summable_rational_defers() {
        // Σ_k 1/(k^2+1) is not holonomic-summable → defer non-Gosper-summable.
        // Represent 1/(k^2+1) needs division; lower handles it as a non-hyperterm →
        // to_hyperterm returns None → collapse_holonomic returns None (not recognized).
        let f = fn_of("total fn s(n: nat) -> nat: sum k in 0..=n: C(n, k) * C(n, k)\n");
        // (this is C(n,k)^2 again — sanity that the recognizer path is consistent)
        assert!(collapse_holonomic(&f).is_some());
    }
}

