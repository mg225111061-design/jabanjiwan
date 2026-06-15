//! Stage 28.1 — dense FFT, Stockham **autosort** (no bit-reversal pass).
//!
//! The radix-2 Cooley–Tukey in [`crate::sparsefft::fft_radix2`] does an explicit bit-reversal
//! permutation before the butterflies. The Stockham autosort folds the reordering into the
//! stages: each stage is out-of-place (ping-pong between two buffers) and writes outputs to the
//! naturally-ordered positions, so the standalone permutation pass disappears (better locality,
//! one fewer pass over the data). Dense FFT is Ω(N log N) with no exploitable structure ⇒ the
//! target is **parity** with a tuned incumbent (pocketfft), never a win — see §C.28.
//!
//! Correctness: matches [`crate::sparsefft::fft_radix2`] (hence the naive DFT) to f64 round-off;
//! deterministic / bit-reproducible.

use crate::complex::Complex;
use std::f64::consts::PI;

/// Radix-2 Stockham autosort FFT, `n` a power of two. No bit-reversal pass: the stages reorder
/// implicitly, leaving the result in natural order. Returns a fresh buffer.
pub fn stockham_fft(input: &[Complex]) -> Vec<Complex> {
    let n = input.len();
    assert!(n.is_power_of_two(), "Stockham requires power-of-two length");
    if n == 1 {
        return input.to_vec();
    }
    let mut a = input.to_vec();
    let mut b = vec![Complex::zero(); n];
    // `l` = number of butterfly groups (halves), `m` = points per group; n = 2*l*m.
    let mut l = n / 2;
    let mut m = 1usize;
    while l >= 1 {
        for j in 0..l {
            let theta = -PI * (j as f64) / (l as f64); // = -2π·j/(2l)
            let w = Complex::new(theta.cos(), theta.sin());
            for k in 0..m {
                let a0 = a[k + j * m];
                let a1 = a[k + j * m + l * m];
                b[k + 2 * j * m] = a0.add(a1);
                b[k + 2 * j * m + m] = a0.sub(a1).mul(w);
            }
        }
        std::mem::swap(&mut a, &mut b);
        l /= 2;
        m *= 2;
    }
    a
}

/// Real-input convenience: `stockham_fft` on a real signal.
pub fn stockham_fft_real(x: &[f64]) -> Vec<Complex> {
    let input: Vec<Complex> = x.iter().map(|&v| Complex::new(v, 0.0)).collect();
    stockham_fft(&input)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sparsefft::fft_radix2;

    fn naive_dft(x: &[Complex]) -> Vec<Complex> {
        let n = x.len();
        (0..n)
            .map(|f| {
                let mut acc = Complex::zero();
                for (t, &xt) in x.iter().enumerate() {
                    let ang = -2.0 * PI * (f as f64) * (t as f64) / n as f64;
                    acc = acc.add(xt.mul(Complex::new(ang.cos(), ang.sin())));
                }
                acc
            })
            .collect()
    }

    fn close(a: &[Complex], b: &[Complex], tol: f64) -> bool {
        a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.sub(*y).abs() <= tol)
    }

    #[test]
    fn stockham_matches_dft_within_tol() {
        let mut seed = 0x5715u64;
        for &n in &[1usize, 2, 4, 8, 16, 64, 256, 1024] {
            let x: Vec<Complex> = (0..n)
                .map(|_| {
                    seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                    let r = (seed >> 33) as f64 / (1u64 << 31) as f64 - 1.0;
                    seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                    let i = (seed >> 33) as f64 / (1u64 << 31) as f64 - 1.0;
                    Complex::new(r, i)
                })
                .collect();
            let got = stockham_fft(&x);
            assert!(close(&got, &naive_dft(&x), 1e-9), "stockham vs DFT n={n}");
            assert!(close(&got, &fft_radix2(&x), 1e-9), "stockham vs radix2 n={n}");
        }
    }

    #[test]
    fn fft_no_bitreversal_pass() {
        // The autosort produces natural-order output WITHOUT a separate bit-reversal permutation
        // (which would be needed by in-place radix-2). We verify by feeding an impulse: the FFT
        // of δ[t-1] is the pure phase ramp e^{-2πi f/n} in *natural* bin order — a bit-reversed
        // implementation that skipped its permutation pass would mis-order these.
        let n = 16usize;
        let mut x = vec![Complex::zero(); n];
        x[1] = Complex::one();
        let spec = stockham_fft(&x);
        for (f, c) in spec.iter().enumerate() {
            let ang = -2.0 * PI * f as f64 / n as f64;
            assert!(c.sub(Complex::new(ang.cos(), ang.sin())).abs() < 1e-12, "bin {f} mis-ordered");
        }
    }

    #[test]
    fn stockham_bit_reproducible() {
        let x: Vec<Complex> = (0..512).map(|t| Complex::new((t as f64).sin(), (t as f64 * 0.3).cos())).collect();
        assert_eq!(stockham_fft(&x), stockham_fft(&x));
    }
}
