"""STAGE U2 tests (v6) — Compressed Sensing runtime residual certificate. Run: python3 test_u2.py"""
from compressed_sensing import recover_and_certify, omp
import numpy as np
PASS, FAIL = [], []
def check(n, c, d=""):
    (PASS if c else FAIL).append(n); print(f"  [{'PASS' if c else 'FAIL'}] {n}" + (f" — {d}" if d and not c else ""))

def cs_recovery_works():
    r = recover_and_certify(n=256, k=8, m=80, noise=0.0)
    ok = r.verdict == "PROVEN-BOUND" and r.residual < 1e-6
    check("cs_recovery_works", ok, str(r))
    print(f"      → noiseless k=8/n=256/m=80: {r.detail}")

def cs_runtime_residual_certified():
    rows = []
    for noise in (0.0, 1e-3, 1e-2):
        r = recover_and_certify(n=256, k=8, m=80, noise=noise)
        rows.append((noise, r))
        print(f"      → noise={noise:.0e}: residual={r.residual:.2e} ≤ ε={r.epsilon:.2e} → {r.verdict} ({r.cert_kind})")
    ok = all(r.verdict == "PROVEN-BOUND" and r.residual <= r.epsilon for _, r in rows)
    check("cs_runtime_residual_certified", ok, str([(n, r.verdict) for n, r in rows]))

def rip_not_claimed_statically():
    r = recover_and_certify()
    ok = (r.rip_claimed_statically is False and r.cert_kind == "runtime (per-execution)"
          and "NOT a static RIP" in r.detail)
    check("rip_not_claimed_statically", ok, str(r))
    print("      → RIP NOT claimed statically (NP-hard); certificate is PER-EXECUTION runtime residual.")

if __name__ == "__main__":
    print("STAGE U2 — Compressed Sensing runtime residual certificate")
    cs_recovery_works()
    cs_runtime_residual_certified()
    rip_not_claimed_statically()
    print(f"\nStage U2: {len(PASS)} passed, {len(FAIL)} failed")
    import sys; sys.exit(1 if FAIL else 0)
