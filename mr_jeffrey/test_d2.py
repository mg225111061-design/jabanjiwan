"""v17 Part D · D2 tests — HIR → Z3 correctness injection. Run: python3 test_d2.py

D2.1 user spec (comment) on a general-language loop → Z3 proves it ∀ (via the fold closed form).
D2.2 no spec → properties used as the spec proxy (honest: only as strong as the properties).
D2.3 demo: correct spec → PROVEN; wrong spec → FAILED + counterexample.
"""
import sys

import hir
import fusion

PASS, FAIL, SKIP = [], [], []


def check(n, c, d=""):
    (PASS if c else FAIL).append(n)
    print(f"  [{'PASS' if c else 'FAIL'}] {n}" + (f" — {d}" if d and not c else ""))


SQ_OK = "# ensures result == n*(n+1)*(2*n+1)/6\ndef slow(n):\n s=0\n for i in range(1, n+1):\n  s += i*i\n return s\n"
SQ_BAD = "# ensures result == n*n\ndef slow(n):\n s=0\n for i in range(1, n+1):\n  s += i*i\n return s\n"
SORT_BUG = ("def sortf(a):\n b=list(a)\n for i in range(len(b)):\n  for j in range(len(b)-1):\n"
            "   if b[j] < b[j+1]:\n    b[j],b[j+1]=b[j+1],b[j]\n return b\n")
SORT_OK = SORT_BUG.replace("b[j] < b[j+1]", "b[j] > b[j+1]")


def _z3(src):
    m = hir.to_hir(src, "x.py")
    return fusion.z3_inject(m.module.functions[0], source=src)


def hir_to_z3():
    v = _z3(SQ_OK)
    ok = v.tier == "PROVEN" and v.spec is not None and "jeff" in v.detail.lower()
    check("hir_to_z3", ok, f"tier={v.tier} spec={v.spec}")
    print(f"      → general-language loop + spec → Z3/JEFF proves it ∀: {v.tier} ({v.spec}). "
          f"Formal verification injected into ordinary code via the fold closed form.")


def spec_or_property_verify():
    with_spec = _z3(SQ_OK)                 # spec present → Z3 ∀ proof
    no_spec = _z3(SORT_BUG)               # no spec → properties proxy catches the bug
    no_spec_ok = _z3(SORT_OK)            # no spec, correct sort → properties hold
    ok = (with_spec.tier == "PROVEN" and with_spec.method.startswith("Z3")
          and no_spec.tier == "FAILED" and "propert" in no_spec.method
          and no_spec_ok.tier == "PROPERTY-ONLY")
    check("spec_or_property_verify", ok,
          f"spec→{with_spec.tier} nospec_bug→{no_spec.tier} nospec_ok→{no_spec_ok.tier}")
    print(f"      → with spec: Z3 ∀ proof ({with_spec.method}); no spec: properties as proxy "
          f"(buggy sort → {no_spec.tier}, correct sort → {no_spec_ok.tier}). Honest: no-spec is only "
          f"as strong as the properties.")


def general_lang_z3_demo():
    good = _z3(SQ_OK)
    bad = _z3(SQ_BAD)
    ok = good.tier == "PROVEN" and bad.tier == "FAILED" and bad.counterexample is not None
    check("general_lang_z3_demo", ok, f"good={good.tier} bad={bad.tier} cx={bad.counterexample}")
    print(f"      → correct spec (Σi²=n(n+1)(2n+1)/6) → PROVEN ∀; wrong spec (n²) → FAILED with "
          f"counterexample {bad.counterexample}. Z3 is only as good as the spec (D2 discipline).")


if __name__ == "__main__":
    print("v17 Part D · D2 — HIR → Z3 correctness injection")
    hir_to_z3(); spec_or_property_verify(); general_lang_z3_demo()
    print(f"\nD2: {len(PASS)} passed, {len(FAIL)} failed, {len(SKIP)} skipped")
    sys.exit(1 if FAIL else 0)
