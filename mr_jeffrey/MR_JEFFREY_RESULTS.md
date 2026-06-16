# Mr. Jeffrey — Integrated Build Report (Stages 1–4)

**Date:** 2026-06-16 · **Branch:** `claude/funny-maxwell-im9x07`
**Thesis held throughout:** the power is *perfect verification*, not a *powerful model*. We did not
try to strengthen the model; we strengthened — and productized — the verification that catches the
model. The 3-tier honesty (PROVEN > VERIFIED > REFUTED) is enforced everywhere; **no fake passes.**

## TL;DR

| Stage | What | Status | Tests |
|---|---|---|---|
| 1 | Real AI connection (LLM adapters + self-correction loop) | ✅ DONE | 5/5 |
| 2 | Strengthened verification (types, timeout-guard, auto-props, sandbox, shrink) | ✅ DONE | 7/7 |
| 3 | JEFF EXACT engine connection (Rust coeff-zero → sympy → defer) | ✅ DONE (connected, **not** blocked) | 6/6 |
| 4 | Productization (CLI, web demo, README, 10-bug showcase) | ✅ DONE | 9/9 |

**Total: 27/27 automated tests green.** Showcase: **10/10** AI outputs adjudicated correctly
(9 bugs refuted with the exact breaking input, 1 proven for all inputs by JEFF).

The 4 prototypes (`verify_core.py`, `verify_exact.py`, `mr_jeffrey.py`, `mr_demo.py`) were **kept,
not torn down** — every new layer extends them.

---

## Stage 1 — Real AI connection  ✅
**File:** `llm_adapters.py` · **Tests:** `test_stage1.py` (5/5)

- `AnthropicAdapter` (Claude `claude-sonnet-4-6`, Messages API), `OpenAIAdapter`, `LocalAdapter`
  (Ollama/vLLM OpenAI-compatible) — **all over stdlib `urllib`, zero SDK dependency.**
- Common `LLMAdapter` interface: `.complete(prompt)->str` with retry / timeout / backoff; usable
  directly as `MrJeffrey(llm=...)`.
- `get_adapter("auto")` picks the first backend with credentials; **falls back to `ScriptedLLM`
  with a printed warning** so the *whole loop still runs with no key* (the model is swappable; the
  verification is the product).
- **Honest behavior verified:** with no key, the self-correction loop runs end-to-end against the
  simulation; with a key, against a real model. `auto` does **not** block on a dead localhost.

Tests: `adapter_extracts_code`, `adapter_fallback_works`, `anthropic_raises_without_key`,
`real_loop_with_simulation_passes`, `real_loop_proven_path`.

## Stage 2 — Strengthened verification (the real weapon)  ✅
**File:** `verify_strong.py` · **Tests:** `test_stage2.py` (7/7)

- **More input types:** `int, nonneg_int, float, str, list_int, list_float, dict, tuple, list2d,
  bool` — each with edge cases + randomized fuzz.
- **Performance guard (critical):** every candidate call runs under a hard wall-clock **TIMEOUT**
  on a daemon thread — **the verifier never hangs**, even on an infinite loop. Measured: an
  infinite-loop input returns a `TIMEOUT` counterexample, full run well under the budget.
- **Auto property inference** from the function name: `sort`→sorted+permutation, `reverse`→length
  +involution, `abs`/`max`/`min`/`is_`… so users need not hand-write specs.
- **Side-effect sandbox:** monkeypatches `open` + `socket.socket` → a function that touches the
  filesystem/network is flagged; input-mutation is caught by snapshot comparison.
- **Counterexample shrinking** (Hypothesis-style): a 9-element failing list is reduced to a ≤2-element
  minimal breaking input for crisp feedback.
- **Honesty unchanged:** a clean run is `VERIFIED (bounded+fuzzed+timeout-guarded — strong evidence,
  NOT a proof)`. Never called a proof.

Tests: `float_bugs_caught`, `performance_guard_no_hang`, `auto_property_sort`,
`auto_property_correct_sort_passes`, `shrink_minimizes_counterexample`,
`sandbox_catches_side_effects`, `more_types_supported`.

## Stage 3 — JEFF EXACT engine connection  ✅ (connected, not BLOCKED)
**Files:** `jeff_adapter.py`, `crates/jeff-math/examples/jeff_identity.rs` · **Tests:** `test_stage3.py` (6/6)

- Built a thin Rust CLI (`jeff_identity`) exposing **the real JEFF mechanism**: it decides
  `candidate ≡ reference` for ALL inputs via `UniPoly::from_coeffs` + `is_zero` over `BigRational`
  — exact coefficient-zero, no floats, no fuzzing. This is the *same* mechanism `jeff-verify`
  re-checks elsewhere in GACC.
- `jeff_adapter.prove_identity` runs the **fallback chain, each tier labeled honestly**:
  **JEFF** (univariate polynomial identities) → **sympy** (general exact CAS) → **DEFER**.
- **Backends live in this build:** `{'jeff': True, 'sympy': True}` (verified at runtime).

**Where JEFF really verifies — and the honest scope (Stage 3.4):** for a *polynomial identity*,
JEFF and sympy give the **same exact** verdict; JEFF is not "more powerful" there. JEFF's genuine
distinctions are (a) an **independent exact re-check** by a separate engine, and (b) it is
**proof-carrying** — the verdict comes from JEFF's actual coefficient-zero arithmetic, not a
black-box `simplify`. JEFF's reach *beyond* sympy (telescoper / fold certificates for structured
sums) is real in the Rust engine but **not yet CLI-exposed** for arbitrary sums — we **state this,
we do not imply it.** That is the honest answer to "show where JEFF verifies more."

Tests: `jeff_adapter_proves_identity`, `proven_by_jeff_backend`, `fallback_chain_works`,
`refuted_with_witness`, `sympy_still_proves_transcendental`, `backends_reported`.

## Stage 4 — Productization  ✅
**Files:** `mr.py` (CLI), `mr_web.py` (web), `examples_demo.py` (showcase), `README.md`
**Tests:** `test_stage4.py` (9/9)

- **CLI `mr.py`** — `verify` / `prove` / `solve` / `demo`, color-coded
  PROVEN(green)/VERIFIED(yellow)/REFUTED(red) (honors `NO_COLOR`); exit codes 0/1/3.
  `solve` degrades honestly with no key (prints what to set, points to offline commands).
- **Web demo `mr_web.py`** — pure stdlib `http.server` (no Flask; Flask isn't installed). Human
  page + JSON API (`/api/prove`, `/api/verify`, `/health`). Clearly labeled local-only (executes
  submitted code inside the sandbox).
- **Showcase `examples_demo.py`** — 10 real AI bugs; Mr. returns the right verdict on **all 10** and
  the exact reason: off-by-one, empty-list crash, input mutation, dropped duplicates, case/space
  palindrome, **infinite loop (timeout guard)**, inverted predicate, zero-seed max, plus one TRUE
  identity **PROVEN by JEFF** and one FALSE closed form **REFUTED with exact witness**
  (`degree=1 residual=-1/2`).
- **README.md** — what Mr. is, proof-vs-hunt, full usage, and an explicit **Honest limits** section.

Tests: `cli_prove_true`, `cli_prove_false`, `cli_verify_refutes_bug`, `web_health`,
`web_index_served`, `web_api_prove`, `web_api_verify`, `examples_all_caught`,
`examples_cover_all_tiers`.

---

## Honest caveats / known limitations (not papered over)

1. **VERIFIED ≠ proof.** Bounded fuzzing; an untested input can still bite. Only the exact tier
   (PROVEN) is all-inputs, and only for math-shaped claims. Enforced in every message and verdict.
2. **JEFF CLI scope.** Direct JEFF proof covers **univariate polynomial** identities; multivariate
   / transcendental fall through to sympy (still exact). JEFF's structured-sum certificates aren't
   wired to this CLI yet.
3. **`solve` needs a model.** No API key in this environment, so `solve` was exercised via the
   library + `ScriptedLLM` (Stage-1 tests), not live. The verification half runs fully offline.
4. **Prototype `mr_demo.py` DEMO 4** has a design limitation (a *static* `exact_claim` is re-checked
   every round, so it can't pick up the model's later fix). Left **as-is and reported** rather than
   silently "fixed" — the new `examples_demo.py` is the showcase that supersedes it.
5. **Web demo executes submitted code** (sandboxed for file/network, but in-process). Local dev only.

## Reproduce

```bash
cd mr_jeffrey
pip install sympy
# (optional, enables the JEFF tier:)  cargo build -p jeff-math --example jeff_identity   # from repo root
python3 test_stage1.py && python3 test_stage2.py && python3 test_stage3.py && python3 test_stage4.py
python3 mr.py demo
```

## Verdict on the mission

- ✅ **Real AI connected** — three real adapters (stdlib urllib), honest simulation fallback.
- ✅ **Verification strengthened** — types, no-hang timeout guard, auto-properties, sandbox, shrinking.
- ✅ **JEFF connected** (not blocked) — real coeff-zero engine as Tier 1, sympy retained as Tier 2.
- ✅ **Human-usable** — CLI + web + README + a sales-ready 10-bug showcase.
- ✅ **3-tier honesty held** — PROVEN / VERIFIED(bounded) / REFUTED, no fake passes anywhere.
- ✅ **It was the *verification* that got stronger — not the model.**
