# Stage 26 → 28 results — §C (conservation-law accounting)

Two regimes, kept strictly separate (the whole thesis lives in this distinction):

- **Structured (infinite / huge ratio, asymptotic).** Where genuine structure exists
  (C-finite / holonomic recurrences in §26; sparse spectra in §28b) the self-relative ratio
  *diverges* with the size argument. This does **not** violate the Ω(N) floor: the output is a
  *single value* (or a small set of coefficients / a Θ(M) table built optimally), not Θ(N) data
  reconstructed from Θ(N) input.
- **Dense (parity is the ceiling, not a win).** Where the work is genuinely Ω(N) per output
  element (dense GEMM §27, dense FFT §28a), the best attainable is *parity* with a tuned
  incumbent. We report "reached X% of OpenBLAS/FFTW", never "beat". Losing on dense is honest
  and expected — JEFF wins by *structure*, not by out-tuning BLAS.

All numbers below are raw measurements on this machine (`Intel Xeon @ 2.80GHz`, AVX-512, no
AMX), `--release`, best-of-N per call. Arithmetic is exact: modular `F_q` (q = 1 000 000 007)
or exact ℚ where stated. "Ratio" is **self-relative** (JEFF naive vs JEFF collapse) unless a
named incumbent is given.

---

## §C.26 — Holonomic / C-finite infinite-ratio engine

The one regime where JEFF shows *divergent* self-relative ratios. Incumbent evaluation of a
linear-recurrence sequence / holonomic sum is Θ(N); structure collapses a single N-th term to
O(log N) (or O(1) for periodic), so the ratio grows without bound as N → ∞.

### §C.26.1 — C-finite N-th term (arbitrary order d), exact over F_q
Three independent paths, all **bit-exact** against each other and the naive oracle
(`cfinite_bostan_matches_naive`, `cfinite_companion_matches_bostan`, `fibonacci_hand_checked`):

| path | complexity (field ops) | role |
|---|---|---|
| naive iterate | **O(N·d)** | oracle / correctness baseline |
| Bostan–Mori (`[x^N] P/Q`) | **O(M(d)·log N)** | collapse |
| Fiduccia / companion (`x^N mod charpoly`) | **O(d²·log N)** | collapse (sparse companion power) |

Measured (d = 3, recurrence `a_n = 2a_{n-1}+a_{n-3}`), naive/collapse time ratio **diverges**:

| N | naive µs | bostan µs | comp µs | naive/bostan | naive/comp |
|---|---|---|---|---|---|
| 1e3 | 28.9 | 2.95 | 1.85 | 9.8 | 15.7 |
| 1e4 | 291 | 3.98 | 2.35 | 73.2 | 123.8 |
| 1e5 | 3032 | 4.75 | 2.85 | 638.7 | 1063.9 |
| 1e6 | 30801 | 5.50 | 3.16 | 5596.1 | 9738.0 |
| 1e7 | 309002 | 6.51 | 3.76 | **47487.6** | **82093.9** |

Ratio grows ≈ N/log N (not a constant factor) — `cfinite_ratio_grows_with_N` asserts the
op-count proxy increases monotonically. **Crossover** at N = 64 (`cfinite_choose`): below it the
naive path is selected (the log-N transforms' constant overhead loses for tiny N).

### §C.26.2 — Closed-form / genuine O(1)
- **Periodic** (all characteristic roots are roots of unity ⇒ pure period p): `a_N = a_{N mod p}`
  in genuine **O(1)** after an O(p) precompute. For the period-6 example `a_n=a_{n-1}-a_{n-2}`,
  verified bit-exact vs Bostan–Mori (`closedform_matches_bostan_periodic`). Measured ratio of
  naive-O(N) (extrapolated past 1e7 from a measured 24.7 ns/term) to O(1) lookup:

  | N | naive (O(N)) | O(1) lookup | ratio |
  |---|---|---|---|
  | 1e6 | 24.5 ms | 0.020 µs | 1.23e6 |
  | 1e9 | ≈24.7 s (extrap.) | 0.020 µs | ≈1.24e9 |
  | 1e12 | ≈24736 s (extrap.) | 0.021 µs | ≈1.18e12 |
  | 1e15 | ≈2.47e7 s (extrap.) | 0.021 µs | **≈1.18e15** |

  This is the genuine "huge ratio" case: O(1) output, divergent ratio, only because the
  *structure* (periodicity) exists. Non-periodic, non-geometric inputs (e.g. Fibonacci) have no
  small period → `closedform_absent_falls_back` (honest fallback to §26.1, no forced closed form).
- **Geometric** (order 1): `a_N = a_0·c^N` in O(log N) (`closedform_o1_verified`).

> Honesty on the O(·): counts are **field operations**. With a *bignum* modulus, the N-th term
> can be Θ(N) bits (e.g. 2^N), so the bit-complexity carries the usual bignum growth — the
> collapse is in field-op count, stated as such, never as bit-cost O(1).

### §C.26.3 — Holonomic definite sums + creative telescoping (proof-carrying)
Zeilberger's creative telescoping (existing `jeff_collapse_arith::zeilberger`, search re-checked
by the rational-identity certificate `jeff_math::hyper::telescoper_holds`) yields a verified
recurrence for `S(n) = Σ_k F(n,k)`. New evaluation layer `jeff_math::holsum`.

- **Certificate verified by the real gate** and the certified recurrence reproduces the direct
  sum bit-exact; a tampered operator is rejected (`telescoper_certificate_verified_and_recurrence_matches_naive`).
- **Honest defer**: no telescoper within budget ⇒ `None`, never a fabricated recurrence
  (`nonholonomic_input_defers`).
- Measured (mod q):
  - **(A) full table** `S(0..M)=Σ_k C(n,k)²` (P-finite, polynomial-coefficient telescoper):
    naive O(M²) vs recurrence O(M) — ratio ≈ M (257.7 → 497.7 → 988.9 → **2004.8** for
    M = 500…4000). Output is the Θ(M) table, computed optimally.
  - **(B) single value** `S(n)=Σ_k C(n,k)=2^n` (C-finite telescoper ⇒ folded into §26.1):
    naive Σ_k O(n) vs C-finite O(log n) — ratio 40.4 → 344.1 → 2627.8 → 21868 → **214847**
    for n = 1e3…1e7. Output is one value.

  Where the telescoper is genuinely polynomial-coefficient, a *single* value is O(n) (parity
  with naive single-eval) — the win there is the table (n×) and the proof; we do **not** claim a
  sublinear single-value ratio for that case (`polynomial_coeff_telescoper_is_not_cfinite`).

**Verifier re-proof (entry & exit): green** — `jeff-verify` integrity suite 49/0
(`false_telescoper_rejected`, `false_recurrence_absence_rejected`, `false_*`, `sorry/unknown →
fallback`, tamper), `wrong_telescoper_is_rejected`, plus the new end-to-end verify/defer tests.
Workspace 439/0, clippy `--all-targets -D warnings` clean.

---

## §C.27 — GEMM parity (vs OpenBLAS)
_(pending — Stage 27)_

## §C.28 — FFT parity + sparse extension (vs FFTW)
_(pending — Stage 28)_
