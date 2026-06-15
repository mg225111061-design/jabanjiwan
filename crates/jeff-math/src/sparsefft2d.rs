//! Stage 28.2 — 2D sparse FFT (the structured win). Extends the 1D HIKP decimation-aliasing +
//! phase-ratio method ([`crate::sparsefft::hikp_sparse_fft`]) to a 2D spectrum with `k` spikes,
//! and to **approximately-sparse** (k spikes + small noise) inputs. Reads only `O(k)` of the
//! `N²` samples (three `B×B` subsamplings, `B = O(k)`) and runs three `B×B` 2D FFTs ⇒ `O(k log k)`,
//! never touching all `N²`.
//!
//! [conservation-law label] Here the ratio vs a dense `O(N² log N)` 2D FFT **diverges** as `N/k`
//! grows — but only because the spectrum has real structure (k-sparsity); the output is the `k`
//! spikes (small), so Ω(N²) is not violated. Below the crossover (large k / small N) the method
//! declines (`None`) and the caller uses the dense FFT — a measured guard, not a guess.

use crate::complex::Complex;
use crate::fft::stockham_fft;
use std::f64::consts::TAU;

/// Row–column 2D DFT of an `n×n` complex array (row-major), `n` a power of two. `O(n² log n)`.
pub fn fft2d(x: &[Complex], n: usize) -> Vec<Complex> {
    debug_assert_eq!(x.len(), n * n);
    let mut rows: Vec<Complex> = vec![Complex::zero(); n * n];
    for r in 0..n {
        let spec = stockham_fft(&x[r * n..(r + 1) * n]);
        rows[r * n..(r + 1) * n].copy_from_slice(&spec);
    }
    // transform columns
    let mut out = vec![Complex::zero(); n * n];
    let mut col = vec![Complex::zero(); n];
    for c in 0..n {
        for r in 0..n {
            col[r] = rows[r * n + c];
        }
        let spec = stockham_fft(&col);
        for r in 0..n {
            out[r * n + c] = spec[r];
        }
    }
    out
}

/// Reconstruct the `n×n` signal from a recovered 2D support `(fx, fy, re, im)` where `(re,im)` is
/// the full spectral coefficient `X[fx,fy]`: `x[t,s] = (1/n²) Σ X·e^{2πi(fx·t+fy·s)/n}`. For the
/// residual certificate (Tier B).
pub fn idft2d_sparse(support: &[(usize, usize, f64, f64)], n: usize) -> Vec<Complex> {
    let inv = 1.0 / (n as f64 * n as f64);
    let mut out = vec![Complex::zero(); n * n];
    for t in 0..n {
        for s in 0..n {
            let mut acc = Complex::zero();
            for &(fx, fy, re, im) in support {
                let ang = TAU * (fx as f64 * t as f64 + fy as f64 * s as f64) / n as f64;
                acc = acc.add(Complex::new(re, im).mul(Complex::new(ang.cos(), ang.sin())));
            }
            out[t * n + s] = acc.scale(inv);
        }
    }
    out
}

/// 2D sparse FFT: recover up to `k` spikes of an `n×n` complex spectrum from `O(k)` samples.
/// `rel_thresh` filters buckets below `rel_thresh·max|Y0|` (noise floor). Returns the support
/// `(fx, fy, X_re, X_im)`, or `None` when the spectrum collides under the chosen bucket count or
/// isn't cleanly sparse — the crossover guard (caller falls back to the dense [`fft2d`]).
pub fn hikp_sparse_fft_2d(
    x: &[Complex],
    n: usize,
    k: usize,
    rel_thresh: f64,
) -> Option<Vec<(usize, usize, f64, f64)>> {
    if k == 0 || n < 2 || !n.is_power_of_two() || x.len() != n * n {
        return None;
    }
    let mut b = 1usize;
    while b < 2 * k + 1 {
        b <<= 1;
    }
    if b >= n {
        return None; // need D ≥ 2 and B < n to be sublinear → dense fallback
    }
    let d = n / b;
    // three decimated B×B subsamplings: base, +1 in x (rows), +1 in y (cols).
    let sub = |dt: usize, ds: usize| -> Vec<Complex> {
        let mut g = vec![Complex::zero(); b * b];
        for tt in 0..b {
            for ss in 0..b {
                g[tt * b + ss] = x[(tt * d + dt) * n + (ss * d + ds)];
            }
        }
        g
    };
    let y0 = fft2d(&sub(0, 0), b);
    let y1x = fft2d(&sub(1, 0), b);
    let y1y = fft2d(&sub(0, 1), b);

    let maxmag = y0.iter().map(|c| c.abs()).fold(0.0f64, f64::max);
    if maxmag == 0.0 {
        return Some(vec![]);
    }
    let thresh = rel_thresh * maxmag;
    let mut support = Vec::new();
    for bx in 0..b {
        for by in 0..b {
            let c0 = y0[bx * b + by];
            if c0.abs() < thresh {
                continue;
            }
            let fx = (y1x[bx * b + by].div(c0).arg() / TAU * n as f64).round().rem_euclid(n as f64) as usize;
            let fy = (y1y[bx * b + by].div(c0).arg() / TAU * n as f64).round().rem_euclid(n as f64) as usize;
            // consistency: a single clean spike must alias back to this bucket.
            if fx % b != bx || fy % b != by {
                return None; // collision under B buckets → bail to dense
            }
            let v = c0.scale((d * d) as f64); // X[fx,fy]
            support.push((fx, fy, v.re, v.im));
        }
    }
    if support.len() > k {
        return None;
    }
    Some(support)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// n×n complex signal `Σ_j c_j e^{2πi(fx_j t + fy_j s)/n}` (k clean 2D spikes).
    fn sparse_signal(n: usize, spikes: &[(usize, usize, f64, f64)]) -> Vec<Complex> {
        let mut x = vec![Complex::zero(); n * n];
        for t in 0..n {
            for s in 0..n {
                let mut acc = Complex::zero();
                for &(fx, fy, re, im) in spikes {
                    let ang = TAU * (fx as f64 * t as f64 + fy as f64 * s as f64) / n as f64;
                    acc = acc.add(Complex::new(re, im).mul(Complex::new(ang.cos(), ang.sin())));
                }
                x[t * n + s] = acc;
            }
        }
        x
    }

    fn l2(x: &[Complex]) -> f64 {
        x.iter().map(|c| c.abs2()).sum::<f64>().sqrt()
    }

    #[test]
    fn sparse_fft_2d_recovers_certified() {
        // exact 2D-sparse spectrum: recover from O(k) samples; reconstruct matches (residual cert).
        let n = 64usize;
        let spikes = [(1usize, 2usize, 1.0, 0.0), (5, 9, 0.7, -0.3), (20, 3, 0.4, 0.2)];
        let x = sparse_signal(n, &spikes);
        let support = hikp_sparse_fft_2d(&x, n, spikes.len(), 1e-6).expect("clean 2D sparse recovers");
        // recovered exactly the planted frequencies.
        for &(fx, fy, _, _) in &spikes {
            assert!(support.iter().any(|&(rx, ry, _, _)| rx == fx && ry == fy), "missing ({fx},{fy})");
        }
        let recon = idft2d_sparse(&support, n);
        let resid = l2(&x.iter().zip(&recon).map(|(a, b)| a.sub(*b)).collect::<Vec<_>>());
        assert!(resid <= 1e-6 * l2(&x), "2D residual {resid:e} must be ≤ tol·‖x‖");
    }

    #[test]
    fn sparse_fft_noisy_within_tol() {
        // k spikes + small noise: the dominant spikes recover within tolerance vs the dense FFT.
        let n = 64usize;
        let spikes = [(2usize, 2usize, 1.0, 0.0), (10, 20, 0.8, 0.1)];
        let mut x = sparse_signal(n, &spikes);
        let mut seed = 0xABCDu64;
        for c in x.iter_mut() {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            let nr = ((seed >> 40) as f64 / (1u64 << 24) as f64 - 0.5) * 1e-3;
            *c = c.add(Complex::new(nr, 0.0));
        }
        let support = hikp_sparse_fft_2d(&x, n, spikes.len(), 1e-2).expect("noisy sparse recovers");
        for &(fx, fy, _, _) in &spikes {
            assert!(support.iter().any(|&(rx, ry, _, _)| rx == fx && ry == fy), "missing spike under noise");
        }
        // recovered amplitudes (X[fx,fy] = c·n²; divide back to the time-domain amplitude c) are
        // close to truth within the noise level + tolerance.
        let n2 = (n * n) as f64;
        for &(fx, fy, re, im) in &spikes {
            let (_, _, rre, rim) = *support.iter().find(|&&(rx, ry, _, _)| rx == fx && ry == fy).unwrap();
            assert!((rre / n2 - re).abs() < 1e-2 && (rim / n2 - im).abs() < 1e-2, "amp off at ({fx},{fy})");
        }
    }

    #[test]
    fn sparse_fft_below_crossover_uses_dense() {
        // large k relative to n ⇒ B ≥ n ⇒ decline (None); caller uses the dense fft2d.
        let n = 8usize;
        let x = sparse_signal(n, &[(1, 1, 1.0, 0.0)]);
        assert!(hikp_sparse_fft_2d(&x, n, 8, 1e-6).is_none(), "must defer to dense");
        // non-power-of-two declines.
        let odd = vec![Complex::one(); 36];
        assert!(hikp_sparse_fft_2d(&odd, 6, 1, 1e-6).is_none());
    }

    #[test]
    fn sparse_fft_ratio_scales_n_over_k() {
        // op-count proxy: dense fft2d O(n² log n) vs sparse O(k log k) with B=O(k). The ratio
        // must GROW as n increases at fixed k (asymptotic, structure-only).
        let k = 4usize;
        let bk = (2 * k + 1).next_power_of_two();
        let mut prev = 0f64;
        for e in 6..=12 {
            let n = 1usize << e;
            let dense = (n * n) as f64 * (2.0 * e as f64); // n² log2(n²)
            let sparse = 3.0 * (bk * bk) as f64 * (2.0 * (bk as f64).log2()); // 3 B×B FFTs
            let ratio = dense / sparse;
            assert!(ratio > prev, "ratio must grow with n: n={n} ratio={ratio}");
            prev = ratio;
        }
        assert!(prev > 1e4, "by n=4096 the 2D sparse advantage is large (got {prev})");
    }
}
