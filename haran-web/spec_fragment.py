"""
STAGE V1 — spec-fragment classification + structuring coverage.
==============================================================
Structuring (명세①) can be forced on almost all code — but a spec is only *useful* if we are honest
about HOW MUCH of it we can actually verify. This classifies every `ensures` into a fragment and a
verifiability level, and measures structuring coverage across a program.

Honest fragment map (★ no Z3/Lean is wired in this tree — see ordinal.rs §32.0 audit, so quantified
first-order specs are NOT exactly provable here; they are bounded-tested at best):

  EXACT_ARITH      result = <polynomial in params>        → EXACT_PROVABLE  (JEFF coeff-zero, ∀)
  GENERAL_PREDICATE relational/predicate over evaluable fns → BOUNDED_ONLY  (evaluator fuzz)
  RELATIONAL       comparisons relating result & inputs    → BOUNDED_ONLY
  QUANTIFIED_FOL   ∀/∃ …                                   → BOUNDED_ONLY (evaluable) | OUTSIDE_FRAGMENT
  (any of the above referencing a NON-evaluable predicate, e.g. temporal `eventually`) → OUTSIDE_FRAGMENT
  NONE             no ensures                              → NO_SPEC

Boundary 1 is preserved everywhere: EXACT_PROVABLE / BOUNDED_ONLY are always *명세 대비*.
"""
from __future__ import annotations

import dataclasses
from dataclasses import dataclass, field
from typing import List, Set

import haran_ast as A
import haran_eval

BUILTIN_PREDS = set(haran_eval._BUILTINS.keys())   # filter, map, length, sorted, permutation


@dataclass
class SpecClass:
    fragment: str          # EXACT_ARITH | GENERAL_PREDICATE | RELATIONAL | QUANTIFIED_FOL | NONE
    verifiability: str     # EXACT_PROVABLE | BOUNDED_ONLY | OUTSIDE_FRAGMENT | NO_SPEC
    note: str
    predicates: List[str] = field(default_factory=list)

    def __str__(self):
        return f"{self.fragment}/{self.verifiability} — {self.note}"


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


def _call_names(e) -> List[str]:
    return [x.func.name for x in _walk(e) if isinstance(x, A.Call) and isinstance(x.func, A.Var)]


def _has_quant(e) -> bool:
    return any(isinstance(x, A.Quant) for x in _walk(e))


def _is_pure_arith(e) -> bool:
    if isinstance(e, A.Num) or isinstance(e, A.Var):
        return True
    if isinstance(e, A.Un) and e.op == "-":
        return _is_pure_arith(e.operand)
    if isinstance(e, A.Bin) and e.op in ("+", "-", "*", "/", "%", "**"):
        return _is_pure_arith(e.lhs) and _is_pure_arith(e.rhs)
    return False


def is_exact_arith(ens) -> bool:
    return (isinstance(ens, A.Bin) and ens.op in ("=", "==")
            and isinstance(ens.lhs, A.Var) and ens.lhs.name == "result"
            and _is_pure_arith(ens.rhs))


def classify(ensures, eval_names: Set[str]) -> SpecClass:
    """Classify a spec expression into its verification fragment. `eval_names` = predicate/function
    names the evaluator can run (builtins + user-defined fns)."""
    if ensures is None:
        return SpecClass("NONE", "NO_SPEC", "no ensures clause")
    calls = _call_names(ensures)
    non_eval = sorted({c for c in calls if c not in eval_names})
    preds = sorted(set(calls))
    if is_exact_arith(ensures):
        return SpecClass("EXACT_ARITH", "EXACT_PROVABLE",
                         "result = closed-form arithmetic → JEFF coefficient-zero, proven ∀", preds)
    if _has_quant(ensures):
        if non_eval:
            return SpecClass("QUANTIFIED_FOL", "OUTSIDE_FRAGMENT",
                             f"∀/∃ over non-evaluable predicate(s) {non_eval}; no Z3 wired → "
                             f"outside the verifiable fragment", preds)
        return SpecClass("QUANTIFIED_FOL", "BOUNDED_ONLY",
                         "∀/∃ over evaluable predicates → bounded-tested (no Z3 → not an exact ∀-proof)", preds)
    if non_eval:
        return SpecClass("GENERAL_PREDICATE", "OUTSIDE_FRAGMENT",
                         f"references non-evaluable predicate(s) {non_eval}", preds)
    if calls:
        return SpecClass("GENERAL_PREDICATE", "BOUNDED_ONLY",
                         "relational/predicate spec over evaluable functions → bounded fuzz", preds)
    return SpecClass("RELATIONAL", "BOUNDED_ONLY",
                     "comparison/relation over result and inputs → bounded fuzz", preds)


# ----------------------------------------------------------------- coverage
@dataclass
class CoverageRow:
    fn: str
    has_spec: bool
    status: str            # verifiable | bounded | outside | inferred | unspecified | proc-spec
    detail: str


@dataclass
class Coverage:
    rows: List[CoverageRow]

    def pct(self, *statuses) -> int:
        if not self.rows:
            return 0
        n = sum(1 for r in self.rows if r.status in statuses)
        return round(100 * n / len(self.rows))


def coverage(src: str, infer=True) -> Coverage:
    from haran_parser import parse
    prog = parse(src)
    fns = prog.fns()
    eval_names = BUILTIN_PREDS | {f.name for f in fns}
    ftab = {f.name: f for f in fns}
    rows = []
    for fn in fns:
        spec = fn.ensures if fn.ensures is not None else (fn.produces if fn.kind == "proc" else None)
        if spec is not None:
            sc = classify(spec, eval_names)
            status = {"EXACT_PROVABLE": "verifiable", "BOUNDED_ONLY": "bounded",
                      "OUTSIDE_FRAGMENT": "outside"}.get(sc.verifiability, "bounded")
            rows.append(CoverageRow(fn.name, True, status, str(sc)))
        else:
            cands = []
            if infer:
                import spec_infer
                cands = spec_infer.infer(fn, ftab)
            if cands:
                rows.append(CoverageRow(fn.name, False, "inferred",
                                        "; ".join(c.text for c in cands)))
            else:
                rows.append(CoverageRow(fn.name, False, "unspecified", "no spec, none inferable"))
    return Coverage(rows)


def render_coverage(cov: Coverage) -> str:
    lines = ["structuring coverage:"]
    for r in cov.rows:
        lines.append(f"   · {r.fn:14s} {r.status:12s} {r.detail}")
    total = len(cov.rows)
    lines.append(f"   ─ {total} functions: "
                 f"{cov.pct('verifiable')}% verifiable(exact), "
                 f"{cov.pct('bounded')}% bounded, "
                 f"{cov.pct('inferred')}% inferred, "
                 f"{cov.pct('outside')}% outside-fragment, "
                 f"{cov.pct('unspecified')}% unspecified")
    structured = cov.pct('verifiable', 'bounded', 'inferred')
    lines.append(f"   ─ STRUCTURING COVERAGE (some verifiable/bounded/inferred spec): {structured}%")
    return "\n".join(lines)
