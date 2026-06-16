"""
STAGE X1 tests — Z3 relational backend (foundation for error proofs).  Run: python3 test_x1.py
"""
from z3_adapter import z3_available, prove_predicate, choose_backend, parse_predicate, FALLBACK_CHAIN

PASS, FAIL, SKIP = [], [], []
def check(name, cond, detail=""):
    (PASS if cond else FAIL).append(name)
    print(f"  [{'PASS' if cond else 'FAIL'}] {name}" + (f" — {detail}" if detail and not cond else ""))
def skip(name, why):
    SKIP.append(name)
    print(f"  [SKIP] {name} — {why}")


def z3_inequality_encode():
    if not z3_available():
        check("z3_inequality_encode", False, "Z3 NOT AVAILABLE — X1 BLOCKED")
        return
    r_true = prove_predicate("x * x >= 0", {"x": "Float"})
    r_false = prove_predicate("x >= 0", {"x": "Float"})
    ok = (r_true.verdict == "PROVEN"
          and r_false.verdict == "REFUTED" and r_false.counterexample is not None)
    check("z3_inequality_encode", ok, f"x²≥0:{r_true}; x≥0:{r_false}")
    print(f"      → x²≥0: {r_true.verdict}; x≥0: {r_false.verdict} (cx {r_false.counterexample})")


def error_bound_proven_simple():
    # midpoint error bound: lo≤x≤hi ⇒ |(lo+hi)/2 − x| ≤ (hi−lo)/2   (the bucket-midpoint guarantee)
    r = prove_predicate("abs((lo + hi) / 2 - x) <= (hi - lo) / 2",
                        {"lo": "Float", "hi": "Float", "x": "Float"},
                        assumptions=["lo <= x", "x <= hi"])
    check("error_bound_proven_simple", r.verdict == "PROVEN", str(r))
    print(f"      → |midpoint − x| ≤ half-width under lo≤x≤hi: {r.verdict} (real ∀-proof by Z3)")
    # a deterministic bucketing chain lemma X2 will reuse: bound ≤ w/2 ∧ w ≤ 2ε ⇒ bound ≤ ε
    r2 = prove_predicate("d <= eps",
                         {"d": "Float", "w": "Float", "eps": "Float"},
                         assumptions=["d >= 0", "d <= w / 2", "w <= 2 * eps"])
    check("bucket_chain_lemma_proven", r2.verdict == "PROVEN", str(r2))


def fallback_chain_extended():
    # the chain jeff → sympy → z3 → fuzz; each spec routes to its first capable backend
    p = {"n": "Nat", "x": "Float", "eps": "Float", "result": "Float", "xs": "List"}
    ident = parse_predicate("result = n * (n + 1) / 2", {"n": "Nat", "result": "Nat"})
    ineq = parse_predicate("abs(result - x) <= eps", {"result": "Float", "x": "Float", "eps": "Float"})
    fol = parse_predicate("forall_k(result)", {"result": "Float"})   # opaque → fuzz
    routes = (choose_backend(ident), choose_backend(ineq), choose_backend(fol))
    ok = routes == ("jeff", "z3", "fuzz") and FALLBACK_CHAIN == ["jeff", "sympy", "z3", "fuzz"]
    check("fallback_chain_extended", ok, f"routes={routes} chain={FALLBACK_CHAIN}")
    print(f"      → identity→jeff, inequality→z3, opaque-predicate→fuzz; chain={FALLBACK_CHAIN}")


if __name__ == "__main__":
    print("STAGE X1 — Z3 relational proof backend")
    print(f"  [INFO] Z3 available: {z3_available()}")
    if not z3_available():
        # Honest BLOCKED per the directive: no fake pass. Install z3-solver to enable (requirements-v3.txt).
        for t in ("z3_inequality_encode", "error_bound_proven_simple", "fallback_chain_extended"):
            skip(t, "Z3 not installed (pip install -r requirements-v3.txt) — error proofs BLOCKED, X2 → TESTED-BOUND only")
    else:
        z3_inequality_encode()
        error_bound_proven_simple()
        fallback_chain_extended()
    print(f"\nStage X1: {len(PASS)} passed, {len(FAIL)} failed, {len(SKIP)} skipped")
    import sys
    sys.exit(1 if FAIL else 0)
