//! Stage 12 — sublinear sparse FFT via Prony (CLAUDE.md PART D). A signal that is a sum
//! of `K` real sinusoids is the output of an order-`2K` linear recurrence, so Prony
//! recovers its frequencies from only `O(K)` time samples — **sublinear in n**, where the
//! naive DFT is `O(n²)` (and even an FFT is `O(n log n)` and must read all n). The
//! recovered frequencies are checked against the naive DFT peaks (the oracle).

use crate::complex::{poly_roots, Complex};
use crate::prony::prony_fit;
use std::f64::consts::PI;

/// Stage 18.2 — genuinely sublinear sparse FFT for an exactly-k-sparse spectrum, via
/// **decimation aliasing + phase-ratio frequency recovery**. It reads only `O(k)` of the `n`
/// samples (two decimated subsamplings) and runs two `O(k)`-point FFTs → `O(k log k)`, never
/// touching all `n`. Returns the support `(freq, re, im)` in `recovery::sparse_fft`'s
/// convention (so the residual certificate re-validates), or `None` when the spectrum
/// collides under the chosen bucket count or isn't cleanly sparse — the caller then falls
/// back to the dense `O(n log n)` path (the crossover guard). Requires `n` a power of two.
///
/// Math: decimating `x` by `D = n/B` aliases bin `b` of the `B`-point DFT onto the spike
/// whose frequency `≡ b (mod B)`; a one-sample shift multiplies that bin by `ω_n^{freq}`, so
/// `Y1[b]/Y0[b] = e^{2πi·freq/n}` recovers the full frequency exactly, and `X[freq] = D·Y0[b]`.
/// A surviving spike must satisfy `freq ≡ b (mod B)`; a violated check signals a collision.
pub fn hikp_sparse_fft(signal: &[f64], k: usize, rel_thresh: f64) -> Option<Vec<(usize, f64, f64)>> {
    let n = signal.len();
    if k == 0 || n < 2 || !n.is_power_of_two() {
        return None;
    }
    let mut b = 1usize;
    while b < 2 * k + 1 {
        b <<= 1;
    }
    if b >= n {
        return None; // need D ≥ 2 (the shift) and B<n to be sublinear → dense fallback
    }
    let d = n / b;
    let y0: Vec<Complex> = (0..b).map(|t| Complex::new(signal[t * d], 0.0)).collect();
    let y1: Vec<Complex> = (0..b).map(|t| Complex::new(signal[t * d + 1], 0.0)).collect();
    let f0 = fft_radix2(&y0);
    let f1 = fft_radix2(&y1);
    let maxmag = f0.iter().map(|c| c.abs()).fold(0.0f64, f64::max);
    if maxmag == 0.0 {
        return Some(vec![]);
    }
    let thresh = rel_thresh * maxmag;
    let two_pi = std::f64::consts::TAU;
    let mut support = Vec::new();
    for (bin, (&c0, &c1)) in f0.iter().zip(&f1).enumerate() {
        if c0.abs() < thresh {
            continue;
        }
        let ratio = c1.div(c0); // ≈ e^{2πi·freq/n}
        let freq = (ratio.arg() / two_pi * n as f64).round().rem_euclid(n as f64) as usize;
        if freq % b != bin {
            return None; // collision: not a single clean spike in this bucket → bail to dense
        }
        let v = c0.scale(d as f64); // X[freq]
        support.push((freq, v.re, v.im));
    }
    if support.len() > k {
        return None;
    }
    Some(support)
}

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

    fn k_sparse_signal(n: usize, tones: &[(usize, f64)]) -> Vec<f64> {
        (0..n)
            .map(|t| {
                tones
                    .iter()
                    .map(|&(f, amp)| amp * (std::f64::consts::TAU * f as f64 * t as f64 / n as f64).cos())
                    .sum()
            })
            .collect()
    }

    #[test]
    fn hikp_matches_naive_within_tol() {
        // exactly-sparse signal: HIKP recovers the spectrum from O(k) samples; reconstructing
        // from the recovered support matches the signal (residual-certified, Tier B).
        let n = 1024usize;
        let tones = [(1usize, 1.0), (5, 0.7), (9, 0.4)]; // 3 cosines ⇒ 6 spectral spikes
        let signal = k_sparse_signal(n, &tones);
        let support = super::hikp_sparse_fft(&signal, 2 * tones.len(), 1e-6)
            .expect("clean sparse spectrum recovers");
        let recon = crate::recovery::idft_sparse(&support, n);
        let resid = crate::recovery::l2(
            &signal.iter().zip(&recon).map(|(a, b)| a - b).collect::<Vec<_>>(),
        );
        assert!(
            resid <= 1e-6 * crate::recovery::l2(&signal),
            "HIKP residual {resid:e} must be ≤ tol·‖x‖"
        );
        // and it found exactly the 6 conjugate-pair spikes.
        assert_eq!(support.len(), 6);
    }

    #[test]
    fn hikp_sublinear_in_k() {
        // self-relative: at large n with small k, HIKP reads O(k) samples and beats a full
        // O(n log n) FFT that must touch all n. Same recovered tones.
        let n = 1 << 16; // 65536
        let tones = [(3usize, 1.0), (777, 0.5), (10001, 0.25)];
        let signal = k_sparse_signal(n, &tones);
        let t0 = std::time::Instant::now();
        let support = super::hikp_sparse_fft(&signal, 2 * tones.len(), 1e-6).expect("recovers");
        let hikp = t0.elapsed().as_secs_f64().max(1e-12);
        let input: Vec<Complex> = signal.iter().map(|&v| Complex::new(v, 0.0)).collect();
        let t1 = std::time::Instant::now();
        let _full = fft_radix2(&input);
        let full = t1.elapsed().as_secs_f64();
        // recovered the 3 positive tones (among the 6 spikes).
        for &(f, _) in &tones {
            assert!(support.iter().any(|&(rf, _, _)| rf == f), "missing tone {f}");
        }
        assert!(
            hikp * 10.0 < full,
            "HIKP must dominate the full FFT at n={n} (hikp {hikp:.6}s, full {full:.6}s)"
        );
    }

    #[test]
    fn below_crossover_uses_dense() {
        // small n / large k ⇒ buckets ≥ n ⇒ HIKP declines (None), caller uses dense FFT.
        let n = 16usize;
        let signal: Vec<f64> = (0..n).map(|t| (t as f64).sin()).collect();
        assert!(super::hikp_sparse_fft(&signal, 8, 1e-6).is_none(), "must defer to dense");
        // non-power-of-two also declines.
        let odd: Vec<f64> = vec![1.0; 100];
        assert!(super::hikp_sparse_fft(&odd, 2, 1e-6).is_none());
    }

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
