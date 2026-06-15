//! PGO cost-model hooks (CLAUDE.md PART C, 10.7). Profile-guided weights parameterize
//! the recognizer's asymptotic cost model; they are a *tuning* input (they change which
//! collapse is chosen, never whether a chosen collapse is verified — P2 is independent).

/// Tunable cost weights. Defaults are neutral; a PGO pass can update them from measured
/// profile_weight data without touching correctness.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CostWeights {
    /// relative cost charged to a constant-time residual.
    pub w_const: f64,
    /// per-log-factor cost.
    pub w_log: f64,
    /// per-linear-factor cost.
    pub w_linear: f64,
    /// penalty multiplier for an approximate (vs exact) certificate.
    pub approx_penalty: f64,
}

impl Default for CostWeights {
    fn default() -> Self {
        CostWeights {
            w_const: 1.0,
            w_log: 4.0,
            w_linear: 100.0,
            approx_penalty: 1.25,
        }
    }
}

impl CostWeights {
    /// Update weights from a measured profile (PGO hook). `hot_fraction` in [0,1] scales
    /// the linear-loop penalty: hot loops are charged more, biasing toward collapsing them.
    pub fn with_profile(mut self, hot_fraction: f64) -> Self {
        let h = hot_fraction.clamp(0.0, 1.0);
        self.w_linear *= 1.0 + 4.0 * h;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_weights_order_asymptotics() {
        let w = CostWeights::default();
        assert!(w.w_const < w.w_log && w.w_log < w.w_linear);
    }

    #[test]
    fn profile_biases_hot_loops() {
        let base = CostWeights::default();
        let hot = base.with_profile(1.0);
        assert!(hot.w_linear > base.w_linear, "hot loops charged more");
        // a cold profile leaves weights unchanged
        assert_eq!(base.with_profile(0.0).w_linear, base.w_linear);
    }
}
