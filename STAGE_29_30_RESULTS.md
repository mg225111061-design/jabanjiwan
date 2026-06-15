# Stage 29 → 30 results — §C (verification-power, NOT speed collapse)

These two stages strengthen JEFF's **verification infrastructure**; neither is an asymptotic speed
ratio. They sit **on top of** the Stage 26–28 honesty split (dense = parity ceiling, never "beat";
structured = asymptotic infinite ratio, Ω(N)-safe):

- **Stage 29 (#5 SOS)** — a *precondition prover*: a sound, machine-checked nonnegativity
  certificate (`prove_nonneg`) that lets other folds discharge "denominator > 0 / error ≥ 0".
- **Stage 30 (#4 BBP)** — a *fold-or-defer gate*: a quantitative ε-level absence certificate for
  "is there low-rank structure, or just noise?".

All numbers raw on this machine (`Intel Xeon @ 2.80GHz`, AVX-512), `--release`.

> **Premise correction (honesty).** The directive frames Stage 29 as "replace the empty
> `prove_nonneg` SMT stub". There is **no such stub in this repository** — `grep -rn prove_nonneg`
> over the whole tree is empty (shown at the entry gate). So Stage 29 **adds** the capability (a new
> sound nonnegativity prover wired as a precondition gate); it does not replace a flagged stub. The
> verification value is identical either way; only the "replace" narrative is corrected.

---

## §C.29 — Rational SOS nonnegativity certificate (`prove_nonneg`)

Port of reference kernel #5 (`kernels/k5_rational_sos.py`) to `jeff_math::sos`. **Sound but
incomplete** (Positivstellensatz / Motzkin): SOS ⇒ exact certificate; nonnegative-but-not-SOS or
undecidable-here ⇒ HONEST_DEFER.

### SDP path chosen: **(ii) verification-only, no SDP solver linked**
Rust has no native SDP solver and we link none (R5; CLARABEL/SCS would be an FFI/GPL concern, and
`cvxpy` is absent here so the reference's numerical SDP cannot be run). Per the directive, the SDP
is **only the guesser**; the soundness lives entirely in the **exact rational** verification. The
guesser here is a built-in **diagonal-placement heuristic** (each target coefficient placed into one
monomial slot, diagonal preferred), which handles the easy SOS/PD cases; harder SOS polynomials
needing a cross-term `Q` can be certified by supplying an external numeric `Q` via
[`prove_nonneg_with_q`] (exercised by `external_numeric_q_path_certifies`).

The exact verification core (over `BigRational`, no float slack): `rational_round_matrix` →
`fix_identity` (minimal repair so `zᵀQz = p` exactly) → `verify_identity` (exact term-by-term) →
`exact_ldl_psd` (PSD iff all pivots ≥ 0 and the factorization completes; a zero pivot with a nonzero
entry below ⇒ saddle ⇒ reject).

### (a) Certification correctness — exact rational LDLᵀ
| polynomial | basis | outcome | exact LDLᵀ pivots |
|---|---|---|---|
| `p1 = x⁴−2x³+4x²+2 = (x²−x+1)²+(x+1)²` | `[1,x,x²]` | **certified** | `[2, 4, 3/4]` (all ≥ 0) |
| `p2 = 2x²+2xy+2y²` (PD) | `[1,x,y]` | **certified** | `[0, 2, 3/2]` (all ≥ 0) |

`ldl_pivots_nonneg_exact` asserts p1's pivots are exactly `[2, 4, 3/4]`. (These are cleaner than the
reference's `[2, 174775/65536, …]`, which come from its *numerical* SDP `Q` rounded at denom 2¹⁶; my
diagonal guesser yields an exact `Q` directly. Both are valid certificates — stated, not silently
matched, per rule 6.) Tests: `sos_poly_certified_exact`, `sos_pd_quadratic_certified`.

### (b) Honest negatives — sound but incomplete
| polynomial | why it defers | mechanism |
|---|---|---|
| **Motzkin** `x⁴y²+x²y⁴−3x²y²+1` (nonneg, NOT SOS) | the exact identity `Q` carries `−3` on the `x²y²` diagonal slot | exact LDLᵀ pivot `< 0` ⇒ not PSD ⇒ **DEFER** |
| **indefinite** `x²−1` | identity `Q` is `diag(−1, 1)` | negative pivot ⇒ **DEFER** |
| **unsatisfiable basis** (`p1`, half_deg=1) | `x⁴` term has no monomial-pair slot | identity-repair fails ⇒ **DEFER** |

The Motzkin defer is **rigorous**, not "no input": every `Q` satisfying `zᵀQz = Motzkin` is non-PSD
(Motzkin not-SOS ⟺ no PSD `Q`), and the engine's exact `Q` exhibits the `−3` pivot directly. Tests:
`motzkin_defers_honestly`, `indefinite_defers`, `rounding_failure_defers`.

### (c) Stub status + precondition gate
- `prove_nonneg` is a real, input-dependent decision procedure (certifies p1/p2, defers
  Motzkin/indefinite) — `prove_nonneg_no_longer_stub`. (As noted, no prior stub existed; this is the
  added capability.)
- Wired as a precondition gate `require_nonneg` → `GateDecision::{Fire, HonestDefer}`: a dependent
  fold fires only when its precondition is certified SOS, else HONEST_DEFERs. Tests:
  `precondition_gate_fires_on_sos`, `precondition_gate_defers_on_nonsos`.
- **No regression**: `jeff-verify` integrity suite 49/0 (all `false_*`/tamper/sorry-unknown), full
  workspace 461/0, clippy `--all-targets -D warnings` clean.

### Measurement (verification-power, not speed)
SDP basis size is `C(n+d, d)`: e.g. (nvars, half_deg) → size = (2,2)→6, (3,2)→10, (4,2)→15,
(6,2)→28 (matches `C(n+d,d)`). Exact certification timing: p1 (1 var, deg 4) **≈ 15 µs**, p2 (2 var,
deg 2) **≈ 11 µs**. Practical envelope ≤ 6 vars / deg ≤ 4.

**Label.** *Not a speed collapse — fills the nonnegativity-proving gap with a sound-but-incomplete
verification certificate. SOS ⇒ exact rational certificate; nonnegative-but-not-SOS ⇒ honest
don't-know. Path (ii): built-in guesser + exact rational verification, no SDP solver linked.*

---

## §C.30 — BBP / Tracy–Widom soundness gate (fold-or-defer meta-gate)

Port of reference kernel #4 (`kernels/k4_bbp_gate.py`) to `jeff_math::bbp` + the dispatch wiring in
`jeff_collapse_arith::bbp_gate`. **A meta-gate, not a collapse**: it decides whether to *attempt* a
low-rank fold and emits a **quantitative ε-level absence certificate** when there is no structure.
**Probabilistic, never exact**; the ε is **valid only under iid-Gaussian noise** — void under
heavy-tailed / correlated / non-Gaussian noise (carried in `EPS_MODEL_NOTE` on every certificate).

### (a) RMT theory reproduced (n=300, m=200, γ=0.667)
| quantity | JEFF | reference |
|---|---|---|
| MP top edge `1+√γ` | **1.8165** | 1.8165 |
| BBP threshold `θ* = γ^{1/4}` | **0.9036** | 0.9036 |
| Tracy–Widom scale `n^{-2/3}` | **0.0223** | 0.0223 |

Top singular value via power iteration on `AᵀA` (`O(nnz)`/iter) — `top_singular_value_matches_known`
(diag(3,2)→3). Tests: `mp_edge_reproduced`, `bbp_threshold_reproduced`, `tw_scale_order_correct`.

### (b) Gate decisions + BBP transition
Calibrated threshold (1−ε=0.99 quantile of the pure-noise top sv) = **1.8553**. Detection
probability rises **sharply through θ*=0.9036**:

| θ | 0.0 | 0.5 | 0.81 | **0.904 (θ*)** | 0.99 | 1.30 | 2.00 |
|---|---|---|---|---|---|---|---|
| P(detect) | 0.013 | 0.013 | 0.013 | **0.050** | 0.175 | 1.000 | 1.000 |
| mean top sv > MP edge? | no | no | no | no | yes | yes | yes |

Pure noise → **DEFER** (top sv 1.82 < 1.855); noise + strong rank-3 → **FOLD** (top sv 2.99 > 1.855,
separated from bulk). Tests: `pure_noise_defers`, `noise_plus_rank3_folds`,
`bbp_transition_sharp_at_threshold`.

### (c) ε honesty (false-positive rate)
On 1000 pure-noise trials the empirical FOLD (false-positive) rate = **0.0030** vs target ε=0.01
(reference 0.018). Same order as ε; **slightly conservative** here (vs the reference's slightly
anti-conservative 0.018). Honest difference: different RNG, calibration trial count (300), and
power-iteration vs the reference's exact-SVD calibration — stated, not silently matched (rule 6).
Test `fp_rate_honors_eps` asserts the rate stays within a few × ε.

### (d) HONEST_DEFER dispatch wiring
`jeff_collapse_arith::bbp_gate::low_rank_gate` consults the gate before a low-rank fold: structure
present → `AttemptFold` (the real fold runs, with its own exact certificate); absence →
`CollapseOutcome::Defer(BarrierTag::BelowDetectionThreshold)` carrying the ε-absence note — wired
into the same `BarrierTag` dispatch the Fourier detectors use. Tests: `gate_wired_into_dispatch`,
`gate_prevents_wasted_lowrank_fit` (the `O(nnz)` gate primitive is measurably cheaper than a full
SVD fit, and on noise the gate defers ⇒ the fit is skipped entirely). **No regression**:
verifier 49/0, workspace 471/0, clippy `--all-targets -D warnings` clean.

**Cost.** Gate primitive `O(nnz)` (a few power iterations) vs a full low-rank fit `O(n²·r)`:
**infinite expected saving under the null** (no structure ⇒ defer ⇒ fit skipped); **ratio 1** when
structure is present (the fit runs anyway). Not an asymptotic speed collapse.

**Label.** *Not a collapse — a meta-gate emitting a quantitative absence certificate.
Probabilistic (explicit ε), never exact, assumes iid-Gaussian noise (ε void under heavy tails). The
hidden-structure precondition detector for the 45→90 coverage push.*

---

# Unified closing — §C.29–30

| stage | kernel | role | nature | certificate |
|---|---|---|---|---|
| **29** | #5 rational SOS | precondition **prover** (`prove_nonneg`) | verification-power | **exact** rational SOS (LDLᵀ pivots ≥ 0); SOS ⇒ certify, else HONEST_DEFER |
| **30** | #4 BBP/TW | fold-or-defer **gate** (`low_rank_gate`) | verification-power | **probabilistic** ε-absence (Gaussian-only; void under heavy tails) |

Both are **verification-power, not speed collapse** (29 = precondition prover that lets other folds
fire safely; 30 = fold-or-defer gate that prevents wasted low-rank fits). They sit **on top of** the
Stage 26–28 honesty split — **dense = parity ceiling** (Stage 27 GEMM ~50% of OpenBLAS-1T, Stage
28.1 FFT radix-2 ~72–85% of pocketfft; "reached X%", never "beat") and **structured = asymptotic
infinite ratio, Ω(N)-safe** (Stage 26 C-finite/holonomic divergent ratios, Stage 28.2 2D sparse
n/k crossover) — strengthening JEFF's verification infrastructure rather than its speed frontier.

Both kernels reproduce the Python reference's certification / measured behavior: #5 the
certification *outcomes* (p1/p2 certified, Motzkin/indefinite defer; exact pivots cleaner than the
reference's numerical-SDP pivots — stated), #4 the RMT constants exactly and the FP rate to the same
order (0.0030 vs ref 0.018, both ~ε).

### Honest premise/limit notes
- **Stage 29**: there was **no pre-existing `prove_nonneg` stub** in this repo — the capability was
  *added*, not a stub *replaced* (the directive's framing corrected, value identical). SDP path (ii)
  chosen: built-in diagonal guesser + exact rational verification, **no SDP solver linked** (R5;
  cvxpy absent). Sound but **incomplete** (Motzkin).
- **Stage 30**: **probabilistic** (ε), never exact; ε **void** under non-Gaussian/heavy-tailed/
  correlated noise. FP rate is finite-sample and calibration-dependent.

### Deferred (not done this round, per directive)
- #1 displacement-rank and #7 Krylov → a later coverage-extension stage.
- Stage 27 GEMM → 80%: needs per-µarch hand assembly (profiled bottleneck), a separate effort; dense
  remains a regime JEFF does not win (parity is the ceiling).
