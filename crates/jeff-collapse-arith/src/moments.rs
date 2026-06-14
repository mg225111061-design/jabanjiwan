//! Stage-6A Batch-3 collapsers: latent-variable / moment methods. Spectral-gap
//! precondition → verified collapse, else honest defer. All certs eps-approximate.

#![allow(clippy::needless_range_loop)]

use jeff_cert::{
    BarrierTag, Certificate, CollapseOutcome, Collapsed, Defer, Evidence, IrRef, Obligation,
};
use jeff_math::moments;
use jeff_span::Span;

fn certify(id: &'static str, evidence: Evidence, claim: &str, tag: BarrierTag) -> CollapseOutcome {
    let span = Span::dummy();
    let source = IrRef::new(0, span);
    let class = evidence.cert_class();
    let cert = Certificate {
        collapser_id: id.into(),
        source,
        collapsed: IrRef::new(1, span),
        obligation: Obligation::new(format!("{claim} [cert-class: {}]", class.as_str())),
        evidence,
        boundaries: vec![],
        fallback: source,
    };
    match jeff_verify::verify(cert) {
        Some(vc) => CollapseOutcome::Collapsed(Collapsed::new(IrRef::new(1, span), vc)),
        None => CollapseOutcome::Defer(Defer::new(tag, source)),
    }
}

fn defer(tag: BarrierTag) -> CollapseOutcome {
    CollapseOutcome::Defer(Defer::new(tag, IrRef::new(0, Span::dummy())))
}

/// 3.1 Orthogonal tensor decomposition (Jennrich). Distinct contracted eigenvalues
/// (gap) ⇒ unique decomposition ⇒ collapse; degenerate ⇒ defer.
pub fn tensor_decomp(tensor: &[f64], p: usize, r: usize, tol: f64) -> CollapseOutcome {
    let (lambdas, factors, gap) = moments::jennrich_decompose(tensor, p, r);
    if gap < 1.05 {
        return defer(BarrierTag::BelowDetectionThreshold); // degenerate / non-unique
    }
    certify(
        "moments/jennrich",
        Evidence::TensorDecomp { tensor: tensor.to_vec(), p, r, lambdas, factors, tol },
        "T = Σ λ_i a_i⊗³ within tol (orthogonal decomposition)",
        BarrierTag::BelowDetectionThreshold,
    )
}

/// 3.2 Spectral HMM. A clean rank-`m` bigram (low residual + singular gap) ⇒ collapse;
/// rank-deficient / no gap ⇒ defer.
pub fn spectral_hmm(bigram: &[f64], rows: usize, cols: usize, m: usize, tol: f64) -> CollapseOutcome {
    let gap_min = 5.0;
    let (resid, gap) = moments::bigram_rank_residual(bigram, rows, cols, m);
    if resid > tol || gap < gap_min {
        return defer(BarrierTag::HighIntrinsicDimension);
    }
    certify(
        "moments/spectral-hmm",
        Evidence::HmmRank { bigram: bigram.to_vec(), rows, cols, m, tol, gap_min },
        "bigram moment matrix is rank-m (σ_m/σ_{m+1} ≥ gap)",
        BarrierTag::HighIntrinsicDimension,
    )
}

/// 3.3 Mixture moment factorization (Anandkumar). Whitening + tensor decomposition; if
/// moments reconstruct within tol ⇒ collapse; misspecified ⇒ defer.
pub fn mixture_moments(m2: &[f64], m3: &[f64], p: usize, k: usize, tol: f64) -> CollapseOutcome {
    let (weights, means) = moments::whiten_decompose(m2, m3, p, k);
    let r2 = moments::moment2_residual(m2, &weights, &means, k, p);
    if r2 > tol {
        return defer(BarrierTag::HighIntrinsicDimension);
    }
    certify(
        "moments/anandkumar",
        Evidence::MomentFactorization { m2: m2.to_vec(), m3: m3.to_vec(), p, k, weights, means, tol },
        "M2,M3 = Σ w_i μ_i⊗ᵈ within tol",
        BarrierTag::HighIntrinsicDimension,
    )
}

/// 3.4 Mixture of point masses from moments (method of moments = Prony). Clean k-mixture
/// ⇒ collapse; ill-conditioned ⇒ defer.
pub fn point_mass_mixture(moments_seq: &[f64], k: usize, tol: f64) -> CollapseOutcome {
    match moments::moment_mixture(moments_seq, k) {
        Some((weights, locations)) => certify(
            "moments/mixture",
            Evidence::MomentMixture { moments: moments_seq.to_vec(), k, weights, locations, tol },
            "moment sequence = Σ w_j x_j^t within tol",
            BarrierTag::BelowDetectionThreshold,
        ),
        None => defer(BarrierTag::BelowDetectionThreshold),
    }
}

/// 3.5 FastICA. A non-Gaussian (high-kurtosis) projection ⇒ collapse; Gaussian sources
/// (no separable structure) ⇒ defer.
pub fn ica(data: &[f64], n: usize, p: usize, threshold: f64) -> CollapseOutcome {
    let Some(direction) = moments::fastica_direction(data, n, p) else {
        return defer(BarrierTag::HighIntrinsicDimension);
    };
    if moments::excess_kurtosis(data, n, p, &direction).abs() < threshold {
        return defer(BarrierTag::FlatSpectrum); // Gaussian: no non-Gaussian direction
    }
    certify(
        "moments/fastica",
        Evidence::IcaProjection { data: data.to_vec(), n, p, direction, threshold },
        "a linear projection of the data is non-Gaussian (|excess kurtosis| ≥ threshold)",
        BarrierTag::FlatSpectrum,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use jeff_math::fmat::Rng;

    fn is_collapsed(o: &CollapseOutcome) -> bool {
        matches!(o, CollapseOutcome::Collapsed(_))
    }
    fn tag(o: &CollapseOutcome) -> Option<BarrierTag> {
        match o {
            CollapseOutcome::Defer(d) => Some(d.tag),
            _ => None,
        }
    }

    #[test]
    fn tensor_decomp_collapses_distinct_defers_degenerate() {
        let p = 3;
        let lam = [3.0, 2.0, 1.0];
        let mut t = vec![0.0; p * p * p];
        for c in 0..3 {
            for i in 0..p {
                for j in 0..p {
                    for k in 0..p {
                        let e = |x: usize| if x == c { 1.0 } else { 0.0 };
                        t[(i * p + j) * p + k] += lam[c] * e(i) * e(j) * e(k);
                    }
                }
            }
        }
        assert!(is_collapsed(&tensor_decomp(&t, p, 3, 1e-6)));

        // non-decomposable: a random tensor has no rank-3 orthogonal structure, so the
        // Jennrich reconstruction residual stays large ⇒ verification fails ⇒ defer.
        let mut rng = Rng::new(0xDEAD);
        let rand_t: Vec<f64> = (0..p * p * p).map(|_| rng.gaussian()).collect();
        assert_eq!(
            tag(&tensor_decomp(&rand_t, p, 3, 1e-6)),
            Some(BarrierTag::BelowDetectionThreshold)
        );
    }

    #[test]
    fn rank_deficient_hmm_deferred() {
        let rows = 6;
        let cols = 6;
        let mut rng = Rng::new(0x9);
        // rank-2 bigram → collapses at m=2
        let u1: Vec<f64> = (0..rows).map(|_| rng.gaussian()).collect();
        let v1: Vec<f64> = (0..cols).map(|_| rng.gaussian()).collect();
        let u2: Vec<f64> = (0..rows).map(|_| rng.gaussian()).collect();
        let v2: Vec<f64> = (0..cols).map(|_| rng.gaussian()).collect();
        let mut b = vec![0.0; rows * cols];
        for i in 0..rows {
            for j in 0..cols {
                b[i * cols + j] = u1[i] * v1[j] + u2[i] * v2[j];
            }
        }
        assert!(is_collapsed(&spectral_hmm(&b, rows, cols, 2, 1e-6)));
        // full-rank random matrix → no rank-2 structure → defer.
        let full: Vec<f64> = (0..rows * cols).map(|_| rng.gaussian()).collect();
        assert_eq!(tag(&spectral_hmm(&full, rows, cols, 2, 1e-6)), Some(BarrierTag::HighIntrinsicDimension));
    }

    #[test]
    fn mixture_moments_collapses() {
        let p = 3;
        let k = 2;
        let unit = |mut v: Vec<f64>| {
            let n = v.iter().map(|x: &f64| x * x).sum::<f64>().sqrt();
            v.iter_mut().for_each(|x| *x /= n);
            v
        };
        let means = [unit(vec![1.0, 0.5, 0.0]), unit(vec![0.2, 1.0, 0.3])];
        let w = [0.6_f64, 0.4];
        let mut m2 = vec![0.0; p * p];
        let mut m3 = vec![0.0; p * p * p];
        for c in 0..k {
            for i in 0..p {
                for j in 0..p {
                    m2[i * p + j] += w[c] * means[c][i] * means[c][j];
                    for kk in 0..p {
                        m3[(i * p + j) * p + kk] += w[c] * means[c][i] * means[c][j] * means[c][kk];
                    }
                }
            }
        }
        assert!(is_collapsed(&mixture_moments(&m2, &m3, p, k, 1e-6)));
    }

    #[test]
    fn point_mass_mixture_collapses() {
        let locs = [2.0_f64, 5.0];
        let w = [0.7_f64, 0.3];
        let moments: Vec<f64> = (0..6).map(|t| w[0] * locs[0].powi(t) + w[1] * locs[1].powi(t)).collect();
        assert!(is_collapsed(&point_mass_mixture(&moments, 2, 1e-6)));
    }

    #[test]
    fn gaussian_sources_deferred() {
        let n = 2000;
        let p = 2;
        let mut rng = Rng::new(0x1CA);
        // non-Gaussian mixture → collapse
        let mut data = vec![0.0; n * p];
        for k in 0..n {
            let s1 = if rng.next_u64() & 1 == 0 { 1.0 } else { -1.0 } + 0.05 * rng.gaussian();
            let s2 = rng.gaussian();
            data[k * p] = 0.8 * s1 + 0.6 * s2;
            data[k * p + 1] = 0.6 * s1 - 0.8 * s2;
        }
        assert!(is_collapsed(&ica(&data, n, p, 0.5)));
        // Gaussian sources → defer[flat-spectrum]
        let g: Vec<f64> = (0..n * p).map(|_| rng.gaussian()).collect();
        assert_eq!(tag(&ica(&g, n, p, 0.5)), Some(BarrierTag::FlatSpectrum));
    }
}
