"""
HARAN v16 Part A · STAGE A1 — verification caching (skip re-verify of unchanged functions).
===========================================================================================
"순식간" only makes sense in the EDIT-VERIFY LOOP: after the first full verification, re-verifying a
file where most functions are untouched should hit cache and skip the prover. We key each function by a
MERKLE hash of (its normalized AST + spec) combined with the content hashes of its transitive callees —
so if a dependency A changes, every caller B's key changes too and B is correctly re-verified (A1.2).

Honest scope (A1.3): the FIRST full verification still pays the prover in full; non-linear SMT is still
slow. Caching buys the incremental edit-verify loop, nothing else. We measure exactly that.
"""
from __future__ import annotations

import dataclasses
import hashlib
import time
from dataclasses import dataclass, field
from typing import Dict, List, Optional, Tuple

import haran_ast as A
from haran_parser import parse
import mr_haran


# ----------------------------------------------------------------- canonical hashing (span-free)
def _canon(node) -> str:
    """Stable, span-free serialization of an AST node / value (so spans don't bust the cache)."""
    if isinstance(node, A.Span):
        return ""
    if dataclasses.is_dataclass(node):
        parts = [type(node).__name__]
        for f in dataclasses.fields(node):
            if f.name == "span":
                continue
            parts.append(f"{f.name}={_canon(getattr(node, f.name))}")
        return "(" + ",".join(parts) + ")"
    if isinstance(node, (list, tuple)):
        return "[" + ",".join(_canon(x) for x in node) + "]"
    if isinstance(node, dict):
        return "{" + ",".join(f"{k}:{_canon(v)}" for k, v in sorted(node.items())) + "}"
    return repr(node)


def fn_content_hash(fn: A.FnDecl) -> str:
    """Hash of the function's own AST + spec (ignores spans, ignores other functions)."""
    return hashlib.sha256(_canon(fn).encode()).hexdigest()[:16]


def _called_names(fn: A.FnDecl, names: set) -> List[str]:
    out = []
    for x in mr_haran._walk(fn.body) if fn.body else []:
        if isinstance(x, A.Call) and isinstance(x.func, A.Var) and x.func.name in names and x.func.name != fn.name:
            if x.func.name not in out:
                out.append(x.func.name)
    return out


def _transitive_dep_hashes(fn: A.FnDecl, ftab: Dict[str, A.FnDecl]) -> List[str]:
    """Content hashes of every function reachable from fn (excluding itself), as a set."""
    names = set(ftab)
    seen, stack = set(), list(_called_names(fn, names))
    while stack:
        nm = stack.pop()
        if nm in seen or nm not in ftab:
            continue
        seen.add(nm)
        stack += _called_names(ftab[nm], names)
    return sorted(fn_content_hash(ftab[n]) for n in seen)


def merkle_key(fn: A.FnDecl, ftab: Dict[str, A.FnDecl]) -> str:
    """Cache key = own content hash + transitive dependency content hashes. A change to any
    dependency changes this key ⇒ automatic invalidation of callers (A1.2)."""
    own = fn_content_hash(fn)
    deps = _transitive_dep_hashes(fn, ftab)
    return hashlib.sha256((own + "|" + "|".join(deps)).encode()).hexdigest()[:16]


# ----------------------------------------------------------------- cached verifier
@dataclass
class CacheStats:
    hits: int = 0
    misses: int = 0
    verified_fns: List[str] = field(default_factory=list)   # functions actually re-verified (misses)
    skipped_fns: List[str] = field(default_factory=list)    # functions served from cache (hits)

    @property
    def total(self):
        return self.hits + self.misses


class CachedVerifier:
    """Persists {merkle_key: FnReport} across verifications within a session."""

    def __init__(self):
        self.cache: Dict[str, mr_haran.FnReport] = {}

    def verify_program(self, src: str) -> Tuple[List[mr_haran.FnReport], CacheStats]:
        prog = parse(src)
        if prog.errors:
            raise ValueError("; ".join(str(e) for e in prog.errors))
        fns = [it for it in prog.items if isinstance(it, A.FnDecl)]
        ftab = {f.name: f for f in fns}
        proc_names = {f.name for f in fns if f.kind == "proc"}
        reports, stats = [], CacheStats()
        for fn in fns:
            key = merkle_key(fn, ftab)
            cached = self.cache.get(key)
            if cached is not None:
                stats.hits += 1
                stats.skipped_fns.append(fn.name)
                reports.append(cached)
            else:
                rep = mr_haran.verify_fn(fn, ftab, proc_names)
                self.cache[key] = rep
                stats.misses += 1
                stats.verified_fns.append(fn.name)
                reports.append(rep)
        return reports, stats


# ----------------------------------------------------------------- A1.3 measurement helper
@dataclass
class EditLoopMeasurement:
    cold_s: float            # first full verification (all miss)
    warm_unchanged_s: float  # re-verify identical source (all hit)
    warm_one_edit_s: float   # re-verify after editing ONE function
    reverified_after_edit: List[str]
    speedup_unchanged: float
    speedup_one_edit: float


def measure_edit_loop(src: str, edited_src: str) -> EditLoopMeasurement:
    """Cold verify → warm (no change) → warm (one function edited). Reports wall-clock + which
    functions actually re-ran after the edit (should be the edited fn + its callers only)."""
    cv = CachedVerifier()
    t = time.perf_counter(); cv.verify_program(src); cold = time.perf_counter() - t
    t = time.perf_counter(); _, s_same = cv.verify_program(src); warm_same = time.perf_counter() - t
    t = time.perf_counter(); _, s_edit = cv.verify_program(edited_src); warm_edit = time.perf_counter() - t
    return EditLoopMeasurement(
        cold_s=cold, warm_unchanged_s=warm_same, warm_one_edit_s=warm_edit,
        reverified_after_edit=s_edit.verified_fns,
        speedup_unchanged=cold / warm_same if warm_same > 0 else float("inf"),
        speedup_one_edit=cold / warm_edit if warm_edit > 0 else float("inf"))
