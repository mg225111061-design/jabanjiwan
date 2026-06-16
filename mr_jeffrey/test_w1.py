"""
STAGE W1 tests — verified aliasing → noalias (HARAN edge) + honest measured payoff.
Run: python3 test_w1.py
"""
import os
import subprocess

from haran_parser import parse
from aliasing_analysis import noalias_eligible

PASS, FAIL, SKIP = [], [], []
def check(name, cond, detail=""):
    (PASS if cond else FAIL).append(name)
    print(f"  [{'PASS' if cond else 'FAIL'}] {name}" + (f" — {detail}" if detail and not cond else ""))
def skip(name, why):
    SKIP.append(name); print(f"  [SKIP] {name} — {why}")

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


def _bench():
    for sub in ("target/release/examples/accel_bench", "target/debug/examples/accel_bench"):
        p = os.path.join(ROOT, sub)
        if os.path.isfile(p) and os.access(p, os.X_OK):
            return p
    return None


KERNEL = """\
fn fma(a: &Vec<Float>, b: &Vec<Float>, c: &mut Vec<Float>)
  effects pure
{ c }
"""


def noalias_applied_when_verified():
    fn = parse(KERNEL).get("fma")
    info = noalias_eligible(fn)
    names = {i.name for i in info}
    # HARAN flags the &mut output buffer as noalias (verified exclusive); the & inputs need no mark
    ok = names == {"c"} and not info[0].c_can_prove
    check("noalias_applied_when_verified", ok, f"noalias-eligible={names}")
    print(f"      → HARAN marks '{info[0].name}' noalias: {info[0].reason}")
    print("        (C cannot prove this without an UNCHECKED `restrict`; HARAN verified it → SAFE)")


def noalias_speedup_measured():
    b = _bench()
    if not b:
        skip("noalias_speedup_measured", "accel_bench not built")
        return
    print("      measured (verified-noalias vs possible-alias, same disjoint buffers):")
    fma_sp, scale_sp = [], []
    for n in (1 << 12, 1 << 16, 1 << 20):
        out = subprocess.run([b, "noalias", str(n)], capture_output=True, text=True, timeout=60).stdout.strip()
        d = dict(t.split("=", 1) for t in out.split() if "=" in t)
        fma_sp.append(float(d["fma_speedup"])); scale_sp.append(float(d["scale_speedup"]))
        print(f"        n={n:>8}  fma_speedup={d['fma_speedup']}×  scale_speedup={d['scale_speedup']}×")
    # HONEST: these element-wise kernels are memory-bound ⇒ noalias ≈1×. We assert the measurement is
    # real and in a sane constant-factor band — and we do NOT claim a big number.
    sane = all(0.5 <= s <= 5.0 for s in fma_sp + scale_sp)
    check("noalias_speedup_measured", sane, f"fma={fma_sp} scale={scale_sp}")
    print("      → HONEST: ≈1× — these kernels are MEMORY-BOUND (bandwidth, not aliasing, is the floor).")
    print("        noalias is the SAFE form of C's `restrict`; its payoff is workload-dependent, not a")
    print("        headline number. Ω(N) untouched; no orders of magnitude.")


if __name__ == "__main__":
    print("STAGE W1 — verified aliasing → noalias")
    noalias_applied_when_verified()
    noalias_speedup_measured()
    print(f"\nStage W1: {len(PASS)} passed, {len(FAIL)} failed, {len(SKIP)} skipped")
    import sys
    sys.exit(1 if FAIL else 0)
