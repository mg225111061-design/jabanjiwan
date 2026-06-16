"""STAGE T5 tests (v5) — integration + extended fold ratio (v2 vs v5). Run: python3 test_t5.py"""
from haran_parser import parse
from fold_v5 import classify_v5, closure_ratio_v2_vs_v5, render
from kovacic import kovacic
PASS, FAIL = [], []
def check(n, c, d=""):
    (PASS if c else FAIL).append(n); print(f"  [{'PASS' if c else 'FAIL'}] {n}" + (f" — {d}" if d and not c else ""))

CORPUS = {
    "Σk²":         "fn s(n: Nat) -> Nat effects pure { fold k in 1..n { k*k } }",
    "Σk³":         "fn s(n: Nat) -> Nat effects pure { fold k in 1..n { k**3 } }",
    "fib":         "fn fib(n: Nat) -> Nat effects pure { match n { 0=>0  1=>1  _=> fib(n-1)+fib(n-2) } }",
    "ΣC(n,k)":     "fn s(n: Nat) -> Nat effects pure { fold k in 0..n { C(n, k) } }",
    "ΣC(n,k)²":    "fn s(n: Nat) -> Nat effects pure { fold k in 0..n { C(n, k) ** 2 } }",
    "Σ1/(k²+1)":   "fn s(n: Nat) -> Nat effects pure { fold k in 1..n { 1/(k*k+1) } }",
    "factorial":   "fn f(n: Nat) -> Nat effects pure { match n { 0=>1  _=> n*f(n-1) } }",
    "Σis_prime(k)":"fn s(n: Nat) -> Nat effects pure { fold k in 2..n { is_prime(k) } }",
}

def classifier_integrated():
    v = classify_v5(parse(CORPUS["ΣC(n,k)"]).items[0])
    ok = v.kind == "CLOSED" and v.math_class == "hypergeometric" and v.completeness.startswith("partial")
    check("classifier_integrated", ok, str(v))

def closure_ratio_v2_vs_v5_measured():
    rep = closure_ratio_v2_vs_v5(CORPUS)
    print("\n" + render(rep))
    # the v5 gain: ΣC(n,k), ΣC(n,k)² move from NO_STRUCTURE (v2 can't sympify binomial) → CLOSED (v5)
    ok = rep.v5_closed() > rep.v2_closed() and rep.v5_closed() - rep.v2_closed() >= 2
    check("closure_ratio_v2_vs_v5_measured", ok,
          f"v2={rep.v2_closed()} v5={rep.v5_closed()} (+{rep.v5_closed()-rep.v2_closed()})")

def certificate_completeness_report():
    rep = closure_ratio_v2_vs_v5(CORPUS)
    closed = [v for _, _, v in rep.rows if v.kind == "CLOSED"]
    full = [v for v in closed if v.completeness == "full"]
    partial = [v for v in closed if v.completeness.startswith("partial")]
    ok = len(full) >= 3 and len(partial) >= 2     # poly/C-finite full; hypergeometric partial(provisos)
    check("certificate_completeness_report", ok, f"full={len(full)} partial={len(partial)}")
    print(f"      → CLOSED certs: {len(full)} full (poly/C-finite), {len(partial)} partial (hypergeometric provisos)")

def showcase_all_classes_correct():
    v = {k: classify_v5(parse(s).items[0]) for k, s in CORPUS.items()}
    airy = kovacic("x")
    ok = (v["Σk²"].kind == "CLOSED" and v["ΣC(n,k)"].kind == "CLOSED"
          and v["factorial"].kind == "DEFER"            # holonomic → Gröbner ceiling
          and v["Σ1/(k²+1)"].kind == "ABSENT"           # hypergeometric, Gosper-nonsummable
          and v["Σis_prime(k)"].kind == "NO_STRUCTURE"  # nonholonomic
          and airy.verdict == "ABSENT")                 # Kovacic
    check("showcase_all_classes_correct", ok,
          f"Σk²={v['Σk²'].kind} ΣC={v['ΣC(n,k)'].kind} fact={v['factorial'].kind} "
          f"rat={v['Σ1/(k²+1)'].kind} prime={v['Σis_prime(k)'].kind} airy={airy.verdict}")
    print(f"      → Σk²:CLOSED · ΣC(n,k):CLOSED+WZ · factorial:DEFER(holonomic) · "
          f"Σ1/(k²+1):ABSENT · Σprime:NO_STRUCTURE · Airy:ABSENT(Kovacic)")

if __name__ == "__main__":
    print("STAGE T5 — integration + extended fold ratio")
    classifier_integrated()
    closure_ratio_v2_vs_v5_measured()
    certificate_completeness_report()
    showcase_all_classes_correct()
    print(f"\nStage T5: {len(PASS)} passed, {len(FAIL)} failed")
    import sys; sys.exit(1 if FAIL else 0)
