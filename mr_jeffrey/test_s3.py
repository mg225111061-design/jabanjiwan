"""v22 Part S · S3 tests — write→verify→FIX, the heart. Run: python3 test_s3.py

converges_via_counterexample : wrong→(counterexample fed back)→fixed → PROVEN in 2 iters.
counterexample_is_fed_back   : iter-2's prompt literally carries iter-1's counterexample.
no_false_convergence         : a model that never fixes → loop does NOT converge (HARAN never rubber-stamps).
honest_provenance            : mock loop is labeled source='mock-sim' (loop+counterexamples real; text scripted).
"""
import sys

import agentic as AG

PASS, FAIL, SKIP = [], [], []


def check(n, c, d=""):
    (PASS if c else FAIL).append(n)
    print(f"  [{'PASS' if c else 'FAIL'}] {n}" + (f" — {d}" if d and not c else ""))

WRONG = "fn triangular(n: Nat) -> Nat\n  ensures result = n*(n+1)/2\n{ fold k in 1..n { k+1 } }"
GOOD = "fn triangular(n: Nat) -> Nat\n  ensures result = n*(n+1)/2\n{ fold k in 1..n { k } }"


def converges_via_counterexample():
    r = AG.write_verify_fix("sum 1..n", mock_sequence=[WRONG, GOOD], max_iters=3)
    ok = r.converged and r.iters == 2 and r.final_status == "VERIFIED"
    check("converges_via_counterexample", ok, f"converged={r.converged} iters={r.iters} status={r.final_status}")
    print(f"      → iter1 {r.trace[0].verdict.status} (cx={r.trace[0].verdict.counterexample}) → "
          f"iter2 {r.trace[1].verdict.status}. Converged in {r.iters} via the fed-back counterexample.")


def counterexample_is_fed_back():
    r = AG.write_verify_fix("sum 1..n", mock_sequence=[WRONG, GOOD], max_iters=3)
    cx_inputs = r.trace[0].verdict.counterexample.get("inputs")          # {'n': 1}
    fix_prompt = r.trace[1].prompt                                       # the prompt that produced iter2
    ok = str(cx_inputs) in fix_prompt and "spec" in fix_prompt.lower()
    check("counterexample_is_fed_back", ok, f"cx={cx_inputs} in_prompt={str(cx_inputs) in fix_prompt}")
    print(f"      → the fix prompt for iter2 literally contains iter1's counterexample {cx_inputs} — "
          f"the model is told exactly where/why it failed (this is what makes a weak model converge).")


def no_false_convergence():
    # the model NEVER produces a correct fix → the loop must NOT claim success.
    r = AG.write_verify_fix("sum 1..n", mock_sequence=[WRONG], max_iters=3)
    ok = (not r.converged) and r.iters == 3 and r.final_status != "VERIFIED"
    check("no_false_convergence", ok, f"converged={r.converged} iters={r.iters} status={r.final_status}")
    print(f"      → ★no false convergence★: model never fixes → converged={r.converged} after {r.iters} "
          f"iters, status={r.final_status}. HARAN never rubber-stamps; a weak model just loops + fails honestly.")


def honest_provenance():
    r = AG.write_verify_fix("sum 1..n", mock_sequence=[WRONG, GOOD])
    ok = r.source == "mock-sim"
    check("honest_provenance", ok, f"source={r.source}")
    print(f"      → mock loop labeled source='{r.source}': the loop + HARAN's counterexamples are REAL; "
          f"only the model's text is scripted. Never a fake 'live'.")


if __name__ == "__main__":
    print("v22 Part S · S3 — write→verify→FIX (the heart)")
    converges_via_counterexample(); counterexample_is_fed_back()
    no_false_convergence(); honest_provenance()
    print(f"\nS3: {len(PASS)} passed, {len(FAIL)} failed, {len(SKIP)} skipped")
    sys.exit(1 if FAIL else 0)
