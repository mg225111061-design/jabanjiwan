# HARAN v22 — Part S: LLM + math agentic coding (write → verify → fix → optimize)

Claude *writes* code; HARAN *verifies* it against its spec, hands back a concrete counterexample when
it's wrong, and (once proven) *mathematically optimizes* it to a closed form. The model is swappable;
**HARAN's verification is the product.** Built on v7's loop, v17's fusion, v21's two modes.

## The pipeline (`agentic.agentic_code(request, mode, api_key, history?)`)
```
[history + request] → Claude writes → HARAN verifies (spec) → wrong? feed counterexample back → fix
                    → PROVEN → fold-optimize (closed form, O(1)) + Type A proof tier → measured ms
```

## Stages
- **S1 — Claude integration + key security LEVEL 1.** `claude_agent.claude_generate()`: real path via the
  official Anthropic SDK (default `claude-opus-4-8`, adaptive thinking, streaming); mock path returns
  deterministic, parseable HARAN labeled `source='mock-sim'` (never a fake "live"). **Key is
  argument-only** — the module doesn't even `import os`, so it structurally cannot touch env/files; the
  key is never stored in env / a file / a log / a cache / a global / the result, and is dropped after one
  call. Entered every call, kept nowhere.
- **S2 — write → verify.** Claude proposes; HARAN returns VERIFIED or the *smallest* failing input +
  impl/spec mismatch. Verification is **spec-relative (`ensures`), not intent** — a "VERIFIED" means the
  code meets the stated spec, not that Claude guessed what you meant.
- **S3 — write → verify → FIX (the heart).** The counterexample is fed back into the next prompt;
  repeat until PROVEN. A weak model just loops more; **HARAN never rubber-stamps, so there is no false
  convergence** (a model that never fixes never gets a false PROVEN).
- **S4 — fold optimization.** A proven fold collapses to a closed form (Faulhaber/C-finite/hypergeometric):
  `Σk → 1/2·n+1/2·n²`, asymptotic **O(1)**. The speedup is a *proven structural* asymptotic class, **not a
  fabricated wall-clock number**; code with no structure is honestly *not* optimized.
- **S5 — two modes (reuse v21).** NORMAL (shallow fix budget = fast, common case) vs EXTENDED (deeper
  budget = solves more). **Both modes: zero wrong answers** — normal's miss is `UNRESOLVED-shallow`
  (didn't look deeper), never a false PROVEN.
- **S6 — Type A (spec-embedded) proof tiers.** The embedded `ensures` is discharged by the exact engine
  into a *graded* tier: `PROVEN` (exact ∀) / `PROVEN-BOUNDED` / `TESTED` (fuzz, honestly weaker) /
  `FAILED` / `UNKNOWN`. Tiers are kept distinct — **TESTED is never inflated to PROVEN.**
- **S7 — integrated `agentic_code()` + honest measurement.** Everything wired, with conversation
  `history` threading (v23's follow-up loop builds on this) and a real measured wall-clock.

## Measured (mock, no network — `measure_agentic`)
4-task corpus, **EXTENDED**: solved 4/4, PROVEN ∀ 4/4, optimized 4/4, **wrong 0**. NORMAL: solved 3/4
(leaves the 3-try task `UNRESOLVED-shallow`), **wrong 0**. (Wall-clock is real but machine-dependent —
dominated by sympy/Z3 warmup; reported by the test run, not hardcoded.)

## Honesty (the v22 bar)
- **Key level-1 is structural**, not a promise: verified by `test_s1.key_not_stored` (no env/global/
  attr/result/error/log; `os` not imported).
- **Mock provenance is always labeled** `source='mock-sim'`; a simulation is never reported as live. The
  loop and HARAN's counterexamples are **real** regardless of whether the model text is live or scripted.
- **Verification is spec-relative, not intent** (Rice): we verify against `ensures`, and say so.
- **No false convergence, no inflated tiers, no fabricated speedups.** Asymptotic O(1) is a proven
  closed-form property; concrete "×N vs other AI" is **not measured here → [TBD: measured]**.
- **Marketing copy ("압도적인…") is UI-only, labeled `// marketing copy`**, never mixed with measured ms /
  PROVEN counts.
- **Live path needs a real key + `pip install anthropic`** (absent in this build env) → the live-success
  path is *user-confirmable*; the key-handling discipline is verified now even on the SDK-missing path.

## One line
**Claude writes, HARAN proves (against the spec) and fixes via real counterexamples until PROVEN, then
mathematically collapses the proven code to a closed form — two modes, both zero-wrong, the key stored
nowhere, every number measured or marked [TBD].**
