//! Stage-6A Batch-5 collapsers: streaming / sublinear sketches. Estimate in small space,
//! certify accuracy against the exact oracle; defer when the structure is absent (flat
//! distribution), the moment is too high (space lower bound), or an exact answer is
//! required (Ω(N)). Honest framing: breadth, not per-pipeline speedup (PART A).

use jeff_cert::{
    BarrierTag, Certificate, CollapseOutcome, Collapsed, Defer, Evidence, IrRef, Obligation,
};
use jeff_math::streaming;
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

/// 5.1 Frequency moment. F₂ collapses (small space); F_k for k≥6 hits the AMS space
/// lower bound (n^{Ω(1)}) ⇒ refuse the small-space promise.
pub fn frequency_moment(items: &[u64], k: usize, lambda: f64) -> CollapseOutcome {
    if k != 2 {
        // only F₂ is cleanly small-space here; high moments hit the space floor.
        return defer(BarrierTag::DataDependentOmegaN);
    }
    let estimate = streaming::ams_f2(items, 51);
    certify(
        "streaming/ams-f2",
        Evidence::StreamF2 { items: items.to_vec(), estimate, lambda },
        "|F̂₂ − F₂| ≤ lambda·F₂",
        BarrierTag::DataDependentOmegaN,
    )
}

/// 5.2 Count-Min point query. Informative only when the estimate clears the `ε‖f‖₁`
/// error floor (the key is a heavy hitter); on a flat distribution it is swamped ⇒ defer.
pub fn count_min_query(items: &[u64], key: u64, eps: f64) -> CollapseOutcome {
    let w = (2.0 / eps).ceil() as usize;
    let estimate = streaming::count_min(items, key, w, 5);
    if (estimate as f64) <= eps * items.len() as f64 {
        return defer(BarrierTag::FlatSpectrum); // estimate dominated by error
    }
    certify(
        "streaming/count-min",
        Evidence::CountMinQuery { items: items.to_vec(), key, estimate, eps },
        "exact ≤ estimate ≤ exact + ε‖f‖₁",
        BarrierTag::FlatSpectrum,
    )
}

/// 5.3 Distinct count (HLL). Always collapses with the accuracy cert; a *membership*
/// query (which item appeared) needs identity, not count ⇒ Ω(N) defer.
pub fn distinct_count(items: &[u64], b: u32, rel_err: f64, membership: bool) -> CollapseOutcome {
    if membership {
        return defer(BarrierTag::DataDependentOmegaN); // identity, not a count
    }
    let estimate = streaming::hyperloglog(items, b);
    certify(
        "streaming/hyperloglog",
        Evidence::DistinctCount { items: items.to_vec(), estimate, rel_err },
        "|distinct̂ − distinct| ≤ rel_err·distinct",
        BarrierTag::DataDependentOmegaN,
    )
}

/// 5.4 Heavy hitters. Misra–Gries candidates filtered to genuine `≥ φn` items; if none
/// exist (flat distribution) ⇒ defer.
pub fn heavy_hitters(items: &[u64], k: usize, phi: f64) -> CollapseOutcome {
    let n = items.len() as f64;
    let candidates = streaming::misra_gries(items, k);
    let hitters: Vec<u64> = candidates
        .into_iter()
        .filter(|&h| streaming::exact_freq(items, h) as f64 >= phi * n)
        .collect();
    if hitters.is_empty() {
        return defer(BarrierTag::NonSparse); // no heavy hitter ⇒ flat
    }
    certify(
        "streaming/heavy-hitters",
        Evidence::HeavyHitters { items: items.to_vec(), hitters, phi },
        "each reported item has frequency ≥ φn",
        BarrierTag::NonSparse,
    )
}

/// 5.5 Sublinear mean. Estimate from random samples with an additive `λ` cert; an exact
/// answer still costs Ω(N) ⇒ defer when `exact_required`.
pub fn sublinear_mean(values: &[f64], s: usize, lambda: f64, exact_required: bool) -> CollapseOutcome {
    if exact_required {
        return defer(BarrierTag::DataDependentOmegaN); // exact mean is Ω(N)
    }
    let estimate = streaming::sublinear_mean(values, s, 0xB0FF1E);
    certify(
        "streaming/sublinear-mean",
        Evidence::SublinearMean { values: values.to_vec(), estimate, lambda },
        "|μ̂ − μ| ≤ lambda",
        BarrierTag::DataDependentOmegaN,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn is_collapsed(o: &CollapseOutcome) -> bool {
        matches!(o, CollapseOutcome::Collapsed(_))
    }
    fn tag(o: &CollapseOutcome) -> Option<BarrierTag> {
        match o {
            CollapseOutcome::Defer(d) => Some(d.tag),
            _ => None,
        }
    }

    fn skewed() -> Vec<u64> {
        let mut v = vec![7u64; 500];
        for i in 0..200 {
            v.push((i % 50) as u64 + 100);
        }
        v
    }

    #[test]
    fn f2_collapses_high_moment_refused() {
        let s = skewed();
        assert!(is_collapsed(&frequency_moment(&s, 2, 0.5)));
        // high_moment_refused: F₆ hits the space lower bound → defer.
        assert_eq!(tag(&frequency_moment(&s, 6, 0.5)), Some(BarrierTag::DataDependentOmegaN));
    }

    #[test]
    fn count_min_collapses_heavy_flat_stream_deferred() {
        let s = skewed();
        assert!(is_collapsed(&count_min_query(&s, 7, 0.01)));
        // flat_stream_deferred: a uniform stream has no key above ε‖f‖₁ → defer.
        let flat: Vec<u64> = (0..2000).map(|i| i as u64).collect();
        assert_eq!(tag(&count_min_query(&flat, 5, 0.05)), Some(BarrierTag::FlatSpectrum));
    }

    #[test]
    fn distinct_count_collapses_membership_deferred() {
        let s: Vec<u64> = (0..5000).collect();
        assert!(is_collapsed(&distinct_count(&s, 10, 0.1, false)));
        assert_eq!(tag(&distinct_count(&s, 10, 0.1, true)), Some(BarrierTag::DataDependentOmegaN));
    }

    #[test]
    fn heavy_hitters_collapse_else_defer() {
        let s = skewed();
        assert!(is_collapsed(&heavy_hitters(&s, 8, 0.3)));
        // flat distribution → no heavy hitter → defer.
        let flat: Vec<u64> = (0..2000).map(|i| i as u64).collect();
        assert_eq!(tag(&heavy_hitters(&flat, 8, 0.1)), Some(BarrierTag::NonSparse));
    }

    #[test]
    fn exact_requested_falls_back() {
        let v: Vec<f64> = (0..10000).map(|i| (i % 7) as f64).collect();
        assert!(is_collapsed(&sublinear_mean(&v, 1000, 0.5, false)));
        // exact answer required → Ω(N) → defer.
        assert_eq!(tag(&sublinear_mean(&v, 1000, 0.5, true)), Some(BarrierTag::DataDependentOmegaN));
    }
}
