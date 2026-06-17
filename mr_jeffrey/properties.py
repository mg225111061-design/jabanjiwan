"""
HARAN v16 Part B · STAGE B2 — property extraction (metamorphic relations, no full spec).
========================================================================================
We don't need a complete specification — only RELATIONS the intended behaviour should satisfy
(metamorphic / preservation / algebraic / range). They are extracted from the function's shape + HIR
operations (+ optional AI), then TESTED by running the real code (B3). The more independent properties
we extract, the harder the later digit-narrowing (B5) can squeeze — but only if they are genuinely
independent, which B2.3 assesses honestly.

Honest strength boundary (B2 discipline): code with clear relations (math / transforms / data
structures — the fold domain) yields many properties → strong. Arbitrary business logic yields few →
"property-poor, weak" is reported, not hidden.
"""
from __future__ import annotations

from dataclasses import dataclass, field
from typing import Callable, List, Optional

import hir


# ----------------------------------------------------------------- compile HIR fn → callable
def compile_callable(hfn: hir.HFunction, glb: Optional[dict] = None):
    """Language-agnostic: Python execs in-process; other languages compile+run via runtime.make_callable
    (the same engine then tests the REAL native output)."""
    if getattr(hfn, "lang", "python") == "python" and glb is not None:
        ns: dict = dict(glb)
        exec(hfn.source, ns)
        return ns[hfn.name]
    import runtime
    return runtime.make_callable(hfn, glb)


# ----------------------------------------------------------------- property model
@dataclass
class Property:
    name: str
    kind: str            # preservation | metamorphic | algebraic | range | sanity
    description: str
    check: Callable      # (fn, input) -> bool   (a metamorphic / preservation test on the REAL fn)
    group: str           # independence group (different groups ≈ independent; B5 multiplies across groups)
    operations: List[str] = field(default_factory=list)   # HIR op kinds this property constrains (→ B4)


def _is_listish(x):
    return isinstance(x, (list, tuple))


# ---- the metamorphic / preservation relations (run the real function) ----
def _determinism(fn, x):
    return fn(_copy(x)) == fn(_copy(x))


def _length_preservation(fn, x):
    y = fn(_copy(x))
    return _is_listish(y) and len(y) == len(x)


def _permutation(fn, x):
    y = fn(_copy(x))
    try:
        return sorted(y) == sorted(x)        # multiset preserved
    except TypeError:
        return False


def _idempotence(fn, x):
    y = fn(_copy(x))
    return fn(_copy(y)) == y


def _ordered_output(fn, x):
    y = fn(_copy(x))
    return _is_listish(y) and all(y[i] <= y[i + 1] for i in range(len(y) - 1))


def _order_invariance(fn, x):
    # sorting is invariant to input order: f(reversed(x)) == f(x)
    y1 = fn(_copy(x))
    y2 = fn(list(reversed(list(x))))
    return y1 == y2


def _nonneg_range(fn, x):
    y = fn(_copy(x))
    return isinstance(y, (int, float)) and y >= 0


def _copy(x):
    return list(x) if isinstance(x, list) else x


# canonical library (name -> Property factory)
def _sort_props() -> List[Property]:
    return [
        Property("determinism", "sanity", "f(x) == f(x)", _determinism, "g_det", ["call", "arith"]),
        Property("length_preservation", "preservation", "len(f(x)) == len(x)", _length_preservation,
                 "g_size", ["append", "pop", "slice", "insert", "extend", "remove"]),
        Property("permutation", "preservation", "multiset(f(x)) == multiset(x)", _permutation,
                 "g_size", ["append", "pop", "index_store", "arith", "insert", "remove"]),
        Property("ordered_output", "metamorphic", "f(x) is sorted", _ordered_output, "g_order",
                 ["compare", "index_store", "swap"]),
        Property("idempotence", "algebraic", "f(f(x)) == f(x)", _idempotence, "g_idem",
                 ["compare", "index_store"]),
        Property("order_invariance", "metamorphic", "f(reverse(x)) == f(x)", _order_invariance, "g_order",
                 ["compare", "index_store"]),
    ]


# ----------------------------------------------------------------- B2.1/B2.2 extraction
def looks_like_sort(hfn: hir.HFunction) -> bool:
    k = hfn.op_kinds()
    return ("sort" in k) or ({"compare", "index_store"} <= k) or ("reverse" in k and "compare" in k)


def returns_listish(hfn: hir.HFunction, sample) -> bool:
    try:
        fn = compile_callable(hfn)
        return _is_listish(fn(_copy(sample)))
    except Exception:
        return False


def extract_properties(hfn: hir.HFunction, sample=None) -> List[Property]:
    """Heuristic extraction from HIR shape/ops. Sort-shaped functions get the full metamorphic suite;
    list→list functions get preservation; everything gets determinism."""
    sample = sample if sample is not None else [3, 1, 2]
    props: List[Property] = []
    if looks_like_sort(hfn):
        props = _sort_props()
    else:
        props = [p for p in _sort_props() if p.name == "determinism"]
        if returns_listish(hfn, sample):
            props += [p for p in _sort_props() if p.name in ("length_preservation",)]
    return props


def ai_extract_properties(hfn: hir.HFunction) -> List[str]:
    """Optional AI augmentation. DISABLED under the level-1 key policy (no env keys ever) — returns []
    so the heuristics stand. Any AI augmentation would take an explicit per-call key (level-1), never
    an environment variable."""
    return []


# ----------------------------------------------------------------- B2.3 independence assessment
@dataclass
class IndependenceReport:
    groups: dict                 # group -> [property names]
    independent_count: int       # number of distinct groups (≈ independent factors for B5)
    note: str


def assess_independence(props: List[Property]) -> IndependenceReport:
    groups: dict = {}
    for p in props:
        groups.setdefault(p.group, []).append(p.name)
    note = ("properties in different groups test different aspects (≈ independent → B5 may multiply); "
            "same-group properties are correlated (e.g. permutation ⇒ length) — NOT independently "
            "multiplied (B6 discounts).")
    return IndependenceReport(groups, len(groups), note)
