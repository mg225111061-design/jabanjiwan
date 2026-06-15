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
