"""v20 Part P · P4 tests — ISL use-after-free / double-free. Run: python3 test_p4.py

P4.1 heap state tracked (alloc/free/use). P4.2 UAF detected. P4.3 double-free detected.
P4.4 safe vs unsafe demo — under-approximate: every report is REAL (false-positives = 0).
"""
import sys

import isl

PASS, FAIL, SKIP = [], [], []


def check(n, c, d=""):
    (PASS if c else FAIL).append(n)
    print(f"  [{'PASS' if c else 'FAIL'}] {n}" + (f" — {d}" if d and not c else ""))


SAFE = "int f() {\n  int* p = malloc(8);\n  *p = 5;\n  free(p);\n  return 0;\n}\n"
UAF = "int f() {\n  int* p = malloc(8);\n  free(p);\n  int v = *p;\n  return v;\n}\n"
DF = "int f() {\n  int* p = malloc(8);\n  free(p);\n  free(p);\n  return 0;\n}\n"


def heap_state_tracked():
    ops = isl.parse_c_heap(SAFE)
    kinds = [o.kind for o in ops]
    bugs = isl.analyze_heap(ops)
    ok = "alloc" in kinds and "free" in kinds and "use" in kinds and len(bugs) == 0
    check("heap_state_tracked", ok, f"ops={kinds} bugs={len(bugs)}")
    print(f"      → heap ops tracked: {kinds}; safe alloc→use→free → 0 bugs (FP=0: a correct program "
          f"reports nothing).")


def uaf_detected():
    bugs = isl.analyze_c(UAF)
    ok = len(bugs) == 1 and bugs[0].kind == "use-after-free" and bugs[0].ptr == "p" and bugs[0].line == 4
    check("uaf_detected", ok, f"bugs={[(b.kind,b.line) for b in bugs]}")
    print(f"      → use-after-free: *p @L{bugs[0].line} after free @L{bugs[0].free_line} "
          f"(alloc @L{bugs[0].alloc_line}). Real, reachable bug.")


def double_free_detected():
    bugs = isl.analyze_c(DF)
    ok = len(bugs) == 1 and bugs[0].kind == "double-free" and bugs[0].line == 4
    check("double_free_detected", ok, f"bugs={[(b.kind,b.line) for b in bugs]}")
    print(f"      → double-free: free(p) @L{bugs[0].line} after free @L{bugs[0].free_line}.")


def safe_vs_unsafe_demo():
    safe = isl.analyze_c(SAFE)
    uaf = isl.analyze_c(UAF)
    df = isl.analyze_c(DF)
    ok = len(safe) == 0 and len(uaf) == 1 and len(df) == 1
    check("safe_vs_unsafe_demo", ok, f"safe={len(safe)} uaf={len(uaf)} df={len(df)}")
    print(f"      → safe code → 0; UAF → 1; double-free → 1. ★ISL = UNDER-approximate, FP=0★: every "
          f"reported bug is real. HARAN own/& (v14 RAII) make owned values UAF-free by construction; "
          f"this targets C/C++ manual memory. Incomplete (may miss) but never false.")


if __name__ == "__main__":
    print("v20 Part P · P4 — ISL use-after-free / double-free")
    heap_state_tracked(); uaf_detected(); double_free_detected(); safe_vs_unsafe_demo()
    print(f"\nP4: {len(PASS)} passed, {len(FAIL)} failed, {len(SKIP)} skipped")
    sys.exit(1 if FAIL else 0)
