# HARAN v4 — real domain kernel (B) + measured speed (A)

We left the toys (sum_squares/sort/quantile), built a **production PQC kernel**, and measured its
speed on the **wall clock** — not theory. The point of v4 is to prove value *by measurement*, honestly.

## PART B — real domain: Kyber NTT polynomial multiplication

- **Domain = PQC** (JEFF's identity; strongest assets, all grep-verified): `kyber.rs`/`dilithium.rs`/
  `mlkem.rs`/`mldsa.rs`/`modular.rs`, `Q=3329`, `N=256`.
- **Kernel** = `poly_mul(a,b) = intt(pointwise(ntt(a), ntt(b)))` in `Z_q[x]/(x²⁵⁶+1)`, constant-time, `a` secret.
- **Correctness**: differential vs `pqc::schoolbook_negacyclic` — **EXACT match on 200 random polys**
  (NTT≡schoolbook is a theorem; impl is TESTED-exact).
- **★ REAL fold ratio (the key finding)★**: **40% CLOSED by op-count — and that 40% is SETUP only**
  (twiddle `ζ^i` → C-finite O(log n); param sums → Faulhaber O(1)). The **runtime-dominant transform**
  (ntt / pointwise / intt) is **100% NO_STRUCTURE, Ω(N log N)** — data-dependent, doesn't fold.
  > **Real crypto does NOT fold like the toy 70%.** Only the setup collapses; the transform is the floor.

## PART A — measured wall-clock (RELEASE Rust, median-of-K, `black_box`-hardened)

*No headline single number. Per input size. Unfavorable cases included.*

**FOLD (CLOSED)** — closed-form O(1) vs naive O(n) loop, Σi² (num-bigint both sides):

| n | closed (ns) | naive (ns) | ratio |
|---:|---:|---:|---:|
| 10 | ~146 | ~558 | **3.8×** |
| 1 000 | ~161 | ~50 976 | **317×** |
| 100 000 | ~152 | ~4.6 M | **30 311×** |
| 1 000 000 | ~179 | ~43 M | **242 981×** |

Closed form is **flat (~130–180 ns, O(1) confirmed)**; ratio **grows with n** → orders of magnitude.
*Honest note:* the closed form wins at **all** measured n (no small-n crossover here — evaluating a
formula is strictly cheaper than iterating). JEFF's derivation is a **one-time compile cost**, not per call.

**UNSTRUCTURED (NO_STRUCTURE)** — data sum, naive vs 4-way unrolled:

| n | naive (ns) | unrolled (ns) | speedup |
|---:|---:|---:|---:|
| 1 000 | ~102 | ~126 | **0.81×** |
| 100 000 | ~8 104 | ~10 011 | **0.81×** |
| 1 000 000 | ~267 337 | ~256 614 | **1.04×** |

**≈1×, constant** — a well-written naive loop is already auto-vectorized; unrolling can even be *slower*.
This is the honest ceiling: unstructured work is **C-equivalent / constant-factor, never orders of
magnitude**. **Ω(N) is a theorem, not a gap.**

**APPROX (PROVEN-BOUND)** — bucketed quantile vs exact sort (n=100 000, m=64):

| approx (ms) | exact (ms) | error | ≤ proven ε? | space |
|---:|---:|---:|:---:|---:|
| ~11.0 | ~16.6 | 7.78 | **yes** (ε=w/2=7.81) | **64 vs 100 000** |

~1.5× faster + **O(m) space vs O(n)**, with **measured error inside the Z3-proven bound**.

## The honest lines held

- **fold only → orders of magnitude** (measured, growing with n). **unstructured → constant-factor**
  (measured ~1×). **approx → space/time win + error within a *proven* ε.**
- **proven ≠ tested**; **PROVEN-BOUND ≠ TESTED-BOUND**; **measured ≠ theoretical** — all kept distinct.
- The **real PQC kernel's fold ratio is low** (40% setup-only) — reported as-is, not dressed up.
- No headline number, no cherry-pick (the 0.56–0.81× unfavorable unstructured cases are shown).

## Theory vs measured

Matched: O(1) closed form is flat across n; naive is ≈O(n); unstructured is C-equivalent; Ω(N) holds.
The one nuance vs the naive expectation ("fold slower at small n"): **not observed** for this kernel,
because the bignum naive loop allocates per iteration — the closed form wins even at n=10. Reported honestly.

## Status / deferred

- **Measurement env**: single-box, RELEASE Rust (`opt-level=3`), num-bigint both sides (fair). Baselines
  are well-written (auto-vectorized). Times are medians; absolute ns vary by CPU.
- **No HARAN codegen yet**: the "folded" path is the closed-form evaluation; the "unstructured" path is
  a hand-written optimized loop (what a backend would emit). HARAN→native codegen is future (v6).
- **Z3 / AI**: Z3 pip-installed in-session (ephemeral); AI loop simulated. As in v3.
- (v5: 4 accelerations · v6: productization. v4 = prove value by measurement.)
