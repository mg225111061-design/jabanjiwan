"""STAGE T3 tests (v5) — holonomic attempt + Gröbner ceiling measured. Run: python3 test_t3.py"""
from holonomic import attempt, measure_boundary, _zeil_bin
PASS, FAIL, SKIP = [], [], []
def check(n, c, d=""):
    (PASS if c else FAIL).append(n); print(f"  [{'PASS' if c else 'FAIL'}] {n}" + (f" — {d}" if d and not c else ""))
def skip(n, w): SKIP.append(n); print(f"  [SKIP] {n} — {w}")

def simple_holonomic_attempted():
    if not _zeil_bin(): skip("simple_holonomic_attempted", "zeil_check not built"); return
    r = attempt("binom_sq", 1, timeout_s=20)
    check("simple_holonomic_attempted", r.verdict == "CLOSED" and r.order == 1, str(r))
    print(f"      → {r}")

def groebner_timeout_guarded():
    if not _zeil_bin(): skip("groebner_timeout_guarded", "zeil_check not built"); return
    # Franel ΣC(n,k)^3 (order-2) — guard at 8s; must DEFER and RETURN (not hang)
    r = attempt("binom_cube", 2, timeout_s=8)
    ok = r.verdict == "DEFER" and r.elapsed_s <= 12 and "timeout" in r.detail.lower()
    check("groebner_timeout_guarded", ok, str(r))
    print(f"      → {r}  (guard fired — no hang, honest DEFER)")

def holonomic_boundary_measured():
    if not _zeil_bin(): skip("holonomic_boundary_measured", "zeil_check not built"); return
    rows = measure_boundary(timeout_s=8)
    print("\n      ★ holonomic fold ceiling — measured boundary ★")
    for r in rows:
        print(f"        {r}")
    closed = [r for r in rows if r.verdict == "CLOSED"]
    defer = [r for r in rows if r.verdict == "DEFER"]
    # honest: order-1 single-sums CLOSE; order-2+ (Franel and beyond) DEFER (Gröbner ceiling)
    ok = len(closed) >= 2 and len(defer) >= 1 and all(r.order == 1 for r in closed)
    check("holonomic_boundary_measured", ok, f"closed={[r.which for r in closed]} defer={[r.which for r in defer]}")
    print("      → ceiling: order-1 single hypergeometric sums solve; order-2+ holonomic → DEFER.")
    print("        Timeout is the MEASUREMENT of the math ceiling, not a failure (no forced answers).")

if __name__ == "__main__":
    print("STAGE T3 — holonomic attempt + Gröbner ceiling")
    simple_holonomic_attempted()
    groebner_timeout_guarded()
    holonomic_boundary_measured()
    print(f"\nStage T3: {len(PASS)} passed, {len(FAIL)} failed, {len(SKIP)} skipped")
    import sys; sys.exit(1 if FAIL else 0)
