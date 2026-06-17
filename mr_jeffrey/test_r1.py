"""v21 Part R · R1 tests — verification-speed baseline. Run: python3 test_r1.py

R1.1/.2 baseline measured (per-tool ms). R1.3 distribution (most fast, a few hard/Coq).
R1.4 baseline recorded for later before/after comparison.
"""
import sys

import verify_speed as VS

PASS, FAIL, SKIP = [], [], []


def check(n, c, d=""):
    (PASS if c else FAIL).append(n)
    print(f"  [{'PASS' if c else 'FAIL'}] {n}" + (f" — {d}" if d and not c else ""))


_D = VS.distribution()


def baseline_measured():
    ok = len(_D.rows) >= 8 and all(r.resolved for r in _D.rows) and all(r.ms > 0 for r in _D.rows)
    check("baseline_measured", ok, f"items={len(_D.rows)} all_resolved={all(r.resolved for r in _D.rows)}")
    for r in _D.rows:
        print(f"      → {r.name:12} [{r.difficulty:6}] {r.tool:14} {r.ms:7.1f}ms")


def distribution_recorded():
    fast, slow = _D.fast(), _D.slow()
    # hypothesis: MOST verification is fast (fold/Z3, ms); a MINORITY is slow (Coq, fundamental induction)
    most_fast = _D.fast_pct() >= 60
    slow_is_coq = all("coq" in r.tool for r in slow)
    slow_dominates = sum(r.ms for r in slow) > sum(r.ms for r in fast)   # the few hard ones cost the most
    ok = most_fast and slow_is_coq and slow_dominates
    check("distribution_recorded", ok, f"fast={_D.fast_pct()}% slow_is_coq={slow_is_coq}")
    print(f"      → {len(fast)}/{len(_D.rows)} fast (<50ms, fold/Z3, total {sum(r.ms for r in fast):.0f}ms); "
          f"{len(slow)} slow (Coq, total {sum(r.ms for r in slow):.0f}ms). avg={_D.avg_ms():.0f}ms.")
    print(f"      → the slow minority is Coq (unbounded ∀, FUNDAMENTAL inductive cost) and dominates total "
          f"time — so: fast-path tiering (R2) avoids Coq when unneeded; background (R5) hides it. Recorded.")


if __name__ == "__main__":
    print("v21 Part R · R1 — verification-speed baseline")
    baseline_measured(); distribution_recorded()
    print(f"\nR1: {len(PASS)} passed, {len(FAIL)} failed, {len(SKIP)} skipped")
    sys.exit(1 if FAIL else 0)
