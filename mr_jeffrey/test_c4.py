"""v17 Part C · C4 tests — Java frontend. Run: python3 test_c4.py

C4.2 Java → HIR (javalang AST + statement-line propagation).
C4.3 engine reuse: javac + persistent JVM runs the real code; B2–B5 localizes the bug.
     (Defects4J — the real Java benchmark — needs a heavy project checkout → DEFER; demo'd here.)
"""
import sys

import hir
import frontend_java
import properties as PR
import property_test as PT
import narrow as NA

PASS, FAIL, SKIP = [], [], []


def check(n, c, d=""):
    (PASS if c else FAIL).append(n)
    print(f"  [{'PASS' if c else 'FAIL'}] {n}" + (f" — {d}" if d and not c else ""))


def skip(n, w):
    SKIP.append(n)
    print(f"  [SKIP] {n} — {w}")


JAVA = ("static int[] sortf(int[] a) {\n  for (int i = 0; i < a.length; i++) {\n"
        "    for (int j = 0; j < a.length - 1; j++) {\n      if (a[j] < a[j+1]) {\n"
        "        int t = a[j]; a[j] = a[j+1]; a[j+1] = t;\n      }\n    }\n  }\n  return a;\n}")


def java_to_hir():
    if not frontend_java.available():
        skip("java_to_hir", "javalang/javac/java absent → BLOCKED"); return
    f = hir.to_hir(JAVA, "S.java").module.fn("sortf")
    ok = (f.lang == "java" and {"compare", "index_store", "return"} <= f.op_kinds()
          and f.signature["kind"] == "array_return"
          and any(o.kind == "compare" and o.line == 5 for o in f.ops))
    check("java_to_hir", ok, f"ops={sorted(f.op_kinds())} compare@{[o.line for o in f.ops if o.kind=='compare']}")
    print(f"      → Java → HIR (javalang): ops={sorted(f.op_kinds())}, signature={f.signature['kind']}; "
          f"statement-line propagation gives the bug comparison line 5.")


def java_bug_localized():
    if not frontend_java.available():
        skip("java_bug_localized", "java toolchain absent → BLOCKED"); return
    f = hir.to_hir(JAVA, "S.java").module.fn("sortf")
    fn = PR.compile_callable(f)              # javac compile + persistent JVM
    out = fn([5, 2, 9, 1, 7])
    props = PR.extract_properties(f)
    rep = PT.test_properties(fn, props, PT.gen_int_lists(40))
    res = NA.bayesian_narrow(f, [p for p in props if p.name in rep.violated_properties()])
    top = res.ranked[0]
    ok = out == [9, 7, 5, 2, 1] and rep.violated_properties() == ["ordered_output"] and top.op_kind == "compare"
    check("java_bug_localized", ok, f"out={out} top={top.op_kind}@{top.lines}")
    print(f"      → javac+JVM run sortf([5,2,9,1,7])={out}; violated={rep.violated_properties()}; "
          f"top suspect {top.op_kind}@{top.lines}. SAME engine, Java frontend.")
    print("        Defects4J (real Java benchmark) = DEFER (heavy per-project checkout); OOP "
          "(inheritance/generics) out of scope; causal mutation + fix loop Python-only.")


if __name__ == "__main__":
    print("v17 Part C · C4 — Java frontend")
    java_to_hir(); java_bug_localized()
    print(f"\nC4: {len(PASS)} passed, {len(FAIL)} failed, {len(SKIP)} skipped")
    sys.exit(1 if FAIL else 0)
