"""v17 Part E · E3 tests — full v17 integration + final measurement. Run: python3 test_e3.py

E3.1 unified pipeline routes correctly: regression (ref) | closed-form | bug-localized | no-regression.
E3.2 final measurement: multilang top-k, fusion fold ratio, Z3/Coq, differential, determinism, mirage-free.
"""
import sys

import haran_v17 as V

PASS, FAIL, SKIP = [], [], []


def check(n, c, d=""):
    (PASS if c else FAIL).append(n)
    print(f"  [{'PASS' if c else 'FAIL'}] {n}" + (f" — {d}" if d and not c else ""))


OLD = "def scale(x):\n return x*2\n"
NEW = "def scale(x):\n return x-1\n"
SK2 = "def f(n):\n s=0\n for i in range(1,n+1):\n  s+=i*i\n return s\n"
SORT_BUG = ("def s(a):\n b=list(a)\n for i in range(len(b)):\n  for j in range(len(b)-1):\n"
            "   if b[j]<b[j+1]:\n    b[j],b[j+1]=b[j+1],b[j]\n return b\n")


def v17_integrated():
    routes = {
        "regression": V.analyze_v17(NEW, "scale.py", reference=OLD).category,
        "no-regression": V.analyze_v17(OLD, "scale.py", reference=OLD).category,
        "closed-form": V.analyze_v17(SK2, "f.py").category,
        "bug-localized": V.analyze_v17(SORT_BUG, "s.py").category,
    }
    ok = all(k == v for k, v in routes.items())
    check("v17_integrated", ok, f"routes={routes}")
    print(f"      → one pipeline routes every case: buggy-scale+ref→regression, correct+ref→no-regression, "
          f"Σi²→closed-form, descending sort→bug-localized.")


def final_measurement():
    m = V.final_measurement()
    ok = (len(m.languages_covered) >= 5 and m.multilang_top1.startswith(str(len(m.languages_covered)))
          and m.fold_close_rate == 1.0 and m.z3_verified == 1 and m.z3_refuted == 1
          and m.coq_unbounded == 1 and m.coq_auto_theorems >= 6 and m.bugs_localized == 1
          and m.differential_invisible_caught and m.deterministic and m.mirage_free)
    check("final_measurement", ok, f"langs={len(m.languages_covered)} top1={m.multilang_top1}")
    print(f"      → languages covered: {m.languages_covered}")
    print(f"      → multilang top-1 = {m.multilang_top1} | fusion fold-close {m.fold_close_rate:.0%} | "
          f"Z3 verified={m.z3_verified}/refuted={m.z3_refuted} | Coq unbounded={m.coq_unbounded}, "
          f"auto-theorems={m.coq_auto_theorems}")
    print(f"      → bugs localized={m.bugs_localized} | differential caught property-invisible="
          f"{m.differential_invisible_caught} | deterministic={m.deterministic} | mirage-free={m.mirage_free}")


if __name__ == "__main__":
    print("v17 Part E · E3 — full integration + final measurement")
    v17_integrated(); final_measurement()
    print(f"\nE3: {len(PASS)} passed, {len(FAIL)} failed, {len(SKIP)} skipped")
    sys.exit(1 if FAIL else 0)
