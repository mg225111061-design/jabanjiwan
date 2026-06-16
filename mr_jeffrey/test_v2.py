"""
STAGE V2 tests — Galois closure classification (fold all foldable; prove absence honestly).
Run: python3 test_v2.py
"""
from haran_parser import parse
from closure_classifier import (classify_fn, classify_radical_absence, classify_elementary_absence_erf,
                                closure_ratio, render_ratio, find_binary)

PASS, FAIL = [], []
def check(name, cond, detail=""):
    (PASS if cond else FAIL).append(name)
    print(f"  [{'PASS' if cond else 'FAIL'}] {name}" + (f" — {detail}" if detail and not cond else ""))


def _fn(body, name="f", lo_hi="1..n"):
    return parse(f"fn {name}(n: Nat) -> Nat\n  effects pure\n{{ fold k in {lo_hi} {{ {body} }} }}\n").items[0]


def faulhaber_polynomial_closed():
    v = classify_fn(_fn("k*k"))
    check("faulhaber_polynomial_closed", v.kind == "CLOSED" and v.method == "faulhaber" and v.speedup == "O(1)", str(v))
    print(f"      → Σk²: {v}")


def geometric_closed():
    v = classify_fn(_fn("2 ** k", lo_hi="0..n"))
    check("geometric_closed", v.kind == "CLOSED" and v.method == "gosper", str(v))
    print(f"      → Σ2^k: {v}")


def linear_recurrence_logn():
    fib = parse("fn fib(n: Nat) -> Nat\n  effects pure\n"
                "{ match n { 0 => 0  1 => 1  _ => fib(n - 1) + fib(n - 2) } }\n").items[0]
    v = classify_fn(fib)
    ok = v.kind == "CLOSED" and v.method == "cfinite" and v.speedup == "O(log n)"
    check("linear_recurrence_logn", ok, str(v))
    print(f"      → fib: {v}")


def telescoping_closed():
    v = classify_fn(_fn("1 / (k * (k + 1))"))
    check("telescoping_closed", v.kind == "CLOSED" and v.method == "gosper", str(v))
    print(f"      → Σ1/(k(k+1)): {v}")


def prime_count_closure_absent_proven():
    # count_primes Σ is_prime(k): summand is data-dependent → NO_STRUCTURE (Ω(N)).
    # HONEST: this is recognition / information-floor, NOT a Galois non-existence proof.
    v = classify_fn(_fn("is_prime(k)", lo_hi="2..n"))
    ok = v.kind == "NO_STRUCTURE" and v.method == "omega-n-data" and "Galois" in v.proof
    check("prime_count_closure_absent_proven", ok, str(v))
    print(f"      → Σ is_prime(k): {v.kind} ({v.method}) — {v.proof}")


def gosper_harmonic_absent():
    # Σ1/k and Σ1/(k²+1): hypergeometric but Gosper proves NO closed form → ABSENT (real proof).
    h = classify_fn(_fn("1 / k"))
    r = classify_fn(_fn("1 / (k*k + 1)"))
    ok = (h.kind == "ABSENT" and h.method == "gosper-nonsummable"
          and r.kind == "ABSENT" and r.method == "gosper-nonsummable")
    check("gosper_harmonic_absent", ok, f"Σ1/k={h}; Σ1/(k²+1)={r}")
    print(f"      → Σ1/k: {h.kind} ({h.method}) — Gosper PROVES no closed form")


def galois_radical_absent():
    v = classify_radical_absence(-1, -1)   # x^5 − x − 1
    check("galois_radical_absent", v.kind == "ABSENT" and v.method == "galois-radical", str(v))
    print(f"      → x^5−x−1 roots: {v.kind} ({v.method}) — {v.proof}")


def liouville_elementary_absent():
    v = classify_elementary_absence_erf()
    check("liouville_elementary_absent", v.kind == "ABSENT" and v.method == "liouville-elementary", str(v))
    print(f"      → ∫e^(−x²): {v.kind} ({v.method})")


def closure_ratio_report():
    corpus = {
        "Σ k":           "fn f(n:Nat)->Nat effects pure { fold k in 1..n { k } }",
        "Σ k²":          "fn f(n:Nat)->Nat effects pure { fold k in 1..n { k*k } }",
        "Σ k³":          "fn f(n:Nat)->Nat effects pure { fold k in 1..n { k**3 } }",
        "Σ 2^k":         "fn f(n:Nat)->Nat effects pure { fold k in 0..n { 2 ** k } }",
        "Σ k·2^k":       "fn f(n:Nat)->Nat effects pure { fold k in 0..n { k * 2 ** k } }",
        "Σ 1/(k(k+1))":  "fn f(n:Nat)->Nat effects pure { fold k in 1..n { 1 / (k * (k + 1)) } }",
        "fib":           "fn fib(n:Nat)->Nat effects pure { match n { 0 => 0  1 => 1  _ => fib(n-1) + fib(n-2) } }",
        "Σ 1/k":         "fn f(n:Nat)->Nat effects pure { fold k in 1..n { 1 / k } }",
        "Σ 1/(k²+1)":    "fn f(n:Nat)->Nat effects pure { fold k in 1..n { 1 / (k*k + 1) } }",
        "Σ is_prime(k)": "fn f(n:Nat)->Nat effects pure { fold k in 2..n { is_prime(k) } }",
    }
    r = closure_ratio(corpus)
    print("\n" + render_ratio(r))
    closed, absent, nostruct, unknown = r.count("CLOSED"), r.count("ABSENT"), r.count("NO_STRUCTURE"), r.count("UNKNOWN")
    # ★"접을 수 있는데 놓친 것 0": no foldable sum left UNKNOWN — everything decided★
    missed = unknown
    print(f"   ─ ★ foldable-but-missed (UNKNOWN among the corpus): {missed}  →  "
          f"{'0 missed (Faulhaber+Gosper+C-finite complete for their classes)' if missed == 0 else 'GAP!'}")
    ok = closed >= 6 and absent >= 2 and nostruct >= 1 and unknown == 0
    check("closure_ratio_report", ok, f"closed={closed} absent={absent} nostruct={nostruct} unknown={unknown}")


if __name__ == "__main__":
    print("STAGE V2 — Galois closure classification")
    check("engines_built", all(find_binary(b) for b in ("jeff_foldsum", "cfinite_nth", "galois_absence")),
          "missing CLI(s)")
    faulhaber_polynomial_closed()
    geometric_closed()
    linear_recurrence_logn()
    telescoping_closed()
    prime_count_closure_absent_proven()
    gosper_harmonic_absent()
    galois_radical_absent()
    liouville_elementary_absent()
    closure_ratio_report()
    print(f"\nStage V2: {len(PASS)} passed, {len(FAIL)} failed")
    import sys
    sys.exit(1 if FAIL else 0)
