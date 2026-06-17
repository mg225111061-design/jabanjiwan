"""v22 Part S · S6 tests — Type A (spec-embedded) verification with proof tiers. Run: python3 test_s6.py

typeA_proves_forall    : Σ(2k-1)=n² → tier PROVEN (exact ∀), proven_forall=True.
typeA_counterexample   : a wrong impl → FAILED + concrete counterexample.
spec_is_embedded       : the spec used is the verbatim embedded `ensures` (not intent).
tier_distinctions_honest: an inequality that only fuzz-passes → TESTED, NEVER inflated to PROVEN.
"""
import sys

import agentic as AG

PASS, FAIL, SKIP = [], [], []


def check(n, c, d=""):
    (PASS if c else FAIL).append(n)
    print(f"  [{'PASS' if c else 'FAIL'}] {n}" + (f" — {d}" if d and not c else ""))

SQ = "fn sq(n: Nat) -> Nat\n  ensures result = n*n\n{ fold k in 1..n { 2*k - 1 } }"
WRONG = "fn triangular(n: Nat) -> Nat\n  ensures result = n*(n+1)/2\n{ fold k in 1..n { k+1 } }"
INEQ = "fn t(n: Nat) -> Nat\n  ensures result >= n\n{ fold k in 1..n { k } }"


def typeA_proves_forall():
    r = AG.verify_typeA(SQ)
    ok = r.tier == "PROVEN" and r.proven_forall and "∀" in r.detail
    check("typeA_proves_forall", ok, f"tier={r.tier} forall={r.proven_forall} detail={r.detail[:50]}")
    print(f"      → Σ(2k-1)=n²: tier={r.tier} (exact ∀, unbounded). Spec-embedded correctness PROVEN, "
          f"not merely tested.")


def typeA_counterexample():
    r = AG.verify_typeA(WRONG)
    ok = r.tier == "FAILED" and r.counterexample and r.counterexample.get("inputs")
    check("typeA_counterexample", ok, f"tier={r.tier} cx={r.counterexample}")
    print(f"      → wrong impl → tier={r.tier} with counterexample {r.counterexample} (same concrete "
          f"witness the fix loop consumes).")


def spec_is_embedded():
    r = AG.verify_typeA(SQ)
    ok = r.spec == "n*n"
    check("spec_is_embedded", ok, f"spec={r.spec!r}")
    print(f"      → the discharged spec is the verbatim embedded `ensures` ({r.spec!r}) — Type A is "
          f"spec-embedded; HARAN proves THAT, not a guess at intent.")


def tier_distinctions_honest():
    proven = AG.verify_typeA(SQ)
    tested = AG.verify_typeA(INEQ)
    ok = proven.tier == "PROVEN" and tested.tier == "TESTED" and not tested.proven_forall
    check("tier_distinctions_honest", ok, f"sq={proven.tier} ineq={tested.tier} ineq_forall={tested.proven_forall}")
    print(f"      → ★tiers kept distinct★: Σ(2k-1)=n² → PROVEN (∀); `result>=n` → TESTED (fuzz found no "
          f"counterexample) — honestly NOT PROVEN ∀ (proven_forall={tested.proven_forall}). No inflation.")


if __name__ == "__main__":
    print("v22 Part S · S6 — Type A (spec-embedded) verification with proof tiers")
    typeA_proves_forall(); typeA_counterexample(); spec_is_embedded(); tier_distinctions_honest()
    print(f"\nS6: {len(PASS)} passed, {len(FAIL)} failed, {len(SKIP)} skipped")
    sys.exit(1 if FAIL else 0)
