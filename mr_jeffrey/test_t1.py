"""STAGE T1 tests (v5) — extended closure-class classifier. Run: python3 test_t1.py"""
from haran_parser import parse
from fold_classes import classify_math_class, is_hypergeometric, detect_recurrence_kind

PASS, FAIL = [], []
def check(n, c, d=""):
    (PASS if c else FAIL).append(n); print(f"  [{'PASS' if c else 'FAIL'}] {n}" + (f" — {d}" if d and not c else ""))


def _fn(src):
    return parse(src).items[0]


HG_BINOM = "fn s(n: Nat) -> Nat effects pure { fold k in 0..n { C(n, k) } }"
HG_RAT = "fn s(n: Nat) -> Nat effects pure { fold k in 1..n { 1 / (k*k + 1) } }"
FACT = "fn fact(n: Nat) -> Nat effects pure { match n { 0 => 1  _ => n * fact(n - 1) } }"
FIB = "fn fib(n: Nat) -> Nat effects pure { match n { 0 => 0  1 => 1  _ => fib(n - 1) + fib(n - 2) } }"
POLY = "fn s(n: Nat) -> Nat effects pure { fold k in 1..n { k*k } }"
PRIMES = "fn s(n: Nat) -> Nat effects pure { fold k in 2..n { is_prime(k) } }"


def classify_hypergeometric():
    mb = classify_math_class(_fn(HG_BINOM))
    mr = classify_math_class(_fn(HG_RAT))
    ok = mb.name == "hypergeometric" and mr.name == "hypergeometric" and mb.decidable == "decidable"
    check("classify_hypergeometric", ok, f"ΣC(n,k)={mb}; Σ1/(k²+1)={mr}")
    print(f"      → ΣC(n,k): {mb}")
    print(f"      → Σ1/(k²+1): {mr}")


def classify_holonomic_candidate():
    m = classify_math_class(_fn(FACT))
    fib = classify_math_class(_fn(FIB))
    # factorial: poly-coeff (n·) recurrence → holonomic-candidate; fib: constant-coeff → C-finite
    ok = (m.name == "holonomic-candidate" and m.route.startswith("holonomic")
          and fib.name == "C-finite")
    check("classify_holonomic_candidate", ok, f"factorial={m}; fib={fib}")
    print(f"      → factorial (n·a(n-1)): {m}")
    print(f"      → fib (constant coeff): {fib}")


def classify_nonholonomic():
    m = classify_math_class(_fn(PRIMES))
    ok = m.name.startswith("nonholonomic") and m.certifiable == "NONE"
    check("classify_nonholonomic", ok, f"Σis_prime(k)={m}")
    print(f"      → Σ is_prime(k): {m}")


def classification_certifiability_tagged():
    cases = {
        "polynomial Σk²": (POLY, "polynomial", "FULL"),
        "C-finite fib": (FIB, "C-finite", "FULL"),
        "hypergeometric ΣC(n,k)": (HG_BINOM, "hypergeometric", "PARTIAL(provisos)"),
        "holonomic factorial": (FACT, "holonomic-candidate", "DEFERRED(Gröbner)"),
        "nonholonomic Σprime": (PRIMES, "nonholonomic/data", "NONE"),
    }
    ok = True
    print("      class → certifiability grade:")
    for label, (src, want_name, want_cert) in cases.items():
        m = classify_math_class(_fn(src))
        good = m.name == want_name and m.certifiable == want_cert
        ok = ok and good
        print(f"        {'✓' if good else '✗'} {label:26s} {m.name:20s} cert={m.certifiable}")
    check("classification_certifiability_tagged", ok)


if __name__ == "__main__":
    print("STAGE T1 — extended closure-class classifier")
    classify_hypergeometric()
    classify_holonomic_candidate()
    classify_nonholonomic()
    classification_certifiability_tagged()
    print(f"\nStage T1: {len(PASS)} passed, {len(FAIL)} failed")
    import sys
    sys.exit(1 if FAIL else 0)
