"""v17 Part C · C1 tests — C frontend + engine reuse. Run: python3 test_c1.py

C1.2 C → HIR (pycparser): operations + line numbers + signature.
C1.3 engine reuse: B2 extracts properties from C HIR; B3 tests the REAL compiled C output.
C1.4 C bug localized: B5 narrows to the suspect operation; B6 emits a proven digit.
"""
import sys

import hir
import frontend_c
import properties as PR
import property_test as PT
import narrow as NA
import digit_proof as DP

PASS, FAIL, SKIP = [], [], []


def check(n, c, d=""):
    (PASS if c else FAIL).append(n)
    print(f"  [{'PASS' if c else 'FAIL'}] {n}" + (f" — {d}" if d and not c else ""))


def skip(n, w):
    SKIP.append(n)
    print(f"  [SKIP] {n} — {w}")


# buggy bubble sort in C: '<' instead of '>' → sorts DESCENDING (ordered_output violated)
CSRC = """int* sort(int* a, int n) {
    int i, j, t;
    for (i = 0; i < n; i++) {
        for (j = 0; j < n - 1; j++) {
            if (a[j] < a[j+1]) {
                t = a[j]; a[j] = a[j+1]; a[j+1] = t;
            }
        }
    }
    return a;
}
"""


def c_parsed_to_hir():
    if not frontend_c.available():
        skip("c_parsed_to_hir", "pycparser not installed → BLOCKED"); return
    m = hir.to_hir(CSRC, "sort.c")
    f = m.module.fn("sort")
    ok = (m.supported and m.lang == "c" and f.lang == "c" and f.params == ["a", "n"]
          and {"compare", "index_store", "arith", "return"} <= f.op_kinds()
          and f.signature.get("kind") in ("array_return", "array_inplace"))
    check("c_parsed_to_hir", ok, f"ops={sorted(f.op_kinds())} sig={f.signature}")
    print(f"      → C → HIR: params={f.params}, ops={sorted(f.op_kinds())}, signature={f.signature} "
          f"(same HIR vocabulary as Python).")


def c_engine_reuse():
    if not frontend_c.available():
        skip("c_engine_reuse", "pycparser not installed → BLOCKED"); return
    f = hir.to_hir(CSRC, "sort.c").module.fn("sort")
    fn = PR.compile_callable(f)                       # compiles with gcc, returns a runnable wrapper
    out = fn([5, 2, 9, 1, 7])
    props = PR.extract_properties(f)
    rep = PT.test_properties(fn, props, PT.gen_int_lists(80))
    ok = (out == [9, 7, 5, 2, 1] and len(props) >= 5
          and rep.violated_properties() == ["ordered_output"])
    check("c_engine_reuse", ok, f"C out={out} violated={rep.violated_properties()}")
    print(f"      → SAME engine: gcc-compiled C run on inputs → sort([5,2,9,1,7])={out}; B2 found "
          f"{len(props)} properties, B3 violated {rep.violated_properties()} from REAL C output.")


def c_bug_localized():
    if not frontend_c.available():
        skip("c_bug_localized", "pycparser not installed → BLOCKED"); return
    f = hir.to_hir(CSRC, "sort.c").module.fn("sort")
    fn = PR.compile_callable(f)
    props = PR.extract_properties(f)
    rep = PT.test_properties(fn, props, PT.gen_int_lists(80))
    violated = [p for p in props if p.name in rep.violated_properties()]
    res = NA.bayesian_narrow(f, violated)
    top = res.ranked[0]
    cert = DP.digit_certificate(res.ranked[-1].op_kind, res.innocent_posterior, 80,
                                {p.name: rep.violations[p.name] for p in violated})
    ok = top.op_kind == "compare" and 5 in top.lines and "CERT" in cert.render()
    check("c_bug_localized", ok, f"top={top.op_kind}@{top.lines}")
    print(f"      → B5 top suspect = {top.op_kind}@{top.lines} (the '<' bug is line 5); "
          f"B6: {cert.render()}")
    print("        NOTE: causal mutation is Python-only → C op→line refinement DEFER; static LR localizes "
          "to the comparison family. Fix loop (B8) is Python-only for now.")


if __name__ == "__main__":
    print("v17 Part C · C1 — C frontend + engine reuse")
    c_parsed_to_hir(); c_engine_reuse(); c_bug_localized()
    print(f"\nC1: {len(PASS)} passed, {len(FAIL)} failed, {len(SKIP)} skipped")
    sys.exit(1 if FAIL else 0)
