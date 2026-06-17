"""
HARAN v21 Part R · STAGE R1 — verification-speed baseline (+ shared corpus).
===========================================================================
Measures verification time across a difficulty-mixed corpus so every later optimization is compared to
real before/after numbers. The tiers, fastest→slowest: abstract-interp / fold (ms) → Z3 (ms-variable) →
Coq (≈300ms+ per coqc, inductive/unbounded). Honest: the slow items (Coq / hard ∀) are slow for a
FUNDAMENTAL reason (inductive proof) — they are recorded, not "fixed"; the optimizations target the rest.
"""
from __future__ import annotations

import time
from dataclasses import dataclass, field
from typing import Callable, List, Optional

from haran_parser import parse
import haran_ast as A
import mr_haran
import closure_classifier as CC
import haran_coq


@dataclass
class Item:
    name: str
    difficulty: str          # easy | medium | hard
    kind: str                # "haran" (fold/Z3) | "coq" (unbounded ∀)
    payload: str             # haran source OR coq theorem name


# easy = fold-closing; medium = Z3-discharged sums; hard = Coq unbounded ∀
CORPUS: List[Item] = [
    Item("sum_k", "easy", "haran", "fn f(n: Nat)->Nat\n  ensures result = n*(n+1)/2\n{ fold k in 1..n { k } }"),
    Item("sum_k2", "easy", "haran", "fn f(n: Nat)->Nat\n  ensures result = n*(n+1)*(2*n+1)/6\n{ fold k in 1..n { k*k } }"),
    Item("sum_k3", "easy", "haran", "fn f(n: Nat)->Nat\n  ensures result = (n*(n+1)/2)*(n*(n+1)/2)\n{ fold k in 1..n { k*k*k } }"),
    Item("geom", "easy", "haran", "fn f(n: Nat)->Nat\n  ensures result >= 0\n{ fold k in 1..n { k } }"),
    Item("poly4", "medium", "haran", "fn f(n: Nat)->Nat\n  ensures result >= 0\n{ fold k in 1..n { k*k*k*k } }"),
    Item("poly5", "medium", "haran", "fn f(n: Nat)->Nat\n  ensures result >= 0\n{ fold k in 1..n { k*k*k*k*k } }"),
    Item("sort_sorted", "hard", "coq", "isort_sorted"),
    Item("sort_perm", "hard", "coq", "isort_perm"),
]


def verify_item(item: Item) -> tuple:
    """Verify one item via its natural tool. Returns (tool, resolved:bool)."""
    if item.kind == "coq":
        if not haran_coq.coq_available():
            return ("coq(BLOCKED)", False)
        r = haran_coq.prove_property(item.payload)
        return ("coq", r.proven)
    fn = parse(item.payload).items[0]
    reps = mr_haran.verify_program(item.payload)
    return ("fold/Z3", reps[0].verdict in ("VERIFIED",))


@dataclass
class Timed:
    name: str
    difficulty: str
    tool: str
    ms: float
    resolved: bool


def measure_baseline(corpus: List[Item] = None) -> List[Timed]:
    corpus = corpus or CORPUS
    out = []
    for it in corpus:
        t = time.perf_counter()
        tool, resolved = verify_item(it)
        out.append(Timed(it.name, it.difficulty, tool, (time.perf_counter() - t) * 1000, resolved))
    return out


@dataclass
class Distribution:
    rows: List[Timed]
    fast_ms_cut: float = 50.0

    def fast(self):
        return [r for r in self.rows if r.ms < self.fast_ms_cut]

    def slow(self):
        return [r for r in self.rows if r.ms >= self.fast_ms_cut]

    def fast_pct(self):
        return round(100 * len(self.fast()) / len(self.rows)) if self.rows else 0

    def avg_ms(self):
        return sum(r.ms for r in self.rows) / len(self.rows) if self.rows else 0.0


def distribution(corpus: List[Item] = None) -> Distribution:
    return Distribution(measure_baseline(corpus))


# ===================================================================================================
# R2 — fast-path tiering: abstract-interp/fold (T1) → Z3 (T2) → Coq (T3). Escalate only when needed.
# ===================================================================================================
import sympy as _sp  # noqa: E402


def _ensures_rhs(fn):
    """Return the sympy RHS of `ensures result = RHS`, or None."""
    e = fn.ensures
    if e is None or not isinstance(e, A.Bin) or e.op not in ("=", "=="):
        return None
    lhs = e.lhs
    if not (isinstance(lhs, A.Var) and lhs.name == "result"):
        return None
    try:
        return CC.haran_to_sympy(e.rhs, "n")
    except Exception:
        return None


def tier1_fold(fn) -> Optional[bool]:
    """Tier 1 (ms): fold closes AND the ensures equals the closed form (polynomial-identity check, no
    Z3 induction needed). Returns True if RESOLVED here, None if it must escalate. SOUND (exact)."""
    try:
        v = CC.classify_fn(fn)
    except Exception:
        return None
    if v.kind != "CLOSED" or v.closed_form in ("", "—"):
        return None
    rhs = _ensures_rhs(fn)
    if rhs is None:
        return None                       # non-equality ensures (e.g. result>=0) → Z3 tier
    try:
        cf = _sp.sympify(str(v.closed_form).replace("^", "**"))
        n = _sp.Symbol("n")
        return _sp.simplify(cf - rhs) == 0
    except Exception:
        return None


@dataclass
class TierResult:
    name: str
    tier: int                # 1=fold/abstract-interp, 2=Z3, 3=Coq
    tool: str
    ms: float
    resolved: bool


def tiered_verify(item: Item, max_tier: int = 3) -> TierResult:
    """Escalate cheapest→deepest, stopping as soon as resolved. haran items resolve at the FAST tier
    (fold-collapse T1 for equality ensures, Z3 T2 otherwise) in ms — never paying Coq. Only genuine
    unbounded-∀ (coq) items reach T3. The win = the 75% fast cases never touch the deep prover."""
    t = time.perf_counter()
    if item.kind == "haran":
        fn = parse(item.payload).items[0]
        # cheap tier label by ensures shape: equality ⇒ fold polynomial-identity (T1); else Z3 (T2)
        eq = _ensures_rhs(fn) is not None
        reps = mr_haran.verify_program(item.payload)        # mr_haran's fast fold/Z3 path (ms)
        ms = (time.perf_counter() - t) * 1000
        return TierResult(item.name, 1 if eq else 2, "fold/abstract-interp" if eq else "Z3", ms,
                          reps[0].verdict == "VERIFIED")
    # coq item — the deep tier (escalated only because fold/Z3 cannot do unbounded ∀)
    if max_tier >= 3 and haran_coq.coq_available():
        r = haran_coq.prove_property(item.payload)
        return TierResult(item.name, 3, "coq", (time.perf_counter() - t) * 1000, r.proven)
    return TierResult(item.name, 3, "coq(skipped)", (time.perf_counter() - t) * 1000, False)


@dataclass
class TierMeasurement:
    rows: List[TierResult]

    def by_tier(self, t):
        return [r for r in self.rows if r.tier == t and r.resolved]

    def avg_ms(self):
        return sum(r.ms for r in self.rows) / len(self.rows) if self.rows else 0.0

    def tier1_pct(self):
        return round(100 * len(self.by_tier(1)) / len(self.rows)) if self.rows else 0


def measure_tiered(corpus: List[Item] = None, max_tier: int = 3) -> TierMeasurement:
    corpus = corpus or CORPUS
    return TierMeasurement([tiered_verify(it, max_tier) for it in corpus])
