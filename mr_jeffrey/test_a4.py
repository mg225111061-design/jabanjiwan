"""v16 Part A · A4 tests — Type A integration + verification-speed summary. Run: python3 test_a4.py

A4.1 FastVerifier = caching + parallelism combined (hits instant, misses re-verified in parallel).
A4.2 verification_speed_summary: caching ×, parallel ×, Coq unbounded count — measured, honest.
"""
import sys

from haran_parser import parse
import haran_typeA as TA
import mr_haran

PASS, FAIL, SKIP = [], [], []


def check(n, c, d=""):
    (PASS if c else FAIL).append(n)
    print(f"  [{'PASS' if c else 'FAIL'}] {n}" + (f" — {d}" if d and not c else ""))


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
"""
EDIT = PROG.replace("fold k in 1..n { k*k }", "fold k in 1..n { k*k + 0 }")


def typeA_integrated():
    fv = TA.FastVerifier()
    r1, s1 = fv.verify(PROG, workers=4)         # cold: all miss, verified in parallel
    r2, s2 = fv.verify(PROG, workers=4)         # warm: all hit
    r3, s3 = fv.verify(EDIT, workers=4)         # one edit: only s2 re-verified
    # correctness must equal the plain sequential verifier
    seq = [(r.name, r.verdict) for r in mr_haran.verify_program(PROG)]
    ok = (s1.misses == 3 and s2.hits == 3 and s2.misses == 0
          and s3.reverified == ["s2"]
          and [(r.name, r.verdict) for r in r1] == seq)
    check("typeA_integrated", ok, f"cold miss={s1.misses} warm hit={s2.hits} edit re-ran={s3.reverified}")
    print(f"      → FastVerifier: cold {s1.misses} verified (parallel) → warm {s2.hits} cache hits → "
          f"edit re-verifies only {s3.reverified}. Verdicts == sequential Mr.Jeffrey.")


def verification_speed_summary():
    s = TA.verification_speed_summary()
    print("      " + TA.render(s).replace("\n", "\n      "))
    ok = (s.cache_edit_loop_speedup > 3 and s.parallel_speedup_4c > 1.5
          and (s.coq_unbounded_proven >= 5 or not s.coq_available))
    check("verification_speed_summary", ok,
          f"cache×{s.cache_edit_loop_speedup:.0f} par×{s.parallel_speedup_4c:.2f} coq={s.coq_unbounded_proven}")


if __name__ == "__main__":
    print("v16 Part A · A4 — Type A integration + verification speed")
    typeA_integrated(); verification_speed_summary()
    print(f"\nA4: {len(PASS)} passed, {len(FAIL)} failed, {len(SKIP)} skipped")
    sys.exit(1 if FAIL else 0)
