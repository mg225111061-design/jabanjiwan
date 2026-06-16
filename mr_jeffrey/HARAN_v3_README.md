# HARAN v3 — proof power + unstructured conquest + AI loop

HARAN turns code into a thing Mr.Jeffrey can **verify**, and pushes three frontiers at once — while
refusing to blur a single honest line. Built on Mr.Jeffrey (`df60cac`) + HARAN v1 (79) + v2 (89).

## The three capabilities

1. **Structuring①** (v1/V1) — forceable on almost all code. Specs (`requires`/`ensures`/`decreases`/
   `produces`) live *in* the code, classified by verifiable fragment (exact / bounded / outside).
2. **Folding②** (v2) — a mathematical fact the universe decides. We fold *everything foldable* and
   leave **0 foldable missed** (Faulhaber + Gosper + C-finite are complete for their classes), and
   prove **absence** where a real decision procedure says so:
   - `CLOSED` — Faulhaber (JEFF coeff-zero) · Gosper (sympy-verified) · C-finite (companion≡naive)
   - `ABSENT` — Gosper-nonsummable (Σ1/k) · Galois-radical (x⁵−x−1) · Liouville-elementary (erf)
   - `NO_STRUCTURE` — data-dependent (Σ is_prime(k)) → Ω(N). *Recognition, NOT a Galois proof.*
3. **Conquest + proof + AI** (v3):
   - **X1** Z3 relational backend → proves inequalities `|approx − exact| ≤ ε` ∀.
   - **X2** unstructured conquest — approximate, then **PROVE** the error:
     - `PROVEN-BOUND` (deterministic, Z3 ∀-proof) — *truly conquered*. e.g. bucketed quantile ≤ w/2.
     - `TESTED-BOUND` (probabilistic sketch, empirical) — *structured ≠ conquered*. e.g. KMV distinct.
     - `REJECTED-EXACT` — payment/crypto/exact-search → approximation **refused**, Ω(N) kept.
   - **X3** exact paths proven too — `PROVEN` (∀, JEFF) / `PROVEN-BOUNDED` (exhaustive over a finite
     domain — sort: all |xs|≤4) / `TESTED`; plus contract / aliasing / ADT-exhaustiveness checks.
   - **X4** AI write→verify→fix loop — Qwen3-32B (think-off to write, think-on to fix); Mr's
     **counterexamples** drive convergence; the model is swappable, Mr's verification is the product.

## The honesty lines (the lifeline)

- **"Conquest" counts only when the error is PROVEN.** `PROVEN-BOUND` and `TESTED-BOUND` are never
  mixed; a probabilistic sketch is never relabeled a worst-case proof.
- **Unstructured speed is constant-factor, never orders of magnitude.** Ω(N) is acknowledged. Sketches
  may win *space* asymptotically — stated with the error caveat; the exact-unstructured path claims only C-equivalence.
- **VERIFIED is always *명세 대비*** (against the spec; the spec's own correctness is unverified — Boundary 1).
- **proven ≠ tested ≠ proven-bounded.** Tiers name exactly what was shown (e.g. sort is bounded-exhaustive,
  *not* a full ∀-proof — Z3 array-induction is out of scope and not claimed).
- **AI is Qwen3-32B (replaceable).** If no local endpoint answers, the model text is a labeled
  **SIMULATION** — the loop and counterexamples stay real. Mr's verification compensates for a weak model.

## The five boundaries

1. Ω(N) information floor — data-dependent work can't be folded (count_primes, sort).
2. #P / NP-hardness — not dissolved by reformulation.
3. Undecidability (Rice/halting) — Total mode + sound over-approximation, never a halting oracle.
4. No Z3/Lean *inside* JEFF (ordinal.rs §32.0) — quantified specs are bounded-tested, not exact-proven,
   unless discharged by the *external* Z3 we wired in X1 (inequalities/FOL over arithmetic).
5. Physics counterfactuals — refused.

## Whole-system measurement (raw, this run)

| capability | result |
|---|---|
| fold ratio (v2) | 70% CLOSED · 20% ABSENT(proven) · 10% NO_STRUCTURE · 0% UNKNOWN · **0 foldable missed** |
| unstructured conquest | of approximated ops, **50% PROVEN-BOUND** (truly conquered); rest TESTED-BOUND / rejected-exact |
| exact proof | **100%** of sample exact specs PROVEN / PROVEN-BOUNDED |
| AI loop | converges (sim): wrong → real cx `{n=2: impl 3, spec 5}` → fixed → VERIFIED in 2 iters |

## Honest status / deferred

- **Z3**: pip-installed in-session (ephemeral container) — `requirements-v3.txt`. If absent, v3 degrades
  to TESTED-BOUND and the Z3 tests **SKIP** (never fake-pass).
- **AI**: **simulated** (no local Qwen3-32B endpoint). `ollama pull qwen3:32b` to go live.
- **Not conquered**: count_primes (data-dependent, Ω(N)); sort's full ∀-proof (bounded-exhaustive only);
  probabilistic sketches (TESTED-BOUND, by nature). All stated, none hidden.

## Run

```
pip install -r mr_jeffrey/requirements-v3.txt      # Z3 (optional; degrades honestly without it)
python3 mr_jeffrey/haran_v3.py                      # measurement + showcase
python3 mr_jeffrey/test_x1.py ... test_x5.py        # stage tests
```
