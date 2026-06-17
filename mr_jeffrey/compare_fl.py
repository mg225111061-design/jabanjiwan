"""
HARAN v18 Part G · STAGE G5 — three-way comparison (the heart). NO favoritism — measurement decides.
====================================================================================================
Run the SAME line-annotated bug corpus through three fault localizers and measure top-1 / top-5:
  1. v16 property  — Bayesian narrow (B5), mapped to lines;
  2. pure diffusion — PRFL-style (Bayesian x₀ diffused over the PDG, NO sinks);
  3. proof-boundary diffusion — v18 (Bayesian x₀ + violation sources + PROVEN-SAFE sinks).

Honest questions: does diffusion beat property? does the proof-sink beat pure diffusion? The conclusion
is whichever the numbers say — if diffusion loses, we keep property and say so.
"""
from __future__ import annotations

from dataclasses import dataclass, field
from typing import Dict, List, Optional, Tuple

import hir
import properties as PR
import property_test as PT
import narrow as NA
import proof_diffuse as PD


# ----------------------------------------------------------------- the three localizers (line rankings)
def property_ranking(source: str, filename: str) -> List[int]:
    fr = hir.to_hir(source, filename)
    hfn = fr.module.functions[0]
    fn = PR.compile_callable(hfn)
    props = PR.extract_properties(hfn)
    rep = PT.test_properties(fn, props, PT.gen_inputs(hfn, 300))
    violated = [p for p in props if p.name in rep.violated_properties()]
    res = NA.bayesian_narrow(hfn, violated)
    score: Dict[int, float] = {}
    for s in res.ranked:
        for ln in (s.lines or []):
            score[ln] = score.get(ln, 0.0) + s.posterior / max(1, len(s.lines))
    return [ln for ln, _ in sorted(score.items(), key=lambda p: p[1], reverse=True)]


def _diffusion_ranking(source: str, filename: str, use_sinks: bool) -> List[int]:
    b = PD.build_boundary(source, filename)
    if b is None:
        return []
    return [ln for ln, h in PD.rank_lines(b, use_sinks=use_sinks) if h > 0]


def pure_diffusion_ranking(source, filename):
    return _diffusion_ranking(source, filename, use_sinks=False)


def proof_diffusion_ranking(source, filename):
    return _diffusion_ranking(source, filename, use_sinks=True)


# ----------------------------------------------------------------- corpus (line-annotated bugs)
@dataclass
class Bug:
    name: str
    source: str
    bug_lines: List[int]      # acceptable fix line(s)


def _corpus() -> List[Bug]:
    return [
        # 1. comparison flipped (bug at the `if`, line 5)
        Bug("sort_cmp",
            "def f(xs):\n a=list(xs)\n for i in range(len(a)):\n  for j in range(len(a)-1):\n"
            "   if a[j] < a[j+1]:\n    a[j],a[j+1]=a[j+1],a[j]\n return a\n", [5]),
        # 2. drops the last element (bug at the pop, line 4)
        Bug("sort_drop", "def f(xs):\n a=sorted(xs)\n if len(a)>1:\n  a.pop()\n return a\n", [4]),
        # 3. drops duplicates via set (bug at line 2)
        Bug("sort_dup", "def f(xs):\n a=sorted(set(xs))\n return a\n", [2]),
        # 4. returns min not max (comparison bug, line 4)
        Bug("max_min", "def f(xs):\n m=xs[0]\n for x in xs:\n  if x < m:\n   m=x\n return [m]\n", [4]),
        # 5. inner loop off-by-one leaves it unsorted (bug at the inner range, line 4)
        Bug("partial", "def f(xs):\n a=list(xs)\n for i in range(len(a)):\n  for j in range(len(a)-2):\n"
            "   if a[j] > a[j+1]:\n    a[j],a[j+1]=a[j+1],a[j]\n return a\n", [4]),
        # 6. doubles the list (length bug, line 2)
        Bug("double", "def f(xs):\n a=list(xs)+list(xs)\n return sorted(a)\n", [2]),
    ]


# ----------------------------------------------------------------- measurement
@dataclass
class MethodScore:
    name: str
    top1: int
    top5: int
    total: int
    per_bug: list = field(default_factory=list)

    def top1_rate(self):
        return self.top1 / self.total if self.total else 0.0

    def top5_rate(self):
        return self.top5 / self.total if self.total else 0.0


def _hit(ranking: List[int], bug_lines: List[int], k: int) -> bool:
    return any(bl in ranking[:k] for bl in bug_lines)


def three_way(corpus: Optional[List[Bug]] = None) -> Dict[str, MethodScore]:
    corpus = corpus or _corpus()
    methods = {"property": property_ranking, "pure_diffusion": pure_diffusion_ranking,
               "proof_diffusion": proof_diffusion_ranking}
    scores = {m: MethodScore(m, 0, 0, len(corpus)) for m in methods}
    for bug in corpus:
        for m, fn in methods.items():
            try:
                r = fn(bug.source, bug.name + ".py")
            except Exception:
                r = []
            h1, h5 = _hit(r, bug.bug_lines, 1), _hit(r, bug.bug_lines, 5)
            scores[m].top1 += int(h1)
            scores[m].top5 += int(h5)
            scores[m].per_bug.append((bug.name, r[:3], h1, h5))
    return scores


def conclude(scores: Dict[str, MethodScore]) -> str:
    prop, pure, proof = scores["property"], scores["pure_diffusion"], scores["proof_diffusion"]

    def cmp(a, b):
        return "BEATS" if a.top1_rate() > b.top1_rate() + 1e-9 else (
            "TIES" if abs(a.top1_rate() - b.top1_rate()) < 1e-9 else "LOSES to")

    pure_v = f"pure diffusion {cmp(pure, prop)} property"
    sink_v = ("proof-sink HELPS over pure" if proof.top1_rate() > pure.top1_rate() + 1e-9 else
              "proof-sink NEUTRAL vs pure" if abs(proof.top1_rate() - pure.top1_rate()) < 1e-9 else
              "proof-sink HURTS vs pure (weak abstract-interp sinks drain real correctness bugs)")
    best = max(scores.values(), key=lambda s: (s.top1_rate(), s.top5_rate()))
    decision = ("KEEP property" if prop.top1_rate() >= best.top1_rate() - 1e-9
                else f"ADOPT {best.name}")
    return f"{pure_v}; {sink_v} → {decision}."
