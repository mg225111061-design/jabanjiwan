# HARAN v7 — AI write→verify→fix loop, live-ready (Claude API)

v3–v6 ran the loop on a ScriptedLLM simulation. v7 wires a LIVE Claude adapter and upgrades the loop
with research-grade techniques — while staying honest when no API key is present.

## What v7 adds
- **Live Claude adapter** (`get_claude_writer_verifier`): reads `ANTHROPIC_API_KEY` from env ONLY,
  selects LIVE Claude when present, else falls back to a labeled SIMULATION. **The key is never logged**
  (verified by a test that captures stdout and asserts the secret never appears).
- **Context separation**: writer and verifier are SEPARATE adapter instances ⇒ separate contexts
  (anti self-deception — the fixer doesn't share the writer's conversation).
- **Structural minimal counterexample** (TraceCoder-style): Mr returns the SMALLEST failing input;
  the fixer prompt is stripped to `at {n=2}, your output 3 ≠ spec 5` — targeted, noise-free.
- **Round limit** (default 3; research: most gains within 2 rounds) — an always-wrong model STOPS at
  the limit (no infinite loop), proven by a test.
- **Grammar guidance + parse-failure measurement**: the Claude API has no GBNF, so we guide via a
  system-prompt HARAN grammar hint and MEASURE the parse-failure rate (0% on generated samples).

## Measured (sim; live needs a key)
- sim loop converges in **2 rounds** (wrong Σk → minimal cx n=2 → fixed Σk² → VERIFIED).
- always-wrong model stops at the **3-round limit**, not converged.
- minimal cx fed to the fixer; API key never leaked.

## Honest status / DEFERRED
- **No `ANTHROPIC_API_KEY` in this environment → live is BLOCKED; the loop runs in SIMULATION.** The
  loop logic and Mr's counterexamples are REAL; only the model's text is scripted. Set the key to go live.
- GBNF grammar-constrained decoding is not available via the Claude API — replaced by prompt guidance
  + parse-failure measurement. Live sim-vs-live convergence comparison requires a key.
