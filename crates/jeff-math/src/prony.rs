//! Batch-1 spectral structure: Prony / matrix-pencil (1.4) and super-resolution (1.5).
//!
//! Core fact (Prony 1795): a signal `s_t = Σ_{j=1}^k c_j z_j^t` satisfies a degree-`k`
//! linear recurrence whose characteristic roots are the `z_j`. So "is this a sum of ≤k
//! exponentials?" reduces to "do the samples satisfy a degree-k recurrence?" — a
//! **real, finitely-checkable** structural fact. The certificate (checked in
//! jeff-verify) is exactly that recurrence residual (real arithmetic, no root-finding
//! in the checker). Super-resolution applies the same idea to low-pass Fourier data,
//! where the recurrence roots lie on the unit circle and give the spike locations.

// Index-based loops read more clearly than iterators for these
// matrix / recurrence kernels; allow it module-wide (numeric code).
#![allow(clippy::needless_range_loop)]

use crate::complex::{poly_roots, solve_complex, Complex};
use crate::fmat::{solve_dense, FMat};

// ---- 1.4 Prony: fit a degree-k linear recurrence ----

/// Fit `s_{t+k} = −Σ_{i<k} a_i s_{t+i}` (i.e. `Σ_{i=0}^{k} a_i s_{t+i}=0`, `a_k=1`).
/// Returns `(a[0..=k], max_residual)` or `None` if under-determined. The recurrence is
/// the certificate: if `max_residual ≤ ε` the samples are a sum of ≤k exponentials.
pub fn prony_fit(samples: &[f64], k: usize) -> Option<(Vec<f64>, f64)> {
    let n = samples.len();
    if k == 0 || n < 2 * k + 1 {
        return None; // need an over-determined system to be meaningful
    }
    let rows = n - k;
    // A (rows×k) with A[t][i]=s[t+i]; rhs[t] = −s[t+k]. Normal equations AᵀA x = Aᵀrhs.
    let mut a = FMat::zeros(rows, k);
    let mut rhs = vec![0.0; rows];
    for t in 0..rows {
        for i in 0..k {
            a.set(t, i, samples[t + i]);
        }
        rhs[t] = -samples[t + k];
    }
    let at = a.transpose();
    let gram = at.matmul(&a);
    let atr: Vec<f64> = (0..k)
        .map(|i| (0..rows).map(|t| a.get(t, i) * rhs[t]).sum())
        .collect();
    let x = solve_dense(&gram, &atr)?;
    let mut coeffs = x; // a_0..a_{k-1}
    coeffs.push(1.0); // a_k = 1
    let res = recurrence_residual(samples, &coeffs);
    Some((coeffs, res))
}

/// `max_t |Σ_i a_i s_{t+i}|` — the checkable Prony certificate quantity (real).
pub fn recurrence_residual(samples: &[f64], a: &[f64]) -> f64 {
    let k = a.len() - 1;
    let n = samples.len();
    if n <= k {
        return f64::INFINITY;
    }
    let mut worst = 0.0_f64;
    for t in 0..(n - k) {
        let acc: f64 = (0..=k).map(|i| a[i] * samples[t + i]).sum();
        worst = worst.max(acc.abs());
    }
    worst
}

// ---- 1.5 Super-resolution: recover spikes from low-pass Fourier data ----

/// A recovered spike: location `t ∈ [0,1)` and complex amplitude.
#[derive(Clone, Copy, Debug)]
pub struct Spike {
    pub t: f64,
    pub amp: Complex,
}

/// Complex least squares `min ‖A x − b‖` via the normal equations `AᴴA x = Aᴴ b`.
fn complex_lstsq(a: &[Vec<Complex>], b: &[Complex]) -> Option<Vec<Complex>> {
    let rows = a.len();
    let cols = a[0].len();
    let mut aha = vec![vec![Complex::zero(); cols]; cols];
    let mut ahb = vec![Complex::zero(); cols];
    for p in 0..cols {
        for q in 0..cols {
            let mut acc = Complex::zero();
            for arow in a.iter().take(rows) {
                acc = acc.add(arow[p].conj().mul(arow[q]));
            }
            aha[p][q] = acc;
        }
        let mut acc = Complex::zero();
        for (r, arow) in a.iter().enumerate().take(rows) {
            acc = acc.add(arow[p].conj().mul(b[r]));
        }
        ahb[p] = acc;
    }
    solve_complex(&aha, &ahb)
}

/// Recover `s` spikes from low-pass Fourier measurements `h[m] = Σ_j a_j e^{−2πi m t_j}`,
/// `m = 0..lowpass.len()`. Returns the spikes, or `None` if recovery is ill-posed.
pub fn super_resolve(lowpass: &[Complex], s: usize) -> Option<Vec<Spike>> {
    let m = lowpass.len();
    if s == 0 || m < 2 * s + 1 {
        return None;
    }
    // 1. fit complex annihilating filter: Σ_{i<s} c_i h[t+i] = −h[t+s]
    let rows = m - s;
    let mut a = vec![vec![Complex::zero(); s]; rows];
    let mut rhs = vec![Complex::zero(); rows];
    for (t, arow) in a.iter_mut().enumerate() {
        for (i, slot) in arow.iter_mut().enumerate() {
            *slot = lowpass[t + i];
        }
        rhs[t] = lowpass[t + s].scale(-1.0);
    }
    let c = complex_lstsq(&a, &rhs)?;
    // polynomial coeffs low→high: c_0..c_{s-1}, then 1
    let mut coeffs = c;
    coeffs.push(Complex::one());
    // 2. roots z_j = e^{−2πi t_j}
    let roots = poly_roots(&coeffs);
    if roots.len() != s {
        return None;
    }
    // 3. locations t_j = wrap(−arg(z_j)/2π)
    let two_pi = std::f64::consts::TAU;
    let locs: Vec<f64> = roots
        .iter()
        .map(|z| {
            let mut t = -z.arg() / two_pi;
            t -= t.floor();
            t
        })
        .collect();
    // 4. amplitudes via Vandermonde least squares h[m]=Σ_j a_j z_j^m
    let mut v = vec![vec![Complex::zero(); s]; m];
    for (mm, vrow) in v.iter_mut().enumerate() {
        for (j, slot) in vrow.iter_mut().enumerate() {
            // z_j^mm where z_j = e^{−2πi t_j}
            let ang = -two_pi * (mm as f64) * locs[j];
            *slot = Complex::from_angle(ang);
        }
    }
    let amps = complex_lstsq(&v, lowpass)?;
    Some(
        locs.iter()
            .zip(&amps)
            .map(|(&t, &amp)| Spike { t, amp })
            .collect(),
    )
}

/// Evaluate the spike model at Fourier index `m` (the checker's oracle, by direct
/// evaluation — no solving): `Σ_j amp_j e^{−2πi m t_j}`.
pub fn eval_spike_model(spikes: &[(f64, Complex)], m: usize) -> Complex {
    let two_pi = std::f64::consts::TAU;
    let mut acc = Complex::zero();
    for &(t, amp) in spikes {
        acc = acc.add(amp.mul(Complex::from_angle(-two_pi * (m as f64) * t)));
    }
    acc
}

/// Minimum circular separation among spike locations in `[0,1)` (the threshold check).
pub fn min_circular_separation(locs: &[f64]) -> f64 {
    let mut best = f64::INFINITY;
    for i in 0..locs.len() {
        for j in (i + 1)..locs.len() {
            let d = (locs[i] - locs[j]).abs();
            let circ = d.min(1.0 - d);
            best = best.min(circ);
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prony_fits_sum_of_two_geometrics() {
        // s_t = 2·(1.5)^t + 3·(0.5)^t : degree-2 recurrence, residual ≈ 0.
        let s: Vec<f64> = (0..10)
            .map(|t| 2.0 * 1.5_f64.powi(t) + 3.0 * 0.5_f64.powi(t))
            .collect();
        let (a, res) = prony_fit(&s, 2).expect("fit");
        assert!(res < 1e-6, "degree-2 recurrence residual {res}");
        assert!((a[2] - 1.0).abs() < 1e-12);
    }

    #[test]
    fn prony_rejects_too_low_order() {
        // a genuine degree-2 signal does NOT satisfy a degree-1 recurrence.
        let s: Vec<f64> = (0..10)
            .map(|t| 2.0 * 1.5_f64.powi(t) + 3.0 * 0.5_f64.powi(t))
            .collect();
        let (_, res) = prony_fit(&s, 1).expect("fit");
        assert!(res > 1e-3, "degree-1 must not explain a degree-2 signal");
    }

    #[test]
    fn super_resolution_recovers_two_spikes() {
        // two spikes at t=0.2, t=0.7 with unit amplitude, separation 0.5 ≫ threshold.
        let true_locs = [0.2_f64, 0.7];
        let amps = [Complex::new(1.0, 0.0), Complex::new(0.8, 0.0)];
        let fc = 8usize;
        let mm = 2 * fc + 1;
        let two_pi = std::f64::consts::TAU;
        let lowpass: Vec<Complex> = (0..mm)
            .map(|m| {
                let mut acc = Complex::zero();
                for (k, &t) in true_locs.iter().enumerate() {
                    acc = acc.add(amps[k].mul(Complex::from_angle(-two_pi * m as f64 * t)));
                }
                acc
            })
            .collect();
        let spikes = super_resolve(&lowpass, 2).expect("resolve");
        let mut locs: Vec<f64> = spikes.iter().map(|s| s.t).collect();
        locs.sort_by(|a, b| a.partial_cmp(b).unwrap());
        assert!((locs[0] - 0.2).abs() < 1e-4, "loc0={}", locs[0]);
        assert!((locs[1] - 0.7).abs() < 1e-4, "loc1={}", locs[1]);
        // model reproduces the data
        let model: Vec<(f64, Complex)> = spikes.iter().map(|s| (s.t, s.amp)).collect();
        for m in 0..mm {
            assert!(eval_spike_model(&model, m).sub(lowpass[m]).abs() < 1e-6);
        }
    }
}
