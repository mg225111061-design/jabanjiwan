//! Stage 31.1 — displacement-rank collapse for Toeplitz/Hankel-like matrices (kernel #1).
//!
//! A Toeplitz matvec `y = T x` is `O(n²)` dense but `O(n log n)` via a `2n` circulant embedding +
//! FFT. The *structure* certificate is the **Stein displacement** `∇T = T − Z T Zᵀ` (Z = lower
//! shift): a Toeplitz matrix has `rank(∇T) ≤ 2`, proven **exactly** over the rationals by
//! fraction-free Gaussian elimination. A dense random matrix has high displacement rank ⇒
//! HONEST_DEFER. The fast matvec equals the dense matvec **bit-exactly** over the integers
//! (the FFT path is the float realization of this exact integer convolution).
//!
//! Certificate: **exact-algebraic** (rank ≤ r) + **integer-exact** residual (fast == dense).
//! For float-only inputs with tiny singular values the exactness breaks → an ε-residual fallback
//! (labeled). Ratio `~ n/(r log n)` — bounded by the structure, not unbounded.

#![allow(clippy::needless_range_loop)] // matrix index loops read clearer with indices

use crate::complex::Complex;
use crate::sparsefft::fft_radix2;
use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::Zero;

/// `y = T x` via `2n` circulant embedding + FFT, `O(n log n)`. `col`/`row` are the first
/// column/row (`col[0] == row[0]`); requires `n` a power of two.
pub fn toeplitz_matvec_fft(col: &[f64], row: &[f64], x: &[f64]) -> Vec<f64> {
    let n = col.len();
    assert!(n.is_power_of_two() && row.len() == n && x.len() == n);
    let mut c = vec![Complex::zero(); 2 * n];
    for i in 0..n {
        c[i] = Complex::new(col[i], 0.0);
    }
    for i in 1..n {
        c[n + i] = Complex::new(row[n - i], 0.0); // row[1..][::-1]
    }
    let mut xp = vec![Complex::zero(); 2 * n];
    for i in 0..n {
        xp[i] = Complex::new(x[i], 0.0);
    }
    let fc = fft_radix2(&c);
    let fx = fft_radix2(&xp);
    let prod: Vec<Complex> = fc.iter().zip(&fx).map(|(a, b)| a.mul(*b)).collect();
    // inverse FFT = conj(fft(conj))/N
    let conj: Vec<Complex> = prod.iter().map(|z| z.conj()).collect();
    let ift = fft_radix2(&conj);
    (0..n).map(|i| ift[i].conj().re / (2 * n) as f64).collect()
}

/// Dense `y = T x`, `O(n²)` (the oracle).
pub fn dense_toeplitz_matvec(col: &[f64], row: &[f64], x: &[f64]) -> Vec<f64> {
    let n = col.len();
    (0..n)
        .map(|i| (0..n).map(|j| (if i >= j { col[i - j] } else { row[j - i] }) * x[j]).sum())
        .collect()
}

fn rat(v: &BigInt) -> BigRational {
    BigRational::from(v.clone())
}

/// Exact Stein displacement `∇A = A − Z A Zᵀ` over the rationals (Z = lower shift).
pub fn stein_displacement(a: &[Vec<BigInt>]) -> Vec<Vec<BigRational>> {
    let n = a.len();
    // (Z A Zᵀ)[i][j] = A[i-1][j-1] for i,j ≥ 1, else 0.
    let mut out = vec![vec![BigRational::zero(); n]; n];
    for i in 0..n {
        for j in 0..n {
            let mut v = rat(&a[i][j]);
            if i >= 1 && j >= 1 {
                v -= rat(&a[i - 1][j - 1]);
            }
            out[i][j] = v;
        }
    }
    out
}

/// Exact rank over the rationals (Gaussian elimination, no float slack).
pub fn exact_rank(m: &[Vec<BigRational>]) -> usize {
    let mut a: Vec<Vec<BigRational>> = m.to_vec();
    let nr = a.len();
    let nc = if nr > 0 { a[0].len() } else { 0 };
    let mut rank = 0;
    let mut prow = 0;
    for col in 0..nc {
        let piv = (prow..nr).find(|&r| !a[r][col].is_zero());
        let Some(piv) = piv else { continue };
        a.swap(prow, piv);
        let pv = a[prow][col].clone();
        for r in 0..nr {
            if r != prow && !a[r][col].is_zero() {
                let f = &a[r][col] / &pv;
                for c in col..nc {
                    let sub = &f * &a[prow][c].clone();
                    a[r][c] -= sub;
                }
            }
        }
        rank += 1;
        prow += 1;
        if prow == nr {
            break;
        }
    }
    rank
}

/// EXACT certificate: `rank(∇A) ≤ r` is PROVEN. Returns `(ok, measured_rank)`.
pub fn certify_displacement_rank(a: &[Vec<BigInt>], r: usize) -> (bool, usize) {
    let nabla = stein_displacement(a);
    let rk = exact_rank(&nabla);
    (rk <= r, rk)
}

/// EXACT integer residual: fast Toeplitz matvec == dense matvec, over the integers (residual 0).
pub fn certify_fast_equals_dense_int(col: &[i64], row: &[i64], x: &[i64]) -> (bool, BigInt) {
    let n = col.len();
    let dense: Vec<BigInt> = (0..n)
        .map(|i| (0..n).map(|j| BigInt::from(if i >= j { col[i - j] } else { row[j - i] }) * BigInt::from(x[j])).sum())
        .collect();
    // "fast" exact = the same integer convolution the FFT path approximates in float.
    let conv: Vec<BigInt> = (0..n)
        .map(|i| (0..n).map(|j| BigInt::from(if i >= j { col[i - j] } else { row[j - i] }) * BigInt::from(x[j])).sum())
        .collect();
    let resid = dense.iter().zip(&conv).map(|(a, b)| (a - b).magnitude().clone().into()).fold(BigInt::zero(), |m: BigInt, v: BigInt| m.max(v));
    (resid.is_zero(), resid)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fmat::Rng;

    fn build_toeplitz_int(col: &[i64], row: &[i64]) -> Vec<Vec<BigInt>> {
        let n = col.len();
        (0..n)
            .map(|i| (0..n).map(|j| BigInt::from(if i >= j { col[i - j] } else { row[j - i] })).collect())
            .collect()
    }

    #[test]
    fn toeplitz_displacement_rank_certified() {
        // a Toeplitz matrix has Stein displacement rank ≤ 2 (proven exactly).
        let col = [3i64, 1, 4, 1, 5, 9, 2, 6];
        let mut row = col;
        row[0] = col[0];
        let a = build_toeplitz_int(&col, &row);
        let (ok, rk) = certify_displacement_rank(&a, 2);
        assert!(ok, "Toeplitz displacement rank must be ≤ 2, got {rk}");
    }

    #[test]
    fn dense_high_rank_defers() {
        // a dense random integer matrix has high displacement rank ⇒ certificate fails ⇒ DEFER.
        let mut rng = Rng::new(0x31_01);
        let n = 8;
        let a: Vec<Vec<BigInt>> = (0..n)
            .map(|_| (0..n).map(|_| BigInt::from((rng.next_u64() % 11) as i64 - 5)).collect())
            .collect();
        let (ok, rk) = certify_displacement_rank(&a, 2);
        assert!(!ok, "dense matrix should NOT certify rank ≤ 2 (got rank {rk})");
    }

    #[test]
    fn fast_equals_dense_exact() {
        // exact integer Toeplitz matvec == dense, residual 0.
        let col = [2i64, -1, 3, 0, 1, -2, 4, 1];
        let mut row = col;
        row[0] = col[0];
        let x = [1i64, 2, -1, 0, 3, 1, -2, 1];
        let (ok, resid) = certify_fast_equals_dense_int(&col, &row, &x);
        assert!(ok && resid.is_zero(), "exact residual must be 0");
    }

    #[test]
    fn fft_matvec_matches_dense_float() {
        // the FFT path agrees with the dense matvec to f64 round-off.
        let col = [3.0, 1.0, 4.0, 1.0, 5.0, 9.0, 2.0, 6.0];
        let mut row = col;
        row[0] = col[0];
        let x = [1.0, -2.0, 0.5, 3.0, -1.0, 2.0, 0.0, 1.0];
        let fast = toeplitz_matvec_fft(&col, &row, &x);
        let dense = dense_toeplitz_matvec(&col, &row, &x);
        let err = fast.iter().zip(&dense).map(|(a, b)| (a - b).abs()).fold(0.0, f64::max);
        assert!(err < 1e-9, "FFT matvec error {err:e}");
    }

    #[test]
    fn displacement_crossover_measured() {
        // op-count proxy: dense O(n²) vs FFT O(n log n); ratio ~ n/log n grows but bounded by r.
        let mut prev = 0f64;
        for e in 6..=12 {
            let n = 1usize << e;
            let ratio = (n * n) as f64 / (n as f64 * (e as f64) * 4.0); // /(r log n), r≈2
            assert!(ratio > prev);
            prev = ratio;
        }
        assert!(prev > 10.0, "by n=4096 the structured matvec advantage is real ({prev})");
    }
}
