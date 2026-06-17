"""
HARAN v20 Part P · STAGE P6 — integration + per-class verdict + honesty labels.
==============================================================================
Routes code to the right analyzer per bug class and tags every finding with its class, technique, and an
HONEST label: SOUND (proof), UNDER-APPROX (false-positives 0, may miss), or HEURISTIC (detection, not
proof). Classes that need a spec (access-control / business-logic — Rice) or are micro-architectural
(Spectre/cache) or predictive (WCP race) are explicitly listed as NOT covered — stated, not pretended.
"""
from __future__ import annotations

from dataclasses import dataclass, field
from typing import List, Optional

import taint
import constant_time as CT
import vector_clock as VCK
import isl

# class → (label, technique, scope note)
LABELS = {
    "injection": ("SOUND", "taint/IFDS + Z3 path refine", "sound modulo aliasing / call-graph"),
    "constant-time": ("SOUND", "relational 2-safety + Z3", "timing/branch/data-access; Spectre DEFER"),
    "race": ("SOUND-FOR-TRACE", "vector clocks (happens-before)", "dynamic: the run trace's race is real"),
    "use-after-free": ("UNDER-APPROX", "Incorrectness Separation Logic", "false-positives 0; incomplete"),
    "double-free": ("UNDER-APPROX", "Incorrectness Separation Logic", "false-positives 0; incomplete"),
    "multi-bug": ("SOUND", "MaxSAT minimal diagnosis", "minimal #faulty statements + suspects"),
    "termination": ("SOUND", "ranking-function synthesis (Z3)", "linear; else termination UNKNOWN"),
}

NOT_COVERED = {
    "access-control": "needs a policy spec (Rice undecidable) → HEURISTIC at best; not done",
    "business-logic": "needs a spec (Rice) → not done",
    "spectre / cache": "micro-architectural side channel → DEFER",
    "predictive-race": "WCP / M2 unobserved races → DEFER (dynamic only here)",
    "cve-match": "database lookup, not mathematics → not done",
}


@dataclass
class Finding:
    bug_class: str
    label: str
    technique: str
    location: str
    detail: str


def analyze_v20(python: Optional[str] = None, c: Optional[str] = None,
                trace: Optional[List] = None, n_threads: int = 2,
                secret_params=None) -> List[Finding]:
    findings: List[Finding] = []
    if python:
        for inj in taint.taint_analyze(python, "x.py"):
            if inj.feasible:
                lab, tech, _ = LABELS["injection"]
                findings.append(Finding("injection", lab, tech, f"L{inj.sink_line}",
                                        f"{inj.sink_fn}({inj.tainted_var}) via {inj.path_lines}"))
        for leak in CT.analyze_constant_time(python, None, secret_params):
            if leak.relational_confirmed:
                lab, tech, _ = LABELS["constant-time"]
                findings.append(Finding("constant-time", lab, tech, f"L{leak.line}",
                                        f"{leak.kind}: {leak.expr}"))
    if c:
        for bug in isl.analyze_c(c):
            lab, tech, _ = LABELS[bug.kind]
            findings.append(Finding(bug.kind, lab, tech, f"L{bug.line}", f"{bug.note} ({bug.ptr})"))
    if trace:
        races, _ = VCK.analyze_trace(trace, n_threads)
        lab, tech, _ = LABELS["race"]
        for r in races:
            findings.append(Finding("race", lab, tech, f"event {r.a.idx}/{r.b.idx}", r.note))
    return findings


def honesty_table() -> dict:
    """The per-class honesty table: covered classes with labels + explicitly NOT-covered classes."""
    return {"covered": {k: {"label": v[0], "technique": v[1], "scope": v[2]} for k, v in LABELS.items()},
            "not_covered": dict(NOT_COVERED)}


def mixed(findings: List[Finding]) -> bool:
    """True if any class carried the WRONG label kind (confidence kinds must not be mixed)."""
    for f in findings:
        expect = LABELS.get(f.bug_class, (None,))[0]
        if f.label != expect:
            return True
    return False
