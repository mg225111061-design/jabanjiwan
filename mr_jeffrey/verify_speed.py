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
