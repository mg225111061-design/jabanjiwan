"""v13 tests (M1-M2) — arbitrary-precision (bignum) codegen via GMP. Run: python3 test_v13.py

M1 bignum codegen: fold → mpz closed form (O(1), exact) + naive mpz loop (O(n), exact); linear
   recurrence → mpz companion (O(log n), exact). All cross-checked against Python's exact integers,
   at sizes where the v11/v12 long-long path OVERFLOWS (the motivation for v13).
M2 GMP availability: gmp_available() gates the build; if GMP were absent, compile reports BLOCKED
   honestly (verified by forcing the gate off) — never a silent lossy fallback.
"""
import sys

from haran_parser import parse
import haran_bignum as B
import haran_recur as R

PASS, FAIL, SKIP = [], [], []
try:
    sys.set_int_max_str_digits(1_000_000)
except AttributeError:
    pass


def check(n, c, d=""):
    (PASS if c else FAIL).append(n)
    print(f"  [{'PASS' if c else 'FAIL'}] {n}" + (f" — {d}" if d and not c else ""))


def skip(n, w):
    SKIP.append(n)
    print(f"  [SKIP] {n} — {w}")


def _fn(s):
    return parse(s).items[0]


def _pysum(body, n):
    return sum(eval(body) for k in range(1, n + 1))   # body in terms of k (test-local oracle)


def _pyfib(n):
    a, b = 0, 1
    for _ in range(n):
        a, b = b, a + b
    return a


FIB = "fn fib(n: Nat) -> Nat effects pure { match n { 0 => 0  1 => 1  _ => fib(n-1) + fib(n-2) } }"
S2 = "fn s(n: Nat) -> Nat effects pure { fold k in 1..n { k*k } }"
S3 = "fn s(n: Nat) -> Nat effects pure { fold k in 1..n { k*k*k } }"


# ---- M1 ----
def bignum_fold_closed_exact():
    if not B.gmp_available():
        skip("bignum_fold_closed_exact", "GMP not available"); return
    cl = B.compile_bignum(B.emit_fold_closed_mpz(_fn(S2)))
    # n=1e7 → Σk² ≈ 3.3e20 (21 digits) — OVERFLOWS i64 (max ~9.2e18); mpz closed form is exact, O(1).
    ok = cl.ok and all(B.run_bignum(cl.binary, n) == _pysum("k*k", n) for n in (100, 100_000, 10_000_000))
    big = B.run_bignum(cl.binary, 10_000_000)
    check("bignum_fold_closed_exact", ok, cl.detail)
    print(f"      → Σk² closed form over mpz, EXACT & O(1); n=1e7 = {big} ({len(str(big))} digits, i64 overflows)")


def bignum_fold_naive_matches_closed():
    if not B.gmp_available():
        skip("bignum_fold_naive_matches_closed", "GMP not available"); return
    cl = B.compile_bignum(B.emit_fold_closed_mpz(_fn(S2)))
    nv = B.compile_bignum(B.emit_fold_naive_mpz(_fn(S2)))
    ok = cl.ok and nv.ok and all(
        B.run_bignum(cl.binary, n) == B.run_bignum(nv.binary, n) == _pysum("k*k", n)
        for n in (1, 50, 1000, 200_000))
    check("bignum_fold_naive_matches_closed", ok, "closed mpz == naive mpz == Python")
    print("      → O(1) closed mpz ≡ O(n) naive mpz ≡ Python exact — the bignum collapse is verified.")


def bignum_fold_cube():
    if not B.gmp_available():
        skip("bignum_fold_cube", "GMP not available"); return
    cl = B.compile_bignum(B.emit_fold_closed_mpz(_fn(S3)))
    ok = cl.ok and all(B.run_bignum(cl.binary, n) == _pysum("k*k*k", n) for n in (10, 1000, 5_000_000))
    check("bignum_fold_cube", ok, "Σk³ closed mpz exact (generality beyond Σk²)")
    print(f"      → Σk³ closed form over mpz exact at n=5e6 ({len(str(B.run_bignum(cl.binary,5_000_000)))} digits).")


def bignum_companion_exact():
    if not B.gmp_available():
        skip("bignum_companion_exact", "GMP not available"); return
    fc = B.compile_bignum(B.emit_companion_mpz(_fn(FIB)))
    ok = fc.ok and all(B.run_bignum(fc.binary, n) == _pyfib(n) for n in (0, 1, 10, 100, 1000, 10_000))
    huge = B.run_bignum(fc.binary, 100_000)
    ok = ok and huge == _pyfib(100_000)
    check("bignum_companion_exact", ok, fc.detail)
    print(f"      → Fibonacci companion over mpz: EXACT & O(log n); fib(100000) = {len(str(huge))} digits "
          f"(naive recursion would need 2^100000 steps — infeasible).")


def i64_overflow_motivation():
    if not B.gmp_available():
        skip("i64_overflow_motivation", "GMP not available"); return
    fib = _fn(FIB)
    comp = R.detect_companion(fib)
    # v12 long-long companion
    import tempfile, os, subprocess
    ll = R.emit_companion_program(fib, comp)
    d = tempfile.mkdtemp(); cp, bp = os.path.join(d, "f.c"), os.path.join(d, "f")
    open(cp, "w").write(ll)
    subprocess.run(["cc", "-O2", cp, "-o", bp], capture_output=True)
    ll200 = R.run_native(bp, 200)
    big = B.compile_bignum(B.emit_companion_mpz(fib))
    big200 = B.run_bignum(big.binary, 200)
    exact = _pyfib(200)
    # the WHOLE point of v13: long long is WRONG at n=200 (overflow); bignum is exact.
    ok = (ll200 != exact) and (big200 == exact)
    check("i64_overflow_motivation", ok, f"i64={ll200} exact={exact}")
    print(f"      → fib(200): i64(v12)={ll200} (WRONG — overflowed); bignum(v13) exact ✓. THIS is why v13 exists.")


# ---- M2 ----
def gmp_available_here():
    # this environment HAS GMP (libgmp + gmp.h via the dev package) → M1 ran for real.
    ok = B.gmp_available()
    check("gmp_available_here", ok, "GMP present (libgmp.so + gmp.h); M1 executed")
    if ok:
        print("      → gmp_available()=True: bignum path compiled & linked with -lgmp (real GMP).")


def blocked_path_is_honest():
    # If GMP were ABSENT, compile must report BLOCKED honestly — NOT silently fall back to lossy i64.
    saved = B.gmp_available
    B.gmp_available = lambda: False
    try:
        res = B.compile_bignum(B.emit_companion_mpz(_fn(FIB)))
        ok = (not res.ok) and "BLOCKED" in res.detail and "GMP" in res.detail
    finally:
        B.gmp_available = saved
    check("blocked_path_is_honest", ok, "compile_bignum returns BLOCKED when GMP absent")
    print("      → forced GMP-absent: compile_bignum → BLOCKED (honest); no silent lossy i64 fallback.")


if __name__ == "__main__":
    print("v13 — arbitrary-precision (bignum) codegen via GMP")
    print(f"[M2 gate] gmp_available = {B.gmp_available()}")
    print("[M1 bignum codegen]")
    bignum_fold_closed_exact(); bignum_fold_naive_matches_closed(); bignum_fold_cube()
    bignum_companion_exact(); i64_overflow_motivation()
    print("[M2 availability / honesty]")
    gmp_available_here(); blocked_path_is_honest()
    print(f"\nv13: {len(PASS)} passed, {len(FAIL)} failed, {len(SKIP)} skipped")
    sys.exit(1 if FAIL else 0)
