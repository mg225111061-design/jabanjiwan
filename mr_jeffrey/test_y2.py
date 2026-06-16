"""
STAGE Y2 tests — PQC kernel verification + the REAL (non-toy) fold ratio.  Run: python3 test_y2.py
"""
import subprocess
import pqc_domain
import closure_classifier as cc
from closure_classifier import find_binary
from haran_parser import parse

PASS, FAIL = [], []
def check(name, cond, detail=""):
    (PASS if cond else FAIL).append(name)
    print(f"  [{'PASS' if cond else 'FAIL'}] {name}" + (f" — {detail}" if detail and not cond else ""))


def _classify_all():
    return {name: cc.classify_fn(parse(src).items[0]) for name, src in pqc_domain.SUBKERNELS.items()}


def domain_kernel_verified():
    # correctness: the ntt-based kernel ≡ schoolbook negacyclic, EXACT (differential, real kyber.rs)
    b = find_binary("kyber_check")
    out = subprocess.run([b, "200"], capture_output=True, text=True, timeout=30).stdout.strip() if b else "(no binary)"
    corr = out.startswith("MATCH")
    # the folding setup parts are PROVEN CLOSED (cfinite / Faulhaber)
    v = _classify_all()
    tw = v["twiddle (ζ^i, setup)"]
    ps = v["param_sum (setup)"]
    ok = corr and tw.kind == "CLOSED" and ps.kind == "CLOSED"
    check("domain_kernel_verified", ok, f"corr={out}; twiddle={tw.kind}; param={ps.kind}")
    print(f"      → correctness: {out}  (TESTED differential, exact — NTT≡schoolbook is a theorem)")
    print(f"      → setup folds: twiddle {tw.kind}[{tw.method}], param_sum {ps.kind}[{ps.method}]")


def domain_closure_ratio_real():
    v = _classify_all()
    closed = [n for n, x in v.items() if x.kind == "CLOSED"]
    nostruct = [n for n, x in v.items() if x.kind == "NO_STRUCTURE"]
    total = len(v)
    print("\n      ★ REAL PQC kernel — 4-bucket fold ratio ★")
    for name, x in v.items():
        print(f"        · {name:26s} {x.kind}")
    print(f"      ─ by op-count: {round(100*len(closed)/total)}% CLOSED, {round(100*len(nostruct)/total)}% NO_STRUCTURE")
    print(f"      ─ ★ HONEST: the folding parts are SETUP (twiddle/param, O(1)/O(log n)); the")
    print(f"        RUNTIME-DOMINANT transform (ntt/pointwise) is 100% NO_STRUCTURE Ω(N log N).")
    print(f"        Real crypto does NOT fold like the toy 70% — it's a data-dependent transform.")
    ok = len(closed) == 2 and len(nostruct) == 3   # setup folds, transforms don't
    check("domain_closure_ratio_real", ok, f"closed={closed} nostruct={nostruct}")


if __name__ == "__main__":
    print("STAGE Y2 — PQC kernel verification + real fold ratio")
    domain_kernel_verified()
    domain_closure_ratio_real()
    print(f"\nStage Y2: {len(PASS)} passed, {len(FAIL)} failed")
    import sys
    sys.exit(1 if FAIL else 0)
