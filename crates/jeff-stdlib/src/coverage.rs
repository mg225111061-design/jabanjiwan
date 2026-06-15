//! Stage 22 — MEASURED hidden-structure coverage over target-domain instances.
//!
//! Coverage is **judged by the verifier** (collapsed-with-a-real-certificate vs deferred),
//! never asserted. Two honesty qualifiers are mandatory and restated wherever a number is:
//!   (1) DOMAIN-CONDITIONAL — these gains hold only in the target domains (numeric / signal /
//!       statistical / crypto / ML-preprocessing). On general / control-flow / graph /
//!       full-entropy software the collapsible fraction is ≈ 0. This is NOT a general-purpose
//!       accelerator.
//!   (2) CEILING, NOT GUARANTEE — a kernel's real end-to-end contribution depends on its
//!       certificate class (exact vs probabilistic) and Amdahl `p` (does it dominate
//!       runtime?). The fraction here counts *recognitions*, it does not promise speedup.

use jeff_cert::{CertClass, CollapseOutcome};
use jeff_collapse_arith::{fourier, geometry, moments, planted, sparse, streaming};
use jeff_math::fmat::Rng;

/// Verifier-judged outcome of one corpus instance.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Judged {
    Collapsed(CertClass),
    Deferred,
}

fn judge(o: CollapseOutcome) -> Judged {
    match o {
        CollapseOutcome::Collapsed(c) => {
            Judged::Collapsed(c.certificate().certificate().evidence.cert_class())
        }
        CollapseOutcome::Defer(_) => Judged::Deferred,
    }
}

/// A measured coverage report over a corpus (counts only — no claim).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CoverageReport {
    pub structured_total: usize,
    pub structured_collapsed: usize,
    pub exact: usize,
    pub probabilistic: usize,
    pub unstructured_total: usize,
    pub unstructured_deferred: usize,
}

impl CoverageReport {
    /// Measured collapsible fraction over the *structured* corpus (verifier-judged).
    pub fn collapsible_fraction(&self) -> f64 {
        if self.structured_total == 0 {
            0.0
        } else {
            self.structured_collapsed as f64 / self.structured_total as f64
        }
    }
    /// Measured honest-defer rate over the *unstructured* corpus.
    pub fn defer_rate(&self) -> f64 {
        if self.unstructured_total == 0 {
            0.0
        } else {
            self.unstructured_deferred as f64 / self.unstructured_total as f64
        }
    }
}

fn structured_instances() -> Vec<(&'static str, CollapseOutcome)> {
    let mut rng = Rng::new(0x22_5C0);
    // signal: two clean exponential modes → exact recurrence.
    let prony_sig: Vec<f64> = (0..14).map(|t| 2.0 * 0.9f64.powi(t) + 0.5f64.powi(t)).collect();
    // numeric: a 1-sparse (constant) signal.
    let fft_sig = vec![1.0; 8];
    // signal/frames: Mercedes-Benz ETF (3 unit vectors in R^2).
    let etf = vec![1.0, 0.0, -0.5, 0.8660254037844386, -0.5, -0.8660254037844386];
    // statistical: method-of-moments point masses (m_t = 0.7·2^t + 0.3·5^t).
    let mom: Vec<f64> = (0..6).map(|t| 0.7 * 2f64.powi(t) + 0.3 * 5f64.powi(t)).collect();
    // ML-preproc: orthogonal CP 3-tensor (diag eigenvalues 3,2,1).
    let mut t3 = vec![0.0; 27];
    t3[0] = 3.0;
    t3[13] = 2.0;
    t3[26] = 1.0;
    // ML-preproc: two well-separated clusters → 2 persistent features.
    let dist = vec![0.0, 0.1, 10.0, 10.0, 0.1, 0.0, 10.0, 10.0, 10.0, 10.0, 0.0, 0.1, 10.0, 10.0, 0.1, 0.0];
    // crypto/coding: RS list decode of p(x)=3+2x+x^2 over GF(97), 2 errors.
    let xs: Vec<u64> = (1..=7).collect();
    let ys = vec![6u64, 16, 18, 27, 47, 51, 66];
    // statistical: a BBP spike above threshold.
    let (n, p) = (200usize, 10usize);
    let mut spike: Vec<f64> = (0..p).map(|_| rng.gaussian()).collect();
    let nrm = spike.iter().map(|x| x * x).sum::<f64>().sqrt();
    spike.iter_mut().for_each(|x| *x /= nrm);
    let mut data = vec![0.0; n * p];
    for kk in 0..n {
        let g = rng.gaussian();
        for i in 0..p {
            data[kk * p + i] = rng.gaussian() + 8.0f64.sqrt() * g * spike[i];
        }
    }
    let cov = jeff_math::planted::sample_covariance(&data, n, p);
    // streaming: skewed multiset (F2 collapses; a heavy hitter exists).
    let mut skew = vec![7u64; 500];
    skew.extend((0..200).map(|i| (i % 50) as u64 + 100));
    // graph: a planted 4-clique.
    let adj4 = vec![
        0u8, 1, 1, 1, 1, 0, 1, 0, 1, 1, 0, 1, 1, 1, 0, 1, 0, 0, 1, 1, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0,
    ];

    vec![
        ("signal/prony", sparse::prony(&prony_sig, 2, 1e-9)),
        ("numeric/sparse-fft", sparse::sparse_fft(&fft_sig, 1, 1e-6)),
        ("signal/welch-etf", sparse::equiangular_tight_frame(&etf, 3, 2, 1e-6)),
        ("statistical/mixture", moments::point_mass_mixture(&mom, 2, 1e-6)),
        ("ml/tensor-decomp", moments::tensor_decomp(&t3, 3, 3, 1e-6)),
        ("ml/persistent-homology", geometry::persistent_homology(&dist, 4, 1.0)),
        ("crypto/list-decode", fourier::list_decode(&xs, &ys, 3, 97)),
        ("statistical/bbp-spike", planted::spiked_covariance(&cov, p, n)),
        ("streaming/F2", streaming::frequency_moment(&skew, 2, 0.5)),
        ("streaming/heavy-hitters", streaming::heavy_hitters(&skew, 8, 0.3)),
        ("graph/planted-clique", planted::planted_clique(&adj4, 6, 4)),
    ]
}

fn unstructured_instances() -> Vec<(&'static str, CollapseOutcome)> {
    let mut rng = Rng::new(0x22_DEF);
    let noise: Vec<f64> = (0..14).map(|_| rng.gaussian()).collect();
    // pure-noise covariance → below BBP threshold.
    let noise2: Vec<f64> = (0..400 * 8).map(|_| rng.gaussian()).collect();
    let ncov = jeff_math::planted::sample_covariance(&noise2, 400, 8);
    let flat: Vec<u64> = (0..2000).map(|i| i as u64).collect();
    let mut skew = vec![7u64; 500];
    skew.extend((0..200).map(|i| (i % 50) as u64 + 100));
    vec![
        ("signal/prony-noise", sparse::prony(&noise, 2, 1e-9)),
        ("statistical/below-bbp", planted::spiked_covariance(&ncov, 8, 400)),
        ("streaming/flat-heavy-hitters", streaming::heavy_hitters(&flat, 8, 0.1)),
        ("streaming/high-moment", streaming::frequency_moment(&skew, 6, 0.5)),
    ]
}

/// Run the target-domain corpus and let the verifier judge each instance.
pub fn measure() -> CoverageReport {
    let mut r = CoverageReport::default();
    for (_name, o) in structured_instances() {
        r.structured_total += 1;
        match judge(o) {
            Judged::Collapsed(c) => {
                r.structured_collapsed += 1;
                if c == CertClass::Exact {
                    r.exact += 1;
                } else {
                    r.probabilistic += 1;
                }
            }
            Judged::Deferred => {}
        }
    }
    for (_name, o) in unstructured_instances() {
        r.unstructured_total += 1;
        if judge(o) == Judged::Deferred {
            r.unstructured_deferred += 1;
        }
    }
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coverage_measured_by_verifier() {
        // the fraction is a MEASUREMENT (verifier-judged), reported with both qualifiers.
        let r = measure();
        assert_eq!(r.structured_total, 11);
        // every structured target-domain instance is recognized + verified here.
        assert_eq!(
            r.structured_collapsed, r.structured_total,
            "measured collapsible fraction = {:.0}% (DOMAIN-CONDITIONAL; CEILING not guarantee)",
            r.collapsible_fraction() * 100.0
        );
        assert_eq!(r.exact + r.probabilistic, r.structured_collapsed);
        assert!(r.exact >= 1 && r.probabilistic >= 1, "tiers read from real certs, not guessed");
    }

    #[test]
    fn kernel_matches_oracle_or_certified() {
        // each structured instance carries a verified certificate (collapse), each
        // unstructured one honestly defers — the verifier is the arbiter.
        for (name, o) in structured_instances() {
            assert!(matches!(judge(o), Judged::Collapsed(_)), "{name} should collapse");
        }
        for (name, o) in unstructured_instances() {
            assert_eq!(judge(o), Judged::Deferred, "{name} should defer");
        }
    }

    #[test]
    fn bbp_below_threshold_defers() {
        // BBP gate: above the spike threshold → collapse; below → HONEST_DEFER.
        let r = measure();
        assert_eq!(r.defer_rate(), 1.0, "all unstructured instances defer");
        // and the labeled below-threshold instance is one of them.
        let below = unstructured_instances()
            .into_iter()
            .find(|(n, _)| *n == "statistical/below-bbp")
            .map(|(_, o)| judge(o));
        assert_eq!(below, Some(Judged::Deferred), "below-BBP must defer");
    }
}
