"""v16 Part B · B3 tests — property testing (fast). Run: python3 test_b3.py

B3.1 each property × thousands of inputs (run the code, inspect outputs only).
B3.2 violations recorded (which property broke, on which input — the counterexamples).
B3.3 speed measured (fast because it's execution, not analysis).
"""
import sys

import hir
import property_test as PT

PASS, FAIL, SKIP = [], [], []


def check(n, c, d=""):
    (PASS if c else FAIL).append(n)
    print(f"  [{'PASS' if c else 'FAIL'}] {n}" + (f" — {d}" if d and not c else ""))


CORRECT = """def sort1(xs):
    a=list(xs)
    for i in range(len(a)):
        for j in range(len(a)-1):
            if a[j] > a[j+1]:
                a[j],a[j+1]=a[j+1],a[j]
    return a
"""
BUG_CMP = CORRECT.replace("a[j] > a[j+1]", "a[j] < a[j+1]")          # not sorted (ordered_output)
BUG_DROP = """def sort1(xs):
    a=sorted(xs)
    if len(a)>1:
        a.pop()
    return a
"""


def _fn(src):
    return hir.to_hir(src, "s.py").module.fn("sort1")


def property_tested_many_inputs():
    r = PT.run(_fn(CORRECT), n_random=2000)
    ok = r.violated_properties() == [] and r.tested_inputs >= 2000 and r.checks >= 12000
    check("property_tested_many_inputs", ok, f"checks={r.checks} violated={r.violated_properties()}")
    print(f"      → correct sort: 0 violations over {r.checks} checks ({r.tested_inputs} inputs × "
          f"{r.properties} properties) — outputs inspected, code never scanned.")


def violations_recorded():
    rc = PT.run(_fn(BUG_CMP), n_random=2000)
    rd = PT.run(_fn(BUG_DROP), n_random=2000)
    cmp_ok = rc.violated_properties() == ["ordered_output"] and len(rc.violations["ordered_output"]) > 0
    drop_ok = set(rd.violated_properties()) >= {"length_preservation", "permutation"}
    # the recorded violation carries the actual failing input (counterexample for B4/B8)
    has_cx = isinstance(rc.violations["ordered_output"][0], list)
    ok = cmp_ok and drop_ok and has_cx
    check("violations_recorded", ok, f"cmp→{rc.violated_properties()} drop→{rd.violated_properties()}")
    print(f"      → comparison bug → ONLY ordered_output violated; drop bug → length+permutation. "
          f"Different bugs ⇒ different violated sets (the localization signal). Counterexamples kept.")


def property_test_speed_measured():
    r = PT.run(_fn(CORRECT), n_random=4000)
    fast = r.checks_per_sec() > 50_000           # execution, not analysis → fast
    check("property_test_speed_measured", fast, f"{r.checks_per_sec():.0f} checks/s")
    print(f"      → {r.checks} checks in {r.elapsed_s*1e3:.0f}ms = {r.checks_per_sec():.0f} checks/s. "
          f"Fast because it RUNS the code (no static scan) — seconds, not minutes.")


if __name__ == "__main__":
    print("v16 Part B · B3 — property testing (fast)")
    property_tested_many_inputs(); violations_recorded(); property_test_speed_measured()
    print(f"\nB3: {len(PASS)} passed, {len(FAIL)} failed, {len(SKIP)} skipped")
    sys.exit(1 if FAIL else 0)
