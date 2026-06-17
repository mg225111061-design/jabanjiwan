"""v19 Part W · W1 tests — feature completeness audit. Run: python3 test_audit1.py
(Named test_audit* to avoid colliding with the pre-existing test_w*.py aliasing suite.)

W1.1/.2 feature coverage table (every language feature classified, no silent failure).
W1.3 missing feature filled (Vec reduce `fold x in xs`); ceilings/spec-only/interpreter-domain marked.
"""
import sys

from haran_parser import parse
import haran_ast as A
import haran_vec as VEC
import audit

PASS, FAIL, SKIP = [], [], []


def check(n, c, d=""):
    (PASS if c else FAIL).append(n)
    print(f"  [{'PASS' if c else 'FAIL'}] {n}" + (f" — {d}" if d and not c else ""))


_ROWS = audit.feature_audit()
_SUM = audit.summary(_ROWS)


def feature_coverage_table():
    no_silent = all(r.works for r in _ROWS)
    buckets = {"CODEGEN", "FILLED", "SPEC_ONLY", "INTERPRETER_DOMAIN", "CEILING"}
    all_classified = all(r.status in buckets for r in _ROWS)
    ok = no_silent and all_classified and len(_SUM.get("CODEGEN", [])) >= 15
    check("feature_coverage_table", ok, f"codegen={len(_SUM.get('CODEGEN',[]))} no_silent={no_silent}")
    print(f"      → {len(_ROWS)} features classified: CODEGEN={len(_SUM.get('CODEGEN',[]))}, "
          f"FILLED={len(_SUM.get('FILLED',[]))}, SPEC_ONLY={len(_SUM.get('SPEC_ONLY',[]))}, "
          f"INTERPRETER_DOMAIN={len(_SUM.get('INTERPRETER_DOMAIN',[]))}, CEILING={len(_SUM.get('CEILING',[]))}. "
          f"No silent failure.")


def missing_features_filled():
    fn = [it for it in parse("fn f(xs: Vec<Int>) -> Int { fold x in xs { x * 2 } }").items
          if isinstance(it, A.FnDecl)][0]
    c = VEC.compile_reduce(fn)
    val = VEC.run_reduce(c.binary, [1, 2, 3, 4]) if c.ok else None
    ok = c.ok and val == 20 and "FILLED" in {r.status for r in _ROWS if "reduce" in r.feature}
    check("missing_features_filled", ok, f"reduce ok={c.ok} f([1,2,3,4])={val}")
    print(f"      → gap FILLED: Vec reduce `fold x in xs` now compiles & runs (f([1,2,3,4]) with x*2 = "
          f"{val}); previously failed 'fold domain not a range'.")


def ceilings_marked():
    by = {r.feature: r for r in _ROWS}
    cofix = by["cofix / Yield (corecursion)"].status == "CEILING"
    quant = by["quantifier ∀/∃"].status == "SPEC_ONLY"
    lists = by["list patterns ([] / [h|t])"].status == "INTERPRETER_DOMAIN"
    clean = all(r.works for r in _ROWS if r.status in ("INTERPRETER_DOMAIN", "SPEC_ONLY", "CEILING"))
    ok = cofix and quant and lists and clean
    check("ceilings_marked", ok, f"cofix={cofix} quant={quant} lists={lists} clean={clean}")
    print(f"      → infinite cofix = CEILING (finite prefix, fundamental); ∀/∃ = SPEC_ONLY (verification, "
          f"not run); lists/ADT = INTERPRETER_DOMAIN (codegen future, NOT a ceiling). Clean rejects.")


if __name__ == "__main__":
    print("v19 Part W · W1 — feature completeness audit")
    feature_coverage_table(); missing_features_filled(); ceilings_marked()
    print(f"\nW1: {len(PASS)} passed, {len(FAIL)} failed, {len(SKIP)} skipped")
    sys.exit(1 if FAIL else 0)
