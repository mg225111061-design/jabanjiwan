//! Layer-0 equality saturation via the `egg` e-graph library (CLAUDE.md D5, 10.1).
//!
//! This module canonicalises a reduction *summand* with **semantics-preserving**
//! rewrites, under a node/iter/time budget (R23), and returns a normalized
//! `CoreExpr`. The recognizer then classifies the normalized summand into a
//! [`crate::DispatchTag`].
//!
//! # Why this cannot break correctness (P0, invariant from the Stage-2 brief)
//!
//! The recognizer only chooses *dispatch*; it never approves a collapse. It emits a
//! tag; the collapser independently builds and **verifies** a certificate. So the
//! worst case of a buggy/aggressive rewrite is a wrong tag → wrong collapser tried →
//! the checker rejects → safe HONEST_DEFER. Structurally, `jeff-recognizer` does not
//! even depend on `jeff-verify`, so it *cannot* construct a `VerifiedCertificate`.
//!
//! Every rewrite below is annotated with why `⟦before⟧ = ⟦after⟧`. Rules that are
//! only conditionally valid (e.g. `x^0 = 1`, ambiguous at `0^0`) are deliberately
//! omitted.

use egg::{define_language, rewrite as rw, CostFunction, Extractor, Id, Language, RecExpr, Runner, Symbol};
use jeff_core_ir::{BinOp, CoreExpr, CoreExprKind};
use jeff_span::Span;
use num_traits::ToPrimitive;
use std::time::Duration;

use crate::SaturationBudget;

define_language! {
    /// The arithmetic-summand language. Closed under the semantics-preserving rules
    /// below; non-arithmetic summands are simply not translated (we fall back to the
    /// original expression, so classification still runs).
    enum Jeff {
        Num(i64),
        "+" = Add([Id; 2]),
        "-" = Sub([Id; 2]),
        "*" = Mul([Id; 2]),
        "neg" = Neg([Id; 1]),
        "pow" = Pow([Id; 2]),
        "C" = Binom([Id; 2]),
        "fact" = Fact([Id; 1]),
        Var(Symbol),
    }
}

/// Canonicalisation cost: like AST size, but `pow` is slightly more expensive than
/// `*`, so a squared term canonicalises to `a*a` rather than `pow a 2` — making the
/// extracted normal form deterministic (R11) and unifying the two spellings.
struct CanonCost;
impl CostFunction<Jeff> for CanonCost {
    type Cost = usize;
    fn cost<C: FnMut(Id) -> usize>(&mut self, enode: &Jeff, mut costs: C) -> usize {
        let base = match enode {
            Jeff::Pow(_) => 2,
            _ => 1,
        };
        enode.fold(base, |s, id| s + costs(id))
    }
}

fn rules() -> Vec<egg::Rewrite<Jeff, ()>> {
    vec![
        // a+b = b+a : addition is commutative.
        rw!("comm-add"; "(+ ?a ?b)" => "(+ ?b ?a)"),
        // a*b = b*a : multiplication is commutative.
        rw!("comm-mul"; "(* ?a ?b)" => "(* ?b ?a)"),
        // (a+b)+c = a+(b+c) : addition is associative.
        rw!("assoc-add"; "(+ (+ ?a ?b) ?c)" => "(+ ?a (+ ?b ?c))"),
        // (a*b)*c = a*(b*c) : multiplication is associative.
        rw!("assoc-mul"; "(* (* ?a ?b) ?c)" => "(* ?a (* ?b ?c))"),
        // a+0 = a : 0 is the additive identity.
        rw!("add-zero"; "(+ ?a 0)" => "?a"),
        // a*1 = a : 1 is the multiplicative identity.
        rw!("mul-one"; "(* ?a 1)" => "?a"),
        // a*0 = 0 : 0 annihilates under multiplication.
        rw!("mul-zero"; "(* ?a 0)" => "0"),
        // a-b = a+(-b) : definition of subtraction.
        rw!("sub-def"; "(- ?a ?b)" => "(+ ?a (neg ?b))"),
        // a^1 = a : first power is the identity (no 0^0 issue, exponent is 1).
        rw!("pow-one"; "(pow ?a 1)" => "?a"),
        // a^2 = a*a : true for all a (unifies `i*i` and `pow i 2`).
        rw!("sq-mul"; "(pow ?a 2)" => "(* ?a ?a)"),
        // a*a = a^2 : the converse, so both forms share an e-class.
        rw!("mul-sq"; "(* ?a ?a)" => "(pow ?a 2)"),
    ]
}

/// Translate a core summand to an egg `RecExpr`, or `None` if it contains a
/// construct outside the arithmetic-summand language (e.g. a nested reduction,
/// division, modulus) — the caller then keeps the original expression.
fn to_recexpr(e: &CoreExpr) -> Option<RecExpr<Jeff>> {
    let mut rec = RecExpr::default();
    add_node(e, &mut rec)?;
    Some(rec)
}

fn add_node(e: &CoreExpr, rec: &mut RecExpr<Jeff>) -> Option<Id> {
    let node = match &e.kind {
        CoreExprKind::Int(v) => Jeff::Num(v.to_i64()?),
        CoreExprKind::Var(x) => Jeff::Var(Symbol::from(x.as_str())),
        CoreExprKind::Neg(a) => {
            let ia = add_node(a, rec)?;
            Jeff::Neg([ia])
        }
        CoreExprKind::Bin(op, a, b) => {
            let ia = add_node(a, rec)?;
            let ib = add_node(b, rec)?;
            match op {
                BinOp::Add => Jeff::Add([ia, ib]),
                BinOp::Sub => Jeff::Sub([ia, ib]),
                BinOp::Mul => Jeff::Mul([ia, ib]),
                BinOp::Pow => Jeff::Pow([ia, ib]),
                _ => return None, // Div/FloorDiv/Rem/comparisons are not summand-poly
            }
        }
        CoreExprKind::Call(name, args) if name == "C" && args.len() == 2 => {
            let ia = add_node(&args[0], rec)?;
            let ib = add_node(&args[1], rec)?;
            Jeff::Binom([ia, ib])
        }
        CoreExprKind::Call(name, args) if name == "fact" && args.len() == 1 => {
            let ia = add_node(&args[0], rec)?;
            Jeff::Fact([ia])
        }
        _ => return None,
    };
    Some(rec.add(node))
}

/// Translate an extracted `RecExpr` back to a `CoreExpr` for classification.
fn from_recexpr(rec: &RecExpr<Jeff>, span: Span) -> CoreExpr {
    let nodes = rec.as_ref();
    build(nodes, nodes.len() - 1, span)
}

fn build(nodes: &[Jeff], idx: usize, span: Span) -> CoreExpr {
    let mk = |k: CoreExprKind| CoreExpr { kind: k, span };
    let child = |id: &Id| build(nodes, usize::from(*id), span);
    match &nodes[idx] {
        Jeff::Num(v) => mk(CoreExprKind::Int((*v).into())),
        Jeff::Var(s) => mk(CoreExprKind::Var(s.to_string())),
        Jeff::Neg([a]) => mk(CoreExprKind::Neg(Box::new(child(a)))),
        Jeff::Add([a, b]) => mk(CoreExprKind::Bin(BinOp::Add, Box::new(child(a)), Box::new(child(b)))),
        Jeff::Sub([a, b]) => mk(CoreExprKind::Bin(BinOp::Sub, Box::new(child(a)), Box::new(child(b)))),
        Jeff::Mul([a, b]) => mk(CoreExprKind::Bin(BinOp::Mul, Box::new(child(a)), Box::new(child(b)))),
        Jeff::Pow([a, b]) => mk(CoreExprKind::Bin(BinOp::Pow, Box::new(child(a)), Box::new(child(b)))),
        Jeff::Binom([a, b]) => mk(CoreExprKind::Call("C".into(), vec![child(a), child(b)])),
        Jeff::Fact([a]) => mk(CoreExprKind::Call("fact".into(), vec![child(a)])),
    }
}

/// Equality-saturate `summand` and return its lowest-`AstSize` canonical form. On
/// any failure (untranslatable, budget exhausted with no improvement) the original
/// summand is returned — so classification always has something sound to inspect.
/// Deterministic (R11): no randomness in saturation or extraction.
pub fn normalize_summand(summand: &CoreExpr, budget: SaturationBudget) -> CoreExpr {
    let Some(expr) = to_recexpr(summand) else {
        return summand.clone();
    };
    let runner = Runner::default()
        .with_node_limit(budget.max_nodes)
        .with_iter_limit(40)
        .with_time_limit(Duration::from_millis(budget.max_millis))
        .with_expr(&expr)
        .run(&rules());
    let root = runner.roots[0];
    let extractor = Extractor::new(&runner.egraph, CanonCost);
    let (_cost, best) = extractor.find_best(root);
    from_recexpr(&best, summand.span)
}

#[cfg(test)]
mod tests {
    use super::*;
    use jeff_core_ir::lower;
    use jeff_syntax::parse;

    fn summand_of(src: &str) -> CoreExpr {
        let p = parse(src, 0).unwrap();
        let f = lower(&p).unwrap().funcs.into_iter().next().unwrap();
        let CoreExprKind::Reduction { body, .. } = f.body.kind else {
            panic!("expected reduction")
        };
        *body
    }

    #[test]
    fn normalization_is_semantics_preserving_on_samples() {
        use jeff_core_ir::eval;
        use num_bigint::BigInt;
        use std::collections::BTreeMap;
        // For several summands, the normalized form must agree with the original at
        // many points (semantics preserved by the rewrites).
        let cases = [
            "total fn s(n: nat) -> nat: sum i in 0..=n: i*i*1 + 0",
            "total fn s(n: nat) -> nat: sum i in 0..=n: (i + 0) * i",
            "total fn s(n: nat) -> nat: sum i in 0..=n: i**2",
        ];
        for src in cases {
            let body = summand_of(src);
            let norm = normalize_summand(&body, SaturationBudget::default());
            for iv in -3..8i64 {
                let mut env = BTreeMap::new();
                env.insert("i".to_string(), BigInt::from(iv));
                assert_eq!(eval(&body, &env).ok(), eval(&norm, &env).ok(), "src={src} i={iv}");
            }
        }
    }

    #[test]
    fn nontranslatable_returns_original() {
        // i % 2 is not in the summand language → original returned unchanged.
        let body = summand_of("total fn s(n: nat) -> nat: sum i in 0..=n: i % 2");
        let norm = normalize_summand(&body, SaturationBudget::default());
        assert_eq!(norm.kind, body.kind);
    }

    /// Structural equality ignoring spans (the normalized form's spans point at the
    /// source summand, which differs between two spellings but is irrelevant here).
    fn shape_eq(a: &CoreExpr, b: &CoreExpr) -> bool {
        use CoreExprKind::*;
        match (&a.kind, &b.kind) {
            (Int(x), Int(y)) => x == y,
            (Var(x), Var(y)) => x == y,
            (Neg(x), Neg(y)) => shape_eq(x, y),
            (Bin(o1, a1, b1), Bin(o2, a2, b2)) => o1 == o2 && shape_eq(a1, a2) && shape_eq(b1, b2),
            (Call(n1, a1), Call(n2, a2)) => {
                n1 == n2 && a1.len() == a2.len() && a1.iter().zip(a2).all(|(x, y)| shape_eq(x, y))
            }
            _ => false,
        }
    }

    #[test]
    fn unifies_square_spellings() {
        // i*i and i**2 saturate into one e-class → same canonical extraction (both
        // become i*i; they would otherwise dispatch identically anyway).
        let a = normalize_summand(
            &summand_of("total fn s(n: nat) -> nat: sum i in 0..=n: i*i"),
            SaturationBudget::default(),
        );
        let b = normalize_summand(
            &summand_of("total fn s(n: nat) -> nat: sum i in 0..=n: i**2"),
            SaturationBudget::default(),
        );
        assert!(shape_eq(&a, &b), "a={:?} b={:?}", a.kind, b.kind);
    }

    #[test]
    fn simplifies_identities_to_the_variable() {
        // i*1 + 0  --(mul-one, add-zero)-->  i
        let s = normalize_summand(
            &summand_of("total fn s(n: nat) -> nat: sum i in 0..=n: i*1 + 0"),
            SaturationBudget::default(),
        );
        assert!(matches!(s.kind, CoreExprKind::Var(ref v) if v == "i"), "got {:?}", s.kind);
    }
}
