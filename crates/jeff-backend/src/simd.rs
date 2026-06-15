//! Vectorizable hot-loop kernels (GEMM / FFT-residual / dot-product inner loops). These
//! are written in a chunked form the LLVM auto-vectorizer turns into SIMD, but the
//! *result* is identical to the scalar oracle — proven by `simd_preserves_result`
//! (CLAUDE.md PART C: speed, never the answer).

/// Scalar dot product (the oracle).
pub fn dot_scalar(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

/// Chunked dot product (8-wide partial sums → vectorizable). Same value as `dot_scalar`
/// up to floating-point summation order; for integer-valued inputs it is exact.
pub fn dot_vectorized(a: &[f64], b: &[f64]) -> f64 {
    let n = a.len().min(b.len());
    let lanes = 8;
    let mut acc = [0.0f64; 8];
    let chunks = n / lanes;
    for c in 0..chunks {
        let base = c * lanes;
        for l in 0..lanes {
            acc[l] += a[base + l] * b[base + l];
        }
    }
    let mut s = acc.iter().sum::<f64>();
    for i in (chunks * lanes)..n {
        s += a[i] * b[i];
    }
    s
}

/// Scalar GEMM `C = A·B` (row-major, the oracle). `a` is `m×k`, `b` is `k×n`.
pub fn gemm_scalar(a: &[f64], b: &[f64], m: usize, k: usize, n: usize) -> Vec<f64> {
    let mut c = vec![0.0; m * n];
    for i in 0..m {
        for p in 0..k {
            let aip = a[i * k + p];
            for j in 0..n {
                c[i * n + j] += aip * b[p * n + j];
            }
        }
    }
    c
}

/// Vectorized GEMM: same i-p-j order (the inner j-loop vectorizes over contiguous rows
/// of B and C). Integer-valued inputs give a bit-identical result to `gemm_scalar`.
pub fn gemm_vectorized(a: &[f64], b: &[f64], m: usize, k: usize, n: usize) -> Vec<f64> {
    let mut c = vec![0.0; m * n];
    for i in 0..m {
        let crow = &mut c[i * n..(i + 1) * n];
        for p in 0..k {
            let aip = a[i * k + p];
            let brow = &b[p * n..(p + 1) * n];
            for j in 0..n {
                crow[j] += aip * brow[j];
            }
        }
    }
    c
}

/// Cache-blocked GEMM (Stage 12.1 — the BLIS-style loop nest, scaled to this environment).
/// Tiles `i`, `p`, `j` for cache reuse, but for every output `C[i,j]` the sum over `p` is
/// still accumulated in increasing-`p` order — identical to [`gemm_scalar`] — so the result
/// is **bit-for-bit** the oracle's, floats included (no reassociation). Speed changes, the
/// answer never does (CLAUDE.md PART C). NOTE: a comparison against a tuned incumrent
/// (OpenBLAS/MKL) is not made here — they are not present in this environment; only
/// correctness is asserted and any timing is measured, never claimed.
pub fn gemm_blocked(a: &[f64], b: &[f64], m: usize, k: usize, n: usize) -> Vec<f64> {
    const BI: usize = 64;
    const BP: usize = 64;
    const BJ: usize = 64;
    let mut c = vec![0.0; m * n];
    let mut ii = 0;
    while ii < m {
        let i_end = (ii + BI).min(m);
        let mut pp = 0;
        while pp < k {
            let p_end = (pp + BP).min(k);
            let mut jj = 0;
            while jj < n {
                let j_end = (jj + BJ).min(n);
                for i in ii..i_end {
                    for p in pp..p_end {
                        let aip = a[i * k + p];
                        let brow = &b[p * n..p * n + n];
                        let crow = &mut c[i * n..i * n + n];
                        for j in jj..j_end {
                            crow[j] += aip * brow[j];
                        }
                    }
                }
                jj += BJ;
            }
            pp += BP;
        }
        ii += BI;
    }
    c
}

/// Naive integer GEMM (the bit-exact oracle), row-major i64. `a` is `m×k`, `b` is `k×n`.
pub fn igemm_scalar(a: &[i64], b: &[i64], m: usize, k: usize, n: usize) -> Vec<i64> {
    let mut c = vec![0i64; m * n];
    for i in 0..m {
        for p in 0..k {
            let aip = a[i * k + p];
            for j in 0..n {
                c[i * n + j] += aip * b[p * n + j];
            }
        }
    }
    c
}

/// Register-tiled integer GEMM (Stage 17.2 — the GotoBLAS/BLIS micro-kernel structure). An
/// MR×NR micro-tile of C is reused across the k-loop so each loaded B element feeds MR rows
/// (cache reuse); integer addition is associative and exact, so the result is **bit-for-bit**
/// the scalar oracle's regardless of tiling (P0).
///
/// HONEST MEASURED OUTCOME (this Xeon, release, `target-cpu=native`): this is **NOT faster**
/// than [`igemm_scalar`] — measured 0.46–0.71× across n∈{64..512} for both NR=4 and NR=16.
/// LLVM already auto-vectorizes the naive streaming inner loop to AVX-512, and a stack-backed
/// accumulator tile cannot keep the micro-tile in registers across the k-loop without
/// hand-written `#[target_feature]` SIMD intrinsics. Per the Stage-17 discipline
/// (`register_tiling_measured_faster_or_reverted`), [`igemm_scalar`] remains the recommended
/// path; this is retained as a bit-exact experiment (and a candidate for the 17.4 autotuner,
/// which correctly rejects it here). The real wins on this CPU are algorithmic (17.3) and
/// building native (17.1: hardware FMA/AVX is off by default).
pub fn igemm_tiled(a: &[i64], b: &[i64], m: usize, k: usize, n: usize) -> Vec<i64> {
    // NR ≥ AVX-512's 8 i64 lanes so the inner store loop vectorizes; MR rows reuse each
    // loaded B element (cache reuse). 4×16 keeps 64 i64 (≤ register budget).
    const MR: usize = 4;
    const NR: usize = 16;
    let mut c = vec![0i64; m * n];
    let mut i0 = 0;
    while i0 < m {
        let mr = (i0 + MR).min(m) - i0;
        let mut j0 = 0;
        while j0 < n {
            let nr = (j0 + NR).min(n) - j0;
            let mut acc = [[0i64; NR]; MR]; // micro-tile in registers
            for p in 0..k {
                for (ii, accrow) in acc.iter_mut().enumerate().take(mr) {
                    let aip = a[(i0 + ii) * k + p];
                    let brow = &b[p * n + j0..p * n + j0 + nr];
                    for (jj, accv) in accrow.iter_mut().enumerate().take(nr) {
                        *accv += aip * brow[jj];
                    }
                }
            }
            for (ii, accrow) in acc.iter().enumerate().take(mr) {
                for (jj, &v) in accrow.iter().enumerate().take(nr) {
                    c[(i0 + ii) * n + (j0 + jj)] = v;
                }
            }
            j0 += NR;
        }
        i0 += MR;
    }
    c
}

#[cfg(test)]
mod tests {
    use super::*;
    use jeff_math::fmat::Rng;

    #[test]
    fn simd_preserves_result_integers_exact() {
        // integer-valued data ⇒ vectorized == scalar BIT-FOR-BIT (no reassociation error).
        let n = 1000;
        let a: Vec<f64> = (0..n).map(|i| (i % 7) as f64).collect();
        let b: Vec<f64> = (0..n).map(|i| (i % 5) as f64).collect();
        assert_eq!(dot_vectorized(&a, &b), dot_scalar(&a, &b));
    }

    #[test]
    fn simd_preserves_result_floats_within_tol() {
        let n = 1000;
        let mut rng = Rng::new(0x5);
        let a: Vec<f64> = (0..n).map(|_| rng.gaussian()).collect();
        let b: Vec<f64> = (0..n).map(|_| rng.gaussian()).collect();
        let d = (dot_vectorized(&a, &b) - dot_scalar(&a, &b)).abs();
        assert!(d < 1e-9, "vectorized dot within tol of scalar, diff {d}");
    }

    #[test]
    fn gemm_vectorized_matches_scalar() {
        // integer GEMM ⇒ bit-for-bit identical.
        let (m, k, n) = (5, 6, 7);
        let a: Vec<f64> = (0..m * k).map(|i| (i % 4) as f64).collect();
        let b: Vec<f64> = (0..k * n).map(|i| (i % 3) as f64).collect();
        assert_eq!(gemm_vectorized(&a, &b, m, k, n), gemm_scalar(&a, &b, m, k, n));
    }

    #[test]
    fn simd_preserves_result_register_tiled_integer() {
        // register tiling on integers is BIT-EXACT vs the scalar oracle (associativity),
        // across sizes that straddle the 4×4 micro-tile (P0). Small values avoid i64 overflow.
        let mut rng = Rng::new(0x17_2A);
        for (m, k, n) in [(1usize, 1, 1), (4, 4, 4), (5, 6, 7), (64, 64, 64), (70, 130, 33)] {
            let a: Vec<i64> = (0..m * k).map(|_| (rng.next_u64() % 13) as i64 - 6).collect();
            let b: Vec<i64> = (0..k * n).map(|_| (rng.next_u64() % 13) as i64 - 6).collect();
            assert_eq!(
                igemm_tiled(&a, &b, m, k, n),
                igemm_scalar(&a, &b, m, k, n),
                "tiled != scalar at {m}x{k}x{n}"
            );
        }
    }

    #[test]
    fn simd_preserves_result_blocked_bit_exact() {
        // The cache-blocked GEMM equals the scalar oracle BIT-FOR-BIT, even on random
        // floats — because it keeps the per-output k-accumulation order (P0: speed never
        // changes the answer). Sizes that straddle the 64-block boundary.
        let mut rng = Rng::new(0xB10C);
        for (m, k, n) in [(1usize, 1, 1), (5, 6, 7), (64, 64, 64), (70, 130, 65)] {
            let a: Vec<f64> = (0..m * k).map(|_| rng.gaussian()).collect();
            let b: Vec<f64> = (0..k * n).map(|_| rng.gaussian()).collect();
            let oracle = gemm_scalar(&a, &b, m, k, n);
            assert_eq!(gemm_blocked(&a, &b, m, k, n), oracle, "blocked != scalar at {m}x{k}x{n}");
        }
    }
}
