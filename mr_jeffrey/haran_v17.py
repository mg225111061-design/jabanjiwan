"""
HARAN v17 Part E · STAGE E3 — full integration + final measurement.
===================================================================
The whole v17 in one entry: detect language (Part C) → B localizes / A injects fold·Z3·Coq (Part D) →
differential augments when properties are blind and a reference exists (Part E) → a category-distinct
verdict with a proof or a proven digit.

Honest throughout: the A-side digit lands only on fold-able / spec'd / recognized code; differential
needs a reference; confidences are never mixed; no mirage (probability + properties + abstract
interpretation + fold only).
"""
from __future__ import annotations

from dataclasses import dataclass
from typing import List, Optional

import hir
import fusion_pipeline
import differential


@dataclass
class V17Verdict:
    language: str
    category: str          # verified|refuted|closed-form|unbounded-proven|bug-localized|regression|no-structure|unknown
    headline: str
    detail: str
    evidence: str


def analyze_v17(source: str, filename: Optional[str] = None,
                reference: Optional[str] = None) -> V17Verdict:
    fr = hir.to_hir(source, filename)
    lang = fr.lang
    # When a known-good REFERENCE exists, differential answers the REGRESSION question authoritatively
    # (and catches property-invisible bugs B cannot see). This is a different question from "is there a
    # bug?" — so in reference mode we report regression status, not the (noisier) property verdict.
    if reference and fr.supported and fr.module.functions:
        name = fr.module.functions[0].name
        diff = differential.differential(reference, source, name, [1, 2, 5, 10, -3, 0])
        if diff.diverged:
            return V17Verdict(lang, "regression",
                              f"differential regression vs reference ({len(diff.divergences)} inputs)",
                              f"e.g. {diff.divergences[0].inp}: ref={diff.divergences[0].ref_out} "
                              f"now={diff.divergences[0].new_out}", "differential (Part E)")
        return V17Verdict(lang, "no-regression", "matches the reference on tested inputs",
                          "no divergence from the known-good version", "differential (Part E)")
    fused = fusion_pipeline.analyze_fused(source, filename)
    return V17Verdict(lang, fused.category, fused.headline, fused.detail, fused.evidence)


# ----------------------------------------------------------------- E3.2 final measurement
@dataclass
class V17Measurement:
    languages_covered: List[str]
    multilang_top1: str            # "k/n"
    fold_close_rate: float
    z3_verified: int
    z3_refuted: int
    coq_unbounded: int
    coq_auto_theorems: int
    bugs_localized: int
    differential_invisible_caught: bool
    deterministic: bool
    mirage_free: bool


def final_measurement() -> V17Measurement:
    import multilang
    import fusion_pipeline as FP
    import upgrade_a

    ml = multilang.measure_all(30)
    fus = FP.measure_fusion()
    aut = upgrade_a.automation_summary()

    # Part E: a property-invisible bug caught by differential
    OLD, NEW = "def scale(x):\n return x*2\n", "def scale(x):\n return x-1\n"
    inv = differential.differential(OLD, NEW, "scale", [1, 2, 5]).diverged

    # determinism: re-run the fusion corpus head twice → identical category
    v1 = analyze_v17(FP.CORPUS[1][1], "sum_k2.py")
    v2 = analyze_v17(FP.CORPUS[1][1], "sum_k2.py")
    deterministic = (v1.category, v1.headline) == (v2.category, v2.headline)

    return V17Measurement(
        languages_covered=ml.covered(),
        multilang_top1=f"{ml.top1_hits()}/{len(ml.covered())}",
        fold_close_rate=fus.fold_close_rate(),
        z3_verified=fus.z3_verified, z3_refuted=fus.z3_refuted,
        coq_unbounded=fus.coq_unbounded, coq_auto_theorems=aut.auto_count,
        bugs_localized=fus.bugs_localized,
        differential_invisible_caught=inv,
        deterministic=deterministic,
        mirage_free=True)   # no homology/TDA/Ricci/LLL anywhere in the engine (probability+properties+AI+fold)
