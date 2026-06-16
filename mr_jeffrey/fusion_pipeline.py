"""
HARAN v17 Part D · STAGE D4 — fused pipeline (B detect → A fold/Z3/Coq) + measurement.
======================================================================================
One entry: ordinary code in →
  • a spec present → Z3/JEFF proves it ∀ or refutes it (verified / refuted+cx);
  • a fold-able loop → fold engine closes it (closed form + proof + O(1)) or NO_STRUCTURE;
  • a sort that passes bounded checks → Coq proves it for all lengths (unbounded-proven);
  • otherwise → B localizes the suspect operation with a proven digit (bug-localized).

★ DISCIPLINE: the A-side value lands ONLY on fold-able / spec'd / recognized code. Arbitrary logic gets
B-level localization, not a closed-form digit — being "general language" changes nothing.
"""
from __future__ import annotations

from dataclasses import dataclass, field
from typing import List, Optional

import hir
import fusion
import typeB


@dataclass
class FusedVerdict:
    category: str        # verified | refuted | closed-form | unbounded-proven | bug-localized | no-structure | unknown
    headline: str
    detail: str
    evidence: str        # which tool produced it (Z3 / fold / Coq / B)


def analyze_fused(source: str, filename: Optional[str] = None) -> FusedVerdict:
    fr = hir.to_hir(source, filename)
    if not fr.supported:
        return FusedVerdict("unknown", "frontend unavailable", fr.detail, "-")
    hfn = fr.module.functions[0]

    # 1. spec present → Z3/JEFF ∀ proof / refutation (most authoritative)
    if fusion.extract_spec(source):
        v = fusion.z3_inject(hfn, source=source)
        if v.tier == "PROVEN":
            return FusedVerdict("verified", f"spec proven ∀: {v.spec}", v.detail, "Z3/JEFF")
        if v.tier == "FAILED":
            return FusedVerdict("refuted", f"spec FAILS: {v.spec}", f"counterexample {v.counterexample}", "Z3/JEFF")

    # 2. fold-able loop → close it
    fold = fusion.fold_inject(hfn)
    if fold.kind == "CLOSED" and fold.verified:
        return FusedVerdict("closed-form", f"loop closed to {fold.closed_form} (O(1))", fold.proof, "fold engine")
    if fold.kind == "NO_STRUCTURE":
        return FusedVerdict("no-structure", "loop does not close (honest)", fold.detail, "fold engine")

    # 3. sort-shaped → bug-localize or prove unbounded
    coq = fusion.coq_inject(hfn)
    if coq.proven:
        return FusedVerdict("unbounded-proven", f"Coq proves {coq.proven} for ALL lengths", coq.detail, "Coq")
    if "FAILS" in coq.detail:                       # sort that fails bounded → localize it
        r = typeB.analyze(source, filename, do_fix=False)
        return FusedVerdict("bug-localized", f"suspect {r.top1}@{r.top1_lines}",
                            f"violated {r.violated}; {r.digit_certificate}", "B (property/Bayes/Caesar)")

    # 4. fall back to B localization
    r = typeB.analyze(source, filename, do_fix=False)
    if r.violated:
        return FusedVerdict("bug-localized", f"suspect {r.top1}@{r.top1_lines}",
                            f"violated {r.violated}; {r.digit_certificate}", "B (property/Bayes/Caesar)")
    return FusedVerdict("no-structure", "no spec, no fold, no property violation", r.detail, "B")


# ----------------------------------------------------------------- D4.2 measurement corpus
import time  # noqa: E402

CORPUS = [
    ("sum_k", "def f(n):\n s=0\n for i in range(1,n+1):\n  s+=i\n return s\n", "foldable"),
    ("sum_k2", "def f(n):\n s=0\n for i in range(1,n+1):\n  s+=i*i\n return s\n", "foldable"),
    ("sum_k3", "def f(n):\n s=0\n for i in range(1,n+1):\n  s+=i*i*i\n return s\n", "foldable"),
    ("recurrence", "def f(n):\n s=1\n for i in range(1,n+1):\n  s=s*31+i\n return s\n", "not-foldable"),
    ("sum_spec_ok", "# ensures result == n*(n+1)/2\ndef f(n):\n s=0\n for i in range(1,n+1):\n  s+=i\n return s\n", "spec"),
    ("sum_spec_bad", "# ensures result == n*n\ndef f(n):\n s=0\n for i in range(1,n+1):\n  s+=i\n return s\n", "spec"),
    ("sort_ok", "def f(a):\n b=list(a)\n for i in range(len(b)):\n  for j in range(len(b)-1):\n   if b[j]>b[j+1]:\n    b[j],b[j+1]=b[j+1],b[j]\n return b\n", "sort"),
    ("sort_bug", "def f(a):\n b=list(a)\n for i in range(len(b)):\n  for j in range(len(b)-1):\n   if b[j]<b[j+1]:\n    b[j],b[j+1]=b[j+1],b[j]\n return b\n", "sort"),
]


@dataclass
class FusionMeasurement:
    rows: list
    fold_closed: int
    fold_attempted: int
    z3_verified: int
    z3_refuted: int
    coq_unbounded: int
    bugs_localized: int
    avg_s: float

    def fold_close_rate(self):
        return self.fold_closed / self.fold_attempted if self.fold_attempted else 0.0


def measure_fusion() -> FusionMeasurement:
    rows = []
    fold_closed = fold_attempted = z3v = z3r = coq = bugs = 0
    t0 = time.perf_counter()
    for name, src, klass in CORPUS:
        v = analyze_fused(src, name + ".py")
        rows.append((name, klass, v.category, v.headline))
        if klass == "foldable":
            fold_attempted += 1
            if v.category == "closed-form":
                fold_closed += 1
        z3v += int(v.category == "verified")
        z3r += int(v.category == "refuted")
        coq += int(v.category == "unbounded-proven")
        bugs += int(v.category == "bug-localized")
    return FusionMeasurement(rows, fold_closed, fold_attempted, z3v, z3r, coq, bugs,
                             (time.perf_counter() - t0) / len(CORPUS))
