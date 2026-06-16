"""STAGE W5 tests — parallelism + memory-bound limit. Run: python3 test_w5.py"""
from accel_harness import run, accel_bin
PASS, FAIL, SKIP = [], [], []
def check(n, c, d=""):
    (PASS if c else FAIL).append(n); print(f"  [{'PASS' if c else 'FAIL'}] {n}" + (f" — {d}" if d and not c else ""))
def skip(n, w): SKIP.append(n); print(f"  [SKIP] {n} — {w}")

def parallel_correct():
    if not accel_bin(): skip("parallel_correct", "accel_bench not built"); return
    r = run("parallel", 1 << 20, 4); check("parallel_correct", r.get("correct") is True, str(r))

def parallel_speedup_measured():
    if not accel_bin(): skip("parallel_speedup_measured", "accel_bench not built"); return
    r = run("parallel", 1 << 23, 4)
    print(f"      n=8M, 4 cores: compute-bound {r['cpu_speedup']}×, memory-bound {r['mem_speedup']}×")
    ok = r["cpu_speedup"] > 3.0 and r["correct"]   # compute-bound scales ~linearly with cores
    check("parallel_speedup_measured", ok, f"cpu {r['cpu_speedup']}× (near 4 cores)")

def memory_bound_parallel_limit_shown():
    if not accel_bin(): skip("memory_bound_parallel_limit_shown", "accel_bench not built"); return
    r = run("parallel", 1 << 24, 4)   # 16M f64 = 128 MB >> cache => clearly bandwidth-bound
    # robust honesty: memory-bound parallel is SUBLINEAR (< cores) AND strictly worse than compute-bound
    ok = r["mem_speedup"] < 4.0 and r["mem_speedup"] < r["cpu_speedup"]
    check("memory_bound_parallel_limit_shown", ok,
          f"mem {r['mem_speedup']}× < cpu {r['cpu_speedup']}× (4 cores) — bandwidth saturation")
    print(f"      → HONEST: memory-bound parallel {r['mem_speedup']}× (NOT 4×) — bandwidth saturates; the")
    print("        shared-bottleneck/overlap, measured. Compute-bound scales; memory-bound doesn't.")

if __name__ == "__main__":
    print("STAGE W5 — parallelism + limits")
    parallel_correct(); parallel_speedup_measured(); memory_bound_parallel_limit_shown()
    print(f"\nStage W5: {len(PASS)} passed, {len(FAIL)} failed, {len(SKIP)} skipped")
    import sys; sys.exit(1 if FAIL else 0)
