//! Stage 10 — Python absorption (CLAUDE.md PART G). JEFF "absorbs" the Python scientific
//! stack: each common idiom (NumPy/SciPy/scikit-learn/NetworkX/statsmodels/streaming) is
//! exposed as a function backed by a **verified collapser**. Every call returns an
//! [`Absorbed`] status — either `Collapsed` (the structure was found and the result
//! carries a machine-checked certificate of the stated class) or `Deferred` (the
//! structure was absent, with the named barrier tag). This is the thesis end-to-end:
//! structure collapses with a certificate; everything else defers honestly; never a
//! wrong answer.
//!
//! The eight absorption layers:
//!   1. `numpy`        — dense linear algebra / transforms
//!   2. `scipy_sparse` — sparse & randomized linear algebra, compressed sensing
//!   3. `scipy_signal` — spectral estimation (sparse FFT, Prony, super-resolution)
//!   4. `sklearn_decomp` — PCA / ICA / tensor decomposition
//!   5. `sklearn_manifold` — manifold learning, clustering, dimension reduction
//!   6. `networkx`     — graph structure (planted clique, SBM, planar matchings)
//!   7. `statsmodels`  — latent-variable / moment methods (mixtures, HMM)
//!   8. `streaming`    — sublinear sketches

use jeff_cert::{BarrierTag, CertClass, CollapseOutcome};

/// Stage 22 — measured hidden-structure coverage over the target domains.
pub mod coverage;

/// The honest outcome of an absorbed operation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Absorbed {
    /// Structure found; result is certified at this class by the named kernel.
    Collapsed { kernel: &'static str, cert: CertClass },
    /// Structure absent; deferred with the named barrier tag (honest, not a failure).
    Deferred { kernel: &'static str, tag: BarrierTag },
}

impl Absorbed {
    /// Classify a collapser outcome into an absorbed status. The certificate class is
    /// read from the **actual verified certificate** (never guessed), so the reported
    /// class can never overclaim the real guarantee (DR2/P1).
    fn of(kernel: &'static str, o: CollapseOutcome) -> Self {
        match o {
            CollapseOutcome::Collapsed(c) => {
                let cert = c.certificate().certificate().evidence.cert_class();
                Absorbed::Collapsed { kernel, cert }
            }
            CollapseOutcome::Defer(d) => Absorbed::Deferred { kernel, tag: d.tag },
        }
    }
    pub fn is_collapsed(&self) -> bool {
        matches!(self, Absorbed::Collapsed { .. })
    }
}

/// Layer 1 — NumPy: dense linear algebra / transforms.
pub mod numpy {
    use super::*;
    use jeff_collapse_tensor::contract;
    use jeff_math::tensornet::Tensor;

    /// `numpy.einsum` over a closed tensor network → scalar (treewidth-aware, exact).
    pub fn einsum(tensors: &[Tensor], dim: usize, n_indices: usize, budget: usize) -> Absorbed {
        Absorbed::of("numpy.einsum", contract(tensors, dim, n_indices, budget))
    }
}

/// Layer 2 — SciPy sparse / randomized linear algebra & compressed sensing.
pub mod scipy_sparse {
    use super::*;
    use jeff_collapse_arith::sparse;
    use jeff_math::fmat::FMat;

    /// `scipy.sparse.linalg`-style compressed-sensing recovery (OMP; precondition:
    /// k-sparse). Collapses on a sparse solution; defers on a dense one.
    pub fn compressed_sensing(phi: &[f64], y: &[f64], m: usize, n: usize, k: usize) -> Absorbed {
        let phi_mat = FMat::from_data(m, n, phi.to_vec());
        Absorbed::of(
            "scipy.sparse.cs",
            sparse::compressed_sensing(&phi_mat, y, k, true, 1e-6),
        )
    }
}

/// Layer 3 — SciPy signal: spectral estimation.
pub mod scipy_signal {
    use super::*;
    use jeff_collapse_arith::sparse;

    /// `scipy.signal`-style spectral-line (Prony) recovery: a few exponential modes →
    /// collapse with an exact recurrence; broadband → defer.
    pub fn prony(samples: &[f64], modes: usize) -> Absorbed {
        Absorbed::of("scipy.signal.prony", sparse::prony(samples, modes, 1e-6))
    }
}

/// Layer 4 — scikit-learn decomposition: PCA / ICA / tensor decomposition.
pub mod sklearn_decomp {
    use super::*;
    use jeff_collapse_arith::{moments, planted};

    /// `sklearn.decomposition.SparsePCA`: collapses on a sparse leading component.
    pub fn sparse_pca(cov: &[f64], p: usize, k: usize, threshold: f64) -> Absorbed {
        Absorbed::of(
            "sklearn.SparsePCA",
            planted::sparse_pca(cov, p, k, threshold),
        )
    }

    /// `sklearn.decomposition.FastICA`: collapses on a non-Gaussian projection; Gaussian
    /// sources → defer[flat-spectrum].
    pub fn fast_ica(data: &[f64], n: usize, p: usize, threshold: f64) -> Absorbed {
        Absorbed::of("sklearn.FastICA", moments::ica(data, n, p, threshold))
    }
}

/// Layer 5 — scikit-learn manifold / cluster / dimension reduction.
pub mod sklearn_manifold {
    use super::*;
    use jeff_collapse_arith::geometry;

    /// `sklearn.manifold.Isomap`: collapses when the geodesic-MDS residual is low.
    pub fn isomap(dist: &[f64], n: usize, k_nn: usize, m: usize, tol: f64) -> Absorbed {
        Absorbed::of("sklearn.Isomap", geometry::isomap(dist, n, k_nn, m, tol))
    }

    /// `sklearn.cluster.SpectralClustering`: collapses on a Laplacian eigengap.
    pub fn spectral_clustering(w: &[f64], n: usize, k: usize, gap_min: f64) -> Absorbed {
        Absorbed::of(
            "sklearn.SpectralClustering",
            geometry::spectral_cluster(w, n, k, gap_min),
        )
    }
}

/// Layer 6 — NetworkX: graph structure.
pub mod networkx {
    use super::*;
    use jeff_collapse_arith::planted;
    use jeff_collapse_holographic::planar_matchings;

    /// `networkx`-style planted-clique recovery (spectral): collapses with an EXACT
    /// clique certificate above the `c√n` threshold.
    pub fn find_planted_clique(adj: &[u8], n: usize, k: usize) -> Absorbed {
        Absorbed::of("networkx.planted_clique", planted::planted_clique(adj, n, k))
    }

    /// FKT perfect-matching count for a planar graph; non-planar → defer (Cai–Lu).
    pub fn perfect_matchings(
        n: usize,
        edges: &[(usize, usize)],
        faces: &[Vec<usize>],
        bipartite: bool,
    ) -> Absorbed {
        Absorbed::of(
            "networkx.perfect_matchings(FKT)",
            planar_matchings(n, edges, faces, bipartite),
        )
    }
}

/// Layer 7 — statsmodels: latent-variable / moment methods.
pub mod statsmodels {
    use super::*;
    use jeff_collapse_arith::moments;

    /// Method-of-moments mixture recovery (point masses via Prony).
    pub fn mixture(moments_seq: &[f64], k: usize, tol: f64) -> Absorbed {
        Absorbed::of(
            "statsmodels.mixture(MoM)",
            moments::point_mass_mixture(moments_seq, k, tol),
        )
    }

    /// Spectral HMM: collapses on a rank-`m` bigram moment matrix.
    pub fn spectral_hmm(bigram: &[f64], rows: usize, cols: usize, m: usize, tol: f64) -> Absorbed {
        Absorbed::of(
            "statsmodels.spectral_hmm",
            moments::spectral_hmm(bigram, rows, cols, m, tol),
        )
    }
}

/// Layer 8 — streaming / big-data sketches.
pub mod streaming {
    use super::*;
    use jeff_collapse_arith::streaming as st;

    /// `F2` frequency moment (AMS): collapses for k=2; high moments hit the space floor.
    pub fn frequency_moment(items: &[u64], k: usize, lambda: f64) -> Absorbed {
        Absorbed::of("streaming.F2(AMS)", st::frequency_moment(items, k, lambda))
    }

    /// Heavy hitters (Misra–Gries): collapses when a `≥ φn` item exists.
    pub fn heavy_hitters(items: &[u64], k: usize, phi: f64) -> Absorbed {
        Absorbed::of("streaming.heavy_hitters", st::heavy_hitters(items, k, phi))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jeff_math::fmat::Rng;

    #[test]
    fn layer1_numpy_einsum() {
        use jeff_math::tensornet::Tensor;
        let net = vec![
            Tensor { indices: vec![0, 1], data: vec![1, 2, 3, 4] },
            Tensor { indices: vec![1, 2], data: vec![5, 6, 7, 8] },
            Tensor { indices: vec![2, 0], data: vec![1, 0, 1, 1] },
        ];
        assert!(numpy::einsum(&net, 2, 3, 8).is_collapsed());
    }

    #[test]
    fn layer3_scipy_signal_prony() {
        // two exponential modes → collapse; broadband noise → defer.
        let samples: Vec<f64> = (0..12).map(|t| 2.0 * 0.9f64.powi(t) + 1.0 * 0.5f64.powi(t)).collect();
        assert!(scipy_signal::prony(&samples, 2).is_collapsed());
        let mut rng = Rng::new(0x9);
        let noise: Vec<f64> = (0..12).map(|_| rng.gaussian()).collect();
        assert!(!scipy_signal::prony(&noise, 2).is_collapsed());
    }

    #[test]
    fn layer4_sklearn_ica_defers_gaussian() {
        let n = 2000;
        let p = 2;
        let mut rng = Rng::new(0x1CA);
        let g: Vec<f64> = (0..n * p).map(|_| rng.gaussian()).collect();
        let res = sklearn_decomp::fast_ica(&g, n, p, 0.5);
        assert_eq!(res, Absorbed::Deferred { kernel: "sklearn.FastICA", tag: BarrierTag::FlatSpectrum });
    }

    #[test]
    fn layer6_networkx_planar_and_nonplanar() {
        // planar C4 collapses
        let c4 = [(0, 1), (1, 2), (2, 3), (3, 0)];
        assert!(networkx::perfect_matchings(4, &c4, &[vec![0, 1, 2, 3]], true).is_collapsed());
        // non-planar K3,3 defers
        let k33: Vec<(usize, usize)> = (0..3).flat_map(|i| (3..6).map(move |j| (i, j))).collect();
        let r = networkx::perfect_matchings(6, &k33, &[], true);
        assert_eq!(r, Absorbed::Deferred { kernel: "networkx.perfect_matchings(FKT)", tag: BarrierTag::NonPlanar });
    }

    #[test]
    fn layer7_statsmodels_mixture() {
        let locs = [2.0_f64, 5.0];
        let w = [0.7_f64, 0.3];
        let m: Vec<f64> = (0..6).map(|t| w[0] * locs[0].powi(t) + w[1] * locs[1].powi(t)).collect();
        assert!(statsmodels::mixture(&m, 2, 1e-6).is_collapsed());
    }

    #[test]
    fn layer8_streaming_moment_and_heavy_hitters() {
        let mut s = vec![7u64; 500];
        s.extend((0..200).map(|i| (i % 50) as u64 + 100));
        assert!(streaming::frequency_moment(&s, 2, 0.5).is_collapsed());
        // F6 hits the space floor → defer
        assert!(!streaming::frequency_moment(&s, 6, 0.5).is_collapsed());
        assert!(streaming::heavy_hitters(&s, 8, 0.3).is_collapsed());
    }

    #[test]
    fn reported_cert_class_is_the_real_one() {
        // honest labeling (DR2): the class is read from the verified cert, not guessed.
        // Prony at tol=1e-6 (> EXACT_EPS=1e-9) is eps-approximate, NOT exact.
        let samples: Vec<f64> = (0..12).map(|t| 2.0 * 0.9f64.powi(t) + 0.5f64.powi(t)).collect();
        assert_eq!(
            scipy_signal::prony(&samples, 2),
            Absorbed::Collapsed { kernel: "scipy.signal.prony", cert: CertClass::EpsApproximate }
        );
        // planted clique carries an EXACT certificate.
        let tri = vec![0u8, 1, 1, 1, 0, 1, 1, 1, 0];
        assert_eq!(
            networkx::find_planted_clique(&tri, 3, 3),
            Absorbed::Collapsed { kernel: "networkx.planted_clique", cert: CertClass::Exact }
        );
    }

    #[test]
    fn all_eight_layers_have_a_working_entry() {
        // smoke: each layer's primary entry resolves to a real Absorbed status.
        use jeff_math::tensornet::Tensor;
        let net = vec![Tensor { indices: vec![], data: vec![1] }];
        let _ = numpy::einsum(&net, 2, 0, 4);
        let _ = scipy_sparse::compressed_sensing(&[1.0, 0.0, 0.0, 1.0], &[1.0, 1.0], 2, 2, 1);
        let _ = scipy_signal::prony(&[1.0, 0.5, 0.25, 0.125], 1);
        let _ = sklearn_decomp::sparse_pca(&[1.0, 0.0, 0.0, 1.0], 2, 1, 0.5);
        let _ = sklearn_manifold::spectral_clustering(&[0.0, 1.0, 1.0, 0.0], 2, 1, 0.0);
        let _ = networkx::find_planted_clique(&[0, 1, 1, 0], 2, 1);
        let _ = statsmodels::mixture(&[1.0, 2.0, 4.0], 1, 1e-6);
        let _ = streaming::heavy_hitters(&[1, 1, 1, 2], 2, 0.5);
    }
}
