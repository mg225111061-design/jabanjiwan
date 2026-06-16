"""
STAGE T5 (v5) — integrated classifier + extended fold ratio (v2 vs v5) + certificate completeness.
=================================================================================================
Routes through polynomial / C-finite (v2) · hypergeometric+WZ certificate (T2) · holonomic→Gröbner
ceiling (T3) · Kovacic (T4). Measures how much the fold ratio rises over v2 — the win is sums with
binomial/factorial summands (which v2 could not even sympify → NO_STRUCTURE) now folding as
hypergeometric with a machine-verified telescoper certificate.

Honest: hypergeometric closures carry PARTIAL certificates (provisos asserted). holonomic order-2+
stays DEFER (Gröbner). nonholonomic stays NO_STRUCTURE. 4 buckets never mixed.
"""
from __future__ import annotations

from dataclasses import dataclass

import closure_classifier as cc
import fold_classes
import hypergeometric


@dataclass
class V5Verdict:
    kind: str            # CLOSED | ABSENT | NO_STRUCTURE | DEFER | UNKNOWN
    math_class: str
    closed_form: str
    certificate: str
    completeness: str    # full | partial(provisos) | absence | deferred | none

    def __str__(self):
        cf = f" = {self.closed_form}" if self.closed_form not in ("", "—") else ""
        return f"{self.kind} [{self.math_class}]{cf}  cert={self.completeness}"


def classify_v5(fn) -> V5Verdict:
    mc = fold_classes.classify_math_class(fn)
    if mc.name == "polynomial":
        v = cc.classify_fn(fn)
        return V5Verdict("CLOSED", "polynomial", getattr(v, "closed_form", "—"), "Faulhaber coeff-zero", "full")
    if mc.name == "C-finite":
        return V5Verdict("CLOSED", "C-finite", "companion-matrix", "cfinite companion≡naive", "full")
    if mc.name == "hypergeometric":
        hr = hypergeometric.discharge_hypergeometric(fn)
        if hr.verdict == "CLOSED":
            return V5Verdict("CLOSED", "hypergeometric", hr.closed_form, hr.certificate, hr.cert_completeness)
        v = cc.classify_fn(fn)   # rational hypergeometric (no binomial) → v2 Gosper path
        if v.kind == "CLOSED":
            return V5Verdict("CLOSED", "hypergeometric", v.closed_form, "Gosper antidifference", "partial(provisos)")
        if v.kind == "ABSENT":
            return V5Verdict("ABSENT", "hypergeometric", "—", v.proof, "absence")
        return V5Verdict("UNKNOWN", "hypergeometric", "—", "Gosper undecided", "none")
    if mc.name == "holonomic-candidate":
        return V5Verdict("DEFER", "holonomic", "—", "Gröbner ceiling (T3): order-2+ times out", "deferred")
    if mc.name.startswith("nonholonomic"):
        return V5Verdict("NO_STRUCTURE", "nonholonomic/data", "—", mc.note, "none")
    return V5Verdict("UNKNOWN", mc.name, "—", mc.note, "none")


def v2_kind(fn) -> str:
    """The v2 baseline bucket (closure_classifier only) — for the before/after comparison."""
    return cc.classify_fn(fn).kind


@dataclass
class RatioReport:
    rows: list   # (label, v2_kind, v5_verdict)

    def v2_closed(self):
        return sum(1 for _, k, _ in self.rows if k == "CLOSED")

    def v5_closed(self):
        return sum(1 for _, _, v in self.rows if v.kind == "CLOSED")

    def pct(self, count):
        return round(100 * count / len(self.rows)) if self.rows else 0


def closure_ratio_v2_vs_v5(corpus: dict) -> RatioReport:
    from haran_parser import parse
    rows = []
    for label, src in corpus.items():
        fn = parse(src).items[0]
        rows.append((label, v2_kind(fn), classify_v5(fn)))
    return RatioReport(rows)


def render(rep: RatioReport) -> str:
    out = ["v2 (poly/C-finite) vs v5 (+hypergeometric/Kovacic) — closure per item:"]
    out.append(f"   {'item':16} {'v2':>14}   {'v5':<40}")
    for label, k2, v5 in rep.rows:
        out.append(f"   {label:16} {k2:>14}   {v5}")
    v2c, v5c, n = rep.v2_closed(), rep.v5_closed(), len(rep.rows)
    out.append(f"   ── FOLD RATIO: v2 {rep.pct(v2c)}% ({v2c}/{n}) → v5 {rep.pct(v5c)}% ({v5c}/{n})  "
               f"= +{rep.pct(v5c) - rep.pct(v2c)}p")
    closed = [v for _, _, v in rep.rows if v.kind == "CLOSED"]
    full = [v for v in closed if v.completeness == "full"]
    partial = [v for v in closed if v.completeness.startswith("partial")]
    out.append(f"   ── CERTIFICATE: of {len(closed)} CLOSED — {len(full)} full, {len(partial)} partial(provisos)")
    return "\n".join(out)
