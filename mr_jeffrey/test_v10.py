"""v10 tests (S1-S3) — ceilings + capability map + completion. Run: python3 test_v10.py"""
import os
import ceilings, capability_map
PASS, FAIL = [], []
def check(n, c, d=""):
    (PASS if c else FAIL).append(n); print(f"  [{'PASS' if c else 'FAIL'}] {n}" + (f" — {d}" if d and not c else ""))

def ceilings_documented_with_rationale():
    cs = ceilings.all_ceilings()
    ok = (len(cs) >= 8 and all(c.rationale and c.where for c in cs)
          and any(c.kind == "FUNDAMENTAL" for c in cs))
    check("ceilings_documented_with_rationale", ok, f"{len(cs)} ceilings, all with rationale+where")
    print(f"      → {len(ceilings.FUNDAMENTAL)} FUNDAMENTAL + {len(ceilings.TOOL_OR_ENGINEERING)} tool/engineering, each with rationale")

def full_capability_map():
    caps = capability_map.CAPABILITIES
    areas = {a for a, *_ in caps}
    need = {"fold (closed-form)", "unstructured acceleration", "verified approximation",
            "AI write→verify→fix", "native codegen", "verification depth"}
    check("full_capability_map", need.issubset(areas), f"areas={sorted(areas)}")

def labels_consistent_systemwide():
    r = capability_map.labels_consistent()
    check("labels_consistent_systemwide", r["consistent"], str(r["issues"]))
    print(f"      → buckets={r['buckets']}")
    print(f"      → error labels={r['error_labels']}  PROVEN kinds={r['proven_kinds']} (no merging)")

def pre_product_completion_documented():
    p = os.path.join(os.path.dirname(os.path.abspath(__file__)), "HARAN_FINAL.md")
    txt = open(p).read() if os.path.isfile(p) else ""
    ok = ("pre-product completion" in txt.lower() and "FUNDAMENTAL" in txt
          and "what works, works; what doesn't, we know why" in txt)
    check("pre_product_completion_documented", ok)
    print("      → HARAN_FINAL.md: pre-product completion declared (works + ceilings + honest meaning)")

if __name__ == "__main__":
    print("v10 — ceilings + capability map + pre-product completion")
    print("[S1]"); ceilings_documented_with_rationale()
    print("[S2]"); full_capability_map(); labels_consistent_systemwide()
    print("[S3]"); pre_product_completion_documented()
    print("\n" + ceilings.render())
    print("\n" + capability_map.render())
    print(f"\nv10: {len(PASS)} passed, {len(FAIL)} failed")
    import sys; sys.exit(1 if FAIL else 0)
