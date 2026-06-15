//! Stage 27 — BLIS/GotoBLAS-style dense f64 GEMM: a hand AVX-512 microkernel + packing + the
//! five-loop nest, blocked from this CPU's measured cache geometry. The goal is **parity** with
//! a tuned incumbent (OpenBLAS), never "beating" it — dense GEMM is Ω(N³) work with no exploitable
//! structure, so parity is the ceiling (see §C.27). We report "reached X% of OpenBLAS".
//!
//! Correctness (P0): for **integer-valued** inputs the packed kernel is **bit-exact** vs the
//! scalar oracle [`crate::simd::gemm_scalar`] (FMA and mul+add coincide when every product/sum
//! is exactly representable). For general f64 it is **deterministic / bit-reproducible** and
//! differs from the naive triple loop only by the FMA's single rounding and the blocked
//! summation grouping — the standard, expected behaviour of any FMA-based BLAS (R8/DR3).
//!
//! Lesson from Stage 17.2 (register tiling lost to LLVM): the miss was *no packing* and *no hand
//! microkernel*. This module supplies both.

use crate::cpuprobe::CpuCaps;

/// Microkernel rows and columns. `NR = 16` = two zmm (16 f64) per row, so each step does
/// 2 B-loads + MR A-broadcasts for `MR·2` FMAs — FMA-bound, not load-bound (the 8×8 shape's
/// 8 broadcasts + 1 load per 8 FMAs was load-port limited). `MR·NR/8 = 16` accumulators hide
/// the FMA latency (≥ L_vfma·N_vfma independent chains).
const MR: usize = 8;
const NR: usize = 16;

/// Blocking parameters derived from cache geometry (BLIS analytical model, scaled to fit).
#[derive(Clone, Copy, Debug)]
pub struct Blocking {
    pub kc: usize,
    pub mc: usize,
    pub nc: usize,
}

impl Blocking {
    /// Derive `(KC, MC, NC)` from L1/L2/L3 so an `MR×KC` + `KC×NR` micropanel pair fits L1, an
    /// `MC×KC` A-panel fits L2, and a `KC×NC` B-panel fits L3. Falls back to BLIS-Sandy-Bridge-ish
    /// defaults (KC=256, MC=96, NC=4096) when a level is unknown.
    pub fn derive(caps: &CpuCaps) -> Blocking {
        let s = std::mem::size_of::<f64>();
        // KC: the two micropanels (MR·KC and KC·NR) live in L1. Larger KC ⇒ fewer streaming
        // passes over C (C is loaded/stored once per KC block), so use most of L1d.
        let kc = caps
            .l1d_bytes
            .map(|l1| (l1 / ((MR + NR) * s)).clamp(64, 512))
            .unwrap_or(256);
        // MC: A-panel MC·KC in ~half of L2; round to a multiple of MR.
        let mc = caps
            .l2_bytes
            .map(|l2| (((l2 / 2) / (kc * s)) / MR * MR).clamp(MR, 1024))
            .unwrap_or(96 / MR * MR + MR);
        // NC: B-panel KC·NC in ~half of L3; round to a multiple of NR.
        let nc = caps
            .l3_bytes
            .map(|l3| (((l3 / 2) / (kc * s)) / NR * NR).clamp(NR, 8192))
            .unwrap_or(4096);
        Blocking { kc, mc, nc }
    }
}

// ---- packing ----

/// Pack an `MC×KC` block of A (row-major `m×k`, top-left at `(ic,pc)`) into MR-row micropanels,
/// column-major within each panel and **zero-padded** to full `MR` rows. Layout:
/// `apack[panel*MR*kc + p*MR + ir]`.
#[allow(clippy::too_many_arguments)] // packing needs the full (matrix, k, block-origin, block-size, bound) tuple
fn pack_a(a: &[f64], k: usize, ic: usize, pc: usize, mc: usize, kc: usize, m: usize, out: &mut Vec<f64>) {
    out.clear();
    let mut ir0 = 0;
    while ir0 < mc {
        for p in 0..kc {
            for ir in 0..MR {
                let gi = ic + ir0 + ir;
                let gp = pc + p;
                out.push(if gi < m { a[gi * k + gp] } else { 0.0 });
            }
        }
        ir0 += MR;
    }
}

/// Pack a `KC×NC` block of B (row-major `k×n`, top-left at `(pc,jc)`) into NR-column micropanels,
/// row-major within each panel and **zero-padded** to full `NR` cols. Layout:
/// `bpack[panel*kc*NR + p*NR + jr]`.
fn pack_b(b: &[f64], n: usize, pc: usize, jc: usize, kc: usize, nc: usize, out: &mut Vec<f64>) {
    out.clear();
    let mut jr0 = 0;
    while jr0 < nc {
        for p in 0..kc {
            for jr in 0..NR {
                let gj = jc + jr0 + jr;
                let gp = pc + p;
                out.push(if gj < n { b[gp * n + gj] } else { 0.0 });
            }
        }
        jr0 += NR;
    }
}

// ---- microkernel ----

/// AVX-512 microkernel: `C[0..MR][0..NR] += Apanel(MR×kc) · Bpanel(kc×NR)`, accumulating into the
/// strided C tile in increasing-`p` order (so the per-element sum is a single running sum across
/// KC blocks — bit-exact for integer inputs). `c` points at `C[r0][c0]`, row stride `ldc`.
///
/// # Safety
/// Requires AVX-512F (checked by the caller via runtime CPUID). `apack`/`bpack` hold ≥ `kc*MR`
/// / `kc*NR` f64; the `MR×NR` tile at `c` (stride `ldc`) is in bounds.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx512f")]
unsafe fn micro_kernel(kc: usize, apack: *const f64, bpack: *const f64, c: *mut f64, ldc: usize) {
    use std::arch::x86_64::*;
    // 16 accumulators: acc[i] = C[i][0..8], acc[8+i] = C[i][8..16].
    let mut lo = [
        _mm512_loadu_pd(c),
        _mm512_loadu_pd(c.add(ldc)),
        _mm512_loadu_pd(c.add(2 * ldc)),
        _mm512_loadu_pd(c.add(3 * ldc)),
        _mm512_loadu_pd(c.add(4 * ldc)),
        _mm512_loadu_pd(c.add(5 * ldc)),
        _mm512_loadu_pd(c.add(6 * ldc)),
        _mm512_loadu_pd(c.add(7 * ldc)),
    ];
    let mut hi = [
        _mm512_loadu_pd(c.add(8)),
        _mm512_loadu_pd(c.add(ldc + 8)),
        _mm512_loadu_pd(c.add(2 * ldc + 8)),
        _mm512_loadu_pd(c.add(3 * ldc + 8)),
        _mm512_loadu_pd(c.add(4 * ldc + 8)),
        _mm512_loadu_pd(c.add(5 * ldc + 8)),
        _mm512_loadu_pd(c.add(6 * ldc + 8)),
        _mm512_loadu_pd(c.add(7 * ldc + 8)),
    ];
    for p in 0..kc {
        let b0 = _mm512_loadu_pd(bpack.add(p * NR));
        let b1 = _mm512_loadu_pd(bpack.add(p * NR + 8));
        let ap = apack.add(p * MR);
        _mm_prefetch::<_MM_HINT_T0>(bpack.add((p + 1) * NR) as *const i8);
        for i in 0..MR {
            let a = _mm512_set1_pd(*ap.add(i));
            lo[i] = _mm512_fmadd_pd(a, b0, lo[i]);
            hi[i] = _mm512_fmadd_pd(a, b1, hi[i]);
        }
    }
    for i in 0..MR {
        _mm512_storeu_pd(c.add(i * ldc), lo[i]);
        _mm512_storeu_pd(c.add(i * ldc + 8), hi[i]);
    }
}

/// Whether the hand AVX-512 path is available on this CPU.
pub fn avx512_available() -> bool {
    #[cfg(target_arch = "x86_64")]
    {
        std::arch::is_x86_feature_detected!("avx512f")
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        false
    }
}

/// BLIS five-loop GEMM with the hand AVX-512 microkernel and packing. Returns `None` if AVX-512
/// is unavailable (caller should fall back to [`crate::simd::gemm_blocked`]).
pub fn gemm_avx512(a: &[f64], b: &[f64], m: usize, k: usize, n: usize) -> Option<Vec<f64>> {
    if !avx512_available() {
        return None;
    }
    let blk = Blocking::derive(&crate::cpuprobe::probe());
    Some(gemm_avx512_with(a, b, m, k, n, blk))
}

/// As [`gemm_avx512`] but with explicit blocking (for autotuning / tests). Assumes AVX-512.
pub fn gemm_avx512_with(a: &[f64], b: &[f64], m: usize, k: usize, n: usize, blk: Blocking) -> Vec<f64> {
    let mut c = vec![0.0f64; m * n];
    let (kc, mc, nc) = (blk.kc.max(1), blk.mc.max(MR), blk.nc.max(NR));
    let mut apack: Vec<f64> = Vec::new();
    let mut bpack: Vec<f64> = Vec::new();
    let mut ctile = [0.0f64; MR * NR];

    let mut jc = 0;
    while jc < n {
        let ncb = nc.min(n - jc);
        let mut pc = 0;
        while pc < k {
            let kcb = kc.min(k - pc);
            pack_b(b, n, pc, jc, kcb, ncb, &mut bpack);
            let npan_n = ncb.div_ceil(NR);
            let mut ic = 0;
            while ic < m {
                let mcb = mc.min(m - ic);
                pack_a(a, k, ic, pc, mcb, kcb, m, &mut apack);
                let npan_m = mcb.div_ceil(MR);
                for jr in 0..npan_n {
                    let bpan = &bpack[jr * kcb * NR..(jr + 1) * kcb * NR];
                    let c0 = jc + jr * NR;
                    // clamp to THIS NC block (ncb), not global n — otherwise a panel that
                    // overshoots a non-NR-aligned ncb would double-write the next jc block.
                    let nrb = NR.min(ncb - jr * NR);
                    for ir in 0..npan_m {
                        let apan = &apack[ir * kcb * MR..(ir + 1) * kcb * MR];
                        let r0 = ic + ir * MR;
                        let mrb = MR.min(mcb - ir * MR);
                        // SAFETY: AVX-512 confirmed; panels hold kcb*MR / kcb*NR; tile in bounds.
                        unsafe {
                            if mrb == MR && nrb == NR {
                                micro_kernel(kcb, apan.as_ptr(), bpan.as_ptr(),
                                    c.as_mut_ptr().add(r0 * n + c0), n);
                            } else {
                                // edge tile: microkernel into a padded MR×NR scratch, copy back.
                                for i in 0..MR {
                                    for j in 0..NR {
                                        ctile[i * NR + j] = if i < mrb && j < nrb {
                                            c[(r0 + i) * n + c0 + j]
                                        } else {
                                            0.0
                                        };
                                    }
                                }
                                micro_kernel(kcb, apan.as_ptr(), bpan.as_ptr(), ctile.as_mut_ptr(), NR);
                                for i in 0..mrb {
                                    for j in 0..nrb {
                                        c[(r0 + i) * n + c0 + j] = ctile[i * NR + j];
                                    }
                                }
                            }
                        }
                    }
                }
                ic += mc;
            }
            pc += kc;
        }
        jc += nc;
    }
    c
}

/// Dispatcher: hand AVX-512 packed GEMM when available, else the cache-blocked LLVM path.
pub fn gemm(a: &[f64], b: &[f64], m: usize, k: usize, n: usize) -> Vec<f64> {
    gemm_avx512(a, b, m, k, n).unwrap_or_else(|| crate::simd::gemm_blocked(a, b, m, k, n))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::simd::gemm_scalar;
    use jeff_math::fmat::Rng;

    fn int_mat(rng: &mut Rng, len: usize) -> Vec<f64> {
        (0..len).map(|_| ((rng.next_u64() % 19) as f64) - 9.0).collect()
    }

    #[test]
    fn microkernel_matches_oracle() {
        if !avx512_available() {
            eprintln!("[gemm] AVX-512 not available — skipping (dispatcher uses blocked path)");
            return;
        }
        // a single full MR×NR tile with arbitrary kc, integer-valued ⇒ bit-exact.
        let mut rng = Rng::new(0x27_01);
        for kc in [1usize, 3, 16, 200] {
            let a = int_mat(&mut rng, MR * kc);
            let b = int_mat(&mut rng, kc * NR);
            let want = gemm_scalar(&a, &b, MR, kc, NR);
            let got = gemm_avx512_with(&a, &b, MR, kc, NR, Blocking { kc: 64, mc: 64, nc: 64 });
            assert_eq!(got, want, "microkernel tile kc={kc}");
        }
    }

    #[test]
    fn microkernel_bit_reproducible() {
        if !avx512_available() {
            return;
        }
        let mut rng = Rng::new(0x27_02);
        let (m, k, n) = (40, 37, 41);
        let a: Vec<f64> = (0..m * k).map(|_| rng.gaussian()).collect();
        let b: Vec<f64> = (0..k * n).map(|_| rng.gaussian()).collect();
        let r1 = gemm_avx512(&a, &b, m, k, n).unwrap();
        let r2 = gemm_avx512(&a, &b, m, k, n).unwrap();
        assert_eq!(r1, r2, "general-f64 GEMM must be bit-reproducible run to run");
    }

    #[test]
    fn packed_gemm_matches_oracle() {
        if !avx512_available() {
            return;
        }
        let mut rng = Rng::new(0x27_03);
        for &(m, k, n) in &[(8, 8, 8), (64, 64, 64), (128, 96, 80)] {
            let a = int_mat(&mut rng, m * k);
            let b = int_mat(&mut rng, k * n);
            assert_eq!(gemm_avx512(&a, &b, m, k, n).unwrap(), gemm_scalar(&a, &b, m, k, n), "{m}x{k}x{n}");
        }
    }

    #[test]
    fn packed_gemm_handles_tile_straddling_sizes() {
        if !avx512_available() {
            return;
        }
        // sizes that are NOT multiples of MR=8 / NR=8 and cross KC/MC/NC blocks.
        let mut rng = Rng::new(0x27_04);
        let small = Blocking { kc: 7, mc: 24, nc: 24 };
        for &(m, k, n) in &[(1, 1, 1), (7, 9, 5), (13, 17, 11), (33, 31, 35), (50, 50, 50)] {
            let a = int_mat(&mut rng, m * k);
            let b = int_mat(&mut rng, k * n);
            let got = gemm_avx512_with(&a, &b, m, k, n, small);
            assert_eq!(got, gemm_scalar(&a, &b, m, k, n), "straddling {m}x{k}x{n}");
        }
    }

    #[test]
    fn blocking_from_caps_is_sane() {
        let blk = Blocking::derive(&crate::cpuprobe::probe());
        assert!(blk.kc >= 1 && blk.mc >= MR && blk.nc >= NR);
        assert!(blk.mc.is_multiple_of(MR) && blk.nc.is_multiple_of(NR), "MC/NC multiples of MR/NR");
    }
}
