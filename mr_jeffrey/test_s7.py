"""v22 Part S · S7 tests — integrated agentic_code() + honest measurement. Run: python3 test_s7.py

end_to_end_proven   : agentic_code → write→verify→fix→optimize→Type A tier, all wired, real ms.
history_threaded    : conversation history is threaded into the task (follow-up accumulation).
honest_measurement  : measured wall-clock + actual solved/proven/optimized; wrong==0 in both modes.
modes_measured      : extended solves >= normal across the corpus (measured), both zero wrong.
only_proven_optimized: unconverged tasks get no optimization/tier (we never optimize unproven code).
"""
import sys

import agentic as AG

PASS, FAIL, SKIP = [], [], []


def check(n, c, d=""):
    (PASS if c else FAIL).append(n)
    print(f"  [{'PASS' if c else 'FAIL'}] {n}" + (f" — {d}" if d and not c else ""))

GOOD = "fn triangular(n: Nat) -> Nat\n  ensures result = n*(n+1)/2\n{ fold k in 1..n { k } }"
WRONG = "fn triangular(n: Nat) -> Nat\n  ensures result = n*(n+1)/2\n{ fold k in 1..n { k+1 } }"


def end_to_end_proven():
    r = AG.agentic_code("sum 1..n", "extended", mock_sequence=[WRONG, GOOD])
    ok = (r.converged and r.status == "VERIFIED" and r.proof_tier == "PROVEN"
          and r.optimization and r.optimization.optimized and r.ms > 0)
    check("end_to_end_proven", ok,
          f"status={r.status} tier={r.proof_tier} opt={r.optimization and r.optimization.optimized} ms={r.ms:.1f}")
    print(f"      → request → fix({r.iters} iters) → VERIFIED → Type A {r.proof_tier} (∀) → optimized to "
          f"'{r.optimization.closed_form}' ({r.optimization.speedup}). Measured {r.ms:.1f}ms (mock, no network).")


def history_threaded():
    hist = [("sum 1..n", GOOD)]
    r = AG.agentic_code("now sum of odds", "normal", history=hist, mock_sequence=[GOOD])
    ok = r.history_len == 1 and r.converged
    check("history_threaded", ok, f"history_len={r.history_len} converged={r.converged}")
    print(f"      → prior turn threaded into context (history_len={r.history_len}); follow-up instructions "
          f"accumulate (this is what v23's continuous-instruction loop builds on).")


def honest_measurement():
    m = AG.measure_agentic(mode="extended")
    ok = m.solved == m.n and m.proven_forall >= 1 and m.optimized >= 1 and m.wrong == 0 and m.total_ms > 0
    check("honest_measurement", ok,
          f"solved={m.solved}/{m.n} proven={m.proven_forall} optimized={m.optimized} wrong={m.wrong} ms={m.total_ms:.1f}")
    print(f"      → MEASURED (mock, no network): extended solved {m.solved}/{m.n}, {m.proven_forall} PROVEN ∀, "
          f"{m.optimized} optimized, wrong={m.wrong}, {m.total_ms:.1f}ms. Real counts/ms — no fabrication.")
    print(f"      → cross-model '×N vs other AI' is NOT measured here → [TBD: measured]. Marketing copy "
          f"stays in the UI labeled `// marketing copy`, never mixed with these numbers.")


def modes_measured():
    n = AG.measure_agentic(mode="normal")
    e = AG.measure_agentic(mode="extended")
    ok = e.solved >= n.solved and n.wrong == 0 and e.wrong == 0
    check("modes_measured", ok, f"normal={n.solved}/{n.n} extended={e.solved}/{e.n} wrong n/e={n.wrong}/{e.wrong}")
    print(f"      → measured: NORMAL solves {n.solved}/{n.n}, EXTENDED {e.solved}/{e.n} (extended ≥ normal); "
          f"both wrong=0. Extended solves the 3-try task normal leaves shallow.")


def only_proven_optimized():
    r = AG.agentic_code("unfixable", "normal", mock_sequence=[WRONG])  # never converges
    ok = (not r.converged) and r.optimization is None and r.proof_tier == "(not proven)"
    check("only_proven_optimized", ok, f"converged={r.converged} opt={r.optimization} tier={r.proof_tier}")
    print(f"      → unconverged task → no optimization, tier='{r.proof_tier}'. We NEVER optimize or "
          f"claim a proof tier for unproven code.")


if __name__ == "__main__":
    print("v22 Part S · S7 — integrated agentic_code() + honest measurement")
    end_to_end_proven(); history_threaded(); honest_measurement(); modes_measured(); only_proven_optimized()
    print(f"\nS7: {len(PASS)} passed, {len(FAIL)} failed, {len(SKIP)} skipped")
    sys.exit(1 if FAIL else 0)
