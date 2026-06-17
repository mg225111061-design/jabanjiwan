"""v22 Part S · S2 tests — write→verify (Claude proposes, HARAN judges). Run: python3 test_s2.py

write_verify_correct      : a correct proposal → VERIFIED (against the spec).
write_verify_counterexample: a wrong proposal → concrete counterexample (smallest input + mismatch).
spec_relative_honest      : VERIFIED is *spec-relative*, not intent — demonstrated + labeled honestly.
provenance_labeled        : mock proposals are labeled source='mock-sim' (never a fake 'live').
"""
import sys

import agentic as AG

PASS, FAIL, SKIP = [], [], []


def check(n, c, d=""):
    (PASS if c else FAIL).append(n)
    print(f"  [{'PASS' if c else 'FAIL'}] {n}" + (f" — {d}" if d and not c else ""))

GOOD = "fn triangular(n: Nat) -> Nat\n  ensures result = n*(n+1)/2\n{ fold k in 1..n { k } }"
WRONG = "fn triangular(n: Nat) -> Nat\n  ensures result = n*(n+1)/2\n{ fold k in 1..n { k+1 } }"


def write_verify_correct():
    r = AG.write_verify("sum 1..n", mock_response=GOOD)
    ok = r.ok and r.status == "VERIFIED" and r.counterexample is None
    check("write_verify_correct", ok, f"status={r.status} ok={r.ok}")
    print(f"      → Claude proposes Σk; HARAN: {r.status} (against the ensures spec). No counterexample.")


def write_verify_counterexample():
    r = AG.write_verify("sum 1..n", mock_response=WRONG)
    cx = r.counterexample or {}
    ok = (not r.ok) and r.status == "FAILED" and cx.get("inputs") and "spec" in r.feedback.lower()
    check("write_verify_counterexample", ok, f"status={r.status} cx={cx} fb={r.feedback!r}")
    print(f"      → wrong proposal (k+1) → HARAN returns a CONCRETE counterexample: {r.feedback}")
    print(f"        (smallest failing input {cx.get('inputs')}: impl={cx.get('impl_value')} vs "
          f"spec={cx.get('spec_value')}) — this is what S3 feeds back to fix.")


def spec_relative_honest():
    # The code below MEETS its (weak) spec `result = n` while the English intent ("sum of squares")
    # is different. HARAN says VERIFIED *against the spec*; it does NOT certify intent. Honest boundary.
    spec_weak = "fn f(n: Nat) -> Nat\n  ensures result = n\n{ n }"
    r = AG.write_verify("sum of squares 1..n", mock_response=spec_weak)
    ok = r.ok and r.status == "VERIFIED"
    check("spec_relative_honest", ok, f"status={r.status}")
    print(f"      → ★verification is SPEC-relative, not intent★: code meets `ensures result = n` → "
          f"{r.status}, even though the request said 'sum of squares'. HARAN never claims to read intent.")


def provenance_labeled():
    r = AG.write_verify("sum 1..n", mock_response=GOOD)
    ok = (not r.live) and r.source == "mock-sim"
    check("provenance_labeled", ok, f"live={r.live} source={r.source}")
    print(f"      → mock proposal labeled source='{r.source}', live={r.live} — never a fake 'live'.")


if __name__ == "__main__":
    print("v22 Part S · S2 — write→verify (Claude proposes, HARAN judges)")
    write_verify_correct(); write_verify_counterexample(); spec_relative_honest(); provenance_labeled()
    print(f"\nS2: {len(PASS)} passed, {len(FAIL)} failed, {len(SKIP)} skipped")
    sys.exit(1 if FAIL else 0)
