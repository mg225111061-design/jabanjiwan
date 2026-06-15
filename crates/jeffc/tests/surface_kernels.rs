//! Stage 11 regression: stdlib kernels are callable from `.jeff` source and the collapse
//! auto-triggers at compile time. Asserts each kernel function's status (collapse with
//! the real certificate class, or honest defer). The certificate class is read from the
//! verified certificate — never overclaimed (DR2).

use jeff_cert::CertClass;
use jeffc::kernels::KernelStatus;
use jeffc::{compile, Options};

const SRC: &str = include_str!("../../../tests/e2e/kernels.jeff");

fn art() -> jeffc::Artifact {
    compile(SRC, &Options::default()).expect("compiles")
}

fn status(name: &str) -> KernelStatus {
    art().kernel(name).unwrap_or_else(|| panic!("kernel fn {name} not found")).status.clone()
}

#[test]
fn all_six_kernels_are_surface_wired() {
    // every demo function must resolve to a kernel (the call was recognized & dispatched).
    let a = art();
    for name in [
        "prony_demo",
        "sparse_fft_demo",
        "welch_etf_demo",
        "planted_clique_demo",
        "persistent_homology_demo",
        "list_decode_demo",
    ] {
        assert!(a.kernel(name).is_some(), "{name} should be surface-wired");
    }
}

#[test]
fn exact_cert_kernels_collapse_with_exact_class() {
    // these carry EXACT certificates (the S-tier candidates).
    for name in ["welch_etf_demo", "planted_clique_demo", "persistent_homology_demo", "list_decode_demo"] {
        match status(name) {
            KernelStatus::Collapsed { cert_class, .. } => {
                assert_eq!(cert_class, CertClass::Exact, "{name} must be exact");
            }
            other => panic!("{name} should collapse, got {other:?}"),
        }
    }
}

#[test]
fn prony_collapses_clean_defers_noise() {
    // clean two-mode signal: exact recurrence (tol 1e-9).
    match status("prony_demo") {
        KernelStatus::Collapsed { cert_class, .. } => assert_eq!(cert_class, CertClass::Exact),
        other => panic!("prony_demo should collapse, got {other:?}"),
    }
    // white noise: no low-order recurrence → honest defer (not a fake collapse).
    assert!(matches!(status("prony_noise"), KernelStatus::Deferred { .. }));
}

#[test]
fn sparse_fft_collapses() {
    // a constant signal is 1-sparse in frequency.
    assert!(matches!(status("sparse_fft_demo"), KernelStatus::Collapsed { .. }));
}

#[test]
fn list_decode_recovers_within_radius() {
    match status("list_decode_demo") {
        KernelStatus::Collapsed { checker, cert_class } => {
            assert_eq!(cert_class, CertClass::Exact);
            assert_eq!(checker, "rs-agreement-exact");
        }
        other => panic!("list_decode should collapse, got {other:?}"),
    }
}

#[test]
fn planted_clique_is_exact_clique() {
    match status("planted_clique_demo") {
        KernelStatus::Collapsed { checker, .. } => assert_eq!(checker, "clique-exact"),
        other => panic!("planted_clique should collapse, got {other:?}"),
    }
}

#[test]
fn streaming_and_mixture_kernels_wired() {
    // broader surface coverage: streaming sketches + method-of-moments mixture.
    assert!(matches!(status("frequency_moment_demo"), KernelStatus::Collapsed { .. }));
    assert!(matches!(status("heavy_hitters_demo"), KernelStatus::Collapsed { .. }));
    assert!(matches!(status("point_mass_mixture_demo"), KernelStatus::Collapsed { .. }));
}
