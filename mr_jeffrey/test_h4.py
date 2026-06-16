"""
STAGE H4 tests — fold collapse certificate (O(n)→O(1) + proof).  Run: python3 test_h4.py
"""
from haran_parser import parse
from fold_collapse import collapse_fn_fold, collapse_fold, find_foldsum_binary
import haran_ast as A

PASS, FAIL = [], []
def check(name, cond, detail=""):
    (PASS if cond else FAIL).append(name)
    print(f"  [{'PASS' if cond else 'FAIL'}] {name}" + (f" — {detail}" if detail and not cond else ""))


SUM_SQUARES = """\
fn sum_squares(n: Nat) -> Nat
  ensures result = n*(n+1)*(2*n+1)/6
  effects pure
{ fold k in 1..n { k*k } }
"""

# nonlinear / non-polynomial summands → must DEFER honestly
NONLINEAR = """\
fn weird(n: Nat) -> Nat
  effects pure
{ fold k in 1..n { g(k) * g(k) } }
"""

GEOMETRIC = """\
fn geo(n: Nat) -> Nat
  effects pure
{ fold k in 1..n { 2 ** k } }
"""


def _fn(src):
    return parse(src).items[0]


def fold_collapses_with_cert():
    r = collapse_fn_fold(_fn(SUM_SQUARES))
    ok = (r.verdict == "COLLAPSED" and r.cert is not None
          and r.cert.speedup == "O(n) → O(1)"
          and "1/3*n^3" in r.cert.closed_form and "1/6*n" in r.cert.closed_form)
    check("fold_collapses_with_cert", ok, str(r))
    print(f"      → {r.cert.fold} = {r.cert.closed_form}")


def fold_cert_verified_by_jeff():
    r = collapse_fn_fold(_fn(SUM_SQUARES))
    ok = (r.verdict == "COLLAPSED"
          and "telescope=ZERO" in r.cert.jeff_proof and "base=ZERO" in r.cert.jeff_proof
          and r.cert.verified_by == "jeff-math")
    check("fold_cert_verified_by_jeff", ok, str(r))
    # the collapsed closed form must MATCH the ensures spec (closed ≡ n(n+1)(2n+1)/6 via jeff_identity)
    check("collapse_matches_ensures", r.matches_ensures is True, f"matches_ensures={r.matches_ensures}")
    check("foldsum_engine_present", find_foldsum_binary() is not None, "jeff_foldsum not built")
    print(f"      → proof: {r.cert.jeff_proof}; matches ensures: {r.matches_ensures}")


def nonlinear_fold_defers():
    r1 = collapse_fn_fold(_fn(NONLINEAR))
    ok1 = r1.verdict == "DEFER" and "g" in r1.detail
    check("nonlinear_fold_defers", ok1, str(r1))
    print(f"      → unknown-call summand: {r1.verdict} — {r1.detail}")
    r2 = collapse_fn_fold(_fn(GEOMETRIC))
    ok2 = r2.verdict == "DEFER" and ("r^k" in r2.detail or "non-polynomial" in r2.detail or "exponent" in r2.detail)
    check("geometric_fold_defers", ok2, str(r2))
    print(f"      → geometric 2^k summand: {r2.verdict} — {r2.detail}")


def collapse_table():
    corpus = {
        "Σ k        ": "fn f(n:Nat)->Nat effects pure { fold k in 1..n { k } }",
        "Σ k²       ": "fn f(n:Nat)->Nat effects pure { fold k in 1..n { k*k } }",
        "Σ k³       ": "fn f(n:Nat)->Nat effects pure { fold k in 1..n { k**3 } }",
        "Σ (2k+1)   ": "fn f(n:Nat)->Nat effects pure { fold k in 1..n { 2*k + 1 } }",
        "Σ 2^k      ": "fn f(n:Nat)->Nat effects pure { fold k in 1..n { 2 ** k } }",
        "Σ g(k)     ": "fn f(n:Nat)->Nat effects pure { fold k in 1..n { g(k) } }",
    }
    print("\n      fold collapse table:")
    print(f"        {'fold':12s} {'verdict':10s} closed-form / reason")
    n_collapsed = n_defer = 0
    for label, src in corpus.items():
        r = collapse_fn_fold(_fn(src))
        if r.verdict == "COLLAPSED":
            n_collapsed += 1
            rhs = r.cert.closed_form
        else:
            n_defer += 1
            rhs = r.detail
        print(f"        {label} {r.verdict:10s} {rhs}")
    # 4 polynomial folds collapse; geometric + unknown-call defer
    check("collapse_table", n_collapsed == 4 and n_defer == 2, f"collapsed={n_collapsed} defer={n_defer}")


if __name__ == "__main__":
    print("STAGE H4 — fold collapse certificate")
    fold_collapses_with_cert()
    fold_cert_verified_by_jeff()
    nonlinear_fold_defers()
    collapse_table()
    print(f"\nStage H4: {len(PASS)} passed, {len(FAIL)} failed")
    import sys
    sys.exit(1 if FAIL else 0)
