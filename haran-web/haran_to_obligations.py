"""
STAGE H2 — HARAN spec → Mr.Jeffrey proof obligations.  THE HEART.
=================================================================
Before HARAN, Mr. guessed the spec from the function *name* (verify_strong.infer_properties) —
the weak link (§0 공리1: "의도가 코드 바깥에 있으면 검증기는 추측하고, 추측은 틀린다"). HARAN puts the
spec *in the code* as `ensures`, so this module READS it and turns it into Mr.'s proof obligations
(§2.1 단계2), then routes each to the right engine (§2.1 단계3):

    polynomial / closed-form identity   →  JEFF exact  (jeff_adapter → jeff_identity coeff-zero)
    transcendental / general algebraic  →  sympy CAS   (jeff_adapter tier 2)
    general proposition over a program  →  bounded fuzzing  [needs a HARAN evaluator — see scope]

H2 scope (honest): the **correctness** obligation (impl ⊨ ensures) is fully wired for the EXACT
arithmetic case — a `fold`/closed-form impl vs a `result = <expr>` spec — proven for ALL inputs by
JEFF (sympy performs the summation step; H4 upgrades that to a JEFF fold *certificate*). General
propositions (e.g. sorted∧permutation) generate an obligation but their discharge needs the HARAN
evaluator, which is NOT yet built — those return DEFER with that exact reason (no fake pass).
All six obligation KINDS (§2.1 단계2 ①–⑥) are GENERATED here; only correctness is discharged at H2,
the rest are tagged with the stage that owns them.
"""
from __future__ import annotations

import dataclasses
from dataclasses import dataclass
from typing import List, Optional

import sympy as sp

import haran_ast as A
import jeff_adapter   # real JEFF exact backend (Stage 3): prove_identity → jeff_identity binary


NUMERIC_TYPES = {"Nat", "Int", "Float", "nat", "int", "float", "Rat", "rat"}


@dataclass
class Obligation:
    kind: str                 # correctness | termination | contract | exhaustiveness | productivity | aliasing
    fn: str
    description: str
    status: str = "pending"   # pending | discharged | deferred
    verdict: str = ""         # PROVEN | REFUTED | DEFER   (when discharged)
    backend: str = ""         # jeff | sympy | none
    detail: str = ""
    counterexample: Optional[dict] = None
    stage: str = ""           # owning stage when deferred

    def __str__(self):
        head = f"[{self.kind}] {self.fn}"
        if self.status == "discharged":
            cx = f"  cx={self.counterexample}" if self.counterexample else ""
            return f"{head}: {self.verdict} via {self.backend} — {self.detail}{cx}"
        if self.status == "deferred":
            return f"{head}: DEFERRED → {self.stage} — {self.description}"
        return f"{head}: {self.description}"


class _NonArith(Exception):
    pass


# --------------------------------------------------------------------- AST helpers
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


def _is_recursive(fn: A.FnDecl) -> bool:
    return any(isinstance(x, A.Call) and isinstance(x.func, A.Var) and x.func.name == fn.name
               for x in _walk(fn.body)) if fn.body else False


def _has_match(fn: A.FnDecl) -> bool:
    return any(isinstance(x, A.Match) for x in _walk(fn.body)) if fn.body else False


def _has_owned_params(fn: A.FnDecl) -> bool:
    return any(isinstance(p.ty, (A.TyOwn, A.TyRef)) for p in fn.params)


def _numeric_params(fn: A.FnDecl) -> List[str]:
    return [p.name for p in fn.params if isinstance(p.ty, A.TyName) and p.ty.name in NUMERIC_TYPES]


def _block_return(b) -> Optional[object]:
    if isinstance(b, A.Block) and b.stmts and isinstance(b.stmts[-1], A.ExprStmt):
        return b.stmts[-1].value
    return None


def _fn_return(fn: A.FnDecl) -> Optional[object]:
    return _block_return(fn.body)


def _is_result(e) -> bool:
    return isinstance(e, A.Var) and e.name == "result"


def _to_arith(e) -> str:
    """HARAN arithmetic expr → a sympy-parseable string. Raises _NonArith on anything else."""
    if isinstance(e, A.Num):
        return e.value
    if isinstance(e, A.Var):
        return e.name
    if isinstance(e, A.Un) and e.op == "-":
        return f"(-{_to_arith(e.operand)})"
    if isinstance(e, A.Bin) and e.op in ("+", "-", "*", "/", "%", "**"):
        return f"({_to_arith(e.lhs)} {e.op} {_to_arith(e.rhs)})"
    raise _NonArith(type(e).__name__)


def _show(e) -> str:
    if isinstance(e, A.Bin):
        return f"({_show(e.lhs)} {e.op} {_show(e.rhs)})"
    if isinstance(e, A.Un):
        return f"({e.op}{_show(e.operand)})"
    if isinstance(e, A.Call):
        return f"{_show(e.func)}({', '.join(_show(a) for a in e.args)})"
    if isinstance(e, A.Var):
        return e.name
    if isinstance(e, A.Num):
        return e.value
    if isinstance(e, A.Quant):
        return f"{e.kind}{','.join(e.vars)}. {_show(e.body)}"
    if isinstance(e, A.Fold):
        return f"fold {e.binder} in {_show(e.domain)} {{...}}"
    if isinstance(e, A.Range):
        return f"{_show(e.lo)}..{_show(e.hi)}"
    return type(e).__name__ if e is not None else "∅"


# --------------------------------------------------------------------- obligation generation
def generate_obligations(fn: A.FnDecl) -> List[Obligation]:
    """Turn a HARAN function's spec into the §2.1-단계2 obligation list (all six kinds)."""
    obs: List[Obligation] = []
    # ② correctness — from ensures (the spec). Discharged at H2 (exact arithmetic) / later (general).
    if fn.ensures is not None:
        obs.append(Obligation("correctness", fn.name, f"impl ⊨ ensures: {_show(fn.ensures)}"))
    # ① termination — from decreases, or flagged for synthesis (H3).
    if fn.kind == "fn":
        if fn.decreases is not None:
            obs.append(Obligation("termination", fn.name,
                                  f"decreases {_show(fn.decreases)} strictly decreases each recursive call",
                                  status="deferred", stage="H3"))
        elif _is_recursive(fn):
            obs.append(Obligation("termination", fn.name,
                                  "recursive with no `decreases` — measure synthesis required",
                                  status="deferred", stage="H3"))
    # ③ contract — from requires.
    if fn.requires is not None:
        obs.append(Obligation("contract", fn.name, f"every call site satisfies requires: {_show(fn.requires)}",
                              status="deferred", stage="H2+"))
    # ④ exhaustiveness — if the body matches.
    if _has_match(fn):
        obs.append(Obligation("exhaustiveness", fn.name, "match arms cover all constructors",
                              status="deferred", stage="H5"))
    # ⑤ productivity — proc only.
    if fn.kind == "proc" and fn.produces is not None:
        obs.append(Obligation("productivity", fn.name, f"proc yields every step: {_show(fn.produces)}",
                              status="deferred", stage="H5"))
    # ⑥ aliasing — own/& params.
    if _has_owned_params(fn):
        obs.append(Obligation("aliasing", fn.name, "own/& ownership and borrows are sound",
                              status="deferred", stage="H5"))
    return obs


# --------------------------------------------------------------------- correctness discharge (the wired one)
def discharge_correctness(fn: A.FnDecl) -> Obligation:
    """Prove (or refute) that the implementation satisfies `ensures`, reading the spec from code.
    EXACT arithmetic route (fold / closed-form vs `result = <expr>`) → JEFF/sympy; else DEFER."""
    ob = Obligation("correctness", fn.name, f"impl ⊨ ensures: {_show(fn.ensures)}", status="discharged")
    ens = fn.ensures
    if ens is None:
        ob.verdict, ob.backend, ob.detail = "DEFER", "none", "no ensures clause"
        return ob

    # Form: result = <arithmetic RHS>   (closed-form / counting spec)
    if isinstance(ens, A.Bin) and ens.op in ("=", "==") and _is_result(ens.lhs):
        try:
            rhs_str = _to_arith(ens.rhs)
        except _NonArith as e:
            ob.verdict, ob.backend = "DEFER", "none"
            ob.detail = f"ensures RHS is not closed-form arithmetic ({e}); needs HARAN evaluator (later stage)"
            return ob
        impl = _implementation_closed_form(fn)
        if impl is None:
            ob.verdict, ob.backend = "DEFER", "none"
            ob.detail = "implementation is neither a fold nor a closed-form expr; needs HARAN evaluator (later stage)"
            return ob
        impl_str, how = impl
        variables = _numeric_params(fn)
        res = jeff_adapter.prove_identity(impl_str, rhs_str, variables)   # → REAL JEFF, then sympy
        if res.verdict == "PROVEN":
            ob.verdict, ob.backend = "PROVEN", res.backend
            ob.detail = f"{how} = {impl_str}  ≡  {rhs_str}  (proven ∀ {','.join(variables) or '∅'})"
        elif res.verdict == "REFUTED":
            ob.verdict, ob.backend = "REFUTED", res.backend
            ob.detail = f"{how} = {impl_str}  ≢  ensures {rhs_str}: {res.detail}"
            ob.counterexample = _int_counterexample(impl_str, rhs_str, variables)
        else:
            ob.verdict, ob.backend, ob.detail = "DEFER", res.backend, res.detail
        return ob

    # General proposition (sorted(result) ∧ permutation(...), etc.)
    ob.verdict, ob.backend = "DEFER", "none"
    ob.detail = ("ensures is a general proposition (not `result = arithmetic`); bounded-fuzz discharge "
                 "needs the HARAN evaluator — NOT built yet (H2 scope = exact arithmetic). Honest DEFER.")
    return ob


def _implementation_closed_form(fn: A.FnDecl):
    """Return (sympy_string, human_label) for the impl's value, or None if not exact-reducible."""
    ret = _fn_return(fn)
    if isinstance(ret, A.Fold):
        try:
            k = ret.binder
            lo = _to_arith(ret.domain.lo)
            hi = _to_arith(ret.domain.hi)
            body = _block_return(ret.body)
            body_str = _to_arith(body)
        except (_NonArith, AttributeError):
            return None
        ks = sp.Symbol(k)
        try:
            summed = sp.summation(sp.sympify(body_str, locals={k: ks}),
                                  (ks, sp.sympify(lo), sp.sympify(hi)))
        except Exception:
            return None
        return (str(sp.expand(summed)), f"Σ_{{{k}={lo}}}^{{{hi}}} ({body_str})")
    if ret is not None:
        try:
            return (_to_arith(ret), "impl")
        except _NonArith:
            return None
    return None


def _int_counterexample(impl_str: str, rhs_str: str, variables: List[str], lo=0, hi=40):
    """Find the smallest integer assignment where impl ≠ ensures — a concrete §2.2-style witness."""
    if not variables:
        variables = ["n"]
    syms = [sp.Symbol(v) for v in variables]
    ie, re = sp.sympify(impl_str), sp.sympify(rhs_str)
    for val in range(lo, hi + 1):
        subs = {s: val for s in syms}
        iv, rv = ie.subs(subs), re.subs(subs)
        if sp.simplify(iv - rv) != 0:
            return {"inputs": {str(s): val for s in syms},
                    "impl_value": str(iv), "spec_value": str(rv)}
    return None


# --------------------------------------------------------------------- top-level entry
def verify_fn(fn: A.FnDecl):
    """Generate all obligations and discharge the correctness one. Returns (correctness_ob, all_obs)."""
    all_obs = generate_obligations(fn)
    correctness = discharge_correctness(fn) if fn.ensures is not None else None
    # splice the discharged correctness back over the generated (pending) one
    out = []
    for o in all_obs:
        out.append(correctness if (correctness and o.kind == "correctness") else o)
    return correctness, out
