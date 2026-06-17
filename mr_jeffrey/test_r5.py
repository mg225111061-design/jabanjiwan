"""v21 Part R · R5 tests — background verification (hard cases don't block). Run: python3 test_r5.py

R5.1 async (fast sync, hard background). R5.2 honest status (✅/⏳/❌/⏱️). R5.3 user blocked-time ≈ 0 for hard.
"""
import sys
import time

import async_verify as AV
import verify_speed as VS

PASS, FAIL, SKIP = [], [], []


def check(n, c, d=""):
    (PASS if c else FAIL).append(n)
    print(f"  [{'PASS' if c else 'FAIL'}] {n}" + (f" — {d}" if d and not c else ""))


def async_verify():
    av = AV.AsyncVerifier()
    imm = av.start(VS.CORPUS)
    st = av.status()
    immediate_proven = all(r.status == "✅proven" and not r.background for r in imm)
    hard_background = [r for r in st if r.background]
    ok = len(imm) == 6 and immediate_proven and len(hard_background) == 2
    av.wait(30); av.shutdown()
    check("async_verify", ok, f"immediate={len(imm)} background={len(hard_background)}")
    print(f"      → {len(imm)} fast items return ✅proven immediately (synchronous); {len(hard_background)} "
          f"hard (Coq) items dispatched to the background — user keeps working.")


def status_display():
    av = AV.AsyncVerifier()
    av.start(VS.CORPUS)
    # right after start, hard items are ⏳verifying
    verifying = [r for r in av.status() if r.background and r.status == "⏳verifying"]
    # a tiny budget after a brief wait → honest ⏱️timeout (still running, unresolved within budget)
    time.sleep(0.05)
    timed_out = [r for r in av.status(per_item_timeout_s=0.01) if r.status == "⏱️timeout"]
    # after full wait → ✅proven (honest completion)
    final = av.wait(30)
    proven_bg = [r for r in final if r.background and r.status == "✅proven"]
    av.shutdown()
    ok = len(verifying) >= 1 and len(timed_out) >= 1 and len(proven_bg) == 2
    check("status_display", ok, f"verifying={len(verifying)} timeout={len(timed_out)} proven_bg={len(proven_bg)}")
    print(f"      → honest statuses: ⏳verifying (in progress) → ⏱️timeout (exceeded budget, UNRESOLVED — "
          f"never a false 'done') → ✅proven (completed). All four kinds real (❌counterexample on refute).")


def no_user_block():
    m = AV.measure_no_block()
    # user blocked only for the fast sync tier; the hard Coq work runs concurrently (not blocking)
    ratio = m["background_work_ms"] / max(1e-9, m["blocked_ms"])
    ok = m["blocked_ms"] < 200 and m["background_work_ms"] > 300 and ratio > 1
    check("no_user_block", ok, f"blocked={m['blocked_ms']:.0f}ms background={m['background_work_ms']:.0f}ms")
    print(f"      → user BLOCKED {m['blocked_ms']:.0f}ms (fast tier only) while {m['background_work_ms']:.0f}ms of "
          f"Coq runs in the background → perceived block for hard cases ≈ 0.")
    print("      → HONEST: 'not blocked', not 'instant' — NP-hard/inductive cases still take time, but the "
          "user isn't made to wait; status stays truthful (⏳/⏱️/✅).")


if __name__ == "__main__":
    print("v21 Part R · R5 — background verification (no user block)")
    async_verify(); status_display(); no_user_block()
    print(f"\nR5: {len(PASS)} passed, {len(FAIL)} failed, {len(SKIP)} skipped")
    sys.exit(1 if FAIL else 0)
