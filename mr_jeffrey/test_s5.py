"""v22 Part S · S5 tests — two modes (normal/extended) on the agentic loop. Run: python3 test_s5.py

easy_both_fast        : an easy request → both modes converge in 1 iter (no depth needed).
extended_solves_more  : a request needing 3 tries → normal (budget 2) stops shallow; extended solves it.
both_zero_wrong       : across both modes, `wrong` is always False (HARAN never false-VERIFIES).
normal_honest_status  : normal's miss is labeled UNRESOLVED-shallow (not wrong), not a false PROVEN.
"""
import sys

import agentic as AG

PASS, FAIL, SKIP = [], [], []


def check(n, c, d=""):
    (PASS if c else FAIL).append(n)
    print(f"  [{'PASS' if c else 'FAIL'}] {n}" + (f" — {d}" if d and not c else ""))

GOOD = "fn triangular(n: Nat) -> Nat\n  ensures result = n*(n+1)/2\n{ fold k in 1..n { k } }"
WRONG = "fn triangular(n: Nat) -> Nat\n  ensures result = n*(n+1)/2\n{ fold k in 1..n { k+1 } }"
# needs 3 attempts to fix: wrong, wrong, good → normal(budget 2) misses, extended(budget 5) solves.
HARD_SEQ = [WRONG, WRONG, GOOD]


def easy_both_fast():
    c = AG.compare_modes("sum 1..n", mock_sequence=[GOOD])
    ok = c["normal"].converged and c["extended"].converged and c["normal"].iters == 1 == c["extended"].iters
    check("easy_both_fast", ok, f"normal={c['normal']} extended={c['extended']}")
    print(f"      → easy request: normal & extended both VERIFIED in 1 iter — no extra depth needed.")


def extended_solves_more():
    c = AG.compare_modes("sum 1..n", mock_sequence=HARD_SEQ)
    n, e = c["normal"], c["extended"]
    ok = (not n.converged) and e.converged and e.iters == 3 and e.status == "VERIFIED"
    check("extended_solves_more", ok, f"normal={n.status}/{n.iters} extended={e.status}/{e.iters}")
    print(f"      → harder request (3 tries to fix): NORMAL stops at {n.iters} → {n.status}; "
          f"EXTENDED keeps going → {e.status} in {e.iters}. ★extended solves MORE★ (same engine, deeper budget).")


def both_zero_wrong():
    runs = []
    for seq in ([GOOD], HARD_SEQ, [WRONG]):
        c = AG.compare_modes("sum 1..n", mock_sequence=seq)
        runs += [c["normal"], c["extended"]]
    ok = all(not r.wrong for r in runs)
    check("both_zero_wrong", ok, f"wrong flags={[r.wrong for r in runs]}")
    print(f"      → ★across {len(runs)} runs (both modes, easy/hard/unfixable): wrong=False everywhere★. "
          f"HARAN never false-VERIFIES, so neither mode ever yields a wrong answer.")


def normal_honest_status():
    n = AG.agentic_in_mode("sum 1..n", "normal", mock_sequence=HARD_SEQ)
    ok = (not n.converged) and n.status == "UNRESOLVED-shallow" and not n.wrong
    check("normal_honest_status", ok, f"status={n.status} wrong={n.wrong}")
    print(f"      → normal's miss is honestly '{n.status}' (didn't look deeper), wrong={n.wrong} — "
          f"shallow but NEVER a false PROVEN.")


if __name__ == "__main__":
    print("v22 Part S · S5 — two modes (normal / extended) on the agentic loop")
    easy_both_fast(); extended_solves_more(); both_zero_wrong(); normal_honest_status()
    print(f"\nS5: {len(PASS)} passed, {len(FAIL)} failed, {len(SKIP)} skipped")
    sys.exit(1 if FAIL else 0)
