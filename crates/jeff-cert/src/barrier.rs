//! Barrier taxonomy (CLAUDE.md PART 12). Every deferral emits a named tag plus a
//! user-facing diagnostic explaining (1) what, (2) why, (3) what the user can do
//! (R4 / R27). Tags are backed by named theorems (R12). Defer is honesty, not
//! failure (D13).

use jeff_span::{DiagCode, Diagnostic, Span};
use serde::{Deserialize, Serialize};

/// Named barrier reasons. Each maps to a rationale theorem (PART 12 table).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BarrierTag {
    /// Information-theoretic Ω(N) floor; backend gives constant-factor only.
    DataDependentOmegaN,
    /// #P-hard counting (Valiant'79; Toda).
    SharpPHard,
    /// NP-hard optimisation (Ising ground state; Lucas'14).
    NpHard,
    /// Halting/termination undecidable.
    Termination,
    /// Rice: nontrivial semantic property of General code.
    RiceUndecidable,
    /// Tensor-network treewidth exceeds budget (Markov–Shi).
    TreewidthBlowup,
    /// Nonlinear gate in a GF(2) region (S-box) — correct to stop here.
    Nonlinearity,
    /// Non-planar #CSP (Cai–Lu): #P-hard outside the matchgate class.
    NonPlanar,
    /// Loop domain is not affine; Barvinok inapplicable.
    NonAffineDomain,
    /// No Gosper/Zeilberger telescoper (not holonomic-summable).
    NonGosperSummable,
    /// Gröbner basis exceeds budget (Mayr–Meyer EXPSPACE).
    GroebnerBlowup,
    /// Linear structure but no asymptotic collapse; backend constant-factor only.
    ConstantFactorOnly,
    /// Memory-hard by design (Argon2); not collapsible — that is the point.
    MemoryHard,
    /// CTC / nonlinear-QM-on-classical / wormhole — rejected (PART 18).
    PhysicsCounterfactual,

    // ---- Stage 6 hidden-structure barriers (the structure was looked for and not
    // found at runtime — "no structure ⇒ defer", never "no structure ⇒ wrong answer") ----
    /// Signal/vector is not k-sparse in any tried basis (compressed sensing, sparse
    /// FFT, frequent-items): residual stays large, no support of size ≤ k explains it.
    NonSparse,
    /// Spectrum / frequency vector is flat (white-noise-like): every bin/coordinate is
    /// comparable, so heavy-coefficient / clustering / coherence structure is absent.
    FlatSpectrum,
    /// A signal is present only above a proven detectability threshold (BBP spike edge,
    /// KS community threshold, Johnson list-decoding radius, super-resolution
    /// separation, persistence band) and this instance is below it.
    BelowDetectionThreshold,
    /// Intrinsic dimension ≈ ambient (matrix completion / manifold / diffusion): no
    /// low-dimensional structure to exploit, so no compression is possible.
    HighIntrinsicDimension,
    /// Statistical-to-computational gap: the signal is information-theoretically present
    /// but no known polynomial algorithm recovers it (spiked tensor √p≲β≲p^{3/4};
    /// low-degree / SoS lower bounds). Conjectural hardness ⇒ defer, class `open`.
    StatComputationalGap,
}

impl BarrierTag {
    /// The canonical tag string used in `HONEST_DEFER[...]` reports (PART 12).
    pub fn as_str(self) -> &'static str {
        use BarrierTag::*;
        match self {
            DataDependentOmegaN => "data-dependent-omega-N",
            SharpPHard => "sharp-P-hard",
            NpHard => "np-hard",
            Termination => "termination",
            RiceUndecidable => "rice-undecidable",
            TreewidthBlowup => "treewidth-blowup",
            Nonlinearity => "nonlinearity",
            NonPlanar => "non-planar",
            NonAffineDomain => "non-affine-domain",
            NonGosperSummable => "non-Gosper-summable",
            GroebnerBlowup => "groebner-blowup",
            ConstantFactorOnly => "constant-factor-only",
            MemoryHard => "memory-hard",
            PhysicsCounterfactual => "physics-counterfactual",
            NonSparse => "non-sparse",
            FlatSpectrum => "flat-spectrum",
            BelowDetectionThreshold => "below-detection-threshold",
            HighIntrinsicDimension => "high-intrinsic-dimension",
            StatComputationalGap => "stat-computational-gap",
        }
    }

    /// The rationale theorem / source (R12).
    pub fn rationale(self) -> &'static str {
        use BarrierTag::*;
        match self {
            DataDependentOmegaN => "information-theoretic Ω(N) floor",
            SharpPHard => "Valiant 1979; Toda — #P-hardness is preserved",
            NpHard => "Lucas 2014 — NP-hard optimisation (Ising ground state)",
            Termination => "halting problem (Turing 1936)",
            RiceUndecidable => "Rice 1953 — nontrivial semantic properties undecidable",
            TreewidthBlowup => "Markov–Shi 2008 — contraction cost = treewidth",
            Nonlinearity => "S-box is nonlinear by design (linearization defence)",
            NonPlanar => "Cai–Lu dichotomy — non-planar #CSP is #P-hard",
            NonAffineDomain => "Barvinok requires an affine domain",
            NonGosperSummable => "Gosper/Zeilberger — no bounded telescoper",
            GroebnerBlowup => "Mayr–Meyer — EXPSPACE worst case",
            ConstantFactorOnly => "Ω(N) floor; no asymptotic collapse",
            MemoryHard => "Argon2 / RFC 9106 — memory-hardness is the design goal",
            PhysicsCounterfactual => "conservation laws + physical realizability (PART 18)",
            NonSparse => "Candès–Tao 2005 — exact recovery needs k-sparsity (δ_2k<√2−1)",
            FlatSpectrum => "Welch 1974 — flat/high-coherence spectrum has no heavy structure",
            BelowDetectionThreshold => "BBP 2005 / KS / Johnson radius — signal below detectability",
            HighIntrinsicDimension => "intrinsic dimension ≈ ambient; no low-rank/manifold structure",
            StatComputationalGap => "low-degree/SoS lower bounds — conjectured no efficient algorithm",
        }
    }

    /// The human diagnostic message (PART 12 catalog, R27).
    pub fn message(self) -> &'static str {
        use BarrierTag::*;
        match self {
            DataDependentOmegaN => {
                "data-dependent across the whole input; Ω(N) information floor — \
                 backend gives constant-factor only"
            }
            SharpPHard => "#P-hard counting; no polynomial closed form exists",
            NpHard => "NP-hard optimization (Ising ground state); refused",
            Termination => "termination is undecidable here; use Total mode or provide a bound",
            RiceUndecidable => {
                "non-trivial semantic property is undecidable; sound over-approximation only"
            }
            TreewidthBlowup => "tensor-network treewidth exceeds budget",
            Nonlinearity => {
                "nonlinear gate reached; GF(2) linear collapse stops here (this is correct for S-boxes)"
            }
            NonPlanar => "non-planar #CSP; #P-hard outside the matchgate class",
            NonAffineDomain => "loop domain is not affine; Barvinok inapplicable",
            NonGosperSummable => "sum is not holonomic-summable (no Gosper/Zeilberger certificate)",
            GroebnerBlowup => "Gröbner basis exceeds budget (EXPSPACE worst case)",
            ConstantFactorOnly => {
                "linear structure but no asymptotic collapse; backend constant-factor only"
            }
            MemoryHard => "memory-hard by design (e.g., Argon2); not collapsible — that is the point",
            PhysicsCounterfactual => "physics-counterfactual construction; rejected (see PART 18)",
            NonSparse => {
                "not k-sparse in any tried basis; sparse recovery cannot collapse this (no speedup)"
            }
            FlatSpectrum => {
                "spectrum is flat / high-coherence; no heavy coefficients to exploit — fall back to dense"
            }
            BelowDetectionThreshold => {
                "signal is below the proven detectability threshold; cannot certify recovery — deferred"
            }
            HighIntrinsicDimension => {
                "intrinsic dimension ≈ ambient; no low-rank/manifold structure to compress"
            }
            StatComputationalGap => {
                "signal present but in the statistical-to-computational gap; no efficient algorithm (open)"
            }
        }
    }

    /// Whether reaching this barrier under a *required* `@collapse` is a hard error
    /// (PART 12 / J.4): the hardness barriers (NP/#P/physics) are refusals.
    pub fn is_refusal(self) -> bool {
        use BarrierTag::*;
        matches!(self, NpHard | SharpPHard | PhysicsCounterfactual)
    }

    /// Build the standard deferral diagnostic for this barrier (Note severity:
    /// deferring is honest, not an error — unless `@collapse` was required, which
    /// the caller upgrades to E0601).
    pub fn diagnostic(self, span: Span) -> Diagnostic {
        Diagnostic {
            severity: jeff_span::Severity::Note,
            code: DiagCode::CollapseRequiredButBarrier,
            span,
            message: format!("HONEST_DEFER[{}] — {}", self.as_str(), self.message()),
            notes: vec![format!("rationale: {}", self.rationale())],
            help: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tags_have_stable_strings() {
        assert_eq!(
            BarrierTag::DataDependentOmegaN.as_str(),
            "data-dependent-omega-N"
        );
        assert_eq!(BarrierTag::Nonlinearity.as_str(), "nonlinearity");
        assert_eq!(BarrierTag::NonGosperSummable.as_str(), "non-Gosper-summable");
    }

    #[test]
    fn hardness_barriers_are_refusals() {
        assert!(BarrierTag::NpHard.is_refusal());
        assert!(BarrierTag::PhysicsCounterfactual.is_refusal());
        assert!(!BarrierTag::ConstantFactorOnly.is_refusal());
        assert!(!BarrierTag::DataDependentOmegaN.is_refusal());
    }
}
