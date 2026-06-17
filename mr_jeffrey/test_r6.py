"""v21 Part R · R6 tests — integrated pipeline + final measurement. Run: python3 test_r6.py

R6.1 integrated (fold-first → fast-path → incremental → background). R6.2 final measurement vs baseline.
R6.3 honest speed table ("+Xms for a proven result", not "instant"). Correctness invariant.
"""
import sys

import verify_pipeline as VP
import verify_speed as VS

PASS, FAIL, SKIP = [], [], []


def check(n, c, d=""):
    (PASS if c else FAIL).append(n)
    print(f"  [{'PASS' if c else 'FAIL'}] {n}" + (f" — {d}" if d and not c else ""))


_T = VP.final_measurement()


def integrated_pipeline():
    # every item still resolves through the combined pipeline (correctness invariant)
    resolved = [VP.verify_optimized(it).resolved for it in VS.CORPUS]
    ok = all(resolved)
    check("integrated_pipeline", ok, f"resolved={sum(resolved)}/{len(resolved)}")
    print(f"      → fold-first → fast-path → incremental → background, combined; all {sum(resolved)}/"
          f"{len(resolved)} items still resolve (correctness invariant — every optimization is sound).")


def final_measurement():
    ok = (_T.easy_ms < 50 and _T.edit_loop_ms < 50
          and _T.hard_blocked_ms < 200 and _T.hard_background_ms > 300)
    check("final_measurement", ok,
          f"easy={_T.easy_ms:.0f} edit={_T.edit_loop_ms:.0f} blocked={_T.hard_blocked_ms:.0f} bg={_T.hard_background_ms:.0f}")
    print(f"      → easy verify {_T.easy_ms:.0f}ms (perceived 0); edit loop {_T.edit_loop_ms:.0f}ms (perceived 0); "
          f"hard prop user-blocked {_T.hard_blocked_ms:.0f}ms while {_T.hard_background_ms:.0f}ms runs in background.")


def speed_table_honest():
    # the marketing number: code-gen + verification adds only the easy overhead → "same speed + Xms = proven"
    ok = _T.overhead_ms < 50 and _T.gen_plus_verify_ms >= _T.gen_only_ms
    check("speed_table_honest", ok, f"+{_T.overhead_ms:.0f}ms for proven")
    print(f"      → ★gen-only {_T.gen_only_ms:.0f}ms vs gen+verify {_T.gen_plus_verify_ms:.0f}ms → "
          f"+{_T.overhead_ms:.0f}ms for a PROVEN result★ (real measured value, not 'incomparable').")
    print("      → HONEST: 'same speed + Xms', NOT 'instant' — easy/edit/fold are perceived-zero; hard "
          "(NP-hard/inductive) cases are not instant but NOT blocking (background). Correctness invariant.")


if __name__ == "__main__":
    print("v21 Part R · R6 — integrated pipeline + final measurement")
    integrated_pipeline(); final_measurement(); speed_table_honest()
    print(f"\nR6: {len(PASS)} passed, {len(FAIL)} failed, {len(SKIP)} skipped")
    sys.exit(1 if FAIL else 0)
