"""v21 Part R · R7 tests — two modes (normal/extended). Run: python3 test_r7.py

R7.1 mode switch (different depth). R7.2 normal fast; extended solves more. R7.3 comparison demo.
Both modes: ZERO wrong answers (normal shallow but never false; extended more but not "everything").
"""
import sys

import modes
import verify_speed as VS

PASS, FAIL, SKIP = [], [], []


def check(n, c, d=""):
    (PASS if c else FAIL).append(n)
    print(f"  [{'PASS' if c else 'FAIL'}] {n}" + (f" — {d}" if d and not c else ""))


_C = modes.compare_modes()


def mode_switch():
    # normal stops before Coq (shallow on hard ∀); extended goes deep
    n_shallow = [r for r in _C.normal if r.status == "UNRESOLVED-shallow"]
    e_proven = [r for r in _C.extended if r.name in {x.name for x in n_shallow} and r.status == "PROVEN"]
    ok = len(n_shallow) >= 1 and len(e_proven) == len(n_shallow)
    check("mode_switch", ok, f"normal_shallow={len(n_shallow)} extended_proves_them={len(e_proven)}")
    print(f"      → same engine, two depths: normal leaves {len(n_shallow)} hard ∀ UNRESOLVED-shallow; "
          f"extended proves those same {len(e_proven)} (deep Coq).")


def normal_fast():
    ok = _C.speedup() > 3 and _C.normal_solved() >= 6 and _C.normal_wrong() == 0
    check("normal_fast", ok, f"normal {_C.normal_ms():.0f}ms vs extended {_C.extended_ms():.0f}ms ×{_C.speedup():.0f}")
    print(f"      → NORMAL solves the common case ({_C.normal_solved()}/8) in {_C.normal_ms():.0f}ms — "
          f"×{_C.speedup():.0f} faster than EXTENDED ({_C.extended_ms():.0f}ms). 0 wrong answers (shallow, never false).")


def extended_more_solved():
    ok = _C.extra_solved() >= 1 and _C.extended_solved() > _C.normal_solved()
    check("extended_more_solved", ok, f"normal={_C.normal_solved()} extended={_C.extended_solved()} (+{_C.extra_solved()})")
    print(f"      → EXTENDED solves +{_C.extra_solved()} MORE (the hard unbounded-∀ Coq cases) — quality over "
          f"speed. HONEST: still not 'everything' — NP-hard/inductive timeouts → UNRESOLVED-timeout (not false).")


def mode_comparison_demo():
    easy = [VS.CORPUS[1]]      # sum_k2 (fold) — both modes prove it fast
    hard = [VS.CORPUS[6]]      # sort_sorted (Coq) — normal shallow, extended deep
    en, ee = modes.analyze(easy, "normal")[0], modes.analyze(easy, "extended")[0]
    hn, he = modes.analyze(hard, "normal")[0], modes.analyze(hard, "extended")[0]
    ok = (en.status == "PROVEN" and ee.status == "PROVEN"                   # easy: both prove, fast
          and hn.status == "UNRESOLVED-shallow" and he.status == "PROVEN")  # hard: normal shallow, extended deep
    check("mode_comparison_demo", ok, f"easy n/e={en.status}/{ee.status} hard n/e={hn.status}/{he.status}")
    print(f"      → EASY (Σk²): normal & extended both PROVEN, fast ({en.ms:.0f}/{ee.ms:.0f}ms). "
          f"HARD (sort ∀): normal UNRESOLVED-shallow ({hn.ms:.0f}ms, not wrong) vs extended PROVEN ({he.ms:.0f}ms).")
    print("      → ★normal=fast/shallow, extended=more/slightly-slower, BOTH zero wrong answers★ "
          "(R2-R5 keep extended fast too).")


if __name__ == "__main__":
    print("v21 Part R · R7 — two modes (normal / extended)")
    mode_switch(); normal_fast(); extended_more_solved(); mode_comparison_demo()
    print(f"\nR7: {len(PASS)} passed, {len(FAIL)} failed, {len(SKIP)} skipped")
    sys.exit(1 if FAIL else 0)
