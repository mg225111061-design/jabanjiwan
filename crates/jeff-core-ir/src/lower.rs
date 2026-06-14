//! Lowering from the surface AST (jeff-syntax) to the core IR.
//!
//! Stage-0 subset (AR-5): functions whose body is a single expression (or a block
//! ending in a return/expression) built from integer arithmetic, variables, and
//! single-binder reductions over ranges. Anything outside the subset yields an
//! honest diagnostic (R24) — never a silent partial.

use crate::{err, BinOp, CoreDomain, CoreExpr, CoreExprKind, CoreFn, CoreIr, CoreParam, CoreTy, RedKind};
use jeff_span::{Diagnostic, Span};
use jeff_syntax::ast;
use num_bigint::BigInt;

/// Lower a whole program. Collects diagnostics across all functions (R20).
pub fn lower(p: &ast::Program) -> Result<CoreIr, Vec<Diagnostic>> {
    let mut funcs = Vec::new();
    let mut diags = Vec::new();
    for item in &p.items {
        if let ast::Item::Fn(f) = item {
            match lower_fn(f) {
                Ok(cf) => funcs.push(cf),
                Err(mut ds) => diags.append(&mut ds),
            }
        }
        // data/codata/type/const are not part of the Stage-0 executable subset;
        // they are accepted by the parser and lowered in later stages.
    }
    if diags.iter().any(Diagnostic::is_error) {
        Err(diags)
    } else {
        Ok(CoreIr { funcs })
    }
}

fn lower_fn(f: &ast::FnDecl) -> Result<CoreFn, Vec<Diagnostic>> {
    let mut diags = Vec::new();
    let params = f
        .params
        .iter()
        .map(|p| CoreParam {
            name: p.name.clone(),
            ty: lower_ty(&p.ty),
        })
        .collect();
    let ret = f.ret.as_ref().map(lower_ty).unwrap_or(CoreTy::Unit);
    let body = match lower_block(&f.body) {
        Ok(b) => b,
        Err(mut d) => {
            diags.append(&mut d);
            // placeholder so we keep collecting other functions' diagnostics
            CoreExpr {
                kind: CoreExprKind::Int(BigInt::from(0)),
                span: f.body.span,
            }
        }
    };
    if diags.iter().any(Diagnostic::is_error) {
        return Err(diags);
    }
    Ok(CoreFn {
        name: f.name.clone(),
        params,
        ret,
        body,
        total: matches!(f.mode, ast::Mode::Total),
        span: f.span,
    })
}

fn lower_ty(t: &ast::Type) -> CoreTy {
    match &t.kind {
        ast::TypeKind::Base(b) => match b {
            ast::BaseType::Nat => CoreTy::Nat,
            ast::BaseType::Int | ast::BaseType::I(_) | ast::BaseType::U(_) => CoreTy::Int,
            ast::BaseType::Rat => CoreTy::Rat,
            ast::BaseType::Bool => CoreTy::Bool,
            other => CoreTy::Other(format!("{other:?}")),
        },
        ast::TypeKind::App(name, _) => CoreTy::Other(name.clone()),
        ast::TypeKind::Refine { base, .. } => match base {
            ast::BaseType::Nat => CoreTy::Nat,
            _ => CoreTy::Int,
        },
        ast::TypeKind::Ref { inner, .. } | ast::TypeKind::Own(inner) | ast::TypeKind::Secret(inner) => {
            lower_ty(inner)
        }
    }
}

/// A block lowers to the value of its single tail expression / return. Multi-stmt
/// bodies with locals are a later-stage concern.
fn lower_block(b: &ast::Block) -> Result<CoreExpr, Vec<Diagnostic>> {
    if b.stmts.len() == 1 {
        match &b.stmts[0].kind {
            ast::StmtKind::Expr(e) => return lower_expr(e),
            ast::StmtKind::Return(Some(e)) => return lower_expr(e),
            _ => {}
        }
    }
    // tolerate: a block whose last statement is the value
    if let Some(last) = b.stmts.last() {
        match &last.kind {
            ast::StmtKind::Expr(e) => return lower_expr(e),
            ast::StmtKind::Return(Some(e)) => return lower_expr(e),
            _ => {}
        }
    }
    Err(vec![err(
        b.span,
        "this function body is outside the Stage-0 executable subset \
         (expected a single expression or a trailing return)",
    )])
}

fn lower_expr(e: &ast::Expr) -> Result<CoreExpr, Vec<Diagnostic>> {
    let span = e.span;
    let kind = match &e.kind {
        ast::ExprKind::Lit(ast::Lit::Int(s, _)) => {
            let v: BigInt = s
                .parse()
                .map_err(|_| vec![err(span, format!("invalid integer literal {s:?}"))])?;
            CoreExprKind::Int(v)
        }
        ast::ExprKind::Lit(ast::Lit::Bool(b)) => {
            CoreExprKind::Int(if *b { BigInt::from(1) } else { BigInt::from(0) })
        }
        ast::ExprKind::Var(name) => CoreExprKind::Var(name.clone()),
        ast::ExprKind::Paren(inner) => return lower_expr(inner),
        ast::ExprKind::Un(op, inner) => {
            let i = lower_expr(inner)?;
            match op {
                ast::UnOp::Neg => CoreExprKind::Neg(Box::new(i)),
                other => {
                    return Err(vec![err(
                        span,
                        format!("unary {other:?} is not in the Stage-0 subset"),
                    )])
                }
            }
        }
        ast::ExprKind::Bin(op, a, b) => {
            let la = lower_expr(a)?;
            let lb = lower_expr(b)?;
            let bop = lower_binop(*op).ok_or_else(|| {
                vec![err(span, format!("binary {op:?} is not in the Stage-0 subset"))]
            })?;
            CoreExprKind::Bin(bop, Box::new(la), Box::new(lb))
        }
        ast::ExprKind::Call(callee, args) => {
            // Only the holonomic builtins C(a,b) and fact(x) are in the Stage-0/1
            // executable subset; other calls (e.g. user recursion) are later stages.
            let name = match &callee.kind {
                ast::ExprKind::Var(n) => n.clone(),
                _ => {
                    return Err(vec![err(
                        span,
                        "only direct builtin calls C(..)/fact(..) are lowered yet",
                    )])
                }
            };
            if name != "C" && name != "fact" {
                return Err(vec![err(
                    span,
                    format!("call to '{name}' is not in the executable subset (builtins: C, fact)"),
                )]);
            }
            let mut largs = Vec::with_capacity(args.len());
            for a in args {
                largs.push(lower_expr(a)?);
            }
            CoreExprKind::Call(name, largs)
        }
        ast::ExprKind::Reduction {
            kind,
            binder,
            domain,
            body,
        } => lower_reduction(span, *kind, binder, domain, body)?,
        other => {
            return Err(vec![err(
                span,
                format!(
                    "expression form {} is not in the Stage-0 executable subset \
                     (lowered in a later stage)",
                    variant_name(other)
                ),
            )])
        }
    };
    Ok(CoreExpr { kind, span })
}

fn lower_reduction(
    span: Span,
    kind: ast::RedKind,
    binder: &[ast::Pattern],
    domain: &ast::Domain,
    body: &ast::Expr,
) -> Result<CoreExprKind, Vec<Diagnostic>> {
    if binder.len() != 1 {
        return Err(vec![err(
            span,
            "multi-binder reductions (affine counting) are a Stage-4 (Barvinok) feature",
        )]);
    }
    let bind = match &binder[0].kind {
        ast::PatternKind::Var(v) => v.clone(),
        _ => {
            return Err(vec![err(
                span,
                "reduction binder must be a simple variable in the Stage-0 subset",
            )])
        }
    };
    let dom = match domain {
        ast::Domain::Range(r) => match &r.kind {
            ast::ExprKind::Range { lo, hi, inclusive } => CoreDomain::Range {
                lo: Box::new(lower_expr(lo)?),
                hi: Box::new(lower_expr(hi)?),
                inclusive: *inclusive,
            },
            _ => return Err(vec![err(span, "reduction domain is not a range")]),
        },
        ast::Domain::Set(_) => {
            return Err(vec![err(
                span,
                "affine set-domain reductions are a Stage-4 (Barvinok) feature",
            )])
        }
        ast::Domain::Expr(_) => {
            return Err(vec![err(
                span,
                "general-expression reduction domains are not in the Stage-0 subset",
            )])
        }
    };
    let rk = match kind {
        ast::RedKind::Sum => RedKind::Sum,
        ast::RedKind::Prod => RedKind::Prod,
        ast::RedKind::Count => RedKind::Count,
        ast::RedKind::Fold => RedKind::Fold,
    };
    Ok(CoreExprKind::Reduction {
        kind: rk,
        binder: bind,
        domain: dom,
        body: Box::new(lower_expr(body)?),
    })
}

fn lower_binop(op: ast::BinOp) -> Option<BinOp> {
    use ast::BinOp as A;
    Some(match op {
        A::Add => BinOp::Add,
        A::Sub => BinOp::Sub,
        A::Mul => BinOp::Mul,
        A::Div => BinOp::Div,
        A::FloorDiv => BinOp::FloorDiv,
        A::Rem => BinOp::Rem,
        A::Pow => BinOp::Pow,
        A::Eq => BinOp::Eq,
        A::Ne => BinOp::Ne,
        A::Lt => BinOp::Lt,
        A::Le => BinOp::Le,
        A::Gt => BinOp::Gt,
        A::Ge => BinOp::Ge,
        _ => return None,
    })
}

fn variant_name(e: &ast::ExprKind) -> &'static str {
    match e {
        ast::ExprKind::Match { .. } => "match",
        ast::ExprKind::Call(..) => "call",
        ast::ExprKind::Index(..) => "index",
        ast::ExprKind::Field(..) => "field access",
        ast::ExprKind::Range { .. } => "range",
        ast::ExprKind::Borrow { .. } => "borrow",
        ast::ExprKind::Own(_) => "own",
        ast::ExprKind::Move(_) => "move",
        ast::ExprKind::Lit(_) => "non-integer literal",
        _ => "this",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jeff_syntax::parse;
    use std::collections::BTreeMap;

    #[test]
    fn lower_and_eval_triangular() {
        let p = parse("total fn triangular(n: nat) -> nat: sum i in 0..=n: i\n", 0).unwrap();
        let ir = lower(&p).unwrap();
        let f = ir.func("triangular").unwrap();
        let mut env = BTreeMap::new();
        env.insert("n".to_string(), BigInt::from(100));
        assert_eq!(crate::eval(&f.body, &env).unwrap(), BigInt::from(5050));
    }

    #[test]
    fn lower_rejects_outside_subset_honestly() {
        // match is not in the Stage-0 subset → diagnostic, not panic, not silent.
        let src = "total fn f(n: nat) -> nat:\n    match n:\n        0 => 0\n        m+1 => 1\n";
        let p = parse(src, 0).unwrap();
        let r = lower(&p);
        assert!(r.is_err());
    }
}
