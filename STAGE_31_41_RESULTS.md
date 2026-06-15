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
