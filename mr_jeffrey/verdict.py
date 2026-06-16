"""
HARAN v16 Part B · STAGE B7 (verdict) — category-distinct confidence (NEVER mixed).
===================================================================================
Three kinds of finding, three kinds of confidence, kept strictly apart (capability_map.py discipline +
the v16 rule "종류별 확신도 안 섞기"):

  • crash / safety   ~99%  — method: traceback (ground-truth line) or sound abstract interpretation;
  • performance      ~99%  — method: MEASURED profile share ("f is X% of runtime");
  • correctness      top-k + PROVEN probability (B5 narrowing + B6 digit certificate). NOT 100% on one
                            line — that is Rice-undecidable; we give ranked suspects with a proven bound.

Borrowing a crash's 99% to dress up a correctness claim is a lie — the fields are separate, each carries
its own method string, and `mixed()` verifies they were not blended.
"""
from __future__ import annotations

from dataclasses import dataclass, field
from typing import List, Optional

import hir
import properties as PR
import property_test as PT
import narrow as NA
import crash_safety as CS


@dataclass
class CategoryVerdict:
    correctness: dict       # {top1, top5, proven_digit, method}
    crash_safety: dict      # {finding, confidence, method}
    performance: dict       # {hotspot, pct, method}  (optional)

    def mixed(self) -> bool:
        """True if any category's confidence/method leaked into another (must always be False)."""
        methods = {self.correctness.get("method"), self.crash_safety.get("method"),
                   self.performance.get("method")}
        # correctness must NOT use a trace/profile method; crash must NOT use 'probabilistic top-k'
        if "traceback" in (self.correctness.get("method") or "") or "profile" in (self.correctness.get("method") or ""):
            return True
        if "top-k" in (self.crash_safety.get("method") or ""):
            return True
        return False

    def render(self) -> str:
        c, s, p = self.correctness, self.crash_safety, self.performance
        out = ["category-distinct verdict (confidences NOT mixed):",
               f"   correctness : top1={c.get('top1')} top5={c.get('top5')}  "
               f"digit≤{c.get('proven_digit')}  [{c.get('method')}]",
               f"   crash/safety: {s.get('finding')}  conf~{s.get('confidence')}  [{s.get('method')}]"]
        if p.get("hotspot"):
            out.append(f"   performance : {p.get('hotspot')} = {p.get('pct')}% of runtime  [{p.get('method')}]")
        return "\n".join(out)


def assess(hfn: hir.HFunction, n: int = 400, proven_digit: Optional[str] = None) -> CategoryVerdict:
    # correctness (probabilistic, top-k) — from narrowing
    m = NA.narrow(hfn, n_random=n)
    correctness = {"top1": m.top1, "top5": m.top5,
                   "proven_digit": proven_digit or "see B6 certificate",
                   "method": "property-based probabilistic narrowing (top-k + B6 proven prob)"}
    # crash / safety — from trace + abstract interpretation
    sv = CS.analyze_safety(hfn, PT.gen_int_lists(n))
    crash_safety = {"finding": sv.summary(), "confidence": round(sv.confidence, 2), "method": sv.method}
    performance = {}
    return CategoryVerdict(correctness, crash_safety, performance)
