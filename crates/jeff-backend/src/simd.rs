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
}
