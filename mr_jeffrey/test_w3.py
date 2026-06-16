"""STAGE W3 tests — cache / SoA layout + SIMD overlap. Run: python3 test_w3.py"""
from accel_harness import run, accel_bin
PASS, FAIL, SKIP = [], [], []
def check(n, c, d=""):
    (PASS if c else FAIL).append(n); print(f"  [{'PASS' if c else 'FAIL'}] {n}" + (f" — {d}" if d and not c else ""))
def skip(n, w): SKIP.append(n); print(f"  [SKIP] {n} — {w}")

def soa_layout_correct():
    if not accel_bin(): skip("soa_layout_correct", "accel_bench not built"); return
    r = run("soa", 1 << 18); check("soa_layout_correct", r.get("correct") is True, str(r))

def cache_speedup_measured():
    if not accel_bin(): skip("cache_speedup_measured", "accel_bench not built"); return
    small = run("soa", 1 << 16); large = run("soa", 1 << 22)
    print(f"      n=64K  (cache-resident): AoS→SoA {small['soa_speedup']}×")
    print(f"      n=4M   (memory-bound):   AoS→SoA {large['soa_speedup']}×")
    # SoA helps when data exceeds cache (better bandwidth utilization); ~1× when cache-resident
    ok = large["soa_speedup"] >= 1.3 and small["correct"]
    check("cache_speedup_measured", ok, f"soa {small['soa_speedup']}/{large['soa_speedup']}×")

def simd_cache_overlap_measured():
    if not accel_bin(): skip("simd_cache_overlap_measured", "accel_bench not built"); return
    large = run("soa", 1 << 22)
    combined = large["soa_speedup"] * large["soa_then_simd_extra"]
    print(f"      n=4M: SoA(cache) {large['soa_speedup']}× × SIMD-on-SoA {large['soa_then_simd_extra']}× "
          f"= {combined:.1f}× combined")
    # ★ the point: stacked techniques give a bounded CONSTANT factor (~single digits), NOT a wild
    #   product (8×8=64). They share the memory path. And Ω(N) is intact (orders of magnitude impossible).
    ok = combined < 20.0
    check("simd_cache_overlap_measured", ok, f"combined={combined:.1f}× (must be a bounded constant, not a product)")
    print("      → HONEST: techniques OVERLAP on the memory bottleneck — bounded constant factor, not a product.")

if __name__ == "__main__":
    print("STAGE W3 — cache / SoA + SIMD overlap")
    soa_layout_correct(); cache_speedup_measured(); simd_cache_overlap_measured()
    print(f"\nStage W3: {len(PASS)} passed, {len(FAIL)} failed, {len(SKIP)} skipped")
    import sys; sys.exit(1 if FAIL else 0)
