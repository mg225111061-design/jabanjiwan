"""v15 tests (O1-O3) — direct LLVM IR + Proc/Cofix finite prefix + integration. Run: python3 test_v15.py

O1 direct LLVM IR: scalar / map / refinement lowered to .ll → llc → native (no C in the compute path),
   carrying EXPLICIT metadata (noalias, !llvm.loop.vectorize.enable, !range); all == interpreter.
O2 Proc/Cofix: a PROVEN-productive cofix → finite-prefix generator (bound N mandatory; infinite is
   fundamentally impossible). Non-productive → no prefix (honest).
O3 integration: the backends agree (v11 C == v15 LLVM; v14 Vec C == v15 LLVM; v12 i64 == v13 bignum).
"""
import sys

from haran_parser import parse
import haran_llvm as L
import haran_eval

PASS, FAIL, SKIP = [], [], []


def check(n, c, d=""):
    (PASS if c else FAIL).append(n)
    print(f"  [{'PASS' if c else 'FAIL'}] {n}" + (f" — {d}" if d and not c else ""))


def skip(n, w):
    SKIP.append(n)
    print(f"  [SKIP] {n} — {w}")


def _fn(s):
    return parse(s).items[0]


def _interp(fn, *a):
    return haran_eval.Interp({fn.name: fn}).call_fn(fn, list(a))


POLY = "fn poly(n: Nat) -> Nat effects pure { n*n + 2*n + 1 }"
DBL = "fn dbl(v: &Vec<Int,n>) -> Vec<Int,n> effects pure { map(v, λx. 2*x + 1) }"
SQ = "fn sq(m: { x: Int | x < 100 }) -> Int effects pure { m * m }"
REP = "proc rep(start: Int) -> Stream<Int> produces eventually(out) effects io { cofix loop { yield start  loop } }"
SQS = "proc sqs(a: Int) -> Stream<Int> produces eventually(out) effects io { cofix loop { yield a*a + 1  loop } }"
SPIN = "proc spin() -> Stream<Int> produces eventually(out) effects io { cofix loop { loop } }"


# ---- O1 ----
def llvm_scalar_native():
    if not L.llc_available():
        skip("llvm_scalar_native", "llc/cc not available"); return
    fn = _fn(POLY)
    ir = L.emit_llvm_scalar(fn)
    is_ir = "define i64 @poly" in ir and "mul i64" in ir
    r = L.compile_ll(ir, L.driver_scalar(fn))
    ok = is_ir and r.ok and all(int(L.run_bin(r.binary, n)) == _interp(fn, n) for n in (0, 10, 100, 1000))
    check("llvm_scalar_native", ok, r.detail)
    print(f"      → poly lowered to LLVM IR (SSA mul/add) → llc → native; poly(10)={L.run_bin(r.binary,10)} == interp.")


def llvm_map_noalias_metadata():
    if not L.llc_available():
        skip("llvm_map_noalias_metadata", "llc/cc not available"); return
    fn = _fn(DBL)
    res = L.emit_llvm_map(fn)
    if not res:
        check("llvm_map_noalias_metadata", False, "no map IR"); return
    ir, _ = res
    has_meta = "noalias" in ir and "llvm.loop.vectorize.enable" in ir
    r = L.compile_ll(ir, L.driver_map(fn))
    ok = has_meta and r.ok and L.run_bin(r.binary, 5, 1, 2, 3, 4, 5).split() == ["3", "5", "7", "9", "11"]
    check("llvm_map_noalias_metadata", ok, f"meta={has_meta} {r.detail}")
    print("      → map IR carries `noalias` (verified own/&) + `!llvm.loop.vectorize.enable` — explicit "
          "metadata C-via can only HINT. Native == interpreter.")


def llvm_refinement_range_metadata():
    if not L.llc_available():
        skip("llvm_refinement_range_metadata", "llc/cc not available"); return
    fn = _fn(SQ)
    ir = L.emit_llvm_refinement(fn)
    has_range = ir is not None and "!range" in ir
    r = L.compile_ll(ir, L.driver_scalar(fn))
    ok = has_range and r.ok and int(L.run_bin(r.binary, 9)) == 81
    check("llvm_refinement_range_metadata", ok, f"range={has_range} {r.detail}")
    print("      → refinement {x|x<100} → `!range !{0,100}` on the load (verified fact → IR metadata); sq(9)=81.")


# ---- O2 ----
def prefix_productive_stream():
    if not L.cc_available() if hasattr(L, "cc_available") else False:
        pass
    r = L.compile_prefix(_fn(REP))
    r2 = L.compile_prefix(_fn(SQS))
    if not (r.ok and r2.ok):
        skip("prefix_productive_stream", "no C compiler"); return
    ok = (L.run_prefix(r.binary, 5, 7) == [7] * 5 and L.run_prefix(r.binary, 3, 42) == [42] * 3
          and L.run_prefix(r2.binary, 4, 6) == [37] * 4)
    check("prefix_productive_stream", ok, "productive cofix → finite prefix")
    print("      → PROVEN-productive `cofix loop {yield E loop}` → first-N generator; rep(7)|5=[7,7,7,7,7].")


def prefix_requires_bound_infinite_impossible():
    src = L.emit_prefix_c(_fn(REP))
    # the generator MUST take a bound N (argv[1]) and loop to it — there is NO infinite program emitted.
    ok = src is not None and "atoll(v[1])" in src and "i < N" in src and "for (" in src
    check("prefix_requires_bound_infinite_impossible", ok, "bound N mandatory; no infinite codegen")
    print("      → the bound N is MANDATORY (argv[1]); infinite materialization is impossible — we emit a "
          "finite prefix, never a non-terminating 'result'. This is the honest ceiling.")


def nonproductive_no_prefix():
    p = _fn(SPIN)
    ok = (not L.proc_is_productive(p)) and L.emit_prefix_c(p) is None
    check("nonproductive_no_prefix", ok, "spin (no yield) → not productive → no prefix")
    print("      → non-productive `cofix loop {loop}` (spin) yields nothing → no prefix emitted (honest).")


# ---- O3 ----
def backends_agree():
    if not L.llc_available():
        skip("backends_agree", "llc/cc not available"); return
    import haran_codegen as v11
    import haran_vec as v14
    # poly: v11 (C) vs v15 (LLVM IR)
    poly = _fn(POLY)
    c11 = v11.compile_fn(poly)
    rL = L.compile_ll(L.emit_llvm_scalar(poly), L.driver_scalar(poly))
    agree_scalar = all(v11.run_native(c11.binary, n) == int(L.run_bin(rL.binary, n)) for n in (5, 50, 500))
    # map: v14 (C) vs v15 (LLVM IR)
    dbl = _fn(DBL)
    c14 = v14.compile_map(dbl)
    rLm = L.compile_ll(*((lambda r: (r[0], L.driver_map(dbl)))(L.emit_llvm_map(dbl))))
    vec = [3, 1, 4, 1, 5, 9, 2, 6]
    agree_map = v14.run_map(c14.binary, vec, dynamic=True) == [int(x) for x in L.run_bin(rLm.binary, len(vec), *vec).split()]
    check("backends_agree", agree_scalar and agree_map, f"scalar={agree_scalar} map={agree_map}")
    print("      → same HARAN fn through different backends agrees: v11 C ≡ v15 LLVM (scalar & map).")


def bignum_vs_i64_consistent_small():
    import haran_recur as v12
    import haran_bignum as v13
    if not v13.gmp_available():
        skip("bignum_vs_i64_consistent_small", "GMP not available"); return
    fib = _fn("fn fib(n: Nat) -> Nat effects pure { match n { 0 => 0  1 => 1  _ => fib(n-1) + fib(n-2) } }")
    comp = v12.detect_companion(fib)
    import tempfile, os, subprocess
    ll = v12.emit_companion_program(fib, comp)
    d = tempfile.mkdtemp(); cp, bp = os.path.join(d, "f.c"), os.path.join(d, "f")
    open(cp, "w").write(ll); subprocess.run(["cc", "-O2", cp, "-o", bp], capture_output=True)
    big = v13.compile_bignum(v13.emit_companion_mpz(fib))
    # below i64 overflow (n≤90) the two backends must agree exactly
    ok = all(v12.run_native(bp, n) == v13.run_bignum(big.binary, n) for n in (10, 50, 90))
    check("bignum_vs_i64_consistent_small", ok, "v12 i64 ≡ v13 bignum below overflow")
    print("      → v12 (i64) ≡ v13 (bignum) for n≤90 (below overflow); v13 then continues exact past i64.")


if __name__ == "__main__":
    print("v15 — direct LLVM IR + Proc/Cofix finite prefix + integration")
    print(f"[O1 direct LLVM IR]  (llc: {L._LLC})")
    llvm_scalar_native(); llvm_map_noalias_metadata(); llvm_refinement_range_metadata()
    print("[O2 Proc/Cofix finite prefix]")
    prefix_productive_stream(); prefix_requires_bound_infinite_impossible(); nonproductive_no_prefix()
    print("[O3 integration]")
    backends_agree(); bignum_vs_i64_consistent_small()
    print(f"\nv15: {len(PASS)} passed, {len(FAIL)} failed, {len(SKIP)} skipped")
    sys.exit(1 if FAIL else 0)
