//! Layer 3 — holographic / FKT (CLAUDE.md PART E / Stage 8).
//!
//! Count perfect matchings of a **planar** graph in polynomial time via a Pfaffian
//! (Kasteleyn) orientation; the certificate is exact (the count equals naive
//! enumeration). Outside the planar class the matchgate method does not apply (Cai–Lu
//! dichotomy — #P-hard) → `defer[non-planar]`. The naive oracle guards correctness, so a
//! mis-oriented or non-planar instance defers rather than ever returning a wrong count.

use jeff_cert::{
    BarrierTag, Certificate, CollapseOutcome, Collapsed, Defer, Evidence, IrRef, Obligation,
};
use jeff_math::holographic::{fkt_count, passes_planarity_bound};
use jeff_span::Span;

/// Count perfect matchings of a planar graph given its inner `faces` (embedding).
/// `bipartite` tightens the planarity bound. Non-planar / un-orientable → defer.
pub fn planar_matchings(
    n: usize,
    edges: &[(usize, usize)],
    faces: &[Vec<usize>],
    bipartite: bool,
) -> CollapseOutcome {
    let span = Span::dummy();
    let source = IrRef::new(0, span);
    if !passes_planarity_bound(n, edges, bipartite) {
        return CollapseOutcome::Defer(Defer::new(BarrierTag::NonPlanar, source));
    }
    let Some(count) = fkt_count(n, edges, faces) else {
        return CollapseOutcome::Defer(Defer::new(BarrierTag::NonPlanar, source));
    };
    let count = count.max(0) as u64;
    let evidence = Evidence::PlanarMatchings { n, edges: edges.to_vec(), count };
    let class = evidence.cert_class();
    let cert = Certificate {
        collapser_id: "holographic/fkt".into(),
        source,
        collapsed: IrRef::new(1, span),
        obligation: Obligation::new(format!(
            "|Pf(A)| == naive perfect-matching count [cert-class: {}]",
            class.as_str()
        )),
        evidence,
        boundaries: vec![],
        fallback: source,
    };
    match jeff_verify::verify(cert) {
        Some(vc) => CollapseOutcome::Collapsed(Collapsed::new(IrRef::new(1, span), vc)),
        None => CollapseOutcome::Defer(Defer::new(BarrierTag::NonPlanar, source)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn planar_graphs_collapse() {
        // C4 (2 matchings)
        let c4 = [(0, 1), (1, 2), (2, 3), (3, 0)];
        assert!(matches!(
            planar_matchings(4, &c4, &[vec![0, 1, 2, 3]], true),
            CollapseOutcome::Collapsed(_)
        ));
        // K4 (3 matchings)
        let k4 = [(0, 1), (1, 2), (2, 0), (0, 3), (1, 3), (2, 3)];
        let faces = vec![vec![0, 1, 3], vec![1, 2, 3], vec![2, 0, 3]];
        assert!(matches!(
            planar_matchings(4, &k4, &faces, false),
            CollapseOutcome::Collapsed(_)
        ));
    }

    #[test]
    fn nonplanar_defers() {
        // K3,3 (bipartite, non-planar) → defer[non-planar]
        let edges: Vec<(usize, usize)> = (0..3).flat_map(|i| (3..6).map(move |j| (i, j))).collect();
        match planar_matchings(6, &edges, &[], true) {
            CollapseOutcome::Defer(d) => assert_eq!(d.tag, BarrierTag::NonPlanar),
            _ => panic!("K3,3 must defer as non-planar"),
        }
    }
}
