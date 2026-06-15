//! Layer 4 — tensor-network contraction (CLAUDE.md PART E / Stage 7).
//!
//! Contract a closed tensor network to a scalar via a treewidth-aware (min-width greedy)
//! order. The runtime precondition is the contraction *width* (≈ treewidth, Markov–Shi):
//! within budget → collapse with an **exact** certificate (the result equals the naive
//! full contraction); over budget → `defer[treewidth-blowup]` (intermediate tensors
//! grow exponentially). Correctness never depends on the order — the cert re-checks
//! against the naive oracle (P0).

use jeff_cert::{
    BarrierTag, Certificate, CollapseOutcome, Collapsed, Defer, Evidence, IrRef, Obligation,
};
use jeff_math::tensornet::{contract_ordered, Tensor};
use jeff_span::Span;

/// Attempt to contract a closed network within `budget` contraction width.
pub fn contract(tensors: &[Tensor], dim: usize, n_indices: usize, budget: usize) -> CollapseOutcome {
    let span = Span::dummy();
    let source = IrRef::new(0, span);
    match contract_ordered(tensors, dim, budget) {
        Some((result, _width)) => {
            let tuples: Vec<(Vec<usize>, Vec<i64>)> =
                tensors.iter().map(|t| (t.indices.clone(), t.data.clone())).collect();
            let evidence = Evidence::TensorContraction {
                tensors: tuples,
                dim,
                n_indices,
                result,
            };
            let class = evidence.cert_class();
            let cert = Certificate {
                collapser_id: "tensor/contraction".into(),
                source,
                collapsed: IrRef::new(1, span),
                obligation: Obligation::new(format!(
                    "ordered contraction == naive full contraction [cert-class: {}]",
                    class.as_str()
                )),
                evidence,
                boundaries: vec![],
                fallback: source,
            };
            match jeff_verify::verify(cert) {
                Some(vc) => CollapseOutcome::Collapsed(Collapsed::new(IrRef::new(1, span), vc)),
                None => CollapseOutcome::Defer(Defer::new(BarrierTag::TreewidthBlowup, source)),
            }
        }
        None => CollapseOutcome::Defer(Defer::new(BarrierTag::TreewidthBlowup, source)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tri() -> Vec<Tensor> {
        vec![
            Tensor { indices: vec![0, 1], data: vec![1, 2, 3, 4] },
            Tensor { indices: vec![1, 2], data: vec![5, 6, 7, 8] },
            Tensor { indices: vec![2, 0], data: vec![1, 0, 1, 1] },
        ]
    }

    #[test]
    fn low_treewidth_collapses() {
        assert!(matches!(contract(&tri(), 2, 3, 8), CollapseOutcome::Collapsed(_)));
    }

    #[test]
    fn high_treewidth_defers() {
        // a 4-index tensor pair forced under a width-2 budget → treewidth blow-up.
        let net = vec![
            Tensor { indices: vec![0, 1, 2, 3], data: vec![1; 16] },
            Tensor { indices: vec![0, 1, 2, 3], data: vec![1; 16] },
        ];
        match contract(&net, 2, 4, 2) {
            CollapseOutcome::Defer(d) => assert_eq!(d.tag, BarrierTag::TreewidthBlowup),
            _ => panic!("must defer on treewidth blow-up"),
        }
    }
}
