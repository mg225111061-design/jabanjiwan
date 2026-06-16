"""STAGE 3 tests — JEFF exact engine connection. Run: python3 test_stage3.py"""
from jeff_adapter import prove_identity, backends_available, find_jeff_binary

PASS, FAIL = [], []
def check(name, cond, detail=""):
    (PASS if cond else FAIL).append(name)
    print(f"  [{'PASS' if cond else 'FAIL'}] {name}" + (f" — {detail}" if detail and not cond else ""))


def jeff_adapter_proves_identity():
    # univariate polynomial identity → PROVEN, and (if the JEFF binary is built) BY JEFF.
    r = prove_identity("(x+1)**2", "x**2 + 2*x + 1", ["x"])
    check("jeff_adapter_proves_identity", r.verdict == "PROVEN", str(r))
    if find_jeff_binary():
        check("proven_by_jeff_backend", r.backend == "jeff", f"backend={r.backend}: {r}")
    else:
        print("  [INFO] jeff_identity binary not built — JEFF tier skipped, sympy tier used (honest fallback)")


def fallback_chain_works():
    # multivariate identity: JEFF-uni can't, so the chain falls to sympy (still exact PROVEN).
    r = prove_identity("(x+y)**2", "x**2 + 2*x*y + y**2", ["x", "y"])
    check("fallback_chain_works", r.verdict == "PROVEN" and r.backend == "sympy", str(r))


def sympy_still_proves():
    # a wrong closed form is REFUTED with a witness; a transcendental identity sympy can still prove.
    bad = prove_identity("n*n/2", "n*(n+1)/2", ["n"])
    check("refuted_with_witness", bad.verdict == "REFUTED", str(bad))
    # a non-polynomial but true identity (sympy tier): sin^2+cos^2 = 1.
    trig = prove_identity("sin(x)**2 + cos(x)**2", "1", ["x"])
    check("sympy_still_proves_transcendental", trig.verdict == "PROVEN" and trig.backend == "sympy", str(trig))


def backends_reported():
    b = backends_available()
    check("backends_reported", isinstance(b, dict) and "jeff" in b and "sympy" in b, str(b))
    print(f"  [INFO] backends available: {b}")


if __name__ == "__main__":
    print("STAGE 3 — JEFF exact engine connection")
    jeff_adapter_proves_identity()
    fallback_chain_works()
    sympy_still_proves()
    backends_reported()
    print(f"\nStage 3: {len(PASS)} passed, {len(FAIL)} failed")
    import sys
    sys.exit(1 if FAIL else 0)
