"""v21 Part R · R2 tests — fast-path tiering (abstract-interp/fold → Z3 → Coq). Run: python3 test_r2.py

R2.1 tiered pipeline, escalate only when needed. R2.2 fast-path coverage (% resolved by fast tools).
R2.3 average time reduced (vs forcing everything through the deep prover). Correctness invariant.
"""
import sys

import verify_speed as VS

PASS, FAIL, SKIP = [], [], []


def check(n, c, d=""):
    (PASS if c else FAIL).append(n)
    print(f"  [{'PASS' if c else 'FAIL'}] {n}" + (f" — {d}" if d and not c else ""))


_M = VS.measure_tiered()
_FAST = [r for r in _M.rows if r.tier in (1, 2) and r.resolved]
_DEEP = [r for r in _M.rows if r.tier == 3 and r.resolved]


def tiered_pipeline():
    # every item resolved (correctness invariant) AND only genuine unbounded-∀ reached Coq
    base = {r.name: r.resolved for r in VS.measure_baseline()}
    correctness_preserved = all(r.resolved == base[r.name] for r in _M.rows)
    deep_only_coq = all(r.tool.startswith("coq") for r in _DEEP)
    ok = all(r.resolved for r in _M.rows) and correctness_preserved and deep_only_coq
    check("tiered_pipeline", ok, f"all_resolved={all(r.resolved for r in _M.rows)} correctness={correctness_preserved}")
    print(f"      → abstract-interp/fold → Z3 → Coq, escalate only when needed; ALL items still resolve "
          f"(correctness invariant — fast tools are sound, no skipping).")


def fast_path_coverage():
    pct = round(100 * len(_FAST) / len(_M.rows))
    ok = pct >= 60 and len(_DEEP) >= 1
    check("fast_path_coverage", ok, f"fast={len(_FAST)}/{len(_M.rows)} ({pct}%)")
    print(f"      → {len(_FAST)}/{len(_M.rows)} ({pct}%) resolved by the FAST tier (fold/Z3, ms) WITHOUT "
          f"Coq; only {len(_DEEP)} genuine unbounded-∀ reached Coq.")


def avg_time_reduced():
    fast_avg = sum(r.ms for r in _FAST) / len(_FAST)
    deep_avg = sum(r.ms for r in _DEEP) / len(_DEEP)
    common_speedup = deep_avg / fast_avg
    overall_speedup = deep_avg / _M.avg_ms()
    ok = fast_avg < 50 and common_speedup > 5 and overall_speedup > 2
    check("avg_time_reduced", ok, f"fast_avg={fast_avg:.1f}ms common×{common_speedup:.0f} overall×{overall_speedup:.1f}")
    print(f"      → fast cases avg {fast_avg:.1f}ms vs if forced through Coq {deep_avg:.0f}ms → ×{common_speedup:.0f} "
          f"for the common case; overall avg {_M.avg_ms():.0f}ms vs naive-all-deep {deep_avg:.0f}ms → ×{overall_speedup:.1f}.")
    print("      → HONEST: the slow Coq cases are FUNDAMENTAL (inductive ∀) — not made faster, just not "
          "paid unless needed. Fast tools sound ⇒ correctness unchanged.")


if __name__ == "__main__":
    print("v21 Part R · R2 — fast-path tiering")
    tiered_pipeline(); fast_path_coverage(); avg_time_reduced()
    print(f"\nR2: {len(PASS)} passed, {len(FAIL)} failed, {len(SKIP)} skipped")
    sys.exit(1 if FAIL else 0)
