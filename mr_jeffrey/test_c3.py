"""v17 Part C · C3 tests — JavaScript / TypeScript frontends. Run: python3 test_c3.py

C3.2 JS/TS → HIR (dynamic types inferred from usage = weak; TS annotations = stronger).
C3.3 engine reuse: persistent node worker runs the real code; B2–B5 localizes the bug.
"""
import sys

import hir
import frontend_js
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


JS = ("function sortf(a) {\n  for (let i = 0; i < a.length; i++) {\n    for (let j = 0; j < a.length - 1; j++) {\n"
      "      if (a[j] < a[j+1]) {\n        let t = a[j]; a[j] = a[j+1]; a[j+1] = t;\n      }\n    }\n  }\n  return a;\n}\n")
TS = ("function sortf(a: number[]): number[] {\n  for (let i = 0; i < a.length; i++) {\n"
      "    for (let j = 0; j < a.length - 1; j++) {\n      if (a[j] < a[j+1]) {\n"
      "        let t = a[j]; a[j] = a[j+1]; a[j+1] = t;\n      }\n    }\n  }\n  return a;\n}\n")


def _engine(src, ext):
    f = hir.to_hir(src, ext).module.fn("sortf")
    fn = PR.compile_callable(f)
    out = fn([5, 2, 9, 1, 7])
    props = PR.extract_properties(f)
    rep = PT.test_properties(fn, props, PT.gen_int_lists(40))
    res = NA.bayesian_narrow(f, [p for p in props if p.name in rep.violated_properties()])
    return f, out, rep.violated_properties(), res.ranked[0]


def js_to_hir():
    if not frontend_js.available():
        skip("js_to_hir", "node absent → BLOCKED"); return
    f = hir.to_hir(JS, "s.js").module.fn("sortf")
    ok = f.lang == "javascript" and {"compare", "index_store"} <= f.op_kinds() and f.signature["kind"] == "array_return"
    check("js_to_hir", ok, f"ops={sorted(f.op_kinds())} sig={f.signature['kind']}")
    print(f"      → JS → HIR: ops={sorted(f.op_kinds())}, array shape inferred from usage (no types — weak).")


def ts_to_hir():
    if not frontend_js.available():
        skip("ts_to_hir", "node absent → BLOCKED"); return
    f = hir.to_hir(TS, "s.ts").module.fn("sortf")
    ok = f.lang == "typescript" and {"compare", "index_store"} <= f.op_kinds()
    check("ts_to_hir", ok, f"ops={sorted(f.op_kinds())}")
    print(f"      → TS → HIR: ops={sorted(f.op_kinds())}; `number[]` annotation makes the shape explicit "
          f"(stronger than dynamic JS).")


def js_bug_localized():
    if not frontend_js.available():
        skip("js_bug_localized", "node absent → BLOCKED"); return
    results = {}
    for lang, src, ext in [("js", JS, "s.js"), ("ts", TS, "s.ts")]:
        f, out, violated, top = _engine(src, ext)
        results[lang] = (out, violated, top.op_kind)
        print(f"      → {lang}: node-run sort([5,2,9,1,7])={out}; violated={violated}; top={top.op_kind}@{top.lines}")
    ok = all(r[0] == [9, 7, 5, 2, 1] and r[1] == ["ordered_output"] and r[2] == "compare"
             for r in results.values())
    check("js_bug_localized", ok, f"langs={list(results)}")
    print("      → SAME engine via a persistent node worker (fast); bug localized to `compare` in JS & TS.")


if __name__ == "__main__":
    print("v17 Part C · C3 — JavaScript / TypeScript frontends")
    js_to_hir(); ts_to_hir(); js_bug_localized()
    print(f"\nC3: {len(PASS)} passed, {len(FAIL)} failed, {len(SKIP)} skipped")
    sys.exit(1 if FAIL else 0)
