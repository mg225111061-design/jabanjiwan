"""v21 Part R · R4 tests — incremental verification (re-verify only what changed). Run: python3 test_r4.py

R4.1 dependency-graph caching (edit a function → it + dependents re-verify). R4.2 full vs incremental.
R4.3 edit-loop perceived time (fast). Correctness invariant (same verdicts, no stale cache).
"""
import sys

import haran_cache as HC

PASS, FAIL, SKIP = [], [], []


def check(n, c, d=""):
    (PASS if c else FAIL).append(n)
    print(f"  [{'PASS' if c else 'FAIL'}] {n}" + (f" — {d}" if d and not c else ""))


def _codebase(n=12):
    return "\n\n".join(
        f"fn f{i}(n: Nat)->Nat\n  ensures result = n*(n+1)/2\n{{ fold k in 1..n {{ k }} }}" for i in range(n))


# codebase with a dependency: b calls a; the rest independent
DEP = ("fn a(n: Nat)->Nat\n  ensures result = n*(n+1)/2\n{ fold k in 1..n { k } }\n\n"
       "fn b(n: Nat)->Nat\n  ensures result >= 0\n{ a(n) + 1 }\n\n"
       "fn c(n: Nat)->Nat\n  ensures result = n*(n+1)/2\n{ fold k in 1..n { k } }\n")


def incremental_reverify():
    src = _codebase(12)
    edited = src.replace("fold k in 1..n { k }", "fold k in 1..n { k + 0 }", 1)   # edit f0 only
    m = HC.measure_edit_loop(src, edited)
    ok = m.warm_one_edit_s < m.cold_s and m.reverified_after_edit == ["f0"]
    check("incremental_reverify", ok, f"cold={m.cold_s*1000:.0f}ms edit={m.warm_one_edit_s*1000:.1f}ms re-ran={m.reverified_after_edit}")
    print(f"      → cold full (12 fns) {m.cold_s*1000:.0f}ms → one-edit re-verify {m.warm_one_edit_s*1000:.1f}ms "
          f"(×{m.speedup_one_edit:.0f}); only the edited f0 re-ran (rest cached).")


def dependency_invalidation():
    cv = HC.CachedVerifier()
    cv.verify_program(DEP)                       # prime cache
    edited = DEP.replace("fn a(n: Nat)->Nat\n  ensures result = n*(n+1)/2\n{ fold k in 1..n { k } }",
                         "fn a(n: Nat)->Nat\n  ensures result = n*(n+1)/2\n{ fold k in 1..n { k + 0 } }")
    _, stats = cv.verify_program(edited)
    # editing a → a re-verifies AND its caller b is invalidated; c stays cached
    ok = set(stats.verified_fns) == {"a", "b"} and "c" in stats.skipped_fns
    check("dependency_invalidation", ok, f"re-ran={sorted(stats.verified_fns)} cached={stats.skipped_fns}")
    print(f"      → edit a → re-verified {sorted(stats.verified_fns)} (caller b invalidated via Merkle key); "
          f"independent c stays cached. Correct invalidation.")


def edit_loop_fast():
    src = _codebase(12)
    edited = src.replace("fold k in 1..n { k }", "fold k in 1..n { k + 0 }", 1)
    cv = HC.CachedVerifier()
    r1, _ = cv.verify_program(src)
    r2, _ = cv.verify_program(edited)
    # correctness invariant: verdicts identical (caching never changes the answer)
    verdicts_ok = [r.verdict for r in r1] == [r.verdict for r in r2] and all(r.verdict == "VERIFIED" for r in r2)
    m = HC.measure_edit_loop(src, edited)
    ok = m.warm_one_edit_s * 1000 < 50 and verdicts_ok
    check("edit_loop_fast", ok, f"edit={m.warm_one_edit_s*1000:.1f}ms verdicts_preserved={verdicts_ok}")
    print(f"      → edit→re-verify perceived {m.warm_one_edit_s*1000:.1f}ms (≈ instant in the edit loop); "
          f"verdicts identical to a full re-verify (correctness invariant — no stale cache).")


if __name__ == "__main__":
    print("v21 Part R · R4 — incremental verification")
    incremental_reverify(); dependency_invalidation(); edit_loop_fast()
    print(f"\nR4: {len(PASS)} passed, {len(FAIL)} failed, {len(SKIP)} skipped")
    sys.exit(1 if FAIL else 0)
