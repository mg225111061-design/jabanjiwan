"""v16 Part A · A1 tests — verification caching. Run: python3 test_a1.py

A1.1 cache hit skips re-verify (unchanged fn served from cache).
A1.2 dependency invalidation (edit A → A and its caller B re-verify; independent fns stay cached).
A1.3 edit-verify loop speedup measured (cold vs warm; honest — first full verify still pays).
"""
import sys

import haran_cache as C

PASS, FAIL, SKIP = [], [], []


def check(n, c, d=""):
    (PASS if c else FAIL).append(n)
    print(f"  [{'PASS' if c else 'FAIL'}] {n}" + (f" — {d}" if d and not c else ""))


# a program with independent Faulhaber-style functions + one dependency chain (B calls A)
PROG = """\
fn s1(n: Nat) -> Nat
  ensures result = n*(n+1)/2
{ fold k in 1..n { k } }

fn s2(n: Nat) -> Nat
  ensures result = n*(n+1)*(2*n+1)/6
{ fold k in 1..n { k*k } }

fn s3(n: Nat) -> Nat
  ensures result = (n*(n+1)/2)*(n*(n+1)/2)
{ fold k in 1..n { k*k*k } }

fn a(n: Nat) -> Nat
  ensures result = n*(n+1)/2
{ fold k in 1..n { k } }

fn b(n: Nat) -> Nat
  ensures result >= 0
{ a(n) + 1 }
"""

# edit ONE independent function (s2's summand k*k → k*k + 0 changes its AST/hash)
EDIT_INDEP = PROG.replace("fold k in 1..n { k*k }", "fold k in 1..n { k*k + 0 }")
# edit the dependency A (its body) — should invalidate A and its caller B
EDIT_DEP = PROG.replace("fn a(n: Nat) -> Nat\n  ensures result = n*(n+1)/2\n{ fold k in 1..n { k } }",
                        "fn a(n: Nat) -> Nat\n  ensures result = n*(n+1)/2\n{ fold k in 1..n { k + 0 } }")


def cache_hit_skips_reverify():
    cv = C.CachedVerifier()
    r1, s1 = cv.verify_program(PROG)
    r2, s2 = cv.verify_program(PROG)   # identical source → all hits
    ok = s1.misses == 5 and s1.hits == 0 and s2.hits == 5 and s2.misses == 0
    ok = ok and [r.verdict for r in r1] == [r.verdict for r in r2]   # same verdicts
    check("cache_hit_skips_reverify", ok, f"cold misses={s1.misses} warm hits={s2.hits}")
    print(f"      → cold: {s1.misses} verified; warm (no change): {s2.hits} cache hits, 0 re-verify. Verdicts identical.")


def dependency_invalidation():
    cv = C.CachedVerifier()
    cv.verify_program(PROG)                       # prime the cache
    _, s_indep = cv.verify_program(EDIT_INDEP)    # edited s2 only
    cv2 = C.CachedVerifier()
    cv2.verify_program(PROG)
    _, s_dep = cv2.verify_program(EDIT_DEP)       # edited a → must also re-verify b (caller)
    indep_ok = s_indep.verified_fns == ["s2"]     # only the edited independent fn re-runs
    dep_ok = set(s_dep.verified_fns) == {"a", "b"}  # a AND its caller b re-run; s1/s2/s3 cached
    check("dependency_invalidation", indep_ok and dep_ok,
          f"indep re-ran={s_indep.verified_fns} dep re-ran={sorted(s_dep.verified_fns)}")
    print(f"      → edit independent s2 → only [s2] re-verified; edit dependency a → {sorted(s_dep.verified_fns)} "
          f"re-verified (caller b invalidated via Merkle key), s1/s3 cached.")


def edit_loop_speedup_measured():
    m = C.measure_edit_loop(PROG, EDIT_INDEP)
    # warm-unchanged should be ~free (all cache hits, no prover); one-edit re-verifies only the edited fn
    ok = (m.speedup_unchanged > 5 and m.warm_unchanged_s < m.cold_s
          and m.reverified_after_edit == ["s2"])
    check("edit_loop_speedup_measured", ok,
          f"cold={m.cold_s*1e3:.1f}ms warm={m.warm_unchanged_s*1e3:.2f}ms ×{m.speedup_unchanged:.0f}")
    print(f"      → cold full verify {m.cold_s*1e3:.0f}ms → warm (unchanged) {m.warm_unchanged_s*1e3:.2f}ms "
          f"= ×{m.speedup_unchanged:.0f} in the edit loop.")
    print(f"      → after editing one fn: {m.warm_one_edit_s*1e3:.1f}ms, re-verified only {m.reverified_after_edit}.")
    print("        HONEST: the FIRST full verification still pays the prover in full; caching only "
          "speeds the incremental edit-verify loop.")


if __name__ == "__main__":
    print("v16 Part A · A1 — verification caching")
    cache_hit_skips_reverify(); dependency_invalidation(); edit_loop_speedup_measured()
    print(f"\nA1: {len(PASS)} passed, {len(FAIL)} failed, {len(SKIP)} skipped")
    sys.exit(1 if FAIL else 0)
