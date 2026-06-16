"""v16 Part B · B7 tests — category-distinct verdicts. Run: python3 test_b7.py

B7.1 crash/safety (~99%): traceback pins the line; sound interval AI flags div-by-zero.
B7.2 performance (~99%): measured profile hotspot ("f is X% of runtime").
B7.3 confidences by category are DISTINCT and never mixed (crash 99% ≠ correctness top-k).
"""
import sys

import hir
import property_test as PT
import crash_safety as CS
import perf
import verdict

PASS, FAIL, SKIP = [], [], []


def check(n, c, d=""):
    (PASS if c else FAIL).append(n)
    print(f"  [{'PASS' if c else 'FAIL'}] {n}" + (f" — {d}" if d and not c else ""))


CRASH = "def f(xs):\n    a=list(xs)\n    return a[len(a)]\n"          # IndexError @ line 3
DZ = "def g(n):\n    d = 0\n    return n / d\n"                       # div-by-zero @ line 3
MOD = ("def slow(xs):\n    s=0\n    for i in range(2000):\n        for j in range(len(xs)):\n"
       "            s+=j\n    return s\ndef fast(xs):\n    return len(xs)\n"
       "def entry(xs):\n    return slow(xs)+fast(xs)\n")
BUG_CMP = ("def sort1(xs):\n a=list(xs)\n for i in range(len(a)):\n  for j in range(len(a)-1):\n"
           "   if a[j] < a[j+1]:\n    a[j],a[j+1]=a[j+1],a[j]\n return a\n")


def _fn(src, name):
    return hir.to_hir(src, "x.py").module.fn(name)


def crash_safety_abstract_interp():
    sv = CS.analyze_safety(_fn(CRASH, "f"), PT.gen_int_lists(40))
    crash_ok = sv.crashes and sv.crashes[0].exc_type == "IndexError" and sv.crashes[0].line == 3 and sv.confidence >= 0.99
    warns = CS.static_safety(_fn(DZ, "g"))
    static_ok = any(w.kind == "div-by-zero" and w.line == 3 for w in warns)
    check("crash_safety_abstract_interp", crash_ok and static_ok, f"crash={sv.summary()} static={warns}")
    print(f"      → {sv.summary()}; interval AI flags div-by-zero@3 (sound). Crash line is ground truth.")


def performance_hotspot():
    hs = perf.profile_hotspots(MOD, "entry", [[1, 2, 3, 4, 5]] * 20)
    top = hs[0]
    ok = top.func == "slow" and top.pct > 50    # measured: slow dominates self-time
    check("performance_hotspot", ok, f"top={top.func} self={top.pct:.0f}%")
    print(f"      → measured hotspot: {top.func} = {top.pct:.0f}% of runtime (self time). "
          f"Measurement, ~99% — NOT a probability digit.")


def confidence_by_category_distinct():
    v = verdict.assess(_fn(BUG_CMP, "sort1"), n=200)
    not_mixed = not v.mixed()
    # correctness uses probabilistic top-k; crash uses trace/AI — different methods, never blended
    corr_method = v.correctness["method"]
    crash_method = v.crash_safety["method"]
    distinct = ("top-k" in corr_method) and ("top-k" not in crash_method) and ("traceback" not in corr_method)
    ok = not_mixed and distinct
    check("confidence_by_category_distinct", ok, f"mixed={v.mixed()}")
    print("      " + v.render().replace("\n", "\n      "))
    print(f"      → mixed()={v.mixed()}: correctness (top-k+proof) and crash/safety (trace ~99%) carry "
          f"SEPARATE confidences/methods — never borrowed (a crash's 99% can't dress up correctness).")


if __name__ == "__main__":
    print("v16 Part B · B7 — category-distinct verdicts")
    crash_safety_abstract_interp(); performance_hotspot(); confidence_by_category_distinct()
    print(f"\nB7: {len(PASS)} passed, {len(FAIL)} failed, {len(SKIP)} skipped")
    sys.exit(1 if FAIL else 0)
