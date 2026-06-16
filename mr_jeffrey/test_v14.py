"""v14 tests (N1-N2) — Vec codegen + own/& RAII. Run: python3 test_v14.py

N1 Vec codegen: map(v, λx. e) → C array loop, STATIC (Vec<T,N>) / DYNAMIC (Vec<T,n>) / SIMD; native
   matches the interpreter element-by-element. Non-map (filter/sorted) → honest DEFER.
N2 own/& RAII: ownership drives malloc/free — own freed once, & never freed; checked with ASan:
   correct = clean, leaky/double-free = CAUGHT (the check has teeth).
"""
import sys

from haran_parser import parse
import haran_vec as V
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


DYN = "fn dbl(v: &Vec<Int, n>) -> Vec<Int, n> effects pure { map(v, λx. 2*x + 1) }"
DYN2 = "fn cube1(v: own Vec<Int, n>) -> Vec<Int, n> effects pure { map(v, λx. x*x*x + 1) }"
STAT = "fn sq(v: own Vec<Int, 4>) -> Vec<Int, 4> effects pure { map(v, λx. x*x) }"


# ---- N1 ----
def map_dynamic_native_matches():
    if not V.cc_available():
        skip("map_dynamic_native_matches", "no C compiler"); return
    fn = _fn(DYN); c = V.compile_map(fn)
    vecs = [[1, 2, 3], [0], [-5, 7, 100, -1, 42], list(range(20))]
    ok = c.ok and c.mode == "dynamic" and all(V.run_map(c.binary, v, dynamic=True) == _interp(fn, v) for v in vecs)
    check("map_dynamic_native_matches", ok, c.detail)
    print(f"      → dynamic map(v, λx. 2x+1) native == interpreter; e.g. [1,2,3]→{V.run_map(c.binary,[1,2,3])}")


def map_static_native_matches():
    if not V.cc_available():
        skip("map_static_native_matches", "no C compiler"); return
    fn = _fn(STAT); c = V.compile_map(fn)
    fixed = "in[4]" in c.c_src and "malloc" not in c.c_src   # static ⇒ fixed buffer, no heap
    ok = c.ok and c.mode == "static" and fixed and V.run_map(c.binary, [3, 4, 5, 6], dynamic=False) == _interp(fn, [3, 4, 5, 6])
    check("map_static_native_matches", ok, f"static fixed-buffer={fixed}")
    print(f"      → static Vec<Int,4> → fixed C buffer (no malloc); [3,4,5,6]→{V.run_map(c.binary,[3,4,5,6],dynamic=False)}")


def map_dynamic_malloc_and_restrict():
    if not V.cc_available():
        skip("map_dynamic_malloc_and_restrict", "no C compiler"); return
    c = V.compile_map(_fn(DYN))
    ok = c.ok and "malloc" in c.c_src and "free(" in c.c_src and c.has_restrict and "restrict" in c.c_src
    check("map_dynamic_malloc_and_restrict", ok, "dynamic uses malloc/free + verified-noalias restrict")
    print("      → dynamic Vec<Int,n> → pointer+len (malloc/free); `restrict` SAFE from verified own/& "
          "(C can't prove this).")


def vec_simd_measured():
    if not V.cc_available():
        skip("vec_simd_measured", "no C compiler"); return
    s = V.map_simd_throughput(_fn(DYN2), 1 << 22)   # compute-heavier body
    if not s or not s.get("ok"):
        check("vec_simd_measured", False, str(s)); return
    # HONEST: verified-noalias restrict is SAFE & correct; this element-wise kernel is bandwidth-bound,
    # so the restrict-vs-plain delta is ~1× (both auto-vectorize). We assert correctness + measurement,
    # never an invented speedup.
    ratio = s["plain_ns"] / s["restrict_ns"] if s["restrict_ns"] else 0
    ok = s["correct"] and s["restrict_ns"] > 0
    check("vec_simd_measured", ok, s.get("raw"))
    print(f"      → SIMD (-march=native) over 4M elems: restrict {s['restrict_ns']/1e6:.1f}ms vs plain "
          f"{s['plain_ns']/1e6:.1f}ms (×{ratio:.2f}). Bandwidth-bound ⇒ ~1×; constant-factor, Ω(N) intact.")


def non_map_defers():
    # filter is length-changing / needs compaction → NOT codegen'd; honest DEFER (interpreter handles it).
    c = V.compile_map(_fn("fn f(v: &Vec<Int, n>) -> Vec<Int, n> effects pure { filter(v, λx. x > 0) }"))
    ok = (not c.ok) and "not a map" in c.detail
    check("non_map_defers", ok, "filter/sorted → honest DEFER (not claimed as codegen'd)")
    print("      → filter/sorted (length-changing / needs indexing) → honest DEFER, not faked as codegen.")


# ---- N2 ----
def raii_correct_asan_clean():
    if not V.asan_available():
        skip("raii_correct_asan_clean", "ASan not available"); return
    r = V.asan_check(V.emit_raii_program())
    ok = r.ok and r.clean and "sum=499500" in r.output
    check("raii_correct_asan_clean", ok, r.output)
    print(f"      → own→malloc, freed once at last use; & borrow never frees → ASan CLEAN ({r.output}).")


def raii_leak_caught():
    if not V.asan_available():
        skip("raii_leak_caught", "ASan not available"); return
    r = V.asan_check(V.emit_raii_program(leaky=True))
    ok = r.ok and (not r.clean)   # leak MUST be caught
    check("raii_leak_caught", ok, "leaky variant must be flagged by LeakSanitizer")
    print("      → dropping the owner's free → LeakSanitizer CATCHES it (the no-leak check has teeth).")


def raii_double_free_caught():
    if not V.asan_available():
        skip("raii_double_free_caught", "ASan not available"); return
    r = V.asan_check(V.emit_raii_program(double_free=True))
    ok = r.ok and (not r.clean)   # double-free MUST be caught
    check("raii_double_free_caught", ok, "double-free variant must be flagged by AddressSanitizer")
    print("      → a borrow freeing the buffer → AddressSanitizer CATCHES the double-free.")


def own_borrow_free_discipline():
    src = V.emit_raii_program()
    # exactly one free(v) (the owner); the & borrow function never frees its input
    one_owner_free = src.count("free(v)") == 1
    borrow_no_free = "free(in)" not in src
    ok = one_owner_free and borrow_no_free
    check("own_borrow_free_discipline", ok, f"owner_free={one_owner_free} borrow_no_free={borrow_no_free}")
    print("      → discipline: own frees exactly once; & borrow has zero frees. Driven by the types.")


if __name__ == "__main__":
    print("v14 — Vec codegen + own/& RAII")
    print("[N1 Vec codegen]")
    map_dynamic_native_matches(); map_static_native_matches()
    map_dynamic_malloc_and_restrict(); vec_simd_measured(); non_map_defers()
    print("[N2 own/& RAII]")
    raii_correct_asan_clean(); raii_leak_caught(); raii_double_free_caught(); own_borrow_free_discipline()
    print(f"\nv14: {len(PASS)} passed, {len(FAIL)} failed, {len(SKIP)} skipped")
    sys.exit(1 if FAIL else 0)
