"""STAGE T4 tests (v5) — Kovacic Liouvillian decision. Run: python3 test_t4.py"""
import os, subprocess
from kovacic import kovacic
PASS, FAIL, SKIP = [], [], []
def check(n, c, d=""):
    (PASS if c else FAIL).append(n); print(f"  [{'PASS' if c else 'FAIL'}] {n}" + (f" — {d}" if d and not c else ""))
def skip(n, w): SKIP.append(n); print(f"  [SKIP] {n} — {w}")
ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))

def kovacic_liouvillian_found():
    r1 = kovacic("1")            # y''=y → exp(±x)
    r2 = kovacic("x**2 + 1")     # y''=(x²+1)y → ω=x, y=exp(x²/2)
    ok = r1.verdict == "CLOSED" and r2.verdict == "CLOSED"
    check("kovacic_liouvillian_found", ok, f"y''=y: {r1}; y''=(x²+1)y: {r2}")
    print(f"      → y''=y: {r1}")
    print(f"      → y''=(x²+1)y: {r2}")

def kovacic_absence_proven():
    airy = kovacic("x")          # Airy y''=xy → NO Liouvillian
    ok = airy.verdict == "ABSENT" and "Airy" in airy.detail
    check("kovacic_absence_proven", ok, str(airy))
    print(f"      → Airy y''=x·y: {airy}")
    # honesty: Case-1-only failure (y''=x²y) is UNKNOWN, not ABSENT
    pc = kovacic("x**2")
    check("kovacic_unknown_not_absent", pc.verdict == "UNKNOWN", str(pc))
    print(f"      → y''=x²·y: {pc.verdict} (Case 1 fails; Cases 2/3 out of scope → UNKNOWN, not ABSENT)")

def kovacic_consistent_with_galois():
    # Kovacic's ABSENT (Airy) is the differential-Galois absence — consistent with v2 galois.rs
    # erf_elementary_absence (Liouville integral case). Both: non-solvable ⇒ no closed form.
    airy = kovacic("x")
    gbin = None
    for sub in ("target/release/examples/galois_absence", "target/debug/examples/galois_absence"):
        p = os.path.join(ROOT, sub)
        if os.path.isfile(p): gbin = p; break
    if not gbin:
        skip("kovacic_consistent_with_galois", "galois_absence not built"); return
    erf = subprocess.run([gbin, "erf"], capture_output=True, text=True, timeout=20).stdout.strip()
    ok = airy.verdict == "ABSENT" and "no_closed_form=true" in erf
    check("kovacic_consistent_with_galois", ok, f"airy={airy.verdict}; erf={erf[:40]}")
    print(f"      → Kovacic Airy ABSENT ∥ galois.rs erf ELEMENTARY_ABSENT — both diff-Galois non-closure")

if __name__ == "__main__":
    print("STAGE T4 — Kovacic Liouvillian decision")
    kovacic_liouvillian_found()
    kovacic_absence_proven()
    kovacic_consistent_with_galois()
    print(f"\nStage T4: {len(PASS)} passed, {len(FAIL)} failed, {len(SKIP)} skipped")
    import sys; sys.exit(1 if FAIL else 0)
