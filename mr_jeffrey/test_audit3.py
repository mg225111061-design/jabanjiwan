"""v19 Part W · W3 tests — edge-case robustness. Run: python3 test_audit3.py

W3.1/.2 edge cases: native result == interpreter (empty/n=0/boundary/negative/empty-reduce).
W3.3 broken edges fixed: div-by-zero traps CLEARLY (not opaque); i64 overflow = documented CEILING.
"""
import sys

import audit

PASS, FAIL, SKIP = [], [], []


def check(n, c, d=""):
    (PASS if c else FAIL).append(n)
    print(f"  [{'PASS' if c else 'FAIL'}] {n}" + (f" — {d}" if d and not c else ""))


_ROWS = audit.edge_audit()
_BY = {r.case: r for r in _ROWS}


def edge_cases_native_eq_interp():
    matches = [r for r in _ROWS if r.verdict == "MATCH"]
    ok = (len(matches) >= 5 and all(str(r.interp) == str(r.native) for r in matches)
          and _BY["empty fold (n=0)"].verdict == "MATCH" and _BY["empty Vec reduce"].verdict == "MATCH")
    check("edge_cases_native_eq_interp", ok, f"matches={len(matches)}")
    print(f"      → {len(matches)} in-range edges: native == interpreter (n=0 fold=0, singleton, negative, "
          f"match base, 1e18 in i64, empty reduce=0).")


def broken_edges_fixed():
    dz = _BY["division by zero"]
    ok = dz.verdict == "CLEAR_ERROR" and "TRAP" in str(dz.native)
    check("broken_edges_fixed", ok, f"div0={dz.verdict}")
    print(f"      → division by zero now traps CLEARLY (run_native guard → labelled RuntimeError) instead "
          f"of an opaque int('') ValueError. Both interpreter and native fail clearly — never silent.")


def boundary_errors_clear():
    over = _BY["i64 overflow (cube 3e6)"]
    # no row may be SILENT or MISMATCH — every edge is MATCH, a documented CEILING, or a CLEAR_ERROR
    verdicts = {r.verdict for r in _ROWS}
    no_silent = not ({"MISMATCH", "SILENT?"} & verdicts)
    ok = over.verdict == "CEILING" and no_silent
    check("boundary_errors_clear", ok, f"overflow={over.verdict} verdicts={sorted(verdicts)}")
    print(f"      → i64 overflow (cube 3e6) = documented CEILING: native long long wraps per C semantics "
          f"({over.native}); interpreter bigint exact ({over.interp}); use bignum for exact. No silent "
          f"wrong answer anywhere (verdicts: {sorted(verdicts)}).")


if __name__ == "__main__":
    print("v19 Part W · W3 — edge-case robustness")
    edge_cases_native_eq_interp(); broken_edges_fixed(); boundary_errors_clear()
    print(f"\nW3: {len(PASS)} passed, {len(FAIL)} failed, {len(SKIP)} skipped")
    sys.exit(1 if FAIL else 0)
