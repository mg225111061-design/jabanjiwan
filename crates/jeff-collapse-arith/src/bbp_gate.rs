//! Stage 30.3(b) — the BBP soundness gate wired into the HONEST_DEFER dispatch.
//!
//! A low-rank fold family consults this gate FIRST: significant low-rank structure ⇒ attempt the
//! (separately, exactly-certified) fold; no structure ⇒ `HONEST_DEFER[below-detection-threshold]`
//! carrying the ε-level absence certificate. The gate itself is **not** a collapse and emits no
//! exact certificate — it is the probabilistic precondition detector (kernel #4), mirroring the
//! Fourier precondition detectors in [`crate::fourier`].

use jeff_cert::{BarrierTag, CollapseOutcome, Defer, IrRef};
use jeff_math::bbp::{self, GateOutcome, EPS_MODEL_NOTE};
use jeff_span::Span;

/// The dispatch decision for a gated low-rank fold.
#[derive(Clone, Debug)]
pub enum LowRankGate {
    /// Structure present — proceed to the real low-rank collapser (which carries its own cert).
    AttemptFold { top_sv: f64, separated_from_bulk: bool },
    /// No significant structure — the HONEST_DEFER outcome with the ε-absence certificate note.
    Defer { outcome: Box<CollapseOutcome>, eps: f64, model_note: &'static str },
}

impl LowRankGate {
    pub fn is_attempt(&self) -> bool {
        matches!(self, LowRankGate::AttemptFold { .. })
    }
}

/// Consult the BBP gate before a low-rank fold. `matrix` is raw `n×m` row-major (the gate scales
/// by `1/√n` internally). Calibration is deterministic given `seed` (R11). On absence the returned
/// `CollapseOutcome` is `Defer(BarrierTag::BelowDetectionThreshold)` — wired into the real dispatch.
pub fn low_rank_gate(
    matrix: &[f64],
    n: usize,
    m: usize,
    sigma: f64,
    eps: f64,
    calib_trials: usize,
    seed: u64,
) -> LowRankGate {
    let gamma = m as f64 / n as f64;
    let threshold = bbp::calibrate_threshold(n, m, sigma, eps, calib_trials, seed);
    match bbp::soundness_gate(matrix, n, m, sigma, threshold, gamma, eps, seed) {
        GateOutcome::Fold { top_sv, separated_from_bulk, .. } => {
            LowRankGate::AttemptFold { top_sv, separated_from_bulk }
        }
        GateOutcome::Defer { .. } => LowRankGate::Defer {
            outcome: Box::new(CollapseOutcome::Defer(Defer::new(
                BarrierTag::BelowDetectionThreshold,
                IrRef::new(0, Span::dummy()),
            ))),
            eps,
            model_note: EPS_MODEL_NOTE,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jeff_math::bbp::noise_plus_spike;
    use jeff_math::fmat::{singular_values, Rng};
    use std::time::Instant;

    #[test]
    fn gate_wired_into_dispatch() {
        let (n, m, sigma, eps) = (100usize, 60usize, 1.0, 0.05);
        // pure noise → HONEST_DEFER carrying BarrierTag::BelowDetectionThreshold.
        let mut rng = Rng::new(2024);
        let noise: Vec<f64> = (0..n * m).map(|_| rng.gaussian()).collect();
        let g = low_rank_gate(&noise, n, m, sigma, eps, 150, 7);
        match g {
            LowRankGate::Defer { outcome, .. } => match *outcome {
                CollapseOutcome::Defer(d) => {
                    assert_eq!(d.tag, BarrierTag::BelowDetectionThreshold, "absence cert tag")
                }
                _ => panic!("expected Defer outcome"),
            },
            LowRankGate::AttemptFold { .. } => panic!("noise must defer in dispatch"),
        }
        // strong rank-3 structure → AttemptFold (the fold then runs separately).
        let structured = noise_plus_spike(n, m, 2.5, 3, 55);
        assert!(low_rank_gate(&structured, n, m, sigma, eps, 150, 7).is_attempt());
    }

    #[test]
    fn gate_prevents_wasted_lowrank_fit() {
        // The deployed gate (one top sv via power iteration, O(nnz)) is far cheaper than a full
        // SVD-based low-rank fit (O(n²·min)). On noise the gate defers ⇒ the fit is SKIPPED
        // entirely (the "infinite expected saving under the null" the directive describes).
        let (n, m) = (200usize, 200usize);
        let mut rng = Rng::new(77);
        let a: Vec<f64> = (0..n * m).map(|_| rng.gaussian()).collect();
        let an: Vec<f64> = a.iter().map(|x| x / (n as f64).sqrt()).collect();

        let t0 = Instant::now();
        let _s = bbp::top_singular_value(&an, n, m, 100, 1); // the deployed gate primitive
        let gate_t = t0.elapsed().as_secs_f64();

        let t1 = Instant::now();
        let _full = singular_values(&a, n, m); // the full fit the gate would otherwise trigger
        let fit_t = t1.elapsed().as_secs_f64();

        assert!(gate_t < fit_t, "gate ({gate_t:.6}s) must be cheaper than full fit ({fit_t:.6}s)");
        // and on this pure-noise input the gate defers, so the fit is avoided.
        let g = low_rank_gate(&a, n, m, 1.0, 0.05, 100, 7);
        assert!(!g.is_attempt(), "noise gate must defer ⇒ wasted fit prevented");
    }
}
