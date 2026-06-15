//! Stage 12 — sublinear sparse FFT via Prony (CLAUDE.md PART D). A signal that is a sum
//! of `K` real sinusoids is the output of an order-`2K` linear recurrence, so Prony
//! recovers its frequencies from only `O(K)` time samples — **sublinear in n**, where the
//! naive DFT is `O(n²)` (and even an FFT is `O(n log n)` and must read all n). The
//! recovered frequencies are checked against the naive DFT peaks (the oracle).

use crate::complex::{poly_roots, Complex};
use crate::prony::prony_fit;
use std::f64::consts::PI;

/// Naive `O(n²)` DFT peak finder: the `k` frequency bins (in `0..n`) with the largest
/// magnitude. The exact oracle the sublinear recovery is checked against.
pub fn naive_dft_peaks(signal: &[f64], k: usize) -> Vec<usize> {
    let n = signal.len();
    let mut mags: Vec<(f64, usize)> = (0..n)
        .map(|f| {
            let (mut re, mut im) = (0.0, 0.0);
            for (t, &x) in signal.iter().enumerate() {
                let ang = -2.0 * PI * (f as f64) * (t as f64) / n as f64;
                re += x * ang.cos();
                im += x * ang.sin();
            }
            ((re * re + im * im).sqrt(), f)
        })
        .collect();
    mags.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    let mut out: Vec<usize> = mags.into_iter().take(k).map(|(_, f)| f).collect();
    out.sort_unstable();
    out
}

/// Sublinear sparse FFT: recover the integer frequencies of a `K`-sinusoid signal by
/// running Prony (order `2K`) on only the first `prefix.len()` samples — reads `O(K)`
/// points, `O(poly K)` work, independent of `n`. Returns the recovered frequency bins
/// (including conjugate mirrors), or `None` if the prefix is not order-`2K`-recurrent.
pub fn sparse_fft_prony(prefix: &[f64], num_sinusoids: usize, n: usize) -> Option<Vec<usize>> {
    let order = 2 * num_sinusoids;
    let (coeffs, resid) = prony_fit(prefix, order)?;
    if resid > 1e-6 {
        return None; // not a clean sum of `num_sinusoids` tones
    }
    let croots = poly_roots(&coeffs.iter().map(|&c| Complex::new(c, 0.0)).collect::<Vec<_>>());
    let mut freqs = Vec::new();
    for r in croots {
        if (r.abs() - 1.0).abs() < 1e-2 {
            // pure tone: z = e^{2πi f / n} ⇒ f = arg(z)·n/2π (mod n)
            let mut ang = r.arg();
            if ang < 0.0 {
                ang += 2.0 * PI;
            }
            let f = (ang * n as f64 / (2.0 * PI)).round() as i64;
            let f = ((f % n as i64) + n as i64) % n as i64;
            freqs.push(f as usize);
        }
    }
    freqs.sort_unstable();
    freqs.dedup();
    Some(freqs)
}

/// Iterative radix-2 Cooley–Tukey FFT (`n` a power of two), `O(n log n)` — a *fast* DFT
/// baseline (fairer than the naive `O(n²)`), so the sublinear win is measured honestly.
pub fn fft_radix2(input: &[Complex]) -> Vec<Complex> {
    let n = input.len();
    let mut a = input.to_vec();
    // bit-reversal permutation
    let mut j = 0usize;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j ^= bit;
        if i < j {
            a.swap(i, j);
        }
    }
    let mut len = 2;
    while len <= n {
        let ang = -2.0 * PI / len as f64;
        let wlen = Complex::new(ang.cos(), ang.sin());
        let mut i = 0;
        while i < n {
            let mut w = Complex::one();
            for k in 0..len / 2 {
                let u = a[i + k];
                let v = a[i + k + len / 2].mul(w);
                a[i + k] = u.add(v);
                a[i + k + len / 2] = u.sub(v);
                w = w.mul(wlen);
            }
            i += len;
        }
        len <<= 1;
    }
    a
}

/// Top-`k` magnitude peaks of a real signal via the `O(n log n)` FFT (power-of-two `n`).
pub fn fft_peaks(signal: &[f64], k: usize) -> Vec<usize> {
    let spec = fft_radix2(&signal.iter().map(|&x| Complex::new(x, 0.0)).collect::<Vec<_>>());
    let mut mags: Vec<(f64, usize)> = spec.iter().enumerate().map(|(f, c)| (c.abs(), f)).collect();
    mags.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    let mut out: Vec<usize> = mags.into_iter().take(k).map(|(_, f)| f).collect();
    out.sort_unstable();
    out
}

/// Sample a `K`-sinusoid test signal `Σ_j cos(2π f_j t / n)` at index `t` (used to read
/// only a prefix — the point of the sublinear method).
pub fn sinusoid_sample(freqs: &[usize], n: usize, t: usize) -> f64 {
    freqs
        .iter()
        .map(|&f| (2.0 * PI * (f as f64) * (t as f64) / n as f64).cos())
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prony_recovers_frequencies_sublinearly() {
        // n=512, two tones at f=5 and f=50. Read only a small prefix.
        let n = 512;
        let tones = [5usize, 50];
        let order = 2 * tones.len(); // 4
        let prefix: Vec<f64> = (0..(2 * order + 4)).map(|t| sinusoid_sample(&tones, n, t)).collect();
        let rec = sparse_fft_prony(&prefix, tones.len(), n).expect("recover");
        // recovered frequencies include both tones (and their conjugate mirrors n-f).
        assert!(rec.contains(&5), "recovered {rec:?}");
        assert!(rec.contains(&50), "recovered {rec:?}");
    }

    #[test]
    fn recovery_matches_naive_dft_peaks() {
        let n = 256;
        let tones = [3usize, 17];
        let full: Vec<f64> = (0..n).map(|t| sinusoid_sample(&tones, n, t)).collect();
        // naive DFT peaks: each cosine contributes two mirror bins ⇒ top 4.
        let peaks = naive_dft_peaks(&full, 4);
        let order = 2 * tones.len();
        let prefix = &full[..(2 * order + 4)];
        let rec = sparse_fft_prony(prefix, tones.len(), n).expect("recover");
        for f in [3usize, 17, n - 3, n - 17] {
            assert!(peaks.contains(&f), "naive peak {f} missing: {peaks:?}");
            assert!(rec.contains(&f), "prony missed {f}: {rec:?}");
        }
    }

    #[test]
    fn fft_matches_naive_dft_peaks() {
        // the fast FFT agrees with the naive DFT on the peak locations (sanity).
        let n = 256;
        let tones = [3usize, 17];
        let full: Vec<f64> = (0..n).map(|t| sinusoid_sample(&tones, n, t)).collect();
        assert_eq!(fft_peaks(&full, 4), naive_dft_peaks(&full, 4));
    }

    #[test]
    fn non_sparse_signal_defers() {
        // white noise has no order-4 recurrence ⇒ None (honest, no fake spectrum).
        let mut seed = 12345u64;
        let prefix: Vec<f64> = (0..12)
            .map(|_| {
                seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                ((seed >> 33) as f64 / (1u64 << 31) as f64) - 1.0
            })
            .collect();
        assert!(sparse_fft_prony(&prefix, 2, 512).is_none());
    }
}
