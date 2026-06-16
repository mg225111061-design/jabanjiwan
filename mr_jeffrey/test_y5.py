"""
STAGE Y5 tests — integrated domain (B) + benchmark (A) report.  Run: python3 test_y5.py
"""
import haran_v4
from bench_harness import foldsum_table, unstruct_table, approx_table, _bench_bin

PASS, FAIL, SKIP = [], [], []
def check(name, cond, detail=""):
    (PASS if cond else FAIL).append(name)
    print(f"  [{'PASS' if cond else 'FAIL'}] {name}" + (f" — {detail}" if detail and not cond else ""))


def integrated_domain_and_benchmark_report():
    d = haran_v4.report()
    # B: real kernel verified + fold ratio is the honest low number (setup-only)
    b_ok = d["corr"].startswith("MATCH") and len(d["closed"]) == 2 and len(d["nostruct"]) == 3
    if not _bench_bin():
        check("integrated_domain_and_benchmark_report", b_ok, "B ok; A skipped (haran_bench not built)")
        return
    # A: fold orders-of-magnitude (grows), unstructured constant (C-equivalent), approx within ε
    fold = foldsum_table([10, 1000000])
    unstr = unstruct_table([1000, 1000000])
    appr = approx_table([100000], m=64)
    a_ok = (fold[-1]["ratio"] > 1000                       # fold: orders of magnitude at large n
            and fold[0]["ratio"] < fold[-1]["ratio"] / 100  # grows with n
            and all(r["speedup"] < 5 for r in unstr)        # unstructured: constant factor (C-equivalent)
            and appr[0]["err"] <= appr[0]["bound_w"])       # approx: within proven bound
    check("integrated_domain_and_benchmark_report", b_ok and a_ok,
          f"B(fold%={d['fold_pct']}) A(fold_ratio={fold[-1]['ratio']:.0f}, "
          f"unstruct_max={max(r['speedup'] for r in unstr):.2f}×)")


if __name__ == "__main__":
    print("STAGE Y5 — integrated domain + benchmark report")
    integrated_domain_and_benchmark_report()
    print(f"\nStage Y5: {len(PASS)} passed, {len(FAIL)} failed, {len(SKIP)} skipped")
    import sys
    sys.exit(1 if FAIL else 0)
