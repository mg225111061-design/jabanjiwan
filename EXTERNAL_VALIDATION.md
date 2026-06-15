# EXTERNAL VALIDATION (GitHub Codespace, network available) — Stage 18→25 ground truth

Raw measurements against world references (NIST hashlib, FFTW, OpenBLAS, NumPy, a FIPS-203
reference). Labels are exact: "vs FFTW/OpenBLAS/NumPy" is **vs the world**, not self-relative.
Where reality differs from the in-container report, it is stated plainly.

## §0 Environment (this Codespace)
- `Intel(R) Xeon(R) @ 2.80GHz`, 4 cores (1 thread/core), 15 GiB RAM.
- Vector ISA: **AVX-512F/DQ/BW/CD/VL + AVX2 + FMA**. **NO AMX** (no `amx_tile`/`amx_int8`).
  → Stage 18.3 AMX numbers are **in-container-only**; not reproducible here (expected, honest).
- Repo cloned, clean, all Stage 17→25 commits present.

## §1 Build portability (target-cpu=native, this CPU)
- `cargo build --workspace --release` → **Finished, 0 errors.** The AMX inline-asm **assembles
  even on this non-AMX CPU** (LLVM accepts the mnemonics); the AMX path is runtime-gated by
  CPUID and falls back to scalar. So the native build is portable.

## §2 Tests reproduced externally
- `cargo test --workspace` → **418 passed / 0 failed** (matches the in-container report).
- AMX test self-skips: `[amx] not available on this CPU — skipping (scalar path is used)` → ok.
- **35** reject/false/tamper integrity tests all pass + type-level-unconstructible (compile_fail)
  + sorry/unknown→fallback. The verifier rejects forged certificates (the core guarantee).

## §3 NIST ground truth
### §3a SHA-3 / SHAKE vs Python `hashlib` — PASS (independent reference)
JEFF and hashlib are **bit-identical** on all inputs (`""`,`"abc"`, and fresh strings), e.g.
`sha3_256("") = a7ffc6f8…8434a`, `shake256("")[:64] = 46b9dd2b…c4be`. Keccak layer is
externally ground-truth-verified (not just hardcoded constants).

### §3b ML-KEM-768 / ML-DSA-44 vs FIPS-203/204 — **FAIL (does NOT match official)**
Tested JEFF vs `kyber-py` 1.2.0 (FIPS-203 reference, `_keygen_internal`/`_encaps_internal`),
identical seeds `d=0x11.., z=0x22.., m=0x33..`:
```
JEFF:  ek_len 1184  ct_len 1088  K_jeff = 318e6793ef4a1960…a3cc2430   (self-roundtrip TRUE)
REF :  ek_len 1184  ct_len 1088  K_ref  = dea5fdd2340a17c7…b10bc8fa
```
Sizes match, but **K_jeff ≠ K_ref and ek bytes differ**. JEFF's PQC is roundtrip-correct and
self-consistent but is **NOT FIPS-byte-conformant**: it uses a struct/`u32` key encoding, not
FIPS `ByteEncode`, and `K = G(m‖H(ek))` so the encoding difference alone changes the shared
secret. This **confirms** (does not contradict) the prior report's stated caveat ("not
validated against official KATs; struct keys, not FIPS byte-packing"). ML-DSA shares the same
root cause. **Elevating to KAT-conformance = a real rework** (FIPS encode/decode + verified-
exact sampling) — possible now that a reference is in hand, but NOT done. No fake pass.

## §4 Incumbent comparison (vs the world) — accuracy identical, the thesis holds
Per-call best-of, n=65536 FFT / n=512 GEMM, native:

| kernel | JEFF | incumbent | verdict |
|---|---|---|---|
| dense FFT | 2135 µs (radix-2) | **FFTW 149 µs** (FFTW_MEASURE) | **FFTW wins ~14.3×** |
| **sparse FFT (k-sparse)** | **HIKP 1.58 µs** | FFTW 149 µs (full) | **JEFF wins ~94×** |
| dense f64 GEMM | 32.8 ms (blocked) | **OpenBLAS 1.53 ms** | **OpenBLAS wins ~21.4×** |

Accuracy: all three FFTs agree exactly (`mag_bin5 = 32768.000000`); JEFF vs OpenBLAS GEMM
identical (`C00=51`). **Honest thesis, validated:** JEFF loses to tuned BLAS/FFT on DENSE
(14–21×) and wins only where STRUCTURE is present (sparse FFT, 94× vs FFTW) — exactly the
conservation-law story, now measured against world-class incumbents.

## §5 NumPy (Stage 13 layers 3/4)
- JEFF's embedded CPython **drives NumPy**: `np.arange(100).sum()=4950`,
  `np.linalg.norm([3,4])=5.0` (trusted) — elevated from "numpy absent".
- **Zero-copy buffer protocol (layer 3, read-side): VERIFIED.** Rust reads a NumPy float64
  array via `PyObject_GetBuffer` with **no copy**:
  `rust_sum=1248750 == np_sum=1248750` and **`addr == np_addr = 0x7fd6a407e0c0`** (same memory),
  `nbytes=8000`. Full bidirectional DLPack/Arrow remains a follow-up.

## §6 Real `.jeff` programs run
- `triangular.jeff --collapse-report` → all fold to **O(1)** (`cert=ok(exact-coeff-zero)`);
  `triangular(1000000)=500000500000` (= n(n+1)/2).
- `holonomic.jeff` → `Σ C(n,k)` collapses (exact-telescoper); `sum_binom(20)=1048576=2²⁰`.
- Stage-23 `match`: `step(0)=10, step(1)=11, step(7)=20`.

## Verdict
Reproduced externally: 418/0, verifier-rejects-forgeries, SHA-3/SHAKE vs hashlib (bit-exact),
the structural-win thesis vs FFTW (94×) with honest dense losses (FFTW 14×, OpenBLAS 21×), and
NumPy zero-copy (same-address, bit-exact). The one report-vs-reality gap, stated plainly:
**JEFF's full ML-KEM/ML-DSA does NOT pass official FIPS KATs** (not byte-conformant) — the
hash layer does, the schemes do not (yet). In-container-only: AMX (absent here).
