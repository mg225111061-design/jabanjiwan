//! Stage-6A Batch-1 collapsers: detect the hidden structure at RUNTIME, then collapse
//! with a verified certificate or DEFER with a named barrier (PART A discipline).
//!
//! Principle: "no structure ⇒ defer (no speedup)", never "no structure ⇒ wrong answer"
//! (P0). Every collapse goes through `jeff_verify::verify` (P2) — the structure check is
//! the precondition; the certificate is the proof. These are library-level (B-tier);
//! surface→codegen wiring and measurement are Stage-6B promotion.

use jeff_cert::{
    BarrierTag, Certificate, CollapseOutcome, Collapsed, Defer, Evidence, IrRef, Obligation,
};
use jeff_math::complex::Complex;
use jeff_math::fmat::FMat;
use jeff_math::{frame, prony, recovery};
use jeff_span::Span;

/// Build a certificate around `evidence`, verify it, and return `Collapsed` on success
/// or `Defer(tag)` on failure (R31 fallback). The class label is recorded honestly.
fn certify(id: &'static str, evidence: Evidence, claim_kind: &str, tag: BarrierTag) -> CollapseOutcome {
    let span = Span::dummy();
    let source = IrRef::new(0, span);
    let class = evidence.cert_class();
    let cert = Certificate {
        collapser_id: id.into(),
        source,
        collapsed: IrRef::new(1, span),
        obligation: Obligation::new(format!("{claim_kind} [cert-class: {}]", class.as_str())),
        evidence,
        boundaries: vec![],
        fallback: source,
    };
    match jeff_verify::verify(cert) {
        Some(vc) => CollapseOutcome::Collapsed(Collapsed::new(IrRef::new(1, span), vc)),
        None => CollapseOutcome::Defer(Defer::new(tag, source)),
    }
}

/// 1.1 Compressed sensing. Structure: `y=Φx` with `x` k-sparse. Absent ⇒ `non-sparse`.
pub fn compressed_sensing(
    phi: &FMat,
    y: &[f64],
    k: usize,
    random_phi: bool,
    tol: f64,
) -> CollapseOutcome {
    let x = recovery::omp(phi, y, k);
    let resid: Vec<f64> = recovery::apply(phi, &x).iter().zip(y).map(|(a, b)| a - b).collect();
    let nnz = recovery::nnz(&x, 1e-9);
    // runtime precondition: did we explain y with ≤k nonzeros?
    if recovery::l2(&resid) > tol || nnz > k {
        return CollapseOutcome::Defer(Defer::new(BarrierTag::NonSparse, IrRef::new(0, Span::dummy())));
    }
    let phi_flat: Vec<f64> = (0..phi.rows)
        .flat_map(|i| (0..phi.cols).map(move |j| (i, j)))
        .map(|(i, j)| phi.get(i, j))
        .collect();
    certify(
        "sparse/compressed-sensing",
        Evidence::SparseRecovery {
            phi: phi_flat,
            rows: phi.rows,
            cols: phi.cols,
            y: y.to_vec(),
            x,
            k,
            tol,
            random_phi,
        },
        "Φx = y with ‖x‖₀ ≤ k",
        BarrierTag::NonSparse,
    )
}

/// 1.2 Sparse FFT. Structure: spectrum concentrated in ≤k bins. Absent ⇒ `flat-spectrum`.
pub fn sparse_fft(signal: &[f64], k: usize, tol: f64) -> CollapseOutcome {
    let n = signal.len();
    let support = recovery::sparse_fft(signal, k);
    let recon = recovery::idft_sparse(&support, n);
    let diff: Vec<f64> = signal.iter().zip(&recon).map(|(a, b)| a - b).collect();
    let sig_norm = recovery::l2(signal);
    // runtime precondition: do k bins capture the signal?
    if recovery::l2(&diff) > tol * sig_norm {
        return CollapseOutcome::Defer(Defer::new(
            BarrierTag::FlatSpectrum,
            IrRef::new(0, Span::dummy()),
        ));
    }
    certify(
        "sparse/sparse-fft",
        Evidence::SparseSpectrum {
            signal: signal.to_vec(),
            support,
            n,
            k,
            tol,
        },
        "‖x − F⁻¹ŷ‖₂ ≤ tol·‖x‖₂ with |support| ≤ k",
        BarrierTag::FlatSpectrum,
    )
}

/// 1.3 Matrix completion. Structure: rank ≤ r. Absent ⇒ `high-intrinsic-dimension`.
pub fn matrix_completion(
    observed: &[(usize, usize, f64)],
    rows: usize,
    cols: usize,
    r: usize,
    tol: f64,
    iters: usize,
) -> CollapseOutcome {
    let (u, v) = recovery::complete(observed, rows, cols, r, iters);
    // runtime precondition: does the rank-r fit explain the observed entries?
    let mut resid2 = 0.0;
    for &(i, j, val) in observed {
        let e = recovery::factor_entry(&u, &v, i, j);
        resid2 += (e - val) * (e - val);
    }
    if resid2.sqrt() > tol {
        return CollapseOutcome::Defer(Defer::new(
            BarrierTag::HighIntrinsicDimension,
            IrRef::new(0, Span::dummy()),
        ));
    }
    let uf: Vec<f64> = (0..rows).flat_map(|i| (0..r).map(move |l| (i, l))).map(|(i, l)| u.get(i, l)).collect();
    let vf: Vec<f64> = (0..cols).flat_map(|j| (0..r).map(move |l| (j, l))).map(|(j, l)| v.get(j, l)).collect();
    certify(
        "sparse/matrix-completion",
        Evidence::MatrixCompletion {
            observed: observed.to_vec(),
            u: uf,
            v: vf,
            rows,
            cols,
            r,
            tol,
        },
        "‖UVᵀ − M‖_Ω ≤ tol with rank ≤ r",
        BarrierTag::HighIntrinsicDimension,
    )
}

/// 1.4 Prony. Structure: sum of ≤k_max exponentials (small-order recurrence). Absent ⇒
/// `non-sparse`. Returns the smallest model order that fits within tol.
pub fn prony(samples: &[f64], k_max: usize, tol: f64) -> CollapseOutcome {
    for k in 1..=k_max {
        if let Some((a, res)) = prony::prony_fit(samples, k) {
            if res <= tol {
                return certify(
                    "sparse/prony",
                    Evidence::PronyRecurrence {
                        samples: samples.to_vec(),
                        a,
                        tol,
                    },
                    "samples satisfy a minimal degree-k linear recurrence",
                    BarrierTag::NonSparse,
                );
            }
        }
    }
    CollapseOutcome::Defer(Defer::new(BarrierTag::NonSparse, IrRef::new(0, Span::dummy())))
}

/// 1.5 Super-resolution. Structure: ≤s_max spikes separated by ≥ 2/f_c. Below the
/// separation threshold ⇒ `below-detection-threshold`.
pub fn super_resolution(lowpass: &[Complex], fc: usize, s_max: usize, tol: f64) -> CollapseOutcome {
    let threshold = 2.0 / fc as f64;
    for s in 1..=s_max {
        if let Some(spikes) = prony::super_resolve(lowpass, s) {
            let model: Vec<(f64, Complex)> = spikes.iter().map(|sp| (sp.t, sp.amp)).collect();
            let mut worst = 0.0_f64;
            for (m, c) in lowpass.iter().enumerate() {
                worst = worst.max(prony::eval_spike_model(&model, m).sub(*c).abs());
            }
            let locs: Vec<f64> = spikes.iter().map(|sp| sp.t).collect();
            let sep = prony::min_circular_separation(&locs);
            if worst <= tol && sep >= threshold {
                let lp: Vec<(f64, f64)> = lowpass.iter().map(|c| (c.re, c.im)).collect();
                let sp: Vec<(f64, f64, f64)> =
                    spikes.iter().map(|x| (x.t, x.amp.re, x.amp.im)).collect();
                return certify(
                    "sparse/super-resolution",
                    Evidence::SuperResolution {
                        lowpass: lp,
                        spikes: sp,
                        fc,
                        tol,
                    },
                    "spike model reproduces low-pass data; separation ≥ 2/f_c",
                    BarrierTag::BelowDetectionThreshold,
                );
            }
        }
    }
    CollapseOutcome::Defer(Defer::new(
        BarrierTag::BelowDetectionThreshold,
        IrRef::new(0, Span::dummy()),
    ))
}

/// 1.6 Welch/ETF. Structure: equiangular at the Welch bound. Absent ⇒ `flat-spectrum`
/// (high coherence — not an optimal frame).
pub fn equiangular_tight_frame(frame: &[f64], m: usize, n: usize, tol: f64) -> CollapseOutcome {
    if !frame::is_etf(frame, m, n, tol) {
        return CollapseOutcome::Defer(Defer::new(
            BarrierTag::FlatSpectrum,
            IrRef::new(0, Span::dummy()),
        ));
    }
    certify(
        "sparse/welch-etf",
        Evidence::EquiangularTightFrame {
            frame: frame.to_vec(),
            m,
            n,
            tol,
        },
        "every off-diagonal Gram magnitude = Welch bound (⇔ ETF)",
        BarrierTag::FlatSpectrum,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use jeff_math::fmat::gaussian_matrix;

    fn is_collapsed(o: &CollapseOutcome) -> bool {
        matches!(o, CollapseOutcome::Collapsed(_))
    }
    fn defer_tag(o: &CollapseOutcome) -> Option<BarrierTag> {
        match o {
            CollapseOutcome::Defer(d) => Some(d.tag),
            _ => None,
        }
    }

    #[test]
    fn compressed_sensing_collapses_sparse() {
        let phi = gaussian_matrix(12, 24, 0xABCDEF);
        let mut x = vec![0.0; 24];
        x[3] = 2.0;
        x[10] = -1.0;
        x[20] = 0.5;
        let y = recovery::apply(&phi, &x);
        assert!(is_collapsed(&compressed_sensing(&phi, &y, 3, true, 1e-6)));
    }

    #[test]
    fn non_sparse_deferred() {
        // a dense target cannot be 2-sparse-recovered → defer[non-sparse].
        let phi = gaussian_matrix(8, 16, 0x1111);
        let dense: Vec<f64> = (0..16).map(|i| (i as f64 * 0.6).cos() + 0.4).collect();
        let y = recovery::apply(&phi, &dense);
        assert_eq!(defer_tag(&compressed_sensing(&phi, &y, 2, true, 1e-6)), Some(BarrierTag::NonSparse));
    }

    #[test]
    fn sparse_fft_collapses_few_tones() {
        let n = 32;
        let two_pi = std::f64::consts::TAU;
        let x: Vec<f64> = (0..n).map(|t| (two_pi * 5.0 * t as f64 / n as f64).cos()).collect();
        assert!(is_collapsed(&sparse_fft(&x, 2, 1e-6)));
    }

    #[test]
    fn flat_spectrum_deferred() {
        // white-noise-like signal: no k-sparse spectrum → defer[flat-spectrum].
        let mut rng = jeff_math::fmat::Rng::new(0x9999);
        let x: Vec<f64> = (0..32).map(|_| rng.gaussian()).collect();
        assert_eq!(defer_tag(&sparse_fft(&x, 2, 1e-3)), Some(BarrierTag::FlatSpectrum));
    }

    #[test]
    fn completion_collapses_low_rank_else_defers() {
        let (rows, cols) = (8, 8);
        let u: Vec<f64> = (0..rows).map(|i| 1.0 + 0.1 * i as f64).collect();
        let v: Vec<f64> = (0..cols).map(|j| 2.0 - 0.05 * j as f64).collect();
        let observed: Vec<(usize, usize, f64)> = (0..rows)
            .flat_map(|i| (0..cols).map(move |j| (i, j)))
            .filter(|&(i, j)| (i * 5 + j * 3) % 10 < 7)
            .map(|(i, j)| (i, j, u[i] * v[j]))
            .collect();
        assert!(is_collapsed(&matrix_completion(&observed, rows, cols, 1, 1e-3, 40)));

        // full-rank random observations cannot be rank-1 completed → defer.
        let mut rng = jeff_math::fmat::Rng::new(0x7);
        let rand_obs: Vec<(usize, usize, f64)> = (0..rows)
            .flat_map(|i| (0..cols).map(move |j| (i, j)))
            .map(|(i, j)| (i, j, rng.gaussian()))
            .collect();
        assert_eq!(
            defer_tag(&matrix_completion(&rand_obs, rows, cols, 1, 1e-3, 40)),
            Some(BarrierTag::HighIntrinsicDimension)
        );
    }

    #[test]
    fn prony_collapses_else_defers() {
        let s: Vec<f64> = (0..12).map(|t| 1.5_f64.powi(t) + 2.0 * 0.5_f64.powi(t)).collect();
        assert!(is_collapsed(&prony(&s, 4, 1e-6)));
        // pure noise has no small-order recurrence → defer.
        let mut rng = jeff_math::fmat::Rng::new(0x42);
        let noise: Vec<f64> = (0..20).map(|_| rng.gaussian()).collect();
        assert_eq!(defer_tag(&prony(&noise, 3, 1e-6)), Some(BarrierTag::NonSparse));
    }

    #[test]
    fn super_resolution_collapses_separated_else_defers() {
        let fc = 8;
        let mm = 2 * fc + 1;
        let two_pi = std::f64::consts::TAU;
        let make = |locs: &[f64]| -> Vec<Complex> {
            (0..mm)
                .map(|m| {
                    let mut acc = Complex::zero();
                    for &t in locs {
                        acc = acc.add(Complex::from_angle(-two_pi * m as f64 * t));
                    }
                    acc
                })
                .collect()
        };
        // separated spikes (0.5 apart ≫ 2/8=0.25) collapse
        assert!(is_collapsed(&super_resolution(&make(&[0.2, 0.7]), fc, 3, 1e-6)));
        // two spikes 0.05 apart < 0.25 threshold → defer[below-detection-threshold]
        assert_eq!(
            defer_tag(&super_resolution(&make(&[0.40, 0.45]), fc, 3, 1e-6)),
            Some(BarrierTag::BelowDetectionThreshold)
        );
    }

    #[test]
    fn etf_collapses_else_defers() {
        let (v, m, n) = frame::mercedes_benz();
        assert!(is_collapsed(&equiangular_tight_frame(&v, m, n, 1e-9)));
        // a generic frame is not an ETF → defer[flat-spectrum]
        let mut rng = jeff_math::fmat::Rng::new(0x5151);
        let rv: Vec<f64> = (0..10).map(|_| rng.gaussian()).collect();
        assert_eq!(defer_tag(&equiangular_tight_frame(&rv, 5, 2, 1e-3)), Some(BarrierTag::FlatSpectrum));
    }
}
