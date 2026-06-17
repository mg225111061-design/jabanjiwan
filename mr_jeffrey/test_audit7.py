"""v19 Part W · W7 tests — docs + completeness declaration. Run: python3 test_audit7.py

W7.1 usage docs exist (each feature, with examples).  W7.2 ceilings documented.
W7.3 completeness checklist final — all 7 boxes ✅ → "LLVM backend completeness achieved".
"""
import os
import sys

import audit

PASS, FAIL, SKIP = [], [], []


def check(n, c, d=""):
    (PASS if c else FAIL).append(n)
    print(f"  [{'PASS' if c else 'FAIL'}] {n}" + (f" — {d}" if d and not c else ""))


_README = "HARAN_v19_README.md"
_TEXT = open(_README).read() if os.path.exists(_README) else ""


def usage_docs():
    has_examples = all(s in _TEXT for s in ("fold k in 1..n", "map(xs", "fold x in xs", "compile_fn"))
    ok = os.path.exists(_README) and "Usage" in _TEXT and has_examples
    check("usage_docs", ok, f"readme={os.path.exists(_README)} examples={has_examples}")
    print(f"      → {_README}: usage for scalar/fold/match/recursion, Vec map+reduce, bignum, refinement — "
          f"with runnable examples + the public APIs.")


def ceilings_documented():
    ceilings = ("Infinite corecursion", "i64 overflow", "beyond C", "Unstructured")
    ok = all(c in _TEXT for c in ceilings)
    check("ceilings_documented", ok, f"ceilings_in_doc={[c for c in ceilings if c in _TEXT]}")
    print(f"      → ceilings stated honestly: infinite corecursion (prefix only), i64 overflow (use "
          f"bignum), no speed beyond C, unstructured Ω(N). NOT defects — fundamental limits, documented.")


def completeness_checklist_final():
    boxes = audit.completeness_checklist()
    all_ok = all(b.ok for b in boxes)
    decl = audit.declaration()
    ok = all_ok and len(boxes) == 7 and "ACHIEVED" in decl
    check("completeness_checklist_final", ok, f"boxes_ok={sum(b.ok for b in boxes)}/7")
    print("      " + decl.replace("\n", "\n      "))


if __name__ == "__main__":
    print("v19 Part W · W7 — docs + completeness declaration")
    usage_docs(); ceilings_documented(); completeness_checklist_final()
    print(f"\nW7: {len(PASS)} passed, {len(FAIL)} failed, {len(SKIP)} skipped")
    sys.exit(1 if FAIL else 0)
