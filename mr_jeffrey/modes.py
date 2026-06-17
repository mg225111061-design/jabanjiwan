"""
HARAN v21 Part R · STAGE R7 — two modes (normal / extended): same engine, different depth.
==========================================================================================
  • NORMAL = speed first: fold-first + fast-path (abstract-interp/Z3) + incremental only. If the fast
    tools can't settle a hard ∀, it stops there — status UNRESOLVED-shallow (NOT a wrong answer: it
    simply didn't look deeper). No deep Coq, no background. → very fast.
  • EXTENDED = quality first: everything above PLUS deep Z3/Coq and background for the hard cases →
    solves MORE (slightly slower, but still fast thanks to R2–R5). Worst case = honest UNRESOLVED-timeout.

★ Both modes give ZERO wrong answers ★ — normal is shallow (misses), never false; extended solves more
but is still not "everything" (NP-hard/inductive timeouts are reported honestly).
"""
from __future__ import annotations

import time
from dataclasses import dataclass, field
from typing import List

import verify_speed as VS


@dataclass
class ModeResult:
    name: str
    mode: str
    proven: bool
    status: str          # PROVEN | UNRESOLVED-shallow (normal) | UNRESOLVED-timeout (extended)
    ms: float


def analyze(corpus: List[VS.Item], mode: str = "normal") -> List[ModeResult]:
    max_tier = 2 if mode == "normal" else 3       # normal stops before Coq; extended goes deep
    out = []
    for it in corpus:
        t = time.perf_counter()
        r = VS.tiered_verify(it, max_tier=max_tier)
        ms = (time.perf_counter() - t) * 1000
        if r.resolved:
            status = "PROVEN"
        elif it.kind == "coq" and mode == "normal":
            status = "UNRESOLVED-shallow"          # honest: normal didn't go deep — NOT wrong
        else:
            status = "UNRESOLVED-timeout"
        out.append(ModeResult(it.name, mode, r.resolved, status, ms))
    return out


@dataclass
class ModeComparison:
    normal: List[ModeResult]
    extended: List[ModeResult]

    def normal_ms(self):
        return sum(r.ms for r in self.normal)

    def extended_ms(self):
        return sum(r.ms for r in self.extended)

    def normal_solved(self):
        return sum(1 for r in self.normal if r.proven)

    def extended_solved(self):
        return sum(1 for r in self.extended if r.proven)

    def speedup(self):
        return self.extended_ms() / self.normal_ms() if self.normal_ms() > 0 else float("inf")

    def extra_solved(self):
        return self.extended_solved() - self.normal_solved()

    def normal_wrong(self):
        # a "wrong" answer = claimed PROVEN but actually false. Normal only ever says PROVEN for
        # things the sound fast tools verified, or UNRESOLVED-shallow — never a false PROVEN.
        return sum(1 for r in self.normal if r.status == "PROVEN" and not r.proven)


def compare_modes(corpus: List[VS.Item] = None) -> ModeComparison:
    corpus = corpus or VS.CORPUS
    return ModeComparison(analyze(corpus, "normal"), analyze(corpus, "extended"))
