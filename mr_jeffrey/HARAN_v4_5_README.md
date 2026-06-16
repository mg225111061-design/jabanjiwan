# HARAN v4.5 — unstructured constant-factor acceleration (measured)

v4 measured unstructured work at 0.81×–1.04× (C-equivalent). v4.5 accelerates it with verified
aliasing + SIMD + cache/SoA + optimal algorithms + parallelism — **constant-factor only; Ω(N) is never
broken** (orders of magnitude are fold/approx, v2/v3). PQC removed; general kernels only.

## Measured table (RELEASE, median, black_box-hardened, 4 cores, AVX2/AVX-512)

| technique / kernel | measured | note |
|---|---:|---|
| SIMD poly8 (compute-bound) | **~8×** | L1; real arithmetic SIMD win |
| SIMD sum (latency→bandwidth) | ~5× | naive single-acc scalar → bandwidth |
| SoA layout (memory-bound) | ~2× | better bandwidth utilization at scale (≈1× when cache-resident) |
| radix vs std sort | 1.65× … **1.05×** | O(n) for fixed-width keys; ~equal at 4M (unfavorable, shown) |
| parallel compute-bound (4c) | **~4×** | near-linear in cores |
| parallel memory-bound (4c) | ~2.5–3× | **SUBLINEAR** — bandwidth saturates |
| verified noalias (memory-bound) | **~1×** | bandwidth-bound here; SAFE vs C's unchecked `restrict` |

**Highest ~8× (compute-bound), lowest ~1× (memory-bound) — both reported. No headline number.**

## Honest findings

- **Constant-factor only.** No order-of-magnitude on unstructured work — that's fold/approx (v2/v3). Ω(N) holds.
- **Techniques overlap, not multiply.** SoA×SIMD ≈ 6× combined (a bounded constant, not 8×8=64). Memory-bound
  parallel is **sublinear** (~2.5–3× on 4 cores, not 4×) — they share the memory bandwidth bottleneck.
- **Memory-bound ≈ 1–3× is normal and is success** (per the brief). Compute-bound gets more (SIMD 8×, parallel 4×).
- **Verified noalias is HARAN's unique edge**: own/& *prove* buffer disjointness → safe LLVM `noalias`, where C
  needs the unchecked `restrict` promise. Measured payoff here ≈1× (memory-bound), but the *safety* is the point.
- **No cherry-pick.** The unfavorable cases (noalias ~0.8–1×, radix 1.05× at 4M, parallel-memory 2.5×) are in the table.

## Deferred / honest limits

- **No HARAN native codegen yet** — these measure the constant-factor ceiling each technique gives in Rust (what a
  backend would emit). HARAN→native is future (v6).
- SIMD via AVX2 intrinsics; 4 cores; absolute times vary by CPU. radix removes the log factor (better order for
  fixed-width keys) but stays Ω(N).
