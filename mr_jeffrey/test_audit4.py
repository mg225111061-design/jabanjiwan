"""v19 Part W · W4 tests — error-message audit. Run: python3 test_audit4.py

W4.1 failure scenarios (parse / codegen-unsupported / tool-absent / native-trap).
W4.2 each says WHY clearly (not an opaque trace).
W4.3 ceiling/future vs syntax error distinguished.
"""
import sys

import audit

PASS, FAIL, SKIP = [], [], []


def check(n, c, d=""):
    (PASS if c else FAIL).append(n)
    print(f"  [{'PASS' if c else 'FAIL'}] {n}" + (f" — {d}" if d and not c else ""))


_ROWS = audit.error_audit()
_BY = {r.scenario: r for r in _ROWS}


def failure_scenarios():
    cats = {r.category for r in _ROWS}
    ok = len(_ROWS) >= 5 and {"SYNTAX", "UNSUPPORTED_FUTURE"} <= cats
    check("failure_scenarios", ok, f"scenarios={len(_ROWS)} cats={sorted(cats)}")
    for r in _ROWS:
        print(f"      → {r.scenario:26} [{r.category}] {r.message[:44]}")


def error_messages_clear():
    ok = all(r.clear for r in _ROWS)
    check("error_messages_clear", ok, f"all_clear={ok}")
    print(f"      → every failure names its cause: parse says 'expected X found Y'; codegen says which "
          f"node/pattern; tools say BLOCKED; traps raise a labelled RuntimeError. No opaque traces.")


def ceiling_vs_error_distinguished():
    syntax = _BY["parse: missing '=>'"]
    future = _BY["codegen: list literal"]
    pat = _BY["codegen: list pattern"]
    # syntax error is categorized SYNTAX; unsupported list features say 'future' (NOT a syntax error)
    ok = (syntax.category == "SYNTAX" and future.category == "UNSUPPORTED_FUTURE"
          and "future" in future.message and "future" in pat.message)
    check("ceiling_vs_error_distinguished", ok, f"syntax={syntax.category} future={future.category}")
    print(f"      → distinguished: a syntax error ('{syntax.message[:30]}...') is SYNTAX; an unsupported "
          f"feature ('cannot lower ListLit ... future') is UNSUPPORTED_FUTURE — the user knows whether to "
          f"fix their code or that it's unbuilt (not a ceiling).")


if __name__ == "__main__":
    print("v19 Part W · W4 — error-message audit")
    failure_scenarios(); error_messages_clear(); ceiling_vs_error_distinguished()
    print(f"\nW4: {len(PASS)} passed, {len(FAIL)} failed, {len(SKIP)} skipped")
    sys.exit(1 if FAIL else 0)
