//! Layer 3b — exact lattice-point counting for affine parametric polytopes
//! (CLAUDE.md 10.3, APPENDIX F.4).
//!
//! Back to the **exact** regime: the certificate is exact (no tolerance). The
//! counting engine (`jeff_math::lattice`) is clean-room — it links no GPL
//! `barvinok`/`LattE`/`PPL`, and not even isl (MIT but a C dependency). The
//! collapser here adds the two essential Barvinok boundaries:
//!   * **non-affine domain** (e.g. `i²+j² ≤ n`) → `HONEST_DEFER[non-affine-domain]`;
//!   * **high / variable dimension** → `HONEST_DEFER[sharp-P-hard]` (Barvinok is
//!     polynomial-time only in *fixed* dimension; variable-dimension counting is
//!     #P-hard).
//!
//! Honesty (R30): the closed form is produced by Ehrhart interpolation, not by
//! Barvinok's signed-cone decomposition (the fixed-dim polynomial-time algorithm);
//! the result and the F.4 certificate are exact either way.

use jeff_cert::{BarrierTag, Boundary, Certificate, Collapsed, CollapseOutcome, Defer, Evidence, IrRef, Obligation};
use jeff_math::lattice::{ehrhart_interpolate, Congr, ConstraintSystem, LinIneq, QuasiPoly, Rel};
use jeff_span::Span;
use jeff_syntax::ast;

/// A lattice-count collapse attempt.
#[derive(Debug)]
pub struct BarvinokOutcome {
    pub outcome: CollapseOutcome,
    pub qp: Option<QuasiPoly>,
}

/// Fixed-dimension budget. Barvinok is polynomial-time only for fixed dimension; at
/// or below this we collapse, beyond it we treat the input as the variable/high-dim
/// (#P-hard) regime and defer (DR8 conservative).
pub const FIXED_DIM_BUDGET: usize = 6;

fn node() -> IrRef {
    IrRef::new(1, Span::dummy())
}

fn defer(tag: BarrierTag) -> BarvinokOutcome {
    BarvinokOutcome {
        outcome: CollapseOutcome::Defer(Defer::new(tag, node())),
        qp: None,
    }
}

/// Collapse a `count` over an affine parametric polytope to its Ehrhart quasi-
/// polynomial, with an exact F.4 certificate.
pub fn collapse_count(cs: &ConstraintSystem) -> BarvinokOutcome {
    // dimension boundary (sharp-P-hard for variable / high dimension).
    if cs.n_vars > FIXED_DIM_BUDGET {
        return defer(BarrierTag::SharpPHard);
    }
    let Some(qp) = ehrhart_interpolate(cs) else {
        // unbounded polytope or box over the cap — cannot count exactly here.
        return defer(BarrierTag::ConstantFactorOnly);
    };
    // F.4 verification window: well beyond the interpolation samples.
    let deg = cs.n_vars as i64;
    let period = qp.period;
    let n_lo = 0;
    let n_hi = period * (deg + 1) + 16;
    let cert = Certificate {
        collapser_id: "barvinok/ehrhart".into(),
        source: node(),
        collapsed: IrRef::new(2, Span::dummy()),
        obligation: Obligation::new(format!(
            "lattice-point count = Ehrhart quasi-polynomial (period {period}, degree ≤ {deg}); \
             exact F.4: residue partition + brute-force enumeration match on [{n_lo},{n_hi}]"
        )),
        evidence: Evidence::LatticeCount {
            cs: cs.clone(),
            qp: qp.clone(),
            n_lo,
            n_hi,
        },
        boundaries: vec![Boundary::new(format!("fixed dimension d={} ≤ budget {FIXED_DIM_BUDGET}", cs.n_vars))],
        fallback: node(),
    };
    match jeff_verify::verify(cert) {
        Some(vc) => BarvinokOutcome {
            outcome: CollapseOutcome::Collapsed(Collapsed::new(IrRef::new(2, Span::dummy()), vc)),
            qp: Some(qp),
        },
        None => defer(BarrierTag::ConstantFactorOnly),
    }
}

/// Result of bridging a surface `count` set-domain to a [`ConstraintSystem`].
#[derive(Debug)]
pub enum Bridge {
    Affine(ConstraintSystem),
    /// A constraint is not affine in the bound variables (e.g. `i*i`, `i**2`).
    NonAffine,
    /// A construct outside the supported subset.
    Unsupported(String),
}

/// Bridge an AST set-domain to a `ConstraintSystem`. `binders` are the bound
/// variables (in order); `param` is the single free parameter (e.g. `n`). Detects
/// non-affine constraints and congruences (`x % m == r`).
pub fn constraints_from_ast(
    binders: &[String],
    param: &str,
    constraints: &[ast::Constraint],
) -> Bridge {
    let mut ineqs = Vec::new();
    let mut congrs = Vec::new();
    for con in constraints {
        // congruence form: `var % m == r`
        if con.parts.len() == 1 {
            let (op, rhs) = &con.parts[0];
            if *op == ast::CmpOp::Eq {
                if let Some(cg) = try_congruence(&con.head, rhs, binders) {
                    congrs.push(cg);
                    continue;
                }
            }
        }
        // otherwise: a chain `head op1 e1 op2 e2 ...` → pairwise comparisons.
        let mut prev = &con.head;
        for (op, rhs) in &con.parts {
            match comparison_to_ineq(prev, *op, rhs, binders, param) {
                Some(Ok(ineq)) => ineqs.push(ineq),
                Some(Err(())) => return Bridge::NonAffine,
                None => return Bridge::Unsupported("unsupported comparison".into()),
            }
            prev = rhs;
        }
    }
    Bridge::Affine(ConstraintSystem {
        n_vars: binders.len(),
        ineqs,
        congrs,
    })
}

fn try_congruence(head: &ast::Expr, rhs: &ast::Expr, binders: &[String]) -> Option<Congr> {
    // head = `var % m`, rhs = integer r
    let ast::ExprKind::Bin(ast::BinOp::Rem, a, b) = &head.kind else {
        return None;
    };
    let ast::ExprKind::Var(v) = &a.kind else { return None };
    let var = binders.iter().position(|x| x == v)?;
    let m = int_lit(b)?;
    let r = int_lit(rhs)?;
    Some(Congr {
        var,
        modulus: m,
        residue: r,
    })
}

/// `lhs op rhs` → a `≥ 0` (or `Eq`) [`LinIneq`]. `Ok` affine, `Err` non-affine,
/// `None` unsupported. Strict `<`/`>` are tightened by 1 (integers).
#[allow(clippy::result_unit_err)]
fn comparison_to_ineq(
    lhs: &ast::Expr,
    op: ast::CmpOp,
    rhs: &ast::Expr,
    binders: &[String],
    param: &str,
) -> Option<Result<LinIneq, ()>> {
    let la = match affine(lhs, binders, param) {
        Some(a) => a,
        None => return Some(Err(())), // non-affine
    };
    let ra = match affine(rhs, binders, param) {
        Some(a) => a,
        None => return Some(Err(())),
    };
    // diff = lhs - rhs
    let diff = sub_aff(&la, &ra, binders.len());
    let neg = |a: &Aff| Aff {
        var: a.var.iter().map(|x| -x).collect(),
        param: -a.param,
        c: -a.c,
    };
    let ineq = |a: Aff, rel: Rel| LinIneq {
        var: a.var,
        param: a.param,
        c: a.c,
        rel,
    };
    Some(Ok(match op {
        // lhs >= rhs  → diff >= 0
        ast::CmpOp::Ge => ineq(diff, Rel::Ge),
        // lhs > rhs   → diff - 1 >= 0
        ast::CmpOp::Gt => {
            let mut d = diff;
            d.c -= 1;
            ineq(d, Rel::Ge)
        }
        // lhs <= rhs  → rhs - lhs >= 0
        ast::CmpOp::Le => ineq(neg(&diff), Rel::Ge),
        // lhs < rhs   → rhs - lhs - 1 >= 0
        ast::CmpOp::Lt => {
            let mut d = neg(&diff);
            d.c -= 1;
            ineq(d, Rel::Ge)
        }
        ast::CmpOp::Eq => ineq(diff, Rel::Eq),
        ast::CmpOp::Ne => return None, // != not supported as a polytope facet
    }))
}

struct Aff {
    var: Vec<i64>,
    param: i64,
    c: i64,
}

fn sub_aff(a: &Aff, b: &Aff, nv: usize) -> Aff {
    Aff {
        var: (0..nv).map(|i| a.var[i] - b.var[i]).collect(),
        param: a.param - b.param,
        c: a.c - b.c,
    }
}

fn int_lit(e: &ast::Expr) -> Option<i64> {
    match &e.kind {
        ast::ExprKind::Lit(ast::Lit::Int(s, _)) => s.parse().ok(),
        ast::ExprKind::Un(ast::UnOp::Neg, inner) => int_lit(inner).map(|v| -v),
        ast::ExprKind::Paren(inner) => int_lit(inner),
        _ => None,
    }
}

/// Affine form of an expression in `binders` + `param`; `None` if non-linear.
fn affine(e: &ast::Expr, binders: &[String], param: &str) -> Option<Aff> {
    let zero = || Aff {
        var: vec![0; binders.len()],
        param: 0,
        c: 0,
    };
    match &e.kind {
        ast::ExprKind::Lit(ast::Lit::Int(s, _)) => {
            let mut a = zero();
            a.c = s.parse().ok()?;
            Some(a)
        }
        ast::ExprKind::Var(v) => {
            let mut a = zero();
            if let Some(i) = binders.iter().position(|x| x == v) {
                a.var[i] = 1;
            } else if v == param {
                a.param = 1;
            } else {
                return None; // unknown symbol
            }
            Some(a)
        }
        ast::ExprKind::Paren(inner) => affine(inner, binders, param),
        ast::ExprKind::Un(ast::UnOp::Neg, inner) => {
            let a = affine(inner, binders, param)?;
            Some(Aff {
                var: a.var.iter().map(|x| -x).collect(),
                param: -a.param,
                c: -a.c,
            })
        }
        ast::ExprKind::Bin(op, x, y) => match op {
            ast::BinOp::Add => add_aff(affine(x, binders, param)?, affine(y, binders, param)?, binders.len()),
            ast::BinOp::Sub => {
                let b = affine(y, binders, param)?;
                add_aff(
                    affine(x, binders, param)?,
                    Aff { var: b.var.iter().map(|v| -v).collect(), param: -b.param, c: -b.c },
                    binders.len(),
                )
            }
            ast::BinOp::Mul => {
                // affine only if one side is a constant
                let a = affine(x, binders, param)?;
                let b = affine(y, binders, param)?;
                let a_const = a.var.iter().all(|&v| v == 0) && a.param == 0;
                let b_const = b.var.iter().all(|&v| v == 0) && b.param == 0;
                if a_const {
                    Some(scale_aff(&b, a.c))
                } else if b_const {
                    Some(scale_aff(&a, b.c))
                } else {
                    None // product of two non-constants → non-affine
                }
            }
            _ => None, // Pow / Div / etc. → non-affine
        },
        _ => None,
    }
}

fn add_aff(a: Aff, b: Aff, nv: usize) -> Option<Aff> {
    Some(Aff {
        var: (0..nv).map(|i| a.var[i] + b.var[i]).collect(),
        param: a.param + b.param,
        c: a.c + b.c,
    })
}
fn scale_aff(a: &Aff, k: i64) -> Aff {
    Aff {
        var: a.var.iter().map(|v| v * k).collect(),
        param: a.param * k,
        c: a.c * k,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jeff_syntax::parse;
    use num_bigint::BigInt;

    /// Parse `count (binders) in {set}: 1` and return its binders + constraints + param.
    fn parse_count(src: &str) -> (Vec<String>, String, Vec<ast::Constraint>) {
        let p = parse(src, 0).unwrap();
        let ast::Item::Fn(f) = &p.items[0] else { panic!() };
        let param = f.params[0].name.clone();
        let ast::StmtKind::Expr(e) = &f.body.stmts[0].kind else { panic!() };
        let ast::ExprKind::Reduction { binder, domain, .. } = &e.kind else { panic!() };
        let binders: Vec<String> = binder
            .iter()
            .map(|p| match &p.kind {
                ast::PatternKind::Var(v) => v.clone(),
                _ => panic!("non-var binder"),
            })
            .collect();
        let ast::Domain::Set(cs) = domain else { panic!("not a set domain") };
        (binders, param, cs.clone())
    }

    fn collapse_src(src: &str) -> BarvinokOutcome {
        let (binders, param, cons) = parse_count(src);
        match constraints_from_ast(&binders, &param, &cons) {
            Bridge::Affine(cs) => collapse_count(&cs),
            Bridge::NonAffine => defer(BarrierTag::NonAffineDomain),
            Bridge::Unsupported(m) => panic!("unsupported: {m}"),
        }
    }

    #[test]
    fn collapses_triangle_count() {
        let out = collapse_src("total fn c(n: nat) -> nat: count (i, j) in {0 <= i <= j <= n}: 1\n");
        assert!(matches!(out.outcome, CollapseOutcome::Collapsed(_)));
        let qp = out.qp.unwrap();
        // (n+1)(n+2)/2
        for n in 0..20 {
            assert_eq!(qp.eval(n), BigInt::from((n + 1) * (n + 2) / 2));
        }
    }

    #[test]
    fn collapses_3d_simplex() {
        let out = collapse_src("total fn c(n: nat) -> nat: count (i, j, k) in {0 <= i <= j <= k <= n}: 1\n");
        assert!(matches!(out.outcome, CollapseOutcome::Collapsed(_)));
        let qp = out.qp.unwrap();
        for n in 0..12 {
            // (n+1)(n+2)(n+3)/6
            assert_eq!(qp.eval(n), BigInt::from((n + 1) * (n + 2) * (n + 3) / 6));
        }
    }

    #[test]
    fn non_affine_domain_defers() {
        // i*i + j*j <= n is non-affine → defer[non-affine-domain] (not a wrong count).
        let out = collapse_src("total fn c(n: nat) -> nat: count (i, j) in {i*i + j*j <= n}: 1\n");
        assert!(matches!(out.outcome, CollapseOutcome::Defer(_)));
        if let CollapseOutcome::Defer(d) = out.outcome {
            assert_eq!(d.tag, BarrierTag::NonAffineDomain);
        }
    }

    #[test]
    fn high_dimension_defers_sharp_p_hard() {
        // dimension above the fixed-dim budget → sharp-P-hard.
        let cs = ConstraintSystem {
            n_vars: FIXED_DIM_BUDGET + 1,
            ineqs: vec![],
            congrs: vec![],
        };
        let out = collapse_count(&cs);
        assert!(matches!(out.outcome, CollapseOutcome::Defer(_)));
        if let CollapseOutcome::Defer(d) = out.outcome {
            assert_eq!(d.tag, BarrierTag::SharpPHard);
        }
    }
}
