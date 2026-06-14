//! Stage-6A Batch-4 collapsers: geometry / dimension / topology. Structure (persistence
//! above the noise band, an eigengap, a low residual, an intrinsic dim < ambient) →
//! verified collapse; absence → honest defer.

#![allow(clippy::needless_range_loop)]

use jeff_cert::{
    BarrierTag, Certificate, CollapseOutcome, Collapsed, Defer, Evidence, IrRef, Obligation,
};
use jeff_math::geometry;
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

/// 4.1 Persistent homology. Features above the noise band ⇒ real topology ⇒ collapse;
/// only the essential component ⇒ no persistent structure ⇒ defer.
pub fn persistent_homology(dist: &[f64], n: usize, band: f64) -> CollapseOutcome {
    let feature_count = geometry::persistent_feature_count(dist, n, band);
    if feature_count <= 1 {
        return defer(BarrierTag::BelowDetectionThreshold); // nothing above the noise band
    }
    certify(
        "geometry/persistent-homology",
        Evidence::PersistentHomology { dist: dist.to_vec(), n, band, feature_count },
        "H0 features with persistence > band (bottleneck-stable)",
        BarrierTag::BelowDetectionThreshold,
    )
}

/// 4.2 JL projection. Reduce to the JL dimension; if the realized distortion is within
/// `eps` ⇒ collapse, else fall back to full dimension.
pub fn jl(points: &[f64], n: usize, d: usize, eps: f64) -> CollapseOutcome {
    let k = geometry::jl_dimension(n, eps).min(d);
    let proj = geometry::jl_project(points, n, d, k, 0x5EED);
    if geometry::max_distortion(points, &proj, n, d, k) > eps {
        return defer(BarrierTag::ConstantFactorOnly); // projection didn't meet eps
    }
    certify(
        "geometry/johnson-lindenstrauss",
        Evidence::JlProjection { points: points.to_vec(), proj, n, d, k, eps },
        "(1±eps) pairwise distance preservation",
        BarrierTag::ConstantFactorOnly,
    )
}

/// 4.3 Spectral clustering. An eigengap after `k` ⇒ k clusters ⇒ collapse; none ⇒ defer.
pub fn spectral_cluster(w: &[f64], n: usize, k: usize, gap_min: f64) -> CollapseOutcome {
    if geometry::cluster_eigengap(w, n, k) < gap_min {
        return defer(BarrierTag::FlatSpectrum);
    }
    certify(
        "geometry/spectral-cluster",
        Evidence::SpectralCluster { w: w.to_vec(), n, k, gap_min },
        "normalized-Laplacian eigengap ≥ gap (k clusters)",
        BarrierTag::FlatSpectrum,
    )
}

/// 4.4 Diffusion map. A diffusion spectral gap after `m` ⇒ m-dim parametrization ⇒
/// collapse; slow eigenvalue decay ⇒ defer.
pub fn diffusion_map(w: &[f64], n: usize, m: usize, gap_min: f64) -> CollapseOutcome {
    if geometry::diffusion_gap(w, n, m) < gap_min {
        return defer(BarrierTag::HighIntrinsicDimension);
    }
    certify(
        "geometry/diffusion-map",
        Evidence::DiffusionMap { w: w.to_vec(), n, m, gap_min },
        "diffusion spectral gap after m ≥ gap",
        BarrierTag::HighIntrinsicDimension,
    )
}

/// 4.5 Isomap. Low residual variance at dimension `m` ⇒ manifold ⇒ collapse; high ⇒
/// defer (data fills the ambient space).
pub fn isomap(dist: &[f64], n: usize, k_nn: usize, m: usize, tol: f64) -> CollapseOutcome {
    let (resid, _) = geometry::isomap_residual(dist, n, k_nn, m);
    if resid > tol {
        return defer(BarrierTag::HighIntrinsicDimension);
    }
    certify(
        "geometry/isomap",
        Evidence::Isomap { dist: dist.to_vec(), n, k_nn, m, tol },
        "geodesic-MDS residual variance at dim m ≤ tol",
        BarrierTag::HighIntrinsicDimension,
    )
}

/// 4.6 Intrinsic dimension (Levina–Bickel). An estimate well below ambient ⇒ low-dim
/// structure ⇒ collapse; ≈ ambient ⇒ defer (no compression).
pub fn intrinsic_dim(points: &[f64], n: usize, d: usize, k1: usize, k2: usize) -> CollapseOutcome {
    let est = geometry::intrinsic_dimension(points, n, d, k1, k2);
    // require the estimate to be at least ~0.75 below the ambient dimension
    if est >= d as f64 - 0.75 {
        return defer(BarrierTag::HighIntrinsicDimension);
    }
    let tol = 0.5;
    certify(
        "geometry/intrinsic-dim",
        Evidence::IntrinsicDim { points: points.to_vec(), n, d, k1, k2, claimed_dim: est, tol },
        "Levina–Bickel intrinsic dimension < ambient",
        BarrierTag::HighIntrinsicDimension,
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
    fn no_persistence_deferred() {
        // tight single blob → no persistent H0 feature above a moderate band → defer.
        let mut rng = Rng::new(0x7);
        let n = 12;
        let d = 2;
        let pts: Vec<f64> = (0..n * d).map(|_| 0.01 * rng.gaussian()).collect();
        let dist = geometry::pairwise_distances(&pts, n, d);
        assert_eq!(tag(&persistent_homology(&dist, n, 1.0)), Some(BarrierTag::BelowDetectionThreshold));
        // two separated clusters → collapse
        let pts2 = vec![0.0, 0.0, 0.1, 0.0, 10.0, 10.0, 10.1, 10.0];
        let d2 = geometry::pairwise_distances(&pts2, 4, 2);
        assert!(is_collapsed(&persistent_homology(&d2, 4, 1.0)));
    }

    #[test]
    fn jl_collapses() {
        // d (256) exceeds the JL target dimension so the reduction is real and certified.
        let n = 24;
        let d = 256;
        let mut rng = Rng::new(0x7);
        let pts: Vec<f64> = (0..n * d).map(|_| rng.gaussian()).collect();
        assert!(is_collapsed(&jl(&pts, n, d, 0.5)));
    }

    #[test]
    fn no_eigengap_deferred() {
        let n = 6;
        // two clusters → collapse
        let mut w = vec![0.0; n * n];
        for i in 0..n {
            for j in 0..n {
                if i != j {
                    w[i * n + j] = if (i < 3) == (j < 3) { 1.0 } else { 0.01 };
                }
            }
        }
        assert!(is_collapsed(&spectral_cluster(&w, n, 2, 0.3)));
        // uniform similarity → no 2-cluster gap → defer
        let uni = vec![1.0; n * n];
        assert_eq!(tag(&spectral_cluster(&uni, n, 2, 0.3)), Some(BarrierTag::FlatSpectrum));
    }

    #[test]
    fn isomap_collapses_manifold() {
        let n = 8;
        let pts: Vec<f64> = (0..n).flat_map(|i| [i as f64, 0.0]).collect();
        let dist = geometry::pairwise_distances(&pts, n, 2);
        assert!(is_collapsed(&isomap(&dist, n, 3, 1, 0.05)));
    }

    #[test]
    fn intrinsic_dim_equals_ambient_deferred() {
        let n = 40;
        let d = 3;
        let mut rng = Rng::new(0x1D);
        // 1-D curve → collapse
        let line: Vec<f64> = (0..n)
            .flat_map(|i| [i as f64 * 0.3 + 0.001 * rng.gaussian(), 0.001 * rng.gaussian(), 0.001 * rng.gaussian()])
            .collect();
        assert!(is_collapsed(&intrinsic_dim(&line, n, d, 5, 12)));
        // space-filling cloud → intrinsic dim ≈ ambient → defer
        let cloud: Vec<f64> = (0..n * d).map(|_| rng.gaussian()).collect();
        assert_eq!(tag(&intrinsic_dim(&cloud, n, d, 5, 12)), Some(BarrierTag::HighIntrinsicDimension));
    }
}
