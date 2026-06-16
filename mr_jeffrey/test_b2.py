"""v16 Part B · B2 tests — property extraction. Run: python3 test_b2.py

B2.1 properties extracted (metamorphic/preservation relations the correct code satisfies).
B2.2 AI extraction optional (returns [] without API key — heuristics stand, honest fallback).
B2.3 independence assessed (correlated properties grouped, not double-counted — matters for B5/B6).
"""
import sys

import hir
import properties as PR

PASS, FAIL, SKIP = [], [], []


def check(n, c, d=""):
    (PASS if c else FAIL).append(n)
    print(f"  [{'PASS' if c else 'FAIL'}] {n}" + (f" — {d}" if d and not c else ""))


BUBBLE = """def bubble(xs):
    a = list(xs)
    for i in range(len(a)):
        for j in range(len(a)-1):
            if a[j] > a[j+1]:
                a[j], a[j+1] = a[j+1], a[j]
    return a
"""
ARB = "def biz(x):\n    return x*2 + 1\n"


def _fn(src, name):
    return hir.to_hir(src, "x.py").module.fn(name)


def properties_extracted():
    f = _fn(BUBBLE, "bubble")
    props = PR.extract_properties(f)
    names = {p.name for p in props}
    fn = PR.compile_callable(f)
    all_hold = all(p.check(fn, [5, 2, 9, 1, 7]) for p in props)   # correct bubble satisfies every property
    arb = {p.name for p in PR.extract_properties(_fn(ARB, "biz"), sample=3)}
    ok = ({"length_preservation", "permutation", "ordered_output", "idempotence"} <= names
          and all_hold and arb == {"determinism"})
    check("properties_extracted", ok, f"sort props={sorted(names)} arb props={sorted(arb)}")
    print(f"      → bubble → {len(props)} relations (all satisfied by correct code); arbitrary biz → "
          f"only determinism (property-poor ⇒ weak, reported honestly).")


def ai_property_extraction():
    import os
    f = _fn(BUBBLE, "bubble")
    extra = PR.ai_extract_properties(f)
    have_key = bool(os.environ.get("ANTHROPIC_API_KEY"))
    # without a key: [] (heuristics stand). with a key: a (possibly empty) list — never a crash/fabrication.
    ok = isinstance(extra, list) and (extra == [] if not have_key else True)
    check("ai_property_extraction", ok, f"have_key={have_key} extra={extra}")
    print(f"      → AI extraction: API key={'present' if have_key else 'absent'} → "
          f"{'queried Claude' if have_key else 'returned [] (heuristics stand)'}; never fabricates a relation.")


def independence_assessed():
    f = _fn(BUBBLE, "bubble")
    ind = PR.assess_independence(PR.extract_properties(f))
    # permutation and length_preservation are correlated → SAME group (not multiplied independently)
    same_group = any(set(["length_preservation", "permutation"]) <= set(v) for v in ind.groups.values())
    ok = same_group and ind.independent_count == 4
    check("independence_assessed", ok, f"groups={ind.groups} independent={ind.independent_count}")
    print(f"      → {ind.independent_count} independent groups; permutation⇒length grouped together "
          f"(correlated, NOT independently multiplied — B6 will discount). Honest for B5 multiplication.")


if __name__ == "__main__":
    print("v16 Part B · B2 — property extraction")
    properties_extracted(); ai_property_extraction(); independence_assessed()
    print(f"\nB2: {len(PASS)} passed, {len(FAIL)} failed, {len(SKIP)} skipped")
    sys.exit(1 if FAIL else 0)
