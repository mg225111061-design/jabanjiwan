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

## §C.27 — GEMM parity (vs OpenBLAS) — dense, parity is the ceiling

Dense f64 GEMM is Ω(N³) with no exploitable structure, so the honest target is **parity**, not a
win. Built the BLIS/GotoBLAS recipe the Stage-17.2 register-tiling attempt was missing: a **hand
AVX-512 microkernel** (intrinsics, `#[target_feature(avx512f)]`, FMA) + **packing** + the
**five-loop nest** blocked from this CPU's measured cache geometry (`jeff_backend::gemm`).

- **Microkernel** `MR×NR = 8×16` (16 zmm accumulators; 2 B-loads + 8 A-broadcasts per 16 FMAs ⇒
  FMA-bound, not load-port-bound — the 8×8 first cut was load-bound at ~20 GFLOP/s). B-micropanel
  prefetch. Blocking `KC=170, MC=384, NC=8192` derived from L1=32K / L2=1M / L3=33M.
- **Correctness (P0)**: bit-exact vs the scalar oracle for integer-valued inputs
  (`microkernel_matches_oracle`, `packed_gemm_matches_oracle`,
  `packed_gemm_handles_tile_straddling_sizes` — sizes not multiples of MR/NR crossing all blocks),
  and bit-reproducible run-to-run for general f64 (`microkernel_bit_reproducible`). Verify entries
  match OpenBLAS exactly (e.g. C0,Cmid,Clast = 35,41,−41 at n=2048). For general f64 the result
  differs from the naive triple loop only by FMA single-rounding / blocked grouping — standard
  FMA-BLAS behaviour, stated not hidden.

**Measured (this machine, `--release`, best-of-N), vs OpenBLAS 0.3.31 single-thread (numpy
backend, `OPENBLAS_NUM_THREADS=1`):**

| n | JEFF GFLOP/s | old blocked | OpenBLAS-1T | **% of OpenBLAS-1T** | JEFF/old |
|---|---|---|---|---|---|
| 256 | 23.9 | ~9.6 | 52.3 | **46%** | 2.5× |
| 512 | 33.8 | ~7.5 | 56.8 | **60%** | 4.4× |
| 1024 | 28.2 | ~7.9 | 58.0 | **49%** | 3.7× |
| 2048 | 30.4 | ~8.1 | 62.1 | **49%** | 3.8× |

**Reached ≈ 50% of OpenBLAS single-thread (best 60% at n=512)** — a **~4× gain** over the prior
blocked path (which was ~13%). We do **not** claim to beat OpenBLAS; this is "reached X%".

**Bottleneck (profiled analytically — no hardware perf counters in this sandbox, stated
honestly).** JEFF's microkernel sustains ~30 GFLOP/s ≈ **33% of the ~90 GFLOP/s vector-FMA peak**
(8 lanes × 2 FMA × 2 flop × 2.8 GHz); OpenBLAS reaches ~65%. The remaining ~2× is the classic
last-mile that needs per-µarch hand asm: (1) software pipelining of the FMA/broadcast/load streams
to fully hide FMA latency, (2) tuned prefetch distance, (3) lower C-streaming traffic (C is
loaded/stored once per KC block; OpenBLAS's larger effective KC and register choreography reduce
this). These are µarch-specific and beyond a portable Rust-intrinsics kernel.

**Multicore: not added.** The recipe gates multicore on reaching ≥70% single-thread; at ~50% it is
not met, and multicore would not change the *fraction* vs the (also-multicore) incumbent — so it is
honestly omitted rather than used to inflate a raw number.

**Conclusion (honest, regime-appropriate):** dense GEMM is the parity-ceiling regime; JEFF reached
~50% of single-thread OpenBLAS (4× over prior), bit-exact, with the remaining gap precisely
attributed to hand-asm pipelining. This is short of the ~80% aspiration — recorded as a measured
partial, not a win, per the dense-vs-structured split at the top of this doc.

Verifier re-proof (entry & exit): green (jeff-verify 49/0, Freivalds). Workspace 444/0, clippy
`--all-targets -D warnings` clean.

## §C.28 — FFT parity + sparse extension

Incumbent here is **numpy.fft = pocketfft** (tuned C SIMD, permissive). **Not FFTW**: FFTW is
GPL-2.0 (R5 forbids linking; out-of-process only) and `pyfftw` is absent — stated, not implied.
Correctness cross-check: JEFF's spectrum matches pocketfft bit-close (`|X[5]|` identical to 6 dp
at every n).

### §C.28.1 — dense FFT (parity ceiling; the Stockham attempt is an honest NON-WIN)
Implemented a radix-2 **Stockham autosort** FFT (`jeff_math::fft`) that removes the bit-reversal
permutation pass (verified: `stockham_matches_dft_within_tol`, `fft_no_bitreversal_pass`,
`stockham_bit_reproducible`). **Measured: it is slower than the existing in-place radix-2** by
~30–40% — the out-of-place ping-pong buffering moves more memory than the bit-reversal pass it
eliminates. Per "adopt only on a measured win; report non-wins honestly (17.2)", Stockham is kept
for its verified property but is **not** adopted as the fast path.

Dense f64 FFT timings (µs, best-of-20, this machine):

| n | JEFF radix-2 | JEFF Stockham | pocketfft | **radix-2 % of pocketfft** | Stockham/radix-2 |
|---|---|---|---|---|---|
| 4096 | 90.4 | 143.5 | 42.3 | **47%** | 0.63× (slower) |
| 16384 | 439.3 | 647.9 | 372.5 | **85%** | 0.68× |
| 65536 | 2131 | 2959 | 1613 | **76%** | 0.72× |
| 262144 | 10172 | 14581 | 7344 | **72%** | 0.70× |

The existing radix-2 already **reaches ~72–85% of pocketfft for n ≥ 16384** (near-parity, dense
ceiling) — no improvement was needed and the Stockham idea did not beat it. Reported as "reached
X%", never "beat".

### §C.28.2 — 2D sparse FFT (the STRUCTURED WIN — divergent ratio)
Extended the 1D HIKP decimation-aliasing + phase-ratio method to a 2D spectrum with `k` spikes and
to approximately-sparse (k spikes + noise) inputs (`jeff_math::sparsefft2d`). Reads `O(k)` of the
`N²` samples (three `B×B` subsamplings, `B=O(k)`), runs three `B×B` 2D FFTs ⇒ `O(k log k)`.
Verified: exact recovery + residual cert (`sparse_fft_2d_recovers_certified`), noisy recovery
within tolerance (`sparse_fft_noisy_within_tol`), crossover guard (`sparse_fft_below_crossover_uses_dense`),
op-count ratio grows with N (`sparse_fft_ratio_scales_n_over_k`).

**Fixed k=4, grow n — ratio vs dense `fft2d` DIVERGES (`≈ N²/(k log k)`):**

| n | sparse µs | dense µs | **ratio** |
|---|---|---|---|
| 64 | 44.0 | 243 | 5.5 |
| 128 | 44.0 | 1086 | 24.7 |
| 256 | 44.5 | 4867 | 109 |
| 512 | 43.9 | 25580 | 582 |
| 1024 | 44.1 | 114035 | **2589** |

Sparse stays ~44 µs (reads `O(k)`, independent of N); dense grows as N². Output is the `k` spikes
(small) ⇒ Ω(N²)-safe; the win is purely from k-sparsity (structure).

**Fixed n=512, grow k — the n/k crossover (where dense overtakes):**

| k | sparse µs | dense µs | ratio | verdict |
|---|---|---|---|---|
| 1 | 7.7 | 25933 | 3353 | sparse wins |
| 4 | 43.9 | 25933 | 591 | sparse wins |
| 16 | 888 | 25933 | 29.2 | sparse wins |
| 64 | 17950 | 25933 | 1.44 | sparse wins |
| 128 | DEFER | 25933 | — | → dense (guard) |

Crossover for n=512 is between k=64 and k=128: above it the bucket count `B ≥ n`, the method
declines (`None`), and the caller uses the dense FFT — a measured guard, not a guess.

Verifier re-proof (entry & exit): green. Workspace 451/0, clippy `--all-targets -D warnings` clean.

---

## §C — unified summary (26 → 28), and the conservation-law line

| stage | regime | result | honest label |
|---|---|---|---|
| **26.1** C-finite N-th term | structured | naive/collapse ratio **9.8→47488** (N=1e3→1e7), ~N/log N | infinite ratio, divergent |
| **26.2** periodic closed-form | structured | O(1) lookup, **ratio ≈1.18e15 at N=1e15** | genuine huge ratio (O(1)) |
| **26.3** holonomic sum / telescoper | structured | table O(M²)→O(M) (≈M); 2^n single value O(n)→O(log n) (**40→214847**) | divergent + proof-carrying |
| **27** dense GEMM | dense | **~50% of OpenBLAS-1T** (best 60%), 4× over prior | parity-half (not a win) |
| **28.1** dense FFT | dense | radix-2 **~72–85% of pocketfft**; Stockham slower (non-win) | near-parity; honest non-win |
| **28.2** 2D sparse FFT | structured | ratio vs dense **5.5→2589** (n=64→1024); n/k crossover measured | structured win, divergent |

**The line we never blur.** Dense work (27, 28.1) is Ω(N²)/Ω(N³)/Ω(N log N) with no exploitable
structure ⇒ the ceiling is **parity**, and we report only "reached X%" (50% of OpenBLAS-1T; 72–85%
of pocketfft) — never "beat". The infinite / huge ratios (26, 28.2) appear **only** where genuine
structure exists (constant-coefficient / holonomic recurrences; k-sparse spectra), are
**asymptotic** (grow with the size argument), and are **Ω(N)-safe**: every such output is a single
value, a low-order recurrence, or `k` spikes — never Θ(N) data conjured from Θ(N) input. Every
collapse on the structured side carries a machine-checked certificate or an exact oracle match;
every dense result is bit-exact (integer) / bit-reproducible (f64) vs its scalar oracle and vs the
incumbent's entries. Non-wins (Stage 28.1 Stockham) are reported as non-wins.
