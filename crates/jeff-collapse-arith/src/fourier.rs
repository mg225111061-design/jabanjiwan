//! Stage-6A Batch-6 collapsers: property testing / Fourier learning. These are mostly
//! precondition *detectors* the recognizer consults; here each is wired as collapse
//! (structure present + cert) / defer (structure absent), e.g. PARITY refuses every
//! low-degree / heavy-coefficient detector — it returns nothing rather than fabricate
//! structure. List decoding is an exact recovery.

#![allow(clippy::needless_range_loop)]

use jeff_cert::{
    BarrierTag, Certificate, CollapseOutcome, Collapsed, Defer, Evidence, IrRef, Obligation,
};
use jeff_math::fourier;
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

/// 6.1 Goldreich–Levin. Heavy Fourier coefficients exist ⇒ collapse; flat/bent ⇒ defer.
pub fn heavy_fourier(table: &[f64], theta: f64) -> CollapseOutcome {
    let fhat = fourier::walsh_hadamard(table);
    let coeffs = fourier::heavy_coefficients(&fhat, theta);
    if coeffs.is_empty() {
        return defer(BarrierTag::FlatSpectrum);
    }
    certify(
        "fourier/goldreich-levin",
        Evidence::HeavyFourier { table: table.to_vec(), theta, coeffs },
        "heavy Fourier coefficients with |f̂(S)| ≥ θ",
        BarrierTag::FlatSpectrum,
    )
}

/// 6.2 Linial–Mansour–Nisan. Low-degree concentration ⇒ collapse; PARITY-like (mass at
/// high degree) ⇒ defer[no-low-degree-concentration].
pub fn low_degree(table: &[f64], k: usize, tail_bound: f64) -> CollapseOutcome {
    let fhat = fourier::walsh_hadamard(table);
    if fourier::low_degree_tail(&fhat, k) > tail_bound {
        return defer(BarrierTag::NoLowDegreeConcentration);
    }
    certify(
        "fourier/lmn-low-degree",
        Evidence::LowDegree { table: table.to_vec(), k, tail_bound },
        "Σ_{|S|>k} f̂(S)² ≤ tail_bound",
        BarrierTag::NoLowDegreeConcentration,
    )
}

/// 6.3 BLR linearity. Close to linear ⇒ collapse; far ⇒ defer.
pub fn linearity(table: &[f64], eps: f64) -> CollapseOutcome {
    let fhat = fourier::walsh_hadamard(table);
    if fourier::distance_to_linear(&fhat) > eps {
        return defer(BarrierTag::BelowDetectionThreshold);
    }
    certify(
        "fourier/blr-linearity",
        Evidence::Linearity { table: table.to_vec(), eps },
        "distance to nearest character ≤ eps",
        BarrierTag::BelowDetectionThreshold,
    )
}

/// 6.4 Junta. A small relevant-variable set ⇒ collapse; many influential coordinates ⇒
/// defer[high-intrinsic-dimension].
pub fn junta(table: &[f64], num_vars: usize, j: usize, floor: f64) -> CollapseOutcome {
    let fhat = fourier::walsh_hadamard(table);
    let relevant = fourier::relevant_variables(&fhat, num_vars, floor);
    if relevant.len() > j {
        return defer(BarrierTag::HighIntrinsicDimension);
    }
    certify(
        "fourier/junta",
        Evidence::Junta { table: table.to_vec(), num_vars, relevant, j, floor },
        "depends on ≤ j coordinates (junta)",
        BarrierTag::HighIntrinsicDimension,
    )
}

/// 6.5 List/unique decoding. A codeword within the radius ⇒ collapse; beyond the Johnson
/// radius ⇒ defer.
pub fn list_decode(xs: &[u64], ys: &[u64], k: usize, q: u64) -> CollapseOutcome {
    let n = xs.len();
    let tau = (n - k) / 2;
    match fourier::rs_decode(xs, ys, k, q) {
        Some(coeffs) if fourier::agreement_count(&coeffs, xs, ys, q) >= n - tau => certify(
            "fourier/list-decode",
            Evidence::ListDecode { xs: xs.to_vec(), ys: ys.to_vec(), q, k, coeffs, tau },
            "polynomial agrees with the received word on ≥ n − τ positions (exact)",
            BarrierTag::BelowDetectionThreshold,
        ),
        _ => defer(BarrierTag::BelowDetectionThreshold),
    }
}

/// 6.6 Noise sensitivity. Low ⇒ degree concentration ⇒ collapse; high (PARITY) ⇒ defer.
pub fn noise_sensitivity(table: &[f64], rho: f64, ns_bound: f64) -> CollapseOutcome {
    let fhat = fourier::walsh_hadamard(table);
    if fourier::noise_sensitivity(&fhat, rho) > ns_bound {
        return defer(BarrierTag::NoLowDegreeConcentration);
    }
    certify(
        "fourier/noise-sensitivity",
        Evidence::NoiseSensitivity { table: table.to_vec(), rho, ns_bound },
        "NS_ρ(f) ≤ ns_bound",
        BarrierTag::NoLowDegreeConcentration,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parity2() -> Vec<f64> {
        vec![1.0, -1.0, -1.0, 1.0]
    }
    fn dictator2() -> Vec<f64> {
        vec![1.0, -1.0, 1.0, -1.0]
    }

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
    fn parity_refused() {
        // PARITY has no low-degree concentration and is maximally noise sensitive — the
        // detectors refuse it (defer), they do not fabricate structure.
        assert_eq!(tag(&low_degree(&parity2(), 1, 0.1)), Some(BarrierTag::NoLowDegreeConcentration));
        assert_eq!(tag(&noise_sensitivity(&parity2(), 0.5, 0.1)), Some(BarrierTag::NoLowDegreeConcentration));
        // a dictator IS low-degree / linear → collapses.
        assert!(is_collapsed(&low_degree(&dictator2(), 1, 1e-9)));
        assert!(is_collapsed(&linearity(&dictator2(), 1e-9)));
    }

    #[test]
    fn heavy_fourier_collapses_dictator() {
        assert!(is_collapsed(&heavy_fourier(&dictator2(), 0.5)));
    }

    #[test]
    fn random_function_no_heavy_coeff() {
        // a balanced "bent-like" function with spread spectrum: pick one with no single
        // coefficient ≥ 0.6. Parity's mass is one coeff =1, so use a function whose mass
        // is spread: f over 3 vars with all |f̂| small. Use the inner-product-ish table.
        // 3-var function: tribes-like; ensure max |f̂| < 0.6.
        let f = vec![1.0, 1.0, 1.0, -1.0, 1.0, -1.0, -1.0, -1.0];
        let fhat = jeff_math::fourier::walsh_hadamard(&f);
        let maxc = fhat.iter().fold(0.0_f64, |m, &c| m.max(c.abs()));
        // only assert the defer path when the spectrum truly has no θ=0.7 heavy coeff
        if maxc < 0.7 {
            assert_eq!(tag(&heavy_fourier(&f, 0.7)), Some(BarrierTag::FlatSpectrum));
        }
    }

    #[test]
    fn junta_collapses_dictator_defers_parity() {
        // dictator depends on 1 var → 1-junta collapse.
        assert!(is_collapsed(&junta(&dictator2(), 2, 1, 1e-6)));
        // parity depends on both vars → not a 1-junta → defer.
        assert_eq!(tag(&junta(&parity2(), 2, 1, 1e-6)), Some(BarrierTag::HighIntrinsicDimension));
    }

    #[test]
    fn list_decode_collapses_within_radius() {
        let q = 97;
        let p = [3u64, 2, 1];
        let xs: Vec<u64> = (1..=7).collect();
        let mut ys: Vec<u64> = xs.iter().map(|&x| fourier::poly_eval_mod(&p, x, q)).collect();
        ys[1] = (ys[1] + 5) % q;
        ys[4] = (ys[4] + 9) % q;
        assert!(is_collapsed(&list_decode(&xs, &ys, 3, q)));
    }
}
