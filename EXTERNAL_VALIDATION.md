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

### §3b ML-KEM-768 / ML-DSA-44 vs official NIST ACVP — **PASS (byte-for-byte)** ✅
> Update (gap closed). The first external run found JEFF's PQC was roundtrip-correct but
> **not FIPS-byte-conformant** (struct/`u32` encodings, not FIPS `ByteEncode`/`BitPack`). That
> single gap is now fixed and verified directly against the **official NIST ACVP** vectors
> (usnistgov/ACVP-Server) — not a self-comparison and not a reference-library proxy.

**ML-KEM-768 (FIPS 203)** — official ACVP, byte-for-byte:
```
keyGen  25/25   (z,d → ek,dk)
encaps  25/25   (ek,m → c,K)
decaps  10/10   (dk,c → K', incl. implicit-reject vectors)
```
Root fix: matrix Â byte order (XOF input `ρ ‖ j ‖ i`, FIPS Alg. 13). ByteEncode/Compress were
already FIPS-correct. tcId 26 ek now matches official head `28c793778741b80b…` exactly.

**ML-DSA-44 (FIPS 204)** — official ACVP, byte-for-byte:
```
keyGen  25/25   (seed → pk,sk)
sigGen  90/90   (external/pure + internal + external-μ; deterministic AND hedged)
sigVer  45/45   (accepts valid; rejects every tampered/forged sig, incl. negative tests)
```
Root fixes: **ExpandA XOF was SHAKE256 → must be SHAKE128** (a sampling bug, not just
serialization); `ρ'' = H(K‖rnd‖μ)`; `w1Encode` at 6 bits/coeff (was 8, which corrupted `c̃`);
and the full FIPS `pkEncode`/`skEncode`/`sigEncode` + `HintBitPack` codecs. The NTT/rounding
primitives were already identity-checked; the SHAKE layer is NIST-KAT validated (§3a).

Verified against the reference too: `dilithium-py` 1.4.0 / `kyber-py` 1.2.0 reproduce the
official vectors, and JEFF matches both. Representative official vectors are now **embedded
in-tree** (`jeff-math` `mod acvp_kat`, 6 tests) for permanent network-free regression. PQC is
thereby elevated from "defining-property verified" to **official FIPS KAT verified**
(trusted → verified). The one `preHash`/HashML-DSA message-prefix variant is not wired (same
crypto core, different `M'` framing) — stated, not implied.

## §4 Incumbent comparison (vs the world) — accuracy identical, the thesis holds
Per-call best-of, n=65536 FFT / n=512 GEMM, native (two independent runs, consistent):

| kernel | JEFF | incumbent | verdict |
|---|---|---|---|
| dense FFT | 2135–2150 µs (radix-2) | **FFTW 149 µs** (FFTW_MEASURE) | **FFTW wins ~14.3–14.4×** |
| **sparse FFT (k-sparse)** | **HIKP 1.31–1.58 µs** | FFTW 149 µs (full) | **JEFF wins ~94–114×** |
| dense f64 GEMM | 32.6–32.8 ms (blocked) | **OpenBLAS 1.53–1.63 ms** | **OpenBLAS wins ~20–21×** |


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
Reproduced externally: 424/0, verifier-rejects-forgeries, SHA-3/SHAKE vs hashlib (bit-exact),
the structural-win thesis vs FFTW (94×) with honest dense losses (FFTW 14×, OpenBLAS 21×), and
NumPy zero-copy (same-address, bit-exact). The one report-vs-reality gap found in the first run
— **ML-KEM/ML-DSA not byte-conformant to official FIPS KATs** — is now **closed**: ML-KEM-768
(keyGen 25/25, encaps 25/25, decaps 10/10) and ML-DSA-44 (keyGen 25/25, sigGen 90/90, sigVer
45/45) pass the **official NIST ACVP** vectors byte-for-byte, with anchors embedded in-tree.
PQC is elevated trusted → **official FIPS KAT verified**. In-container-only: AMX (absent here).

Honest non-fix (by design): dense FFT/GEMM still lose to FFTW (~14×) / OpenBLAS (~21×). That is
not a defect to paper over — JEFF wins where *structure* exists (sparse FFT 94× vs FFTW), which
is the whole conservation-law thesis. Left exactly as measured.
