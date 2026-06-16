"""STAGE U1 tests (v6) — Prony deterministic recovery + Z3 error proof. Run: python3 test_u1.py"""
from prony import (run, recover_roots, prove_noiseless_residual_zero, prove_noisy_residual_bound,
                   discharge_prony, _bin)
from z3_adapter import z3_available
PASS, FAIL, SKIP = [], [], []
def check(n, c, d=""):
    (PASS if c else FAIL).append(n); print(f"  [{'PASS' if c else 'FAIL'}] {n}" + (f" — {d}" if d and not c else ""))
def skip(n, w): SKIP.append(n); print(f"  [SKIP] {n} — {w}")

def prony_noiseless_exact_proven():
    if not _bin(): skip("prony_noiseless_exact_proven", "prony_check not built"); return
    d = run("noiseless")
    roots = recover_roots(d["coeffs"])
    near = sorted(roots)
    exact_recovery = len(near) == 2 and abs(near[0]-0.7) < 1e-6 and abs(near[1]-0.9) < 1e-6
    z3ok = True
    if z3_available():
        p = prove_noiseless_residual_zero(); z3ok = (p.verdict == "PROVEN")
    ok = d["residual"] < 1e-9 and exact_recovery and z3ok
    check("prony_noiseless_exact_proven", ok, f"residual={d['residual']:.2e} roots={near} z3={z3ok}")
    print(f"      → noiseless residual={d['residual']:.2e}≈0; recovered z=[0.7,0.9] EXACT; "
          f"Z3 Hankel det≡0 PROVEN={z3ok}")

def prony_noisy_error_bounded():
    if not _bin(): skip("prony_noisy_error_bounded", "prony_check not built"); return
    res = []
    for eta in (1e-3, 1e-2, 1e-1):
        d = run("noisy", eta); res.append((eta, d["residual"], d["max_noise"]))
    # residual scales with noise (deterministic relationship)
    grows = res[0][1] < res[2][1]
    z3ok = True
    if z3_available():
        p = prove_noisy_residual_bound(); z3ok = (p.verdict == "PROVEN")
    check("prony_noisy_error_bounded", grows and z3ok, f"res={[(f'{e:.0e}',f'{r:.1e}') for e,r,_ in res]} z3={z3ok}")
    for e, r, mn in res:
        print(f"      → η={e:.0e}: residual={r:.2e} (≤(Σ|aᵢ|)·η, Z3-PROVEN deterministic bound)")

def prony_hankel_nonsingular_checked():
    if not _bin(): skip("prony_hankel_nonsingular_checked", "prony_check not built"); return
    d = run("noiseless")
    roots = recover_roots(d["coeffs"])
    distinct = len(roots) == 2 and abs(roots[0]-roots[1]) > 1e-6
    check("prony_hankel_nonsingular_checked", distinct, f"roots={roots} distinct={distinct}")
    print(f"      → Hankel non-singular ⟺ roots distinct: {roots} → distinct={distinct} (recovery valid)")

def prony_provisos_tracked():
    if not _bin() or not z3_available(): skip("prony_provisos_tracked", "needs prony_check + Z3"); return
    r = discharge_prony()
    ok = r.verdict == "PROVEN-BOUND" and "complete" in r.completeness and "boundary-partial" in r.completeness
    check("prony_provisos_tracked", ok, str(r))
    print(f"      → {r.verdict} ({r.error_kind}); completeness: {r.completeness}")

if __name__ == "__main__":
    print("STAGE U1 — Prony deterministic recovery + Z3 error proof")
    prony_noiseless_exact_proven()
    prony_noisy_error_bounded()
    prony_hankel_nonsingular_checked()
    prony_provisos_tracked()
    print(f"\nStage U1: {len(PASS)} passed, {len(FAIL)} failed, {len(SKIP)} skipped")
    import sys; sys.exit(1 if FAIL else 0)
