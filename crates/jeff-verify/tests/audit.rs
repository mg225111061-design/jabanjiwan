//! Stage 9: autonomous integrity audit (CLAUDE.md PART F). A single systematic sweep
//! that proves the verifier is a real ARBITER, not a rubber stamp: for each evidence
//! family, a TRUE certificate must verify and a FALSE one must be rejected. If any
//! variant silently fell through to `Unknown`, its true cert would fail to verify — so
//! this sweep also proves every variant is wired to a real checker. Plus a determinism
//! check (R11): `verify` is pure, same input → same verdict.
//!
//! Per-stage tests already cover specifics; this consolidates the honesty invariant
//! ("the verifier is the arbiter; defer is success; never a wrong answer") in one place.

use jeff_cert::{Certificate, Evidence, IrRef, Obligation};
use jeff_math::fourier::poly_eval_mod;
use jeff_math::planted::signed_adjacency;
use jeff_span::Span;
use jeff_verify::verify;

fn c(ev: Evidence) -> Certificate {
    let s = IrRef::new(0, Span::dummy());
    Certificate {
        collapser_id: "audit".into(),
        source: s,
        collapsed: IrRef::new(1, Span::dummy()),
        obligation: Obligation::new("audit".to_string()),
        evidence: ev,
        boundaries: vec![],
        fallback: s,
    }
}

/// Assert the arbiter property for one family: true verifies, false is rejected.
fn arbiter(name: &str, truth: Evidence, lie: Evidence) {
    assert!(verify(c(truth)).is_some(), "[{name}] a TRUE certificate must verify (checker wired?)");
    assert!(verify(c(lie)).is_none(), "[{name}] a FALSE certificate must be REJECTED");
}

#[test]
fn arbiter_sweep_planted_and_structure() {
    // XOR refutation: dense random unsat (bound bites) vs a single satisfiable constraint.
    let n = 16;
    let mut cons = Vec::new();
    let mut seed = 1u64;
    for _ in 0..300 {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let i = (seed >> 33) as usize % n;
        let mut j = (seed >> 17) as usize % n;
        if j == i {
            j = (j + 1) % n;
        }
        cons.push((i, j, (seed & 1) as u8));
    }
    let sa = signed_adjacency(&cons, n);
    arbiter(
        "XorRefutation",
        Evidence::XorRefutation { signed_adj: sa, n, m: 300 },
        Evidence::XorRefutation { signed_adj: signed_adjacency(&[(0, 1, 0)], n), n, m: 1 },
    );

    // planted clique: a real triangle vs a non-clique claim.
    let tri = vec![0u8, 1, 1, 1, 0, 1, 1, 1, 0];
    let nontri = vec![0u8, 1, 0, 1, 0, 1, 0, 1, 0];
    arbiter(
        "PlantedClique",
        Evidence::PlantedClique { adj: tri, n: 3, clique: vec![0, 1, 2], k: 3 },
        Evidence::PlantedClique { adj: nontri, n: 3, clique: vec![0, 1, 2], k: 3 },
    );
}

#[test]
fn arbiter_sweep_exact_layers() {
    // tensor contraction (triangle): 91 true, 112 false.
    let tensors = vec![
        (vec![0usize, 1], vec![1i64, 2, 3, 4]),
        (vec![1usize, 2], vec![5i64, 6, 7, 8]),
        (vec![2usize, 0], vec![1i64, 0, 1, 1]),
    ];
    arbiter(
        "TensorContraction",
        Evidence::TensorContraction { tensors: tensors.clone(), dim: 2, n_indices: 3, result: 91 },
        Evidence::TensorContraction { tensors, dim: 2, n_indices: 3, result: 112 },
    );

    // FKT planar matchings (C4): 2 true, 7 false.
    let edges = vec![(0, 1), (1, 2), (2, 3), (3, 0)];
    arbiter(
        "PlanarMatchings",
        Evidence::PlanarMatchings { n: 4, edges: edges.clone(), count: 2 },
        Evidence::PlanarMatchings { n: 4, edges, count: 7 },
    );

    // Reed–Solomon list decode: the true codeword vs a wrong polynomial.
    let q = 97;
    let p = vec![3u64, 2, 1];
    let xs: Vec<u64> = (1..=7).collect();
    let ys: Vec<u64> = xs.iter().map(|&x| poly_eval_mod(&p, x, q)).collect();
    arbiter(
        "ListDecode",
        Evidence::ListDecode { xs: xs.clone(), ys: ys.clone(), q, k: 3, coeffs: p, tau: 2 },
        Evidence::ListDecode { xs, ys, q, k: 3, coeffs: vec![0, 0, 0], tau: 2 },
    );
}

#[test]
fn arbiter_sweep_fourier_detectors() {
    let dictator = vec![1.0, -1.0, 1.0, -1.0]; // degree-1, linear
    let parity = vec![1.0, -1.0, -1.0, 1.0]; // degree-2, not low-degree
    arbiter(
        "LowDegree",
        Evidence::LowDegree { table: dictator.clone(), k: 1, tail_bound: 1e-9 },
        Evidence::LowDegree { table: parity.clone(), k: 1, tail_bound: 0.1 },
    );
    arbiter(
        "Linearity",
        Evidence::Linearity { table: dictator.clone(), eps: 1e-9 },
        Evidence::Linearity { table: vec![1.0, 1.0, 1.0, -1.0], eps: 1e-9 }, // AND-ish: far
    );
    arbiter(
        "NoiseSensitivity",
        Evidence::NoiseSensitivity { table: dictator, rho: 0.5, ns_bound: 0.4 },
        Evidence::NoiseSensitivity { table: parity, rho: 0.5, ns_bound: 0.1 },
    );
}

#[test]
fn arbiter_sweep_streaming() {
    let items: Vec<u64> = {
        let mut v = vec![7u64; 300];
        v.extend((0..100).map(|i| i as u64 + 50));
        v
    };
    let exact_f2 = jeff_math::streaming::exact_f2(&items) as f64;
    arbiter(
        "StreamF2",
        Evidence::StreamF2 { items: items.clone(), estimate: exact_f2, lambda: 0.05 },
        Evidence::StreamF2 { items: items.clone(), estimate: 1.0, lambda: 0.05 },
    );
    arbiter(
        "HeavyHitters",
        Evidence::HeavyHitters { items: items.clone(), hitters: vec![7], phi: 0.3 },
        Evidence::HeavyHitters { items, hitters: vec![50], phi: 0.3 },
    );
}

#[test]
fn arbiter_sweep_geometry() {
    let pts = vec![0.0, 0.0, 0.1, 0.0, 10.0, 10.0, 10.1, 10.0];
    let dist = jeff_math::geometry::pairwise_distances(&pts, 4, 2);
    arbiter(
        "PersistentHomology",
        Evidence::PersistentHomology { dist: dist.clone(), n: 4, band: 1.0, feature_count: 2 },
        Evidence::PersistentHomology { dist, n: 4, band: 1.0, feature_count: 9 },
    );
}

#[test]
fn determinism_verify_is_pure() {
    // R11: the same certificate verifies identically across runs.
    let edges = vec![(0, 1), (1, 2), (2, 3), (3, 0)];
    let ev = Evidence::PlanarMatchings { n: 4, edges, count: 2 };
    let r1 = verify(c(ev.clone())).is_some();
    let r2 = verify(c(ev.clone())).is_some();
    let r3 = verify(c(ev)).is_some();
    assert!(r1 && r2 && r3, "verify must be deterministic and true here");
}

#[test]
fn exact_certs_have_no_tolerance_slack() {
    // an Exact-class cert must reject ANY perturbation (off-by-one), unlike eps certs.
    let tensors = vec![(vec![0usize], vec![3i64, 4])]; // 1-tensor, sum over index 0 = 7
    // contract_naive of a single open... use a closed scalar tensor instead:
    let scalar = vec![(vec![], vec![42i64])];
    assert!(verify(c(Evidence::TensorContraction {
        tensors: scalar.clone(),
        dim: 2,
        n_indices: 0,
        result: 42
    }))
    .is_some());
    assert!(verify(c(Evidence::TensorContraction {
        tensors: scalar,
        dim: 2,
        n_indices: 0,
        result: 43 // off by one ⇒ exact reject
    }))
    .is_none());
    let _ = tensors;
}
