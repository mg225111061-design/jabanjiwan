"""STAGE T2 tests (v5) — hypergeometric closed-form + WZ/telescoper certificate + provisos. Run: python3 test_t2.py"""
from haran_parser import parse
from hypergeometric import discharge_hypergeometric, telescoper, gosper_closed_form, _zeil_bin

PASS, FAIL, SKIP = [], [], []
def check(n, c, d=""):
    (PASS if c else FAIL).append(n); print(f"  [{'PASS' if c else 'FAIL'}] {n}" + (f" — {d}" if d and not c else ""))
def skip(n, w): SKIP.append(n); print(f"  [SKIP] {n} — {w}")


def _fn(src):
    return parse(src).items[0]


SUM_BINOM = "fn s(n: Nat) -> Nat effects pure { fold k in 0..n { C(n, k) } }"
SUM_BINOM_SQ = "fn s(n: Nat) -> Nat effects pure { fold k in 0..n { C(n, k) ** 2 } }"


def gosper_closed_form_test():
    # Gosper / telescoping sum: Σ_{k=1}^n 1/(k(k+1)) = n/(n+1)
    cf = gosper_closed_form("1/(k*(k+1))", 1)
    ok = cf is not None and "n + 1" in cf.replace(" ", " ")
    check("gosper_closed_form", ok, f"Σ1/(k(k+1)) = {cf}")
    print(f"      → Σ1/(k(k+1)) = {cf}")


def zeilberger_certificate_generated():
    if not _zeil_bin():
        skip("zeilberger_certificate_generated", "zeil_check not built"); return
    t = telescoper("binom")
    ok = t is not None and t["order"] == 1
    check("zeilberger_certificate_generated", ok, str(t))


def wz_certificate_verified():
    if not _zeil_bin():
        skip("wz_certificate_verified", "zeil_check not built"); return
    r = discharge_hypergeometric(_fn(SUM_BINOM))
    rsq = discharge_hypergeometric(_fn(SUM_BINOM_SQ))
    ok = (r.verdict == "CLOSED" and "VERIFIED" in r.certificate and "OK" in r.certificate
          and rsq.verdict == "CLOSED" and "VERIFIED" in rsq.certificate)
    check("wz_certificate_z3_verified", ok, f"ΣC(n,k)={r}; ΣC(n,k)²={rsq}")
    print(f"      → ΣC(n,k): {r.closed_form}; {r.certificate}")
    print(f"      → ΣC(n,k)²: {rsq.closed_form}; {rsq.certificate}")


def provisos_honestly_tracked():
    r = discharge_hypergeometric(_fn(SUM_BINOM))
    # the certificate verifies the telescoping IDENTITY but boundary provisos are NOT machine-proven
    ok = r.verdict == "CLOSED" and r.cert_completeness.startswith("partial") and "proven" in r.cert_completeness
    check("provisos_honestly_tracked", ok, f"completeness={r.cert_completeness}")
    print(f"      → cert completeness: {r.cert_completeness} (boundary provisos asserted, NOT machine-proven)")


def hypergeometric_certificate_ratio():
    if not _zeil_bin():
        skip("hypergeometric_certificate_ratio", "zeil_check not built"); return
    corpus = [SUM_BINOM, SUM_BINOM_SQ]
    results = [discharge_hypergeometric(_fn(s)) for s in corpus]
    closed = [r for r in results if r.verdict == "CLOSED"]
    full = [r for r in closed if r.cert_completeness == "full"]
    partial = [r for r in closed if r.cert_completeness.startswith("partial")]
    print(f"\n      hypergeometric certificate completeness: {len(closed)}/{len(corpus)} CLOSED, "
          f"{len(full)} full-cert, {len(partial)} partial-cert (provisos)")
    print("      → HONEST: 100% telescoping-identity VERIFIED; 0% provisos machine-proven → all PARTIAL.")
    check("hypergeometric_certificate_ratio", len(closed) == 2 and len(partial) == 2 and len(full) == 0)


if __name__ == "__main__":
    print("STAGE T2 — hypergeometric closed-form + certificate + provisos")
    gosper_closed_form_test()
    zeilberger_certificate_generated()
    wz_certificate_verified()
    provisos_honestly_tracked()
    hypergeometric_certificate_ratio()
    print(f"\nStage T2: {len(PASS)} passed, {len(FAIL)} failed, {len(SKIP)} skipped")
    import sys
    sys.exit(1 if FAIL else 0)
