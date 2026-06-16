"""STAGE U3 tests (v6) — Caesar/HeyVL bridge (probabilistic) + honest BLOCKED. Run: python3 test_u3.py"""
from caesar_bridge import caesar_available, to_heyvl, verify_probabilistic
from prony import discharge_prony, _bin as prony_bin
from z3_adapter import z3_available
PASS, FAIL, SKIP = [], [], []
def check(n, c, d=""):
    (PASS if c else FAIL).append(n); print(f"  [{'PASS' if c else 'FAIL'}] {n}" + (f" — {d}" if d and not c else ""))
def skip(n, w): SKIP.append(n); print(f"  [SKIP] {n} — {w}")

def caesar_installed_or_blocked_honestly():
    avail = caesar_available()
    r = verify_probabilistic("kmv")
    if avail:
        ok = r.verdict == "PROVEN-BOUND" and r.error_kind == "probabilistic"
    else:
        # honest BLOCKED: no fake PROVEN; stays TESTED-BOUND
        ok = r.verdict == "TESTED-BOUND" and "BLOCKED" in r.detail and "NOT installed" in r.detail
    check("caesar_installed_or_blocked_honestly", ok, str(r.verdict))
    print(f"      → Caesar available: {bool(avail)}; verdict: {r.verdict}")
    print(f"        {r.detail}")

def caesar_bridge_hll_expectation():
    cm = to_heyvl("count_min"); kmv = to_heyvl("kmv")
    ok = ("coproc" in cm and "pre" in cm and "@ghost" in cm and "E[" in cm
          and "coproc" in kmv and "@ghost" in kmv and "variance" in kmv.lower())
    check("caesar_bridge_hll_expectation", ok, "HeyVL expectation specs generated")
    print("      → bridge generates HeyVL expectation specs (Count-Min E[err]≤‖a‖₁/w ; KMV relvar≤c/m):")
    for line in cm.splitlines()[:6]:
        print(f"        | {line}")

def probabilistic_tested_to_proven_measured():
    # corpus of probabilistic sketches; how many upgraded TESTED→PROVEN via Caesar?
    sketches = ["kmv", "count_min", "hll"]
    results = [verify_probabilistic(s) for s in sketches]
    upgraded = sum(1 for r in results if r.verdict == "PROVEN-BOUND")
    total = len(sketches)
    if caesar_available():
        ok = upgraded >= 1
    else:
        ok = upgraded == 0 and all(r.verdict == "TESTED-BOUND" for r in results)   # honest: none upgraded
    check("probabilistic_tested_to_proven_measured", ok, f"upgraded {upgraded}/{total}")
    print(f"      → TESTED→PROVEN upgrade: {upgraded}/{total} "
          f"({'Caesar present' if caesar_available() else 'Caesar BLOCKED → 0 upgraded, honest'})")

def deterministic_vs_probabilistic_proven_distinct():
    prob = verify_probabilistic("kmv")
    if not (prony_bin() and z3_available()):
        skip("deterministic_vs_probabilistic_proven_distinct", "needs prony+Z3"); return
    det = discharge_prony()   # deterministic PROVEN-BOUND (Z3)
    # the two are DIFFERENT kinds and must never be merged/relabeled
    ok = (det.error_kind == "deterministic" and det.verdict == "PROVEN-BOUND"
          and prob.error_kind == "probabilistic" and prob.error_kind != det.error_kind)
    check("deterministic_vs_probabilistic_proven_distinct", ok,
          f"prony={det.verdict}({det.error_kind}); kmv={prob.verdict}({prob.error_kind})")
    print(f"      → deterministic Prony: {det.verdict} ({det.error_kind}, Z3)")
    print(f"      → probabilistic KMV: {prob.verdict} ({prob.error_kind}, Caesar) — distinct kinds, not merged")

if __name__ == "__main__":
    print("STAGE U3 — Caesar/HeyVL bridge (probabilistic)")
    caesar_installed_or_blocked_honestly()
    caesar_bridge_hll_expectation()
    probabilistic_tested_to_proven_measured()
    deterministic_vs_probabilistic_proven_distinct()
    print(f"\nStage U3: {len(PASS)} passed, {len(FAIL)} failed, {len(SKIP)} skipped")
    import sys; sys.exit(1 if FAIL else 0)
