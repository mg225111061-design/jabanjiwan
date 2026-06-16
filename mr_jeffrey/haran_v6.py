"""
STAGE U5 (v6) — integrated verified-approximation + conquest ratio (v3 vs v6) + error-type breakdown.
====================================================================================================
Routes unstructured/approx work through: U1 Prony (deterministic, Z3) · U2 Compressed Sensing (runtime
residual) · U3 sketches (probabilistic, Caesar/HeyVL or TESTED) · U4 exact-required rejection.

Four error verdicts, never merged:
  PROVEN-BOUND (deterministic, Z3) · PROVEN-BOUND (runtime, per-execution) ·
  PROVEN-BOUND (probabilistic, Caesar) · TESTED-BOUND · REJECTED-EXACT.
"""
from __future__ import annotations

from dataclasses import dataclass


@dataclass
class V6Task:
    name: str
    verdict: str        # PROVEN-BOUND | TESTED-BOUND | REJECTED-EXACT
    error_kind: str     # deterministic(Z3) | runtime(per-exec) | probabilistic(Caesar) | exact
    detail: str


def assess():
    tasks = []
    # quantile (v3) — deterministic, Z3
    from approx_lib import approx_quantile_conquest
    q = approx_quantile_conquest()
    tasks.append(V6Task("quantile (v3)", q.kind, "deterministic(Z3)", q.proof))
    # distinct/KMV (v3 → v6 Caesar bridge) — probabilistic
    from caesar_bridge import verify_probabilistic
    d = verify_probabilistic("kmv")
    tasks.append(V6Task("distinct/KMV", d.verdict, "probabilistic(Caesar)", d.detail))
    # Prony (v6) — deterministic, Z3
    from prony import discharge_prony
    p = discharge_prony()
    tasks.append(V6Task("Prony (v6)", p.verdict, "deterministic(Z3)", p.detail))
    # Compressed Sensing (v6) — runtime residual
    from compressed_sensing import recover_and_certify
    c = recover_and_certify()
    tasks.append(V6Task("CompressedSensing (v6)", c.verdict, "runtime(per-exec)", c.detail))
    # payment — exact-required rejection
    from approx_router import route
    pay = route("payment_total", "payment")
    tasks.append(V6Task("payment", pay.decision, "exact", pay.reason))
    return tasks


def conquest_v3_vs_v6():
    tasks = assess()
    approximated = [t for t in tasks if t.verdict != "REJECTED-EXACT"]
    v6_proven = sum(1 for t in approximated if t.verdict == "PROVEN-BOUND")
    # v3 HISTORICAL baseline (fixed): quantile PROVEN-BOUND, distinct/KMV TESTED-BOUND → 1/2.
    # (Not recomputed live — v3 had no Caesar; this is what v3 actually was.)
    v3_proven, v3_total = 1, 2
    return {
        "tasks": tasks,
        "v3_pct": round(100 * v3_proven / v3_total),
        "v6_pct": round(100 * v6_proven / len(approximated)) if approximated else 0,
        "v3_proven": v3_proven, "v3_total": v3_total,
        "v6_proven": v6_proven, "v6_total": len(approximated),
    }


def error_breakdown():
    tasks = assess()
    proven = [t for t in tasks if t.verdict == "PROVEN-BOUND"]
    return {
        "deterministic": [t.name for t in proven if "deterministic" in t.error_kind],
        "runtime": [t.name for t in proven if "runtime" in t.error_kind],
        "probabilistic": [t.name for t in proven if "probabilistic" in t.error_kind],
        "tested": [t.name for t in tasks if t.verdict == "TESTED-BOUND"],
        "rejected": [t.name for t in tasks if t.verdict == "REJECTED-EXACT"],
    }


def render():
    r = conquest_v3_vs_v6()
    out = ["HARAN v6 — verified approximation, conquest ratio + error types"]
    for t in r["tasks"]:
        out.append(f"   · {t.name:24s} {t.verdict:14s} [{t.error_kind}]")
    out.append(f"   ── CONQUEST (PROVEN-BOUND / approximated): v3 {r['v3_pct']}% ({r['v3_proven']}/{r['v3_total']}) "
               f"→ v6 {r['v6_pct']}% ({r['v6_proven']}/{r['v6_total']})  = +{r['v6_pct'] - r['v3_pct']}p")
    b = error_breakdown()
    out.append(f"   ── ERROR TYPES: deterministic(Z3)={b['deterministic']} · runtime={b['runtime']} · "
               f"probabilistic(Caesar)={b['probabilistic']} · TESTED={b['tested']} · REJECTED={b['rejected']}")
    return "\n".join(out)


if __name__ == "__main__":
    print(render())
