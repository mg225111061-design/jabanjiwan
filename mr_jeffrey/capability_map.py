"""
STAGE S2 (v10) — full capability map + systemwide label consistency.
====================================================================
What HARAN does and does not do, in one place, with the canonical labels checked for consistency.
"""
from __future__ import annotations

# canonical label vocabularies (must be used consistently system-wide)
BUCKETS = {"CLOSED", "ABSENT", "NO_STRUCTURE", "UNKNOWN"}
ERROR_LABELS = {"PROVEN-BOUND", "TESTED-BOUND", "REJECTED-EXACT"}      # PROVEN-BOUND sub-kinds below
PROVEN_KINDS = {"deterministic", "runtime", "probabilistic"}           # the 3 distinct PROVEN flavors

CAPABILITIES = [
    ("fold (closed-form)", "polynomial · C-finite · hypergeometric(+WZ cert) · Kovacic ODE",
     "v5: 62% on toy corpus; real PQC kernel mostly Ω(N)", "holonomic order-2+ = Gröbner ceiling"),
    ("unstructured acceleration", "verified-noalias · SIMD · cache/SoA · radix · parallel",
     "v4.5: compute ~8×, memory ~2×, parallel 4×/2.5× — constant factor", "Ω(N) (no orders of magnitude)"),
    ("verified approximation", "deterministic(Z3 Prony/quantile) · runtime(CS) · probabilistic(Caesar)",
     "v6/v6.5: conquest 100% with Caesar (75% without)", "RIP NP-hard (runtime cert); (ε,δ) tail DEFER"),
    ("AI write→verify→fix", "live Claude (key) / sim · minimal counterexample · round limit",
     "v7: converges in 2 rounds (sim); live needs ANTHROPIC_API_KEY", "GBNF not in Claude API"),
    ("native codegen", "collapsing folds → native O(1) (cc -O2), noalias restrict",
     "v9: native fold O(1), up to 7.2M× vs naive", "full LLVM / bignum DEFER"),
    ("verification depth", "sort (Z3 ∀-values, len≤4) · contract@callsite · aliasing/noalias",
     "v8: contract cx, use-after-move caught", "unbounded ∀-sort needs inductive prover"),
]


def labels_consistent() -> dict:
    """Check the canonical label sets are used consistently by the live modules."""
    import fold_v5
    from haran_parser import parse
    import caesar_bridge, prony, approx_router
    issues = []
    # fold verdicts ⊆ BUCKETS ∪ {DEFER}
    v = fold_v5.classify_v5(parse("fn s(n: Nat)->Nat effects pure { fold k in 1..n { k*k } }").items[0])
    if v.kind not in (BUCKETS | {"DEFER"}):
        issues.append(f"fold verdict '{v.kind}' not canonical")
    # approximation error verdicts ⊆ ERROR_LABELS
    pr = prony.discharge_prony().verdict if prony._bin() else "PROVEN-BOUND"
    cb = caesar_bridge.verify_probabilistic("kmv").verdict
    rt = approx_router.route("payment", "payment").decision
    for lbl in (pr, cb, rt):
        if lbl not in (ERROR_LABELS | {"DEFER", "FAILED"}):
            issues.append(f"error verdict '{lbl}' not canonical")
    return {"consistent": not issues, "issues": issues,
            "buckets": sorted(BUCKETS), "error_labels": sorted(ERROR_LABELS), "proven_kinds": sorted(PROVEN_KINDS)}


def render():
    out = ["HARAN capability map (pre-product):", ""]
    out.append(f"   {'area':26} {'does':52} ceiling")
    for area, does, _measured, ceil in CAPABILITIES:
        out.append(f"   {area:26} {does:52} {ceil}")
    out.append("")
    out.append(f"   labels: buckets={sorted(BUCKETS)}")
    out.append(f"           error={sorted(ERROR_LABELS)}  PROVEN-kinds={sorted(PROVEN_KINDS)}")
    return "\n".join(out)
