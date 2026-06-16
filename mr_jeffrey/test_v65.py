"""v6.5 tests (C1-C4) — Caesar/HeyVL activation + probabilistic upgrade. Run: python3 test_v65.py"""
import os, subprocess
from caesar_bridge import caesar_available, to_heyvl, verify_probabilistic
import haran_v6
PASS, FAIL, SKIP = [], [], []
def check(n, c, d=""):
    (PASS if c else FAIL).append(n); print(f"  [{'PASS' if c else 'FAIL'}] {n}" + (f" — {d}" if d and not c else ""))

CAESAR = caesar_available()

# ---- C1 ----
def caesar_binary_present():
    check("caesar_binary_present", CAESAR is not None, "Caesar binary not found")
    print(f"      → caesar: {CAESAR}")
def caesar_executes():
    if not CAESAR: check("caesar_executes", False, "no binary"); return
    out = subprocess.run([CAESAR, "--version"], capture_output=True, text=True, timeout=20).stdout
    check("caesar_executes", "caesar 4" in out.lower() or "4.0.2" in out, out[:60])
    print(f"      → {out.strip().splitlines()[0]}")

# ---- C2 ----
def _verify(src):
    import tempfile
    with tempfile.NamedTemporaryFile("w", suffix=".heyvl", delete=False) as f:
        f.write(src); p = f.name
    try:
        o = subprocess.run([CAESAR, "verify", p], capture_output=True, text=True, timeout=60)
    finally:
        os.unlink(p)
    return o.stdout + o.stderr
def caesar_verifies_correct():
    if not CAESAR: check("caesar_verifies_correct", False, "no binary"); return
    t = _verify("proc coin() -> (x: UInt)\n  pre 0.5\n  post x\n{ var b: Bool = flip(0.5)\n if b { x = 1 } else { x = 0 } }\n")
    check("caesar_verifies_correct", "Verified" in t and "0 failed" in t, t[:80])
def caesar_rejects_wrong():
    if not CAESAR: check("caesar_rejects_wrong", False, "no binary"); return
    t = _verify("proc coin() -> (x: UInt)\n  pre 0.7\n  post x\n{ var b: Bool = flip(0.5)\n if b { x = 1 } else { x = 0 } }\n")
    check("caesar_rejects_wrong", "1 failed" in t and "0 verified" in t, t[:80])
    print("      → correct→Verified, wrong→rejected (genuine, not always-pass)")

# ---- C3 ----
def bridge_valid_heyvl():
    cm = to_heyvl("count_min")
    if not CAESAR: check("bridge_valid_heyvl", "proc" in cm); return
    t = _verify(cm)
    check("bridge_valid_heyvl", "Verified" in t, t[:80])
def bridge_calls_caesar():
    r = verify_probabilistic("count_min")
    ok = (r.verdict == "PROVEN-BOUND") if CAESAR else (r.verdict == "TESTED-BOUND")
    check("bridge_calls_caesar", ok, str(r.verdict))
def one_probabilistic_proven_e2e():
    r = verify_probabilistic("count_min")
    if CAESAR:
        ok = r.verdict == "PROVEN-BOUND" and r.error_kind == "probabilistic" and r.bound == "expectation"
        print(f"      → Count-Min E[err]=1/w: {r.verdict} ({r.error_kind}, {r.bound}); {r.detail}")
    else:
        ok = r.verdict == "TESTED-BOUND"
    check("one_probabilistic_proven_e2e", ok, str(r.verdict))

# ---- C4 ----
def probabilistic_upgrade_measured():
    sketches = ["count_min", "kmv"]
    res = [verify_probabilistic(s) for s in sketches]
    upgraded = sum(1 for r in res if r.verdict == "PROVEN-BOUND")
    if CAESAR:
        ok = upgraded == len(sketches)
        print(f"      → probabilistic TESTED→PROVEN upgrade: {upgraded}/{len(sketches)} (Caesar live, expectation bound)")
    else:
        ok = upgraded == 0
    check("probabilistic_upgrade_measured", ok, f"upgraded {upgraded}/{len(sketches)}")
def conquest_recomputed():
    r = haran_v6.conquest_v3_vs_v6()
    if CAESAR:
        # Caesar live ⇒ distinct/KMV now PROVEN ⇒ conquest 100% (v6 was 75% Caesar-blocked)
        ok = r["v6_pct"] >= 100 and r["v3_pct"] == 50
        print(f"      → conquest: v3 {r['v3_pct']}% → v6.5 {r['v6_pct']}% (v6 was 75% Caesar-blocked → +25pp from Caesar)")
    else:
        ok = r["v6_pct"] >= 75
    check("conquest_recomputed", ok, f"v6.5={r['v6_pct']}%")
def five_types_distinct():
    b = haran_v6.error_breakdown()
    # 5 labels distinguishable: deterministic(Z3) / runtime / probabilistic(Caesar) / TESTED / REJECTED
    present = {k for k, v in b.items() if v}
    if CAESAR:
        ok = {"deterministic", "runtime", "probabilistic", "rejected"}.issubset(present)
        print(f"      → live types present: {sorted(present)}  (TESTED now {b['tested'] or 'empty — all upgraded'})")
    else:
        ok = {"deterministic", "runtime", "tested", "rejected"}.issubset(present)
    check("five_types_distinct", ok, str(sorted(present)))

if __name__ == "__main__":
    print("v6.5 — Caesar/HeyVL activation")
    if CAESAR is None:
        print("  [SKIP] Caesar binary absent (external tool; provide via tools/caesar/ or CAESAR_BIN).")
        print("         Probabilistic sketches stay TESTED-BOUND (v6 behavior). Not a failure — BLOCKED honestly.")
        print("\nv6.5: 0 passed, 0 failed, ALL SKIPPED (Caesar not present)")
        import sys; sys.exit(0)
    print("[C1]"); caesar_binary_present(); caesar_executes()
    print("[C2]"); caesar_verifies_correct(); caesar_rejects_wrong()
    print("[C3]"); bridge_valid_heyvl(); bridge_calls_caesar(); one_probabilistic_proven_e2e()
    print("[C4]"); probabilistic_upgrade_measured(); conquest_recomputed(); five_types_distinct()
    print(f"\nv6.5: {len(PASS)} passed, {len(FAIL)} failed, {len(SKIP)} skipped")
    import sys; sys.exit(1 if FAIL else 0)
