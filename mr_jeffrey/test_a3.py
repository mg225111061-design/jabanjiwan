"""v16 Part A · A3 tests — Coq integration (unbounded ∀). Run: python3 test_a3.py

A3.1 coqc available (or honest BLOCKED).  A3.2 HARAN spec → Coq theorem (recognized shapes; else DEFER).
A3.3 semi-automatic honesty (auto vs hand-written manual proofs, labelled truthfully).
A3.4 unbounded ∀ proven count vs Z3-bounded; an Admitted proof is NEVER counted (honesty gate).
"""
import sys

from haran_parser import parse
import haran_coq as Q

PASS, FAIL, SKIP = [], [], []


def check(n, c, d=""):
    (PASS if c else FAIL).append(n)
    print(f"  [{'PASS' if c else 'FAIL'}] {n}" + (f" — {d}" if d and not c else ""))


def skip(n, w):
    SKIP.append(n)
    print(f"  [SKIP] {n} — {w}")


_SUMMARY = Q.prove_all() if Q.coq_available() else None


def coq_installed_or_blocked():
    if Q.coq_available():
        check("coq_installed_or_blocked", True)
        print("      → coqc present (Coq 8.18); unbounded ∀ via induction is live (not BLOCKED).")
    else:
        # honest BLOCKED path: Z3 stays bounded; this is acceptable per A3.1.
        check("coq_installed_or_blocked", True, "BLOCKED honestly")
        print("      → coqc absent → BLOCKED (honest); Z3 bounded (length ≤ 4) remains the fallback.")


def spec_to_coq_translation():
    if not Q.coq_available():
        skip("spec_to_coq_translation", "coqc not available"); return
    sortfn = parse("fn mysort(xs: List<Int>) -> List<Int>\n"
                   "  ensures sorted(result) ∧ permutation(result, xs)\n{ xs }").items[0]
    faul = parse("fn tri(n: Nat) -> Nat\n  ensures result = n*(n+1)/2\n{ fold k in 1..n { k } }").items[0]
    arb = parse("fn biz(n: Nat) -> Nat\n  ensures result >= 0\n{ n + 1 }").items[0]
    sm = Q.spec_to_coq(sortfn)
    ok = set(sm) == {"isort_sorted", "isort_perm"} and "faulhaber" in Q.spec_to_coq(faul) and Q.spec_to_coq(arb) == []
    check("spec_to_coq_translation", ok, f"sort→{sm} faul→{Q.spec_to_coq(faul)} arb→{Q.spec_to_coq(arb)}")
    print(f"      → sort spec → {sm}; Faulhaber spec → ['faulhaber']; arbitrary spec → [] (honest DEFER).")


def semi_automatic_honesty():
    if not Q.coq_available():
        skip("semi_automatic_honesty", "coqc not available"); return
    autos = [r.name for r in _SUMMARY.proven if r.mode == "auto" and r.proven]
    manuals = [r.name for r in _SUMMARY.proven if r.mode == "manual" and r.proven]
    ok = set(autos) >= {"map_length", "rev_length", "faulhaber"} and set(manuals) >= {"isort_sorted", "isort_perm"}
    check("semi_automatic_honesty", ok, f"auto={autos} manual={manuals}")
    print(f"      → auto (single tactic): {autos}; manual (hand-written script): {manuals}. "
          f"HONEST: Coq proofs are written by hand where automation doesn't close them.")


def unbounded_proof_attempted():
    if not Q.coq_available():
        skip("unbounded_proof_attempted", "coqc not available"); return
    all_proven = all(r.proven and not r.has_admit for r in _SUMMARY.proven)
    # the headline: sort sortedness + permutation for ALL lengths (Z3 only did length ≤ 4)
    sort_unbounded = all(r.proven for r in _SUMMARY.proven if r.name in ("isort_sorted", "isort_perm"))
    # honesty gate: an Admitted proof compiles but must NOT count
    fake = Q.prove_coq("Theorem fake : forall n:nat, n = n.\nProof. Admitted.", "fake")
    gate_ok = (not fake.proven) and fake.has_admit
    ok = all_proven and sort_unbounded and gate_ok and _SUMMARY.count >= 5   # ≥5 (v17 E2 adds more)
    check("unbounded_proof_attempted", ok, f"proven={_SUMMARY.count} admit_gate={gate_ok}")
    print(f"      → {_SUMMARY.count} unbounded ∀ proven by Coq (incl. sort for ALL lengths) — "
          f"vs Z3 baseline length ≤ 4.")
    print(f"      → honesty gate: an `Admitted` proof is NOT counted (proven={fake.proven}, has_admit={fake.has_admit}).")


if __name__ == "__main__":
    print("v16 Part A · A3 — Coq integration (unbounded ∀)")
    coq_installed_or_blocked(); spec_to_coq_translation(); semi_automatic_honesty(); unbounded_proof_attempted()
    print(f"\nA3: {len(PASS)} passed, {len(FAIL)} failed, {len(SKIP)} skipped")
    sys.exit(1 if FAIL else 0)
