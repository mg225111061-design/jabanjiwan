"""
STAGE X5 — HARAN v3 integration + whole-system measurement.
==========================================================
Ties the three v3 capabilities into one view and measures them honestly:
  1. fold / closure (v2)      — fold all foldable; prove absence (Gosper/Galois) where real.
  2. unstructured conquest (X2) — approximate + PROVE the error (PROVEN-BOUND) or label TESTED-BOUND.
  3. exact proof (X3)         — PROVEN / PROVEN-BOUNDED / TESTED, kept distinct.
  4. AI loop (X4)             — write→verify→fix, Mr's counterexamples drive convergence.

Every number below is raw and the distinctions (proven vs tested vs absent vs Ω(N)) are never blurred.
"""
from __future__ import annotations

import approx_lib
import prove_exact
from ai_loop import write_verify_fix
from llm_adapters import get_writer_verifier
from z3_adapter import z3_available


# ----- showcase HARAN programs -----
SUM_SQUARES = "fn sum_squares(n: Nat) -> Nat ensures result = n*(n+1)*(2*n+1)/6 effects pure { fold k in 1..n { k*k } }"
SORT = ("fn sort(xs: List<Int>) -> List<Int> ensures sorted(result) ∧ permutation(result, xs) effects pure "
        "{ match xs { [] => []  [p|rest] => sort(filter(rest, λy. y ≤ p)) ++ [p] ++ sort(filter(rest, λy. y > p)) } }")
PAYMENT = "fn payment_total(xs: List<Int>) -> Int ensures result = exact_sum(xs) effects pure { fold_exact(xs) }"

TASK = ("Write a HARAN function sum_squares(n: Nat) -> Nat with "
        "ensures result = n*(n+1)*(2*n+1)/6, effects pure.")
_WRONG = "fn sum_squares(n: Nat) -> Nat\n  ensures result = n*(n+1)*(2*n+1)/6\n  effects pure\n{ fold k in 1..n { k } }"
_RIGHT = "fn sum_squares(n: Nat) -> Nat\n  ensures result = n*(n+1)*(2*n+1)/6\n  effects pure\n{ fold k in 1..n { k*k } }"


def measure(verbose=True):
    out = {}

    # (2) unstructured conquest
    cr = approx_lib.conquest_ratio()
    out["conquest"] = cr

    # (3) exact proof tiers
    corpus = {
        "sum_squares": SUM_SQUARES,
        "sort": SORT,
        "evens": "fn evens(xs: List<Int>) -> List<Int> ensures length(result) <= length(xs) effects pure { filter(xs, λy. y % 2 == 0) }",
    }
    rows = prove_exact.proven_ratio(corpus)
    out["exact_rows"] = rows
    proven = sum(1 for _, v in rows if v.proven())
    out["exact_proven_pct"] = round(100 * proven / len(rows))

    # (4) AI loop
    w, v, mode = get_writer_verifier(prefer="qwen3", scripted_writer=[_WRONG], scripted_verifier=[_RIGHT],
                                     verbose=False)
    res = write_verify_fix(TASK, w, v, verbose=False)
    out["ai_mode"] = mode
    out["ai_converged"] = res.converged
    out["ai_iters"] = res.iters

    if verbose:
        print("HARAN v3 — whole-system measurement")
        print("\n[2] UNSTRUCTURED CONQUEST")
        print(approx_lib.render_ratio(cr))
        print("\n[3] EXACT PROOF TIERS")
        for label, ver in rows:
            print(f"   · {label:12s} {ver.tier}")
        print(f"   ─ exact specs PROVEN/PROVEN-BOUNDED: {out['exact_proven_pct']}%")
        print("\n[4] AI WRITE→VERIFY→FIX LOOP")
        print(f"   · mode={mode.upper()}  converged={res.converged}  iters={res.iters}  "
              f"(Mr's counterexamples drive it; model swappable)")
        print(f"\n[backends] Z3={'on' if z3_available() else 'OFF (error bounds→TESTED)'}  AI={mode}")
    return out


def showcase():
    print("\n===== HARAN v3 showcase =====")
    print("· unstructured quantile → approx + PROVEN-BOUND:")
    print("   ", approx_lib.approx_quantile_conquest())
    print("· unstructured distinct → approx + TESTED-BOUND (not conquered):")
    print("   ", approx_lib.approx_distinct_conquest())
    print("· payment (exact required) → approximation REFUSED:")
    print("   ", approx_lib.route("payment", approx_ok=False, exact_required=True,
                                  conquest_fn=approx_lib.approx_quantile_conquest))
    print("· sort (exact, list) → exhaustive bounded proof:")
    from haran_parser import parse
    ftab = {f.name: f for f in parse(SORT).fns()}
    print("   ", prove_exact.prove_correctness(parse(SORT).get("sort"), ftab))


if __name__ == "__main__":
    measure(verbose=True)
    showcase()
