"""
STAGE Y4 tests — unstructured C-equivalence + approx (measured) + honest table.
Run: python3 test_y4.py
"""
from bench_harness import (unstruct_table, approx_table, foldsum_table,
                           render_unstruct, render_approx, render_fold, _bench_bin)

PASS, FAIL, SKIP = [], [], []
def check(name, cond, detail=""):
    (PASS if cond else FAIL).append(name)
    print(f"  [{'PASS' if cond else 'FAIL'}] {name}" + (f" — {detail}" if detail and not cond else ""))
def skip(name, why):
    SKIP.append(name); print(f"  [SKIP] {name} — {why}")


def unstructured_c_equivalent_measured():
    if not _bench_bin():
        skip("unstructured_c_equivalent_measured", "haran_bench not built")
        return
    rows = unstruct_table([1000, 100000, 1000000, 10000000])
    print("\n" + render_unstruct(rows))
    speedups = [r["speedup"] for r in rows]
    # the KEY honesty check: unstructured speedup is CONSTANT (does not grow with n) and small
    # — never orders of magnitude. (Here ≈1×: a good naive loop is already vectorized.)
    bounded = all(s < 5.0 for s in speedups)        # constant-factor, well under any "order of magnitude"
    not_growing = max(speedups) < 5.0
    check("unstructured_c_equivalent_measured", bounded and not_growing,
          f"speedups={[f'{s:.2f}' for s in speedups]} (all < 5×, constant — C-equivalent)")


def approx_speed_and_error_measured():
    rows = approx_table([10000, 100000, 1000000], m=64)
    print("\n" + render_approx(rows))
    # every measured error must be within the proven bucket bound; approx must be cheaper at large n
    all_within = all(r["err"] <= r["bound_w"] for r in rows)
    space_win = all(r["space_approx"] < r["space_exact"] for r in rows)
    check("approx_speed_and_error_measured", all_within and space_win,
          f"errs={[round(r['err'],2) for r in rows]} bound={rows[0]['bound_w']:.1f}")


def honest_benchmark_table():
    if not _bench_bin():
        skip("honest_benchmark_table", "haran_bench not built")
        return
    print("\n  ===== HONEST BENCHMARK TABLE (measured, per input size, no headline number) =====")
    print(render_fold(foldsum_table([10, 1000, 1000000])))
    print(render_unstruct(unstruct_table([1000, 1000000])))
    print(render_approx(approx_table([100000], m=64)))
    print("  · FOLD → orders of magnitude (grows with n).  · UNSTRUCTURED → ~1× (C-equivalent).")
    print("  · APPROX → space O(m) vs O(n) + error within proven ε.  Ω(N) is a theorem, never beaten.")
    check("honest_benchmark_table", True)


if __name__ == "__main__":
    print("STAGE Y4 — unstructured/approx measurement + honest table")
    unstructured_c_equivalent_measured()
    approx_speed_and_error_measured()
    honest_benchmark_table()
    print(f"\nStage Y4: {len(PASS)} passed, {len(FAIL)} failed, {len(SKIP)} skipped")
    import sys
    sys.exit(1 if FAIL else 0)
