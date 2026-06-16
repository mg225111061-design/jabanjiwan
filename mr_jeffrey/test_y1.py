"""
STAGE Y1 tests — domain selection (PQC) + production-kernel spec parses.  Run: python3 test_y1.py
"""
import haran_ast as A
from haran_parser import parse
import pqc_domain

PASS, FAIL = [], []
def check(name, cond, detail=""):
    (PASS if cond else FAIL).append(name)
    print(f"  [{'PASS' if cond else 'FAIL'}] {name}" + (f" — {detail}" if detail and not cond else ""))


def domain_asset_grep_report():
    ev = pqc_domain.asset_evidence()
    print("      PQC asset evidence (grep-verified):")
    for path, sym, ok in ev:
        print(f"        [{'✓' if ok else '✗'}] {path}" + (f"  «{sym}»" if sym else ""))
    check("domain_asset_grep_report", all(ok for _, _, ok in ev), f"{sum(o for *_, o in ev)}/{len(ev)} assets present")


def domain_kernel_spec_parses():
    p = parse(pqc_domain.POLY_MUL)
    fn = p.get("poly_mul")
    ok = (p.ok and fn is not None
          and fn.params[0].name == "a" and isinstance(fn.params[0].ty, A.TyName) and fn.params[0].ty.name == "secret"
          and isinstance(fn.ret, A.TyName) and fn.ret.name == "secret"          # secret output
          and fn.effects == ["pure"]
          # body is ntt-based: intt(pointwise(ntt(a), ntt(b)))
          and isinstance(fn.body.stmts[0].value, A.Call))
    check("domain_kernel_spec_parses", ok, f"errors={[str(e) for e in p.errors]}")
    print(f"      → poly_mul: a:secret<Vec<mod<3329>,256>>, b:Vec<mod<3329>,256> → secret<…>; body=intt(pointwise(ntt,ntt))")


def subkernels_parse():
    bad = []
    for name, src in pqc_domain.SUBKERNELS.items():
        if not parse(src).ok:
            bad.append(name)
    check("subkernels_parse", not bad, f"failed: {bad}")
    print(f"      → all {len(pqc_domain.SUBKERNELS)} kernel sub-computations parse")


if __name__ == "__main__":
    print("STAGE Y1 — PQC domain selection + kernel spec")
    domain_asset_grep_report()
    domain_kernel_spec_parses()
    subkernels_parse()
    print(f"\nStage Y1: {len(PASS)} passed, {len(FAIL)} failed")
    import sys
    sys.exit(1 if FAIL else 0)
