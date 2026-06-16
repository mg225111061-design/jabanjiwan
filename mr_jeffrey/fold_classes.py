"""
STAGE T1 (v5) — closure-class classifier extension: recognize hypergeometric & holonomic candidates.
====================================================================================================
v2's closure_classifier decides polynomial(Faulhaber) / C-finite / Gosper-hypergeometric / absence.
v5 adds an explicit MATH-CLASS layer with a certifiability grade, and recognizes:

  polynomial / rational   — Faulhaber                      decidable · certifiable: FULL (coeff-zero)
  C-finite                — constant-coeff linear recurrence decidable · certifiable: FULL (companion≡naive)
  hypergeometric          — summand ratio a_{k+1}/a_k ∈ ℚ(k) decidable (ratio test) · certifiable: PARTIAL
                            (WZ certificate, singularity provisos — T2)
  holonomic-candidate     — poly-coeff linear recurrence (P-recursive); explicit in code = given,
                            from samples = GUESSED.   decidable to DETECT · certificate: DEFERRED (T3, Gröbner)
  nonholonomic / data     — data-dependent (is_prime…) or no recurrence  → NO_STRUCTURE (Ω(N))

★ Honesty: holonomic detection from samples is a GUESS ("추측됨 — 확인 필요"), not a proof. The math
class is tagged with whether its closure is decidable and whether a machine certificate is available.
"""
from __future__ import annotations

import dataclasses
from dataclasses import dataclass
from typing import Optional

import haran_ast as A
import closure_classifier as cc


@dataclass
class MathClass:
    name: str                 # polynomial | C-finite | hypergeometric | holonomic-candidate | nonholonomic | unknown
    decidable: str            # "decidable" | "guessed" | "no"
    certifiable: str          # FULL | PARTIAL(provisos) | DEFERRED(Gröbner) | ABSENCE | NONE
    route: str                # which stage/engine handles it
    note: str = ""

    def __str__(self):
        return f"{self.name} [{self.decidable}, cert={self.certifiable}] → {self.route}"


# ---------- sympy bridge that also understands binomial / factorial (for hypergeometric tests) ----------
class _DataDependent(Exception):
    pass


def _to_sympy_hyper(e, binder: str):
    import sympy as sp
    if isinstance(e, A.Num):
        return sp.Float(e.value) if e.is_float else sp.Integer(int(e.value))
    if isinstance(e, A.Var):
        return sp.Symbol(e.name)
    if isinstance(e, A.Un) and e.op == "-":
        return -_to_sympy_hyper(e.operand, binder)
    if isinstance(e, A.Bin):
        l = _to_sympy_hyper(e.lhs, binder); r = _to_sympy_hyper(e.rhs, binder)
        return {"+": l + r, "-": l - r, "*": l * r, "/": l / r, "**": l ** r}.get(e.op) \
            or _raise(_DataDependent(f"op {e.op}"))
    if isinstance(e, A.Call) and isinstance(e.func, A.Var):
        nm = e.func.name
        args = [_to_sympy_hyper(a, binder) for a in e.args]
        if nm in ("C", "binom", "binomial", "choose") and len(args) == 2:
            return sp.binomial(args[0], args[1])
        if nm in ("fact", "factorial") and len(args) == 1:
            return sp.factorial(args[0])
        raise _DataDependent(f"call {nm}")
    raise _DataDependent(type(e).__name__)


def _raise(x):
    raise x


def is_hypergeometric(summand, binder: str) -> bool:
    """Ratio test: a_{k+1}/a_k ∈ ℚ(k). Decidable. Handles binomial/factorial summands."""
    import sympy as sp
    try:
        expr = _to_sympy_hyper(summand, binder)
    except _DataDependent:
        return False
    k = sp.Symbol(binder)
    try:
        return sp.hypersimp(expr, k) is not None
    except Exception:
        return False


# ---------- recurrence kind: constant-coeff (C-finite) vs polynomial-coeff (holonomic) ----------
def _involves(e, name: str) -> bool:
    return any(isinstance(x, A.Var) and x.name == name for x in _walk(e))


def _walk(node):
    yield node
    if dataclasses.is_dataclass(node) and not isinstance(node, A.Span):
        for f in dataclasses.fields(node):
            v = getattr(node, f.name)
            if isinstance(v, list):
                for x in v:
                    if dataclasses.is_dataclass(x):
                        yield from _walk(x)
            elif dataclasses.is_dataclass(v):
                yield from _walk(v)


def _flatten_plus(e):
    if isinstance(e, A.Bin) and e.op == "+":
        return _flatten_plus(e.lhs) + _flatten_plus(e.rhs)
    return [e]


def detect_recurrence_kind(fn: A.FnDecl) -> Optional[str]:
    """Return 'c-finite' (constant coeffs), 'holonomic' (poly-in-n coeffs), or None."""
    ret = cc._block_return(fn.body) if fn.body else None
    if not isinstance(ret, A.Match):
        return None
    nparam = fn.params[0].name if fn.params else "n"
    rec_expr = None
    for arm in ret.arms:
        if isinstance(arm.pattern, (A.PWild, A.PVar)):
            rec_expr = cc._block_return(arm.body) if isinstance(arm.body, A.Block) else arm.body
    if rec_expr is None:
        return None
    has_self = any(isinstance(x, A.Call) and isinstance(x.func, A.Var) and x.func.name == fn.name
                   for x in _walk(rec_expr))
    if not has_self:
        return None
    # for each additive term containing a self-call, the COEFFICIENT is the rest of the product.
    poly_coeff = False
    found = False
    for term in _flatten_plus(rec_expr):
        calls = [x for x in _walk(term) if isinstance(x, A.Call) and isinstance(x.func, A.Var) and x.func.name == fn.name]
        if not calls:
            continue
        found = True
        # coefficient factors = the multiplicands that are NOT the self-call
        if isinstance(term, A.Bin) and term.op == "*":
            for side in (term.lhs, term.rhs):
                if not (isinstance(side, A.Call) and isinstance(side.func, A.Var) and side.func.name == fn.name):
                    if _involves(side, nparam):
                        poly_coeff = True
    if not found:
        return None
    return "holonomic" if poly_coeff else "c-finite"


# ---------- the extended classifier ----------
def classify_math_class(fn: A.FnDecl) -> MathClass:
    # 1. recurrence (C-finite vs holonomic)
    kind = detect_recurrence_kind(fn)
    if kind == "c-finite":
        return MathClass("C-finite", "decidable", "FULL", "cfinite O(log n)", "constant-coeff linear recurrence")
    if kind == "holonomic":
        return MathClass("holonomic-candidate", "decidable", "DEFERRED(Gröbner)", "holonomic → T3",
                         "polynomial-coefficient linear recurrence (P-recursive)")
    # 2. fold / sum
    ret = cc._block_return(fn.body) if fn.body else None
    if isinstance(ret, A.Fold):
        body = cc._block_return(ret.body)
        # polynomial?
        try:
            from fold_collapse import body_to_coeffs, NonPoly
            body_to_coeffs(body, ret.binder)
            return MathClass("polynomial", "decidable", "FULL", "Faulhaber O(1)", "polynomial summand")
        except Exception:
            pass
        # hypergeometric? (ratio test, incl. binomial/factorial)
        if is_hypergeometric(body, ret.binder):
            return MathClass("hypergeometric", "decidable", "PARTIAL(provisos)", "Gosper/Zeilberger → T2",
                             "summand ratio a_{k+1}/a_k ∈ ℚ(k)")
        # data-dependent / nonholonomic
        if any(isinstance(x, A.Call) for x in _walk(body)):
            return MathClass("nonholonomic/data", "no", "NONE", "NO_STRUCTURE (Ω(N))",
                             "data-dependent summand")
    # 3. fall back to the v2 4-bucket verdict for anything else
    v = cc.classify_fn(fn)
    if v.kind == "ABSENT":
        return MathClass("hypergeometric", "decidable", "ABSENCE", "ABSENT (proven)", v.proof)
    if v.kind == "NO_STRUCTURE":
        return MathClass("nonholonomic/data", "no", "NONE", "NO_STRUCTURE (Ω(N))", v.proof)
    return MathClass("unknown", "no", "NONE", "UNKNOWN", v.proof)
