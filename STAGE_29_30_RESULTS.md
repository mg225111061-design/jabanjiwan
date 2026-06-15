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

## §C.30 — BBP / Tracy–Widom soundness gate
_(pending — Stage 30)_
