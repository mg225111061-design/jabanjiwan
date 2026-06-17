"""v22 Part S · S4 tests — fold optimization on the PROVEN result. Run: python3 test_s4.py

optimizes_closed_form  : a proven fold (Σk) collapses to a closed form, asymptotic O(1) (proven structural).
no_fake_speedup        : code with no structure → NOT optimized, speedup='none' (no fabricated number).
only_proven_optimized  : the loop proves, THEN optimizes — closed form attached to a verified function.
speedup_is_asymptotic  : the speedup is an asymptotic class (proven), not a wall-clock claim — honest label.
"""
import sys

import agentic as AG

PASS, FAIL, SKIP = [], [], []


def check(n, c, d=""):
    (PASS if c else FAIL).append(n)
    print(f"  [{'PASS' if c else 'FAIL'}] {n}" + (f" — {d}" if d and not c else ""))

GOOD = "fn triangular(n: Nat) -> Nat\n  ensures result = n*(n+1)/2\n{ fold k in 1..n { k } }"
WRONG = "fn triangular(n: Nat) -> Nat\n  ensures result = n*(n+1)/2\n{ fold k in 1..n { k+1 } }"
NOSTRUCT = "fn ident(n: Nat) -> Nat\n  ensures result = n\n{ n }"


def optimizes_closed_form():
    o = AG.optimize(GOOD)
    ok = o.optimized and o.kind == "CLOSED" and o.speedup == "O(1)" and o.closed_form not in ("—", "")
    check("optimizes_closed_form", ok, f"kind={o.kind} method={o.method} closed={o.closed_form} speedup={o.speedup}")
    print(f"      → Σk collapses ({o.method}) → closed form '{o.closed_form}', asymptotic {o.speedup} "
          f"(O(n) loop → O(1) evaluation). Speedup is PROVEN structural (closed form exists), not a benchmark.")


def no_fake_speedup():
    o = AG.optimize(NOSTRUCT)
    ok = (not o.optimized) and o.speedup == "none"
    check("no_fake_speedup", ok, f"kind={o.kind} speedup={o.speedup}")
    print(f"      → ★no fabricated speedup★: no exploitable structure (kind={o.kind}) → optimized=False, "
          f"speedup='{o.speedup}'. Honest: not every program collapses (Ω(N) / no closed form).")


def only_proven_optimized():
    # full S3 loop then S4 optimize: prove first, optimize the verified result.
    r = AG.write_verify_fix("sum 1..n", mock_sequence=[WRONG, GOOD])
    o = AG.optimize(r.final_code) if r.converged and r.final_status == "VERIFIED" else None
    ok = r.converged and o is not None and o.optimized
    check("only_proven_optimized", ok, f"converged={r.converged} status={r.final_status} optimized={o and o.optimized}")
    print(f"      → write→verify→fix → VERIFIED, THEN optimize: closed form '{o.closed_form}'. "
          f"We only ever optimize PROVEN code (never something unverified).")


def speedup_is_asymptotic():
    o = AG.optimize(GOOD)
    # the proven artifact is the closed form + asymptotic class; concrete ×N wall-clock is measured
    # elsewhere (S7) — here we assert we expose the asymptotic class, not a faked numeric speedup.
    ok = o.speedup.startswith("O(") and "x" not in o.speedup.lower() and "faster" not in o.speedup.lower()
    check("speedup_is_asymptotic", ok, f"speedup={o.speedup!r}")
    print(f"      → speedup exposed as asymptotic class {o.speedup!r} (proven via closed form), NOT a "
          f"'×N faster' wall-clock claim. Concrete measured ms is S7's job → otherwise [TBD: measured].")


if __name__ == "__main__":
    print("v22 Part S · S4 — fold optimization (closed-form collapse on the proven result)")
    optimizes_closed_form(); no_fake_speedup(); only_proven_optimized(); speedup_is_asymptotic()
    print(f"\nS4: {len(PASS)} passed, {len(FAIL)} failed, {len(SKIP)} skipped")
    sys.exit(1 if FAIL else 0)
