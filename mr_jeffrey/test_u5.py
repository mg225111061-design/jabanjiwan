"""STAGE U5 tests (v6) — integration + conquest ratio v3 vs v6. Run: python3 test_u5.py"""
import haran_v6
from z3_adapter import z3_available
PASS, FAIL, SKIP = [], [], []
def check(n, c, d=""):
    (PASS if c else FAIL).append(n); print(f"  [{'PASS' if c else 'FAIL'}] {n}" + (f" — {d}" if d and not c else ""))
def skip(n, w): SKIP.append(n); print(f"  [SKIP] {n} — {w}")

def integrated_approx_routing():
    tasks = haran_v6.assess()
    kinds = {t.name: t.verdict for t in tasks}
    ok = len(tasks) == 5 and any(t.verdict == "REJECTED-EXACT" for t in tasks)
    check("integrated_approx_routing", ok, str(kinds))

def conquest_ratio_v3_vs_v6_measured():
    if not z3_available(): skip("conquest_ratio_v3_vs_v6_measured", "Z3 absent (Prony/quantile need it)"); return
    r = haran_v6.conquest_v3_vs_v6()
    print("\n" + haran_v6.render())
    # v6 conquest must exceed v3 (Prony + CS add deterministic/runtime PROVEN-BOUNDs)
    ok = r["v6_pct"] > r["v3_pct"] and r["v6_proven"] >= 3
    check("conquest_ratio_v3_vs_v6_measured", ok, f"v3={r['v3_pct']}% v6={r['v6_pct']}%")

def error_type_breakdown_distinct():
    if not z3_available(): skip("error_type_breakdown_distinct", "Z3 absent"); return
    b = haran_v6.error_breakdown()
    # the 4 kinds are distinct: deterministic (Z3) + runtime have proofs; probabilistic empty (Caesar
    # blocked) → stays TESTED; exact rejected. None merged.
    ok = (len(b["deterministic"]) >= 2 and len(b["runtime"]) >= 1
          and len(b["probabilistic"]) == 0 and len(b["tested"]) >= 1 and len(b["rejected"]) >= 1)
    check("error_type_breakdown_distinct", ok, str({k: len(v) for k, v in b.items()}))
    print(f"      → deterministic(Z3)={b['deterministic']}")
    print(f"      → runtime={b['runtime']}  probabilistic(Caesar)={b['probabilistic'] or '∅ (BLOCKED)'}")
    print(f"      → TESTED={b['tested']}  REJECTED={b['rejected']}")

def showcase_all_approx_correct():
    if not z3_available(): skip("showcase_all_approx_correct", "Z3 absent"); return
    t = {x.name: x for x in haran_v6.assess()}
    ok = (t["Prony (v6)"].verdict == "PROVEN-BOUND" and t["Prony (v6)"].error_kind.startswith("deterministic")
          and t["CompressedSensing (v6)"].verdict == "PROVEN-BOUND" and "runtime" in t["CompressedSensing (v6)"].error_kind
          and t["distinct/KMV"].verdict == "TESTED-BOUND"          # Caesar blocked → honest TESTED
          and t["payment"].verdict == "REJECTED-EXACT"
          and t["quantile (v3)"].verdict == "PROVEN-BOUND")
    check("showcase_all_approx_correct", ok, str({k: v.verdict for k, v in t.items()}))
    print("      → Prony:PROVEN(Z3) · CS:PROVEN(runtime) · KMV:TESTED(Caesar BLOCKED) · "
          "payment:REJECTED · quantile:PROVEN")

if __name__ == "__main__":
    print("STAGE U5 — integration + conquest ratio v3 vs v6")
    integrated_approx_routing()
    conquest_ratio_v3_vs_v6_measured()
    error_type_breakdown_distinct()
    showcase_all_approx_correct()
    print(f"\nStage U5: {len(PASS)} passed, {len(FAIL)} failed, {len(SKIP)} skipped")
    import sys; sys.exit(1 if FAIL else 0)
