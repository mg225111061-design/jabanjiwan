//! Typed core IR, lowering from the surface AST, and an exact evaluator.
//!
//! Authority: CLAUDE.md PART 8 (IR stack), B.3 (skeleton). This is the Stage-0
//! subset (AR-5: only what the thin vertical slice needs): integer arithmetic,
//! variables, and single-binder reductions over ranges. Richer constructs (match,
//! affine-set domains, codata) are lowered in later stages and currently produce an
//! honest diagnostic rather than a silent partial (R24).
//!
//! The [`eval`] evaluator computes the *original* semantics exactly over `BigInt`
//! (R33). It is therefore both (a) the fallback executor (P0: a deferred region
//! runs its original loop) and (b) the naive-correct reference every collapse is
//! checked against (AR-4 / DR7).

use jeff_span::{DiagCode, Diagnostic, Span};
use num_bigint::BigInt;
use num_traits::{One, Zero};
use std::collections::BTreeMap;

pub mod lower;
pub use lower::lower;

pub type Var = String;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RedKind {
    Sum,
    Prod,
    Count,
    Fold,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    FloorDiv,
    Rem,
    Pow,
    // comparisons (yield 0/1), used by count predicates later
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CoreTy {
    Nat,
    Int,
    Rat,
    Bool,
    Unit,
    Other(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CoreExpr {
    pub kind: CoreExprKind,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CoreExprKind {
    Var(Var),
    Int(BigInt),
    Bin(BinOp, Box<CoreExpr>, Box<CoreExpr>),
    Neg(Box<CoreExpr>),
    /// A builtin call: `C(a,b)` (binomial) or `fact(x)` (factorial). These give the
    /// holonomic (Layer 1) recognizer/collapser concrete hypergeometric summands.
    Call(String, Vec<CoreExpr>),
    /// Single-binder reduction over a range (the first-class recognizer input).
    Reduction {
        kind: RedKind,
        binder: Var,
        domain: CoreDomain,
        body: Box<CoreExpr>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CoreDomain {
    Range {
        lo: Box<CoreExpr>,
        hi: Box<CoreExpr>,
        inclusive: bool,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CoreParam {
    pub name: Var,
    pub ty: CoreTy,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CoreFn {
    pub name: String,
    pub params: Vec<CoreParam>,
    pub ret: CoreTy,
    pub body: CoreExpr,
    pub total: bool,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CoreIr {
    pub funcs: Vec<CoreFn>,
}

impl CoreIr {
    pub fn func(&self, name: &str) -> Option<&CoreFn> {
        self.funcs.iter().find(|f| f.name == name)
    }
}

/// Evaluation errors are *internal* invariant issues (e.g. unbound var after a
/// successful lowering) or genuinely undefined arithmetic (div by zero). Returned
/// as `Result`, never panics (R38).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EvalError {
    Unbound(Var),
    DivByZero,
    NegativeExponent,
    /// A reduction range was unbounded/too large to evaluate concretely.
    RangeTooLarge,
    /// A call to an unknown/unsupported builtin.
    UnknownCall(String),
}

/// Binomial coefficient `C(n,k)` over the integers; `0` when `k<0` or `k>n` (for
/// `n>=0`). Exact (R33). Used by the naive oracle for holonomic sums (AR-4).
pub fn binomial(n: &BigInt, k: &BigInt) -> BigInt {
    use num_traits::Signed;
    if k.is_negative() {
        return BigInt::zero();
    }
    if n.is_negative() {
        // generalized binomial: C(n,k) = (-1)^k C(k-n-1, k); not needed for the
        // nonneg summation ranges here — return 0 conservatively.
        return BigInt::zero();
    }
    if k > n {
        return BigInt::zero();
    }
    // multiplicative: Π_{i=1}^{k} (n-k+i)/i, exact
    let mut num = BigInt::one();
    let mut den = BigInt::one();
    let mut i = BigInt::one();
    let kk = k.clone();
    while i <= kk {
        num *= n - &kk + &i;
        den *= &i;
        i += 1;
    }
    num / den
}

/// Factorial `x!` for `x >= 0`.
pub fn factorial(x: &BigInt) -> BigInt {
    use num_traits::Signed;
    if x.is_negative() {
        return BigInt::zero();
    }
    let mut acc = BigInt::one();
    let mut i = BigInt::one();
    while &i <= x {
        acc *= &i;
        i += 1;
    }
    acc
}

/// Exact evaluation of a core expression under an integer environment.
/// This *is* the original semantics (ground truth). Reductions iterate.
pub fn eval(e: &CoreExpr, env: &BTreeMap<Var, BigInt>) -> Result<BigInt, EvalError> {
    match &e.kind {
        CoreExprKind::Int(v) => Ok(v.clone()),
        CoreExprKind::Var(x) => env.get(x).cloned().ok_or_else(|| EvalError::Unbound(x.clone())),
        CoreExprKind::Neg(a) => Ok(-eval(a, env)?),
        CoreExprKind::Bin(op, a, b) => {
            let l = eval(a, env)?;
            let r = eval(b, env)?;
            eval_bin(*op, l, r)
        }
        CoreExprKind::Call(name, args) => {
            let vals: Result<Vec<BigInt>, EvalError> = args.iter().map(|a| eval(a, env)).collect();
            let vals = vals?;
            match (name.as_str(), vals.as_slice()) {
                ("C", [n, k]) => Ok(binomial(n, k)),
                ("fact", [x]) => Ok(factorial(x)),
                _ => Err(EvalError::UnknownCall(name.clone())),
            }
        }
        CoreExprKind::Reduction {
            kind,
            binder,
            domain,
            body,
        } => eval_reduction(*kind, binder, domain, body, env),
    }
}

fn eval_bin(op: BinOp, l: BigInt, r: BigInt) -> Result<BigInt, EvalError> {
    use BinOp::*;
    let b = |c: bool| if c { BigInt::one() } else { BigInt::zero() };
    Ok(match op {
        Add => l + r,
        Sub => l - r,
        Mul => l * r,
        Div | FloorDiv => {
            if r.is_zero() {
                return Err(EvalError::DivByZero);
            }
            // floor division (exact integers)
            div_floor(&l, &r)
        }
        Rem => {
            if r.is_zero() {
                return Err(EvalError::DivByZero);
            }
            let q = div_floor(&l, &r);
            l - q * &r
        }
        Pow => {
            let exp = r;
            if exp < BigInt::zero() {
                return Err(EvalError::NegativeExponent);
            }
            let mut acc = BigInt::one();
            let mut base = l;
            let mut e = exp;
            let two = BigInt::from(2);
            while e > BigInt::zero() {
                if (&e % &two) == BigInt::one() {
                    acc *= &base;
                }
                base = &base * &base;
                e /= &two;
            }
            acc
        }
        Eq => b(l == r),
        Ne => b(l != r),
        Lt => b(l < r),
        Le => b(l <= r),
        Gt => b(l > r),
        Ge => b(l >= r),
    })
}

fn div_floor(a: &BigInt, b: &BigInt) -> BigInt {
    let q = a / b;
    let r = a - &q * b;
    if (!r.is_zero()) && ((r < BigInt::zero()) != (b < &BigInt::zero())) {
        q - 1
    } else {
        q
    }
}

/// Safety cap so the evaluator (used in tests and fallback) cannot loop forever on
/// a pathological range (R23). Honest: beyond this we report, not hang.
const EVAL_RANGE_CAP: u64 = 50_000_000;

fn eval_reduction(
    kind: RedKind,
    binder: &str,
    domain: &CoreDomain,
    body: &CoreExpr,
    env: &BTreeMap<Var, BigInt>,
) -> Result<BigInt, EvalError> {
    let CoreDomain::Range { lo, hi, inclusive } = domain;
    let lo = eval(lo, env)?;
    let hi = eval(hi, env)?;
    let last = if *inclusive { hi.clone() } else { hi - 1 };
    // count of iterations guard
    let span = (&last - &lo) + 1;
    if span > BigInt::from(EVAL_RANGE_CAP) {
        return Err(EvalError::RangeTooLarge);
    }
    let mut acc = match kind {
        RedKind::Sum | RedKind::Count => BigInt::zero(),
        RedKind::Prod => BigInt::one(),
        RedKind::Fold => BigInt::zero(),
    };
    let mut i = lo;
    let mut local = env.clone();
    while i <= last {
        local.insert(binder.to_string(), i.clone());
        match kind {
            RedKind::Sum => acc += eval(body, &local)?,
            RedKind::Prod => acc *= eval(body, &local)?,
            RedKind::Count => acc += eval(body, &local)?, // body is 0/1 predicate
            RedKind::Fold => acc += eval(body, &local)?,
        }
        i += 1;
    }
    Ok(acc)
}

/// Helper for diagnostics from this crate.
pub(crate) fn err(span: Span, msg: impl Into<String>) -> Diagnostic {
    Diagnostic::error(DiagCode::TypeMismatch, span, msg)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn n_env(n: i64) -> BTreeMap<Var, BigInt> {
        let mut m = BTreeMap::new();
        m.insert("n".to_string(), BigInt::from(n));
        m
    }

    #[test]
    fn eval_sum_triangular() {
        // sum i in 0..=n: i
        let span = Span::dummy();
        let body = CoreExpr {
            kind: CoreExprKind::Var("i".into()),
            span,
        };
        let red = CoreExpr {
            kind: CoreExprKind::Reduction {
                kind: RedKind::Sum,
                binder: "i".into(),
                domain: CoreDomain::Range {
                    lo: Box::new(int(0)),
                    hi: Box::new(var("n")),
                    inclusive: true,
                },
                body: Box::new(body),
            },
            span,
        };
        assert_eq!(eval(&red, &n_env(10)).unwrap(), BigInt::from(55));
        assert_eq!(eval(&red, &n_env(0)).unwrap(), BigInt::from(0));
    }

    #[test]
    fn eval_sum_of_squares() {
        let span = Span::dummy();
        let body = CoreExpr {
            kind: CoreExprKind::Bin(BinOp::Mul, Box::new(var("i")), Box::new(var("i"))),
            span,
        };
        let red = CoreExpr {
            kind: CoreExprKind::Reduction {
                kind: RedKind::Sum,
                binder: "i".into(),
                domain: CoreDomain::Range {
                    lo: Box::new(int(0)),
                    hi: Box::new(var("n")),
                    inclusive: true,
                },
                body: Box::new(body),
            },
            span,
        };
        assert_eq!(eval(&red, &n_env(3)).unwrap(), BigInt::from(14)); // 0+1+4+9
    }

    fn int(v: i64) -> CoreExpr {
        CoreExpr {
            kind: CoreExprKind::Int(BigInt::from(v)),
            span: Span::dummy(),
        }
    }
    fn var(n: &str) -> CoreExpr {
        CoreExpr {
            kind: CoreExprKind::Var(n.into()),
            span: Span::dummy(),
        }
    }

    #[test]
    fn floor_division_semantics() {
        assert_eq!(div_floor(&BigInt::from(-7), &BigInt::from(2)), BigInt::from(-4));
        assert_eq!(div_floor(&BigInt::from(7), &BigInt::from(2)), BigInt::from(3));
    }
}
