"""v16 Part B · B4 tests — property-violation → operation mapping. Run: python3 test_b4.py

B4.1 violated property → responsible operation (ordered→compare/swap, length/elem→pop/append/...).
B4.2 likelihood ratio computed (an op implicated by more violated properties gets a higher LR).
"""
import sys

import hir
import properties as PR
import property_test as PT
import fault_map as FM

PASS, FAIL, SKIP = [], [], []


def check(n, c, d=""):
    (PASS if c else FAIL).append(n)
    print(f"  [{'PASS' if c else 'FAIL'}] {n}" + (f" — {d}" if d and not c else ""))


CORRECT = ("def sort1(xs):\n a=list(xs)\n for i in range(len(a)):\n  for j in range(len(a)-1):\n"
           "   if a[j] > a[j+1]:\n    a[j],a[j+1]=a[j+1],a[j]\n return a\n")
BUG_CMP = CORRECT.replace("a[j] > a[j+1]", "a[j] < a[j+1]")   # comparison bug @ line 5
BUG_DROP = "def sort1(xs):\n a=sorted(xs)\n if len(a)>1:\n  a.pop()\n return a\n"   # drop @ line 4


def _map(src):
    f = hir.to_hir(src, "s.py").module.fn("sort1")
    props = PR.extract_properties(f)
    r = PT.run(f, n_random=1000)
    violated = [p for p in props if p.name in r.violated_properties()]
    return FM.map_violations(f, violated), r.violated_properties()


def violation_to_operation_mapped():
    fm_cmp, _ = _map(BUG_CMP)
    fm_drop, _ = _map(BUG_DROP)
    # comparison bug → `compare` is a top suspect, at the actual bug line (5)
    cmp_top = fm_cmp.top(2)
    cmp_ok = any(s.op_kind == "compare" and 5 in s.lines for s in cmp_top)
    # drop bug → `pop` is THE top suspect, at the actual bug line (4)
    drop_top = fm_drop.top(1)[0]
    drop_ok = drop_top.op_kind == "pop" and 4 in drop_top.lines
    check("violation_to_operation_mapped", cmp_ok and drop_ok,
          f"cmp top={[s.op_kind for s in cmp_top]} drop top={drop_top.op_kind}@{drop_top.lines}")
    print(f"      → comparison bug → compare@line5 among top suspects; drop bug → pop@line4 is THE top "
          f"suspect. Violated properties accuse the operation that caused them.")


def likelihood_ratio_computed():
    fm_drop, _ = _map(BUG_DROP)
    pop = next(s for s in fm_drop.suspects if s.op_kind == "pop")
    unrelated = [s for s in fm_drop.suspects if s.lr == 1.0]
    # pop is implicated by ≥2 violated properties → LR = 16^2 = 256; unrelated ops stay at LR 1
    ok = pop.lr >= 256 and len(pop.implicated_by) >= 2 and len(unrelated) >= 1
    check("likelihood_ratio_computed", ok, f"pop LR={pop.lr} by={pop.implicated_by}")
    print(f"      → pop LR={pop.lr:.0f} (implicated by {pop.implicated_by}); unrelated ops LR=1. "
          f"More violated properties implicating an op ⇒ higher likelihood ratio (B5 multiplies these).")


if __name__ == "__main__":
    print("v16 Part B · B4 — property-violation → operation mapping")
    violation_to_operation_mapped(); likelihood_ratio_computed()
    print(f"\nB4: {len(PASS)} passed, {len(FAIL)} failed, {len(SKIP)} skipped")
    sys.exit(1 if FAIL else 0)
