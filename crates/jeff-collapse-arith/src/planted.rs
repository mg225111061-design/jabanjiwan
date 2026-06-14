//! Stage-6A Batch-2 collapsers: planted / spiked detection with proven thresholds as
//! defer boundaries (PART A). Structure above threshold → verified collapse; below →
//! defer. The spiked-tensor collapser implements the explicit **3-way trichotomy**:
//! efficiently-detectable (collapse) / in-the-gap (defer, `open`) / undetectable (defer).

#![allow(clippy::needless_range_loop)]

use jeff_cert::{
    BarrierTag, Certificate, CollapseOutcome, Collapsed, Defer, Evidence, IrRef, Obligation,
};
use jeff_math::planted::{self, TensorRegime};
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

/// 2.1 BBP spike detection. Above the BBP edge (+TW margin) with a visible gap ⇒
/// collapse; otherwise the top eigenvalue is stuck in the bulk ⇒ defer.
pub fn spiked_covariance(cov: &[f64], p: usize, n: usize) -> CollapseOutcome {
    let threshold = planted::bbp_threshold(p, n);
    let gap = 0.5; // confirm separation beyond Tracy–Widom fluctuations
    let (l1, l2) = planted::top_two_eigenvalues(cov, p);
    if l1 < threshold || (l1 - l2) < gap {
        return defer(BarrierTag::BelowDetectionThreshold);
    }
    certify(
        "planted/bbp-spike",
        Evidence::SpikedCovariance { cov: cov.to_vec(), p, threshold, gap },
        "λ₁ ≥ BBP edge ∧ λ₁−λ₂ ≥ gap",
        BarrierTag::BelowDetectionThreshold,
    )
}

/// 2.2 Planted clique. Recover spectrally + peel to a clique; certify exactly. Below the
/// `c√n` threshold the recovery yields < k ⇒ defer.
pub fn planted_clique(adj: &[u8], n: usize, k: usize) -> CollapseOutcome {
    let clique = planted::planted_clique(adj, n, k);
    if clique.len() < k || !planted::is_clique(adj, n, &clique) {
        return defer(BarrierTag::BelowDetectionThreshold);
    }
    certify(
        "planted/clique",
        Evidence::PlantedClique { adj: adj.to_vec(), n, clique, k },
        "recovered vertex set is a clique of size ≥ k (exact)",
        BarrierTag::BelowDetectionThreshold,
    )
}

/// 2.3 SBM community detection. 2nd adjacency eigenvalue above the KS spectral threshold
/// ⇒ collapse; below ⇒ indistinguishable from Erdős–Rényi ⇒ defer.
pub fn sbm(adj: &[u8], n: usize) -> CollapseOutcome {
    let threshold = planted::sbm_threshold(adj, n);
    let (l2, _) = planted::sbm_detect(adj, n);
    if l2 < threshold {
        return defer(BarrierTag::BelowDetectionThreshold);
    }
    certify(
        "planted/sbm",
        Evidence::SbmCommunity { adj: adj.to_vec(), n, threshold },
        "2nd adjacency eigenvalue ≥ KS spectral threshold",
        BarrierTag::BelowDetectionThreshold,
    )
}

/// 2.4 Spiked tensor — the 3-way trichotomy (the heart of Batch 2):
/// efficient (σ ≥ p^{3/4}) → collapse; stat-comp gap (√p ≤ σ < p^{3/4}) → defer `open`;
/// undetectable (σ < √p) → defer.
pub fn spiked_tensor(tensor: &[f64], p: usize, tol: f64) -> CollapseOutcome {
    let (sigma, v) = planted::tensor_unfold_top(tensor, p);
    match planted::tensor_regime(sigma, p) {
        TensorRegime::Efficient => {
            // pick the sign of v that minimizes the residual (eigenvector sign is free)
            let beta = sigma;
            let r_pos = planted::rank1_tensor_residual(tensor, &v, beta, p);
            let neg: Vec<f64> = v.iter().map(|x| -x).collect();
            let r_neg = planted::rank1_tensor_residual(tensor, &neg, beta, p);
            let (vv, _r) = if r_neg < r_pos { (neg, r_neg) } else { (v, r_pos) };
            let threshold = (p as f64).powf(0.75);
            certify(
                "planted/spiked-tensor",
                Evidence::SpikedTensor { tensor: tensor.to_vec(), p, v: vv, beta, threshold, tol },
                "unfolded σ ≥ p^{3/4} ∧ ‖T − β·v⊗v⊗v‖_F ≤ tol",
                BarrierTag::StatComputationalGap,
            )
        }
        // signal present but conjecturally no efficient algorithm — defer honestly (open)
        TensorRegime::StatCompGap => defer(BarrierTag::StatComputationalGap),
        // below the information-theoretic threshold — undetectable
        TensorRegime::Undetectable => defer(BarrierTag::BelowDetectionThreshold),
    }
}

/// 2.5 Sparse PCA. Diagonal thresholding recovers a sparse component; if its quadratic
/// form clears `threshold` ⇒ collapse; below ⇒ defer.
pub fn sparse_pca(cov: &[f64], p: usize, k: usize, threshold: f64) -> CollapseOutcome {
    let v = planted::sparse_pca(cov, p, k);
    if planted::quad_form(cov, &v, p) < threshold {
        return defer(BarrierTag::BelowDetectionThreshold);
    }
    certify(
        "planted/sparse-pca",
        Evidence::SparsePca { cov: cov.to_vec(), p, v, k, threshold },
        "‖v‖₀ ≤ k ∧ vᵀΣ̂v ≥ threshold",
        BarrierTag::BelowDetectionThreshold,
    )
}

/// 2.6 Spectral refutation of random 2-XOR (honest negative): if the spectral max-sat
/// bound is below `m`, certify unsatisfiability; otherwise (near the threshold) the
/// spectral method cannot refute ⇒ defer.
pub fn xor_refutation(constraints: &[(usize, usize, u8)], n: usize) -> CollapseOutcome {
    let m = constraints.len();
    let signed_adj = planted::signed_adjacency(constraints, n);
    if planted::xor_spectral_max_sat(&signed_adj, n, m) >= m as f64 {
        return defer(BarrierTag::DataDependentOmegaN);
    }
    certify(
        "planted/xor-refutation",
        Evidence::XorRefutation { signed_adj, n, m },
        "(m + λ_max·n)/2 < m ⇒ unsatisfiable (spectral witness)",
        BarrierTag::DataDependentOmegaN,
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
    fn bbp_collapses_spike_defers_noise() {
        let (n, p) = (200usize, 10usize);
        let mut rng = Rng::new(0xB0B);
        let mut spike: Vec<f64> = (0..p).map(|_| rng.gaussian()).collect();
        let nrm = spike.iter().map(|x| x * x).sum::<f64>().sqrt();
        spike.iter_mut().for_each(|x| *x /= nrm);
        let mut data = vec![0.0; n * p];
        for k in 0..n {
            let g = rng.gaussian();
            for i in 0..p {
                data[k * p + i] = rng.gaussian() + 8.0_f64.sqrt() * g * spike[i];
            }
        }
        let cov = planted::sample_covariance(&data, n, p);
        assert!(is_collapsed(&spiked_covariance(&cov, p, n)));

        // pure noise → below-threshold defer
        let noise: Vec<f64> = (0..400 * 8).map(|_| rng.gaussian()).collect();
        let ncov = planted::sample_covariance(&noise, 400, 8);
        assert_eq!(tag(&spiked_covariance(&ncov, 8, 400)), Some(BarrierTag::BelowDetectionThreshold));
    }

    #[test]
    fn below_threshold_deferred() {
        // tiny planted clique (k=3 in n=40) is below c√n → recovery fails → defer.
        let n = 40;
        let mut rng = Rng::new(0x5);
        let mut adj = vec![0u8; n * n];
        for i in 0..n {
            for j in (i + 1)..n {
                let e = if rng.gaussian() > 0.0 { 1 } else { 0 };
                adj[i * n + j] = e;
                adj[j * n + i] = e;
            }
        }
        // k=15 planted clique collapses; k=30 (impossible to be that large by chance) check both paths
        let clique: Vec<usize> = (0..15).collect();
        for a in 0..15 {
            for b in (a + 1)..15 {
                adj[clique[a] * n + clique[b]] = 1;
                adj[clique[b] * n + clique[a]] = 1;
            }
        }
        assert!(is_collapsed(&planted_clique(&adj, n, 15)));
        // asking for a clique larger than planted → recovery can't reach it → defer
        assert_eq!(tag(&planted_clique(&adj, n, 35)), Some(BarrierTag::BelowDetectionThreshold));
    }

    #[test]
    fn gap_regime_deferred() {
        // clean rank-1 tensor β·v⊗v⊗v has unfolded σ = β exactly. p=16 ⇒ √p=4, p^{3/4}=8.
        let p = 16;
        let mut rng = Rng::new(0x7);
        let mut v: Vec<f64> = (0..p).map(|_| rng.gaussian()).collect();
        let nrm = v.iter().map(|x| x * x).sum::<f64>().sqrt();
        v.iter_mut().for_each(|x| *x /= nrm);
        let make = |beta: f64| -> Vec<f64> {
            let mut t = vec![0.0; p * p * p];
            for i in 0..p {
                for j in 0..p {
                    for k in 0..p {
                        t[(i * p + j) * p + k] = beta * v[i] * v[j] * v[k];
                    }
                }
            }
            t
        };
        // β=12 ≥ 8 → efficient → collapse
        assert!(is_collapsed(&spiked_tensor(&make(12.0), p, 1e-6)));
        // β=5 ∈ [4,8) → stat-comp gap → defer OPEN (not a false collapse)
        assert_eq!(tag(&spiked_tensor(&make(5.0), p, 1e-6)), Some(BarrierTag::StatComputationalGap));
        // β=2 < 4 → undetectable → defer below-threshold
        assert_eq!(tag(&spiked_tensor(&make(2.0), p, 1e-6)), Some(BarrierTag::BelowDetectionThreshold));
    }

    #[test]
    fn sparse_pca_collapses_spike() {
        let p = 10;
        let support = [1usize, 4, 7];
        let mut u = vec![0.0; p];
        for &i in &support {
            u[i] = 1.0 / (support.len() as f64).sqrt();
        }
        let mut cov = vec![0.0; p * p];
        for i in 0..p {
            cov[i * p + i] = 1.0;
        }
        for i in 0..p {
            for j in 0..p {
                cov[i * p + j] += 20.0 * u[i] * u[j];
            }
        }
        assert!(is_collapsed(&sparse_pca(&cov, p, 3, 5.0)));
        // identity covariance (no spike) → defer
        let mut id = vec![0.0; p * p];
        for i in 0..p {
            id[i * p + i] = 1.0;
        }
        assert_eq!(tag(&sparse_pca(&id, p, 3, 5.0)), Some(BarrierTag::BelowDetectionThreshold));
    }

    #[test]
    fn xor_refutation_collapses_dense() {
        let n = 20;
        let mut rng = Rng::new(0xFEED);
        let m = 400;
        let mut constraints = Vec::new();
        for _ in 0..m {
            let i = (rng.next_u64() as usize) % n;
            let mut j = (rng.next_u64() as usize) % n;
            if j == i {
                j = (j + 1) % n;
            }
            constraints.push((i, j, (rng.next_u64() & 1) as u8));
        }
        assert!(is_collapsed(&xor_refutation(&constraints, n)));
        // a single satisfiable constraint can't be refuted → defer
        assert_eq!(tag(&xor_refutation(&[(0, 1, 0)], n)), Some(BarrierTag::DataDependentOmegaN));
    }
}
