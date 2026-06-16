"""v16 Part B · B1 tests — language detection + Python frontend → HIR. Run: python3 test_b1.py

B1.1 language detected (extension + syntax heuristics).
B1.2 Python → HIR (operations with line numbers — the substrate for fault mapping & slicing).
B1.3 extension point exists (C/Rust/Go registered but DEFER — honest, not faked).
"""
import sys

import hir

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


def language_detected():
    ok = (hir.detect_language("f.py", "") == "python"
          and hir.detect_language("m.c", "") == "c"
          and hir.detect_language("x.rs", "") == "rust"
          and hir.detect_language("g.go", "") == "go"
          and hir.detect_language(None, "#include <stdio.h>\nint main(){return 0;}") == "c"
          and hir.detect_language(None, "def f(x):\n    return x+1") == "python")
    check("language_detected", ok, "by extension + by syntax")
    print("      → .py/.c/.rs/.go by extension; C-by-#include & Python-by-def detected from source.")


def python_to_hir():
    r = hir.to_hir(BUBBLE, "b.py")
    f = r.module.fn("bubble")
    kinds = f.op_kinds()
    # the operations a sortedness bug lives in must be captured with line numbers
    ok = (r.supported and f.params == ["xs"]
          and {"compare", "index_store", "index_load", "arith", "return"} <= kinds
          and any(o.kind == "compare" and o.line == 5 for o in f.ops)
          and any(o.kind == "index_store" and o.line == 6 for o in f.ops))
    check("python_to_hir", ok, f"ops={sorted(kinds)}")
    print(f"      → bubble → HIR: params={f.params}, compare@line5, swap(index_store)@line6 — "
          f"operations carry line numbers for fault mapping (B4) & slicing (B8).")


def extension_point_exists():
    # C/Rust/Go are registered keys (engine is shared) but not implemented → honest DEFER, not a fake.
    registered = {"python", "c", "rust", "go"} <= set(hir.FRONTENDS)
    py_ok = hir.FRONTENDS["python"] is not None
    c_defer = hir.FRONTENDS["c"] is None
    rc = hir.to_hir("#include <x.h>\nint main(){return 0;}", "m.c")
    defer_honest = (not rc.supported) and "DEFER" in rc.detail
    ok = registered and py_ok and c_defer and defer_honest
    check("extension_point_exists", ok, f"frontends={sorted(hir.FRONTENDS)}")
    print("      → new language = add a frontend only (engine shared); C/Rust/Go registered as DEFER "
          "extension points, reported honestly (not pretended-supported).")


if __name__ == "__main__":
    print("v16 Part B · B1 — language detection + Python frontend → HIR")
    language_detected(); python_to_hir(); extension_point_exists()
    print(f"\nB1: {len(PASS)} passed, {len(FAIL)} failed, {len(SKIP)} skipped")
    sys.exit(1 if FAIL else 0)
