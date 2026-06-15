# Stage 31 → 41 — overnight autonomous build (§C)

Honest status table at the end. Disciplines (all stages): certificate kind always labeled
(exact-algebraic / Las-Vegas-residual / probabilistic-ε / SOS-exact / ordinal-termination /
absence-unsat / interval-bound / integer-exact); measured-win-only ("X% reached", never "beat" on
dense); no fake pass/number/absorption ("built" = Rust-reimplemented + tests green); premise
mismatches corrected and noted; dense = parity ceiling; infinite/huge/square ratios only with
structure, asymptotic, Ω(N)-safe. Entry & exit verifier re-proof (`jeff-verify` 49/0) every stage.

Reference kernels in `kernels/` (gitignored — external specs). cvxpy/scipy absent; FFTW is GPL
(out-of-process only, R5); OpenBLAS/pocketfft present.

---

## §C.31 — #1 displacement + #7 Krylov (7-kernel absorption complete) — **BUILT**

`jeff_math::displacement`, `jeff_math::krylov`. Extends the existing `fft_radix2`/`Complex`.

**31.1 displacement-rank** (kernel #1). Toeplitz matvec `O(n²)→O(n log n)` via 2n circulant
embedding + FFT. Structure certificate = **exact** Stein displacement `∇T = T − ZTZᵀ`, `rank(∇T) ≤ 2`
proven by fraction-free Gaussian elimination over ℚ. Dense random → high rank → HONEST_DEFER. Fast
matvec == dense **integer-exact** (residual 0); FFT path matches dense to f64 round-off.
- Certificate: **exact-algebraic** (rank) + **integer-exact** (residual). Float-only tiny-σ →
  ε-residual fallback (labeled). Ratio `~ n/(r log n)` — **bounded by the structure** (not infinite).
- Tests: `toeplitz_displacement_rank_certified`, `dense_high_rank_defers`, `fast_equals_dense_exact`,
  `fft_matvec_matches_dense_float`, `displacement_crossover_measured` (5/5).

**31.2 Krylov / CG** (kernel #7). SPD solve in a Krylov subspace. Integer/rational → **exact**
residual `b−Ax == 0` (CG terminates in ≤ n steps over ℚ). Float → **ε-residual** `‖r‖/‖b‖ ≤ tol`.
Ill-conditioned (Hilbert(8)) → CG stalls → residual large → certificate FAILS → HONEST_DEFER.
- Certificate: **integer-exact** / **ε-residual** / **absence** (ill-conditioned). Ratio `~ n²/√κ`
  **semi-bounded** (κ-dependent). Per `REPORT.md` #7 is "2/10 repackaging" — included for
  certificate uniformity, **not over-claimed** (this is the matrix/vector extension of the Stage-26
  C-finite companion exponentiation).
- Tests: `cg_residual_exact_integer`, `cg_solve_exact_larger_spd`, `cg_float_residual_certified`,
  `ill_conditioned_defers`, `krylov_crossover_measured` (5/5).

**7-kernel absorption complete**: #1 displacement, #2 ESPRIT/Bostan (Stage 26.1), #3 Ben-Or/Tiwari
(prior), #4 BBP gate (Stage 30), #5 rational SOS (Stage 29), #6 Zeilberger (prior),
#7 Krylov (this stage). All Rust-reimplemented + tests green.

Note (test robustness): a Stage-30 timing-based test (`gate_prevents_wasted_lowrank_fit`) was
flaky in debug (wall-clock assert) — replaced with a deterministic op-count proxy + the real
"defer skips the fit" fact. Workspace 481/0, clippy `--all-targets -D warnings` clean.

---

## §C.32 — ‖JEFF‖ strength audit + ordinal termination (OIFC) — **BUILT (with keystone negative)**

`jeff_math::ordinal`.

**32.0 strength audit (keystone) — RESULT: ‖JEFF‖ = ω^ω, NOT ε₀.** The directive's premise ("Z3
induction gate ⇒ arbitrary-predicate first-order induction ⇒ ε₀") is **false for this tree**
(rule 4). Raw grep of `jeff-verify`: **no Z3, no Lean** ("Neither is wired"); the checker discharges
**quantifier-free exact coefficient-zero polynomial identities** + exact replay. That is PRA-style;
its proof-theoretic ordinal is **ω^ω**. → **All ε₀ claims downstream (OIFC 32.3, HBFC 38.2) are
DOWNGRADED to ω^k.** This is the honest negative the keystone exists to catch (a win: the audit did
its job). Two-tier epistemic status labeled: lower bound `≥ ω^k` *internal machine-checked* (ordinal
CNF comparisons, exact); upper bound `≤ ω^ω` *paper metatheorem* — **not** provable inside JEFF
(Gödel II). Tests: `jeff_strength_lower_bound_certified`, `jeff_strength_two_tier_labeled`.

**32.1 ordinal_cnf**: recursive Cantor normal form below ε₀ (`Ord`), `ord_cmp`/`nat`/`omega`/
`omega_pow`/`is_below_epsilon0`/`in_omega_k_fragment`. Tests `ordinal_cnf_compare_correct`,
`ordinal_cnf_recursive_epsilon0` (ω < ω^ω < ω^(ω^ω) < ε₀).

**32.2 measure_synth**: `lex_measure` → `ω^{k-1}i₁+…+iₖ` for fixed-depth nested loops; strong
induction over μ realized by the transition checks. Tests `lexicographic_measure_synthesized`,
`strong_induction_finite_decomposed`.

**32.3 OIFC** (prefix-sum-of-prefix-sum): closed form `C(i)=Σ(i−j+1)A[j]` certified to equal the
loop output for **all i** via ordinal induction `μ = ω·i + j` (`< ω²`, in the certifiable ω^k
fragment — the ε₀→ω^k downgrade does **not** weaken it, since the real measure is only ω²). The
transition-2 boundary lemma `cum(i+1,0)=cum(i,i)+A[0]` is exactly where the off-by-one is caught:
the wrong closed form `(i−j)` is **rejected**. Certificate: **ordinal-termination + integer-exact**.
Tests: `nested_fold_globally_certified`, `transition2_offbyone_caught_by_ordinal`.

**32.4 FGH labels** (bonus): `fgh_level` reads complexity grade off the measure; explosive recursion
(level > intended) flagged. Tests `fgh_level_inferred`, `explosive_recursion_warned`.

Honest scope: measure auto-synthesis is general-undecidable (termination) — automatic only for
fixed-depth lexicographic; rest is annotation/HONEST_DEFER. Verification-power (compile-time, runtime
0). Veblen/Γ₀/OCF are domain-out, not built. 10 tests. Workspace green, clippy clean, verifier 49/0.

---

## §C.33 — lazy modular giant numbers (tetration mod p) — **BUILT**

`jeff_math::tetration`. **Value, not speed**: `2↑↑1000` cannot be materialized by any bignum (GMP
included) — but `a↑↑h mod m` is computable via the **totient ladder** `a^b ≡ a^{(b mod φ(m))+φ(m)}
(mod m)` for `b ≥ log₂ m`; the φ-chain reaches 1 in `O(log² m)` steps so the tower collapses.

- 33.1/33.3: `tetration_mod` — exact small towers taken directly (`2↑↑3=16`, `2↑↑4=65536`), huge
  towers via the **guarded** lift. Safety guard `lift_guard_ok` (`b ≥ log₂ m`) is load-bearing —
  only lift when the exact tower overflows the 2^64 cap (⇒ exponent ≥ 64 > log₂ m). `2↑↑1000 mod p`
  completes (GMP cannot). Certificate: **exact modular** (strongest); powerless for the full-value
  question (residues only) — stated.
- 33.2: `LazyBignum` demand-driven residues (only demanded moduli materialized + cached).
- 33.4: `ackermann_small` (m≤3, hard guard), `lucas_binomial_mod_p` (Lucas, no bignum) — oracle
  cross-checks. coprime guard demonstrated (gcd(2,12)≠1 still valid because tower ≥ log₂ m).

5 tests: `tetration_mod_p_exact`, `totient_ladder_coprime_guarded`, `lazy_modulus_demand_driven`,
`ackermann_small_oracle_diff`, `lucas_binomial_mod_p_correct`. Workspace green, clippy clean, verifier 49/0.

---

## §C.34 — tropical (min,+) DP fold family — **BUILT** (new domain: combinatorial optimization)

`jeff_math::tropical`. Min-plus semiring (`⊕=min, ⊗=+`). Structured win mirrors Stage 26: a layered
DAG whose layer matrix `M` **repeats** N times → length-N shortest paths `= M^{⊗N}` via min-plus
**fast exponentiation** O(w³ log N) vs naive O(N·w²); ratio `~ N/(w log N)` diverges for fixed w.

- `min_plus_matmul`/`min_plus_matpow`/`repeated_layer_naive`/`layered_dag_naive`/`viterbi_path`.
- Certificate: **integer-exact** (matpow == naive). No "O(1) collapse" claimed (tropical varieties
  can be exponential) — gain only for repeated/low-rank structure; **distinct layers ⇒ zero gain**
  (`no_structure_zero_gain`, honest). Viterbi path decode exact.
- Extends coverage into the **combinatorial-optimization** domain (a different axis from
  numeric/signal). 5 tests: `minplus_semiring_correct`, `layered_dag_collapse_measured`,
  `viterbi_path_exact`, `no_structure_zero_gain`, `tropical_cert`. Workspace green, clippy clean.

---

## §C.35 — exponential_fit unification (Prony = ODA) — **BUILT**

`jeff_math::expfit`. Prony (signal recovery) and ODA (series acceleration) are the **same Hankel
nullspace problem**: `s_n = Σ c_j ξ_j^n` annihilated by a constant-coefficient recurrence (roots =
`ξ_j`). Shared core `hankel_recurrence` (exact over ℚ, reuses `recurrence::fit_recurrence`).

- `aitken_is_prony_k1_verified`: Aitken Δ² λ == Prony(k=1) λ == 0.5 (single mode, exact); Δ²
  recovers limit L=10 exactly. The **k=1 unification**, verified.
- `oda_signal_unified`: the same core serves a convergence sequence (ODA, `2+5(1/3)ⁿ`) and a signal
  (`2ⁿ+3ⁿ` → order-2). `hankel_nullspace_core`: Fibonacci → order-2.
- **Shared weakness in the cert**: Hankel condition number. Well-separated modes → moderate cond
  (`condition_number_in_cert`); near-coincident modes (λ=1.001 vs 1.0) → near-singular Hankel →
  flagged (`near_mode_instability_flagged`).
- Certificate: **exact** (ℚ Hankel) / **interval** (condition-number bound). Coverage in two
  directions (signal recovery + series acceleration). 5 tests. Workspace green, clippy clean.

---

## §C.36 — structure-discovery meta-selector — **BUILT** (the 45→90 engine)

`jeff_math::selector`. Extends the BBP gate to *all* fold families: cheaply probe → classify
(`CFinite`/`LowRank`/`Sparse`/`Structureless`) → dispatch the right fold, or absence-certify when no
structure. Not a new kernel — *knowing which kernel to use and honestly giving up when none applies*.

- Probes: `hutchinson_trace` (Rademacher, exact on diagonals), `numerical_rank` (singular values,
  tol 1e-5 to skip the iterative SVD's ~1e-6 noise — stated), `count_nnz` (sparsity),
  `hankel_recurrence` (C-finite). `classify_matrix`/`classify_sequence`/`dispatch_matrix`.
- C-finite seq → CFinite{order:2} (Fibonacci); sparse → Sparse; rank-1 → LowRank; full-rank random
  → **AbsenceCertificate** (defer). Certificate: **probabilistic** (probing/union-bound) for the
  negative; **exact** for detected structure (the chosen fold carries its own cert).
- Cost `O(n·probes)` ≪ full fit `O(n²·r)` (`meta_selector_cheaper_than_full`). 5 tests:
  `structure_taxonomy_classified`, `probing_dispatches_correct_fold`, `no_structure_certified_absent`,
  `hutchinson_trace_accurate`, `meta_selector_cheaper_than_full`. Workspace green, clippy clean.

---

## §C.37 — Galois radical-absence + SCF cohomology obstruction — **BUILT** (niche / domain-edge)

Twin **impossibility** proofs (prove "no closed form exists", not "couldn't find it"). Certificate:
**absence (exact unsat)** — every check is a finite integer / finite-enumeration / exact-rational
decision (no Z3 needed; "unsat" is exactly decidable). Labeled niche (symbolic algebra / algebraic
topology) per rule 7 — built but not over-valued (outside JEFF's numeric/signal core domain).

**37.1 Galois / Liouville** (`jeff_math::galois`):
- `x⁵−x−1` radical-absence: Δ = 256(−1)⁵+3125(−1)⁴ = **2869** (not a perfect square ⇒ Gal ⊄ A₅) +
  **A₅ simple** by the conjugacy-class-sum test (no `1 + subset of {15,20,12,12}` strictly between 1
  and 60 divides 60) ⇒ Gal = S₅ unsolvable ⇒ **no radical solution**.
- `erf` (∫e^{−x²}) elementary-absence via Liouville: `R'−2xR=1` has no rational R — the odd
  coefficient ladder `a_{k+2}=2a_k/(k+2)` (computed exactly over ℚ) never terminates ⇒ no
  polynomial; pole argument rules out the rest. Tests: `quintic_radical_absence_via_unsat`,
  `a5_simple_conjugacy_unsat`, `erf_elementary_absence_via_unsat`, `liouville_no_rational_r` (+2) (6).

**37.2 cohomology obstruction** (`jeff_math::cohomology`):
- 3 sensors on S¹, each +120° relative → holonomy 360° ≠ 0 ⇒ **ungluable** (no global section); the
  unsat is exact (`Σ g_ij = 0` is necessary by telescoping, `Σ = 360 ≠ 0` is a direct contradiction).
  `H¹(S¹) ≅ ℝ` (dim 1) via Euler characteristic `b₁=|E|−|V|+comp` and via exact incidence rank
  (3-cycle δ⁰ rank 2 ⇒ H¹ = 3−2 = 1). Generalizes to n-sensor nerve graphs. Tests:
  `sensor_gluing_obstruction_h1_nonzero`, `holonomy_360_obstruction`, `gluing_unsat_proven`,
  `nerve_b1_generalizes` (4). Workspace green, clippy clean, verifier 49/0.
