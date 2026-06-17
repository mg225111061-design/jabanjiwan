"""
STAGE H5 — proc/cofix productivity checker (FRESH BUILD — honest-scope stage).  ⚠️
==================================================================================
Unlike H1–H4 (which assemble onto confirmed assets: ordinal.rs / poly.rs / jeff_identity), there is
NO existing productivity checker to connect to — only the `GuardedCorecursion E0402` diagnostic slot
in jeff-span. So this is built from scratch, and its scope is declared honestly.

A `proc` produces a `Stream` via a `cofix` loop (§1.3). It is PRODUCTIVE iff every corecursive call
(reference to the cofix label) is *guarded*: a `yield` (a constructor application) definitely executes
before the recursion on that path. We check this SYNTACTICALLY:

  • straight-line guardedness  — a `yield` lexically precedes the recursion in the same block;
  • per-arm guardedness        — inside a `match`, the guard must come before the recursion in
                                 that same arm (a yield in a *different* arm does NOT count).

Verdicts:  PROVEN (all corecursive calls guarded) · REFUTED (a straight-line unguarded recursion —
unambiguously non-productive) · OUT_OF_SCOPE (features our simple syntactic check can't soundly
classify: conditional productivity, nested cofix, mutual/external corecursion). We never guess: a
case we can't soundly decide is OUT_OF_SCOPE, not a fake PROVEN/REFUTED.

OUT OF SCOPE (stated, not hidden): conditional/data-dependent productivity, mutual recursion between
procs, nested cofix, guardedness through called functions (deep guardedness), semantic productivity.
"""
from __future__ import annotations

import dataclasses
from dataclasses import dataclass, field
from typing import List, Optional, Set

import haran_ast as A


@dataclass
class Site:
    node: object
    guarded: bool
    in_match: bool


@dataclass
class ProductivityResult:
    verdict: str           # PROVEN | REFUTED | OUT_OF_SCOPE | N/A
    proc: str
    detail: str
    unguarded: List[object] = field(default_factory=list)   # spans of unguarded recursion sites
    scope_note: str = ""

    def __str__(self):
        ug = ""
        if self.unguarded:
            ug = "  unguarded@[" + ", ".join(str(s) for s in self.unguarded) + "]"
        sn = f"  (scope: {self.scope_note})" if self.scope_note else ""
        return f"{self.proc}: {self.verdict} — {self.detail}{ug}{sn}"


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


def _find_cofix(node) -> Optional[A.Cofix]:
    for x in _walk(node):
        if isinstance(x, A.Cofix):
            return x
    return None


def _count_cofix(node) -> int:
    return sum(1 for x in _walk(node) if isinstance(x, A.Cofix))


def _collect_sites(node, label: str, guarded: bool, in_match: bool, sites: List[Site],
                   mutual: Set[str], proc_names: Set[str]):
    """Walk control flow, recording each reference to `label` with whether a yield definitely
    preceded it (guarded) and whether it sits inside a match arm (conditional)."""
    if isinstance(node, A.Block):
        g = guarded
        for st in node.stmts:
            _collect_sites(st, label, g, in_match, sites, mutual, proc_names)
            if isinstance(st, A.Yield):
                g = True              # straight-line yield guards everything after it in this block
        return
    if isinstance(node, A.Yield):
        _collect_sites(node.value, label, guarded, in_match, sites, mutual, proc_names)
        return
    if isinstance(node, (A.ExprStmt, A.Un)):
        _collect_sites(node.value if isinstance(node, A.ExprStmt) else node.operand,
                       label, guarded, in_match, sites, mutual, proc_names)
        return
    if isinstance(node, A.Let):
        _collect_sites(node.value, label, guarded, in_match, sites, mutual, proc_names)
        return
    if isinstance(node, A.Match):
        _collect_sites(node.scrut, label, guarded, in_match, sites, mutual, proc_names)
        for arm in node.arms:
            _collect_sites(arm.body, label, guarded, True, sites, mutual, proc_names)  # arm = conditional
        return
    if isinstance(node, A.Var):
        if node.name == label:
            sites.append(Site(node, guarded, in_match))
        return
    if isinstance(node, A.Call):
        if isinstance(node.func, A.Var) and node.func.name == label:
            sites.append(Site(node.func, guarded, in_match))
            return
        if isinstance(node.func, A.Var) and node.func.name in proc_names:
            mutual.add(node.func.name)            # call to another proc → possible mutual corecursion
        _collect_sites(node.func, label, guarded, in_match, sites, mutual, proc_names)
        for a in node.args:
            _collect_sites(a, label, guarded, in_match, sites, mutual, proc_names)
        return
    if isinstance(node, A.Bin):
        _collect_sites(node.lhs, label, guarded, in_match, sites, mutual, proc_names)
        _collect_sites(node.rhs, label, guarded, in_match, sites, mutual, proc_names)
        return
    if isinstance(node, (A.Lambda, A.Quant)):
        _collect_sites(node.body, label, guarded, in_match, sites, mutual, proc_names)
        return
    if isinstance(node, A.Fold):
        _collect_sites(node.domain, label, guarded, in_match, sites, mutual, proc_names)
        _collect_sites(node.body, label, guarded, in_match, sites, mutual, proc_names)
        return
    if isinstance(node, A.Range):
        _collect_sites(node.lo, label, guarded, in_match, sites, mutual, proc_names)
        _collect_sites(node.hi, label, guarded, in_match, sites, mutual, proc_names)
        return
    if isinstance(node, A.ListLit):
        for e in node.elems:
            _collect_sites(e, label, guarded, in_match, sites, mutual, proc_names)
        return
    # A.Cofix (nested) handled by _count_cofix; Num/BoolLit etc.: nothing to do.


def check_cofix(cofix: A.Cofix, name: str = "cofix", proc_names: Set[str] = frozenset()) -> ProductivityResult:
    scope = ("syntactic guardedness only: straight-line + per-arm. Out of scope: conditional/"
             "data-dependent productivity, mutual recursion, nested cofix, deep guardedness.")
    # structural out-of-scope features first
    if _count_cofix(cofix.body) > 0:
        return ProductivityResult("OUT_OF_SCOPE", name,
                                  "nested cofix — only a single, non-nested cofix is handled", scope_note=scope)
    sites: List[Site] = []
    mutual: Set[str] = set()
    _collect_sites(cofix.body, cofix.name, False, False, sites, mutual, set(proc_names))
    if mutual:
        return ProductivityResult("OUT_OF_SCOPE", name,
                                  f"mutual/external corecursion (calls {sorted(mutual)})", scope_note=scope)
    if not sites:
        return ProductivityResult("PROVEN", name,
                                  "no corecursive call — finite production, trivially productive", scope_note=scope)
    unguarded = [s for s in sites if not s.guarded]
    if not unguarded:
        return ProductivityResult("PROVEN", name,
                                  f"every corecursive call to '{cofix.name}' is guarded by a preceding yield",
                                  scope_note=scope)
    straight = [s for s in unguarded if not s.in_match]
    if straight:
        return ProductivityResult("REFUTED", name,
                                  f"corecursive call to '{cofix.name}' with NO preceding yield (unguarded) — "
                                  f"recurses without producing",
                                  unguarded=[s.node.span for s in straight], scope_note=scope)
    return ProductivityResult("OUT_OF_SCOPE", name,
                              "conditional productivity: a corecursive call is guarded in some match arms "
                              "but not all; deciding this needs semantic reasoning we do not do",
                              unguarded=[s.node.span for s in unguarded], scope_note=scope)


def check_proc(fn: A.FnDecl, proc_names: Set[str] = frozenset()) -> ProductivityResult:
    if fn.kind != "proc":
        return ProductivityResult("N/A", fn.name, "not a proc — productivity applies to coinductive procs")
    cofix = _find_cofix(fn.body) if fn.body else None
    if cofix is None:
        return ProductivityResult("OUT_OF_SCOPE", fn.name,
                                  "no cofix loop found — productivity check targets cofix corecursion")
    others = {p for p in proc_names if p != fn.name}
    return check_cofix(cofix, fn.name, others)
