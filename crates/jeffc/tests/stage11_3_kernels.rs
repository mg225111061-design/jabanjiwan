//! Stage 11.3 regression (master directive PART D): the boolean-function analysis and
//! small-matrix latent-structure kernels are callable from `.jeff`, and the collapse
//! auto-triggers with the real certificate class — or HONESTLY DEFERS with a named
//! barrier when the structure is absent (P1/P3/R4). Inputs match the kernels' own unit
//! tests, so the surface behavior is the proven library behavior.

use jeff_cert::BarrierTag;
use jeffc::kernels::KernelStatus;
use jeffc::{compile, Options};

const SRC: &str = include_str!("../../../tests/e2e/analysis_kernels.jeff");

fn art() -> jeffc::Artifact {
    compile(SRC, &Options::default()).expect("compiles")
}

fn status(name: &str) -> KernelStatus {
    art()
        .kernel(name)
        .unwrap_or_else(|| panic!("kernel fn {name} not surface-wired"))
        .status
        .clone()
}

fn is_collapsed(name: &str) -> bool {
    matches!(status(name), KernelStatus::Collapsed { .. })
}

fn defer_tag(name: &str) -> BarrierTag {
    match status(name) {
        KernelStatus::Deferred { tag } => tag,
        other => panic!("{name} should defer, got {other:?}"),
    }
}

#[test]
fn structured_boolean_functions_collapse() {
    assert!(is_collapsed("low_degree_dictator"));
    assert!(is_collapsed("linearity_dictator"));
    assert!(is_collapsed("heavy_fourier_dictator"));
    assert!(is_collapsed("junta_dictator"));
}

#[test]
fn unstructured_boolean_functions_defer_with_named_barrier() {
    // PARITY: no low-degree concentration; not a 1-junta; maximally noise-sensitive.
    assert_eq!(defer_tag("low_degree_parity"), BarrierTag::NoLowDegreeConcentration);
    assert_eq!(defer_tag("junta_parity"), BarrierTag::HighIntrinsicDimension);
    assert_eq!(defer_tag("noise_sensitivity_parity"), BarrierTag::NoLowDegreeConcentration);
}

#[test]
fn spectral_clustering_collapses_two_communities_defers_flat() {
    assert!(is_collapsed("spectral_cluster_two"));
    assert_eq!(defer_tag("spectral_cluster_flat"), BarrierTag::FlatSpectrum);
}

#[test]
fn tensor_decomposition_collapses_orthogonal_cp() {
    assert!(is_collapsed("tensor_decomp_diag"));
}

#[test]
fn all_stage_11_3_kernels_resolve() {
    let a = art();
    for name in [
        "low_degree_dictator",
        "low_degree_parity",
        "linearity_dictator",
        "heavy_fourier_dictator",
        "junta_dictator",
        "junta_parity",
        "noise_sensitivity_parity",
        "spectral_cluster_two",
        "spectral_cluster_flat",
        "tensor_decomp_diag",
    ] {
        assert!(a.kernel(name).is_some(), "{name} must be surface-wired");
    }
}
