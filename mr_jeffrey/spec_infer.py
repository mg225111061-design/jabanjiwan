"""
STAGE V1.3 — candidate-spec inference (proposal, never confirmation).
====================================================================
If a function has no `ensures`, propose candidate specs from its body — but mark them clearly as
INFERRED (needs confirmation). Explicit `ensures` always wins (callers skip inference when present).

Two honest sources of candidates:
  • fold of a polynomial summand  → `result = <closed form>`  (inferred-EXACT: itself JEFF-verifiable);
  • list→list functions           → test structural hypotheses (permutation / length-preserving /
                                     sortedness) on random inputs; keep those that hold → inferred-TESTED(N).

A tested candidate is evidence, not proof (Boundary 1) — labeled "inferred-tested(N)".
"""
from __future__ import annotations

from dataclasses import dataclass
from typing import List

import haran_ast as A
import haran_eval
import fold_collapse

_S = A.Span(0, 0)


@dataclass
class InferredSpec:
    text: str
    confidence: str        # inferred-exact | inferred-tested(N)
    evidence: str


def _returns_list(fn: A.FnDecl) -> bool:
    return isinstance(fn.ret, A.TyName) and fn.ret.name in ("List", "Vec", "Seq")


def _first_list_param(fn: A.FnDecl):
    for p in fn.params:
        if isinstance(p.ty, A.TyName) and p.ty.name in ("List", "Vec", "Seq"):
            return p.name
    return None


def _call(name, args):
    return A.Call(A.Var(name, _S), args, _S)


def _var(n):
    return A.Var(n, _S)


def infer(fn: A.FnDecl, ftab: dict, n: int = 100) -> List[InferredSpec]:
    if fn.kind != "fn":
        return []
    out: List[InferredSpec] = []

    # (1) fold of a polynomial → closed-form candidate (itself exactly verifiable)
    fc = fold_collapse.collapse_fn_fold(fn)
    if fc.verdict == "COLLAPSED":
        out.append(InferredSpec(f"result = {fc.cert.closed_form}",
                                "inferred-exact",
                                f"fold collapses; {fc.cert.jeff_proof}"))

    # (2) list→list structural hypotheses, validated by bounded testing
    if _returns_list(fn) and _first_list_param(fn):
        p = _first_list_param(fn)
        hyps = [
            (f"permutation(result, {p})", _call("permutation", [_var("result"), _var(p)])),
            (f"length(result) = length({p})",
             A.Bin("=", _call("length", [_var("result")]), _call("length", [_var(p)]), _S)),
            ("sorted(result)", _call("sorted", [_var("result")])),
        ]
        for text, hyp in hyps:
            status, _, _ = haran_eval.bounded_fuzz(fn, ftab, hyp, n=n)
            if status == "PASS":
                out.append(InferredSpec(text, f"inferred-tested({n})", f"held on {n} random inputs"))
    return out


def propose_ensures(fn: A.FnDecl, ftab: dict) -> str:
    """A single human-readable proposed `ensures`, conjoining tested structural candidates."""
    cands = infer(fn, ftab)
    tested = [c.text for c in cands if c.confidence.startswith("inferred-tested")]
    exact = [c.text for c in cands if c.confidence == "inferred-exact"]
    if exact:
        return f"ensures {exact[0]}   // inferred-exact (needs confirmation)"
    if tested:
        return "ensures " + " ∧ ".join(tested) + "   // inferred-tested (needs confirmation)"
    return "(no candidate spec inferable)"
