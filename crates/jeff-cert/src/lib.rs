//! Certificate schema and the proof-carrying gate.
//!
//! Authority: CLAUDE.md PART 6.1 (schema), PART 11 (verification), PART 12
//! (barrier taxonomy), APPENDIX F (proof walkthroughs).
//!
//! # The P0/P2 invariant, enforced by the type system
//!
//! `Collapsed` carries a private [`VerifiedCertificate`]. The *only* way to build a
//! `VerifiedCertificate` is [`verify_with`], which constructs one **only** when a
//! [`Checker`] returns [`VerifyResult::Valid`]. Therefore a `Collapsed` cannot be
//! constructed without a certificate that actually passed a check (R31: Unknown /
//! Invalid → `None` → caller must fall back to the original, R1). The concrete
//! checkers live in `jeff-verify`; this crate owns *what counts as verified*.
//!
//! Trust model (honest, DR1/D3): the type system guarantees "*a* checker said
//! Valid". The honesty discipline + `jeff-verify`'s real exact checkers + the
//! certificate-replay tests (R25) guarantee the checker is sound, not a stub that
//! always says Valid (which would be the PART 18 #6 anti-pattern).

use jeff_math::{Gf2Matrix, Gf2Vec, HyperTerm, Poly, RatFunc, RatMatrix};
use jeff_span::{Diagnostic, Span};
use num_bigint::BigInt;
use num_rational::BigRational;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;

pub mod barrier;
pub use barrier::BarrierTag;

/// Identifier of a node in some IR (core IR / JLIR). Certificates reference source
/// and collapsed nodes by id + span (R37).
pub type NodeId = u32;

/// A static collapser identifier, e.g. `"arith/faulhaber"` (PART 6.1).
pub type CollapserId = &'static str;

/// A reference into an IR: which node, and where it came from in source (R37).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct IrRef {
    pub id: NodeId,
    pub span: Span,
}

impl IrRef {
    pub fn new(id: NodeId, span: Span) -> Self {
        IrRef { id, span }
    }
}

/// The equivalence claim a certificate discharges (PART 6.1 `Obligation`).
/// Human-readable; the machine-checkable content lives in [`Evidence`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Obligation {
    pub claim: String,
}

impl Obligation {
    pub fn new(claim: impl Into<String>) -> Self {
        Obligation {
            claim: claim.into(),
        }
    }
}

/// A boundary condition that must hold for the collapse to be sound (R16): base
/// cases, domain endpoints, nonzero denominators, etc. (APPENDIX P.2).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Boundary {
    pub description: String,
}

impl Boundary {
    pub fn new(d: impl Into<String>) -> Self {
        Boundary {
            description: d.into(),
        }
    }
}

/// Witness for a planar #CSP / Pfaffian collapse (APPENDIX E.5), replayed exactly
/// over the integers: the skew-symmetric matrix and the claimed Pfaffian/count.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HolantWitness {
    /// Skew-symmetric adjacency (row-major, i64). `det = pf^2` is re-checked.
    pub skew: Vec<i64>,
    pub dim: usize,
    pub claimed_pfaffian: i64,
}

/// What a [`Evidence::NumericResidual`] should recompute exactly to confirm the
/// collapse (APPENDIX F.6 ReplayChecker). All exact integer/modular work.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReplayKind {
    /// `A^exp mod q` checked against the slow product for small `exp` (A15).
    MatrixPowerMod {
        matrix: Vec<u64>,
        dim: usize,
        q: u64,
        exp: u64,
        claimed: Vec<u64>,
    },
    /// `[x^index] P/Q mod q` (Bostan–Mori) checked against direct unrolling (A08).
    LinearRecTerm {
        rec: Vec<i64>, // recurrence coeffs a_1..a_d (a_n = Σ a_i a_{n-i})
        init: Vec<i64>,
        modulus: u64,
        index: u64,
        claimed: u64,
    },
    /// Exact cyclic convolution via NTT vs naive (PQC poly_mul P01).
    Convolution {
        a: Vec<u64>,
        b: Vec<u64>,
        q: u64,
        root: u64,
        n: usize,
        claimed: Vec<u64>,
    },
    /// Exact **negacyclic** convolution `a*b mod (x^n + 1)` over `Z_q` — the lattice
    /// PQC poly_mul (ML-KEM / ML-DSA). The collapser computes `claimed` via the fast
    /// NTT (jeff-math::pqc); the checker recomputes the Θ(n²) schoolbook definition
    /// independently and confirms equality (AR-4 oracle, exact mod q — no float).
    NegacyclicConvolution {
        a: Vec<u64>,
        b: Vec<u64>,
        q: u64,
        claimed: Vec<u64>,
    },
    /// Finite sample agreement: closed form evaluated at given inputs must equal
    /// the listed expected values (Barvinok chamber sampling, APPENDIX F.4).
    SampleAgreement { samples: Vec<SamplePoint> },
}

/// One `(inputs → expected)` sample for [`ReplayKind::SampleAgreement`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SamplePoint {
    pub inputs: Vec<i64>,
    pub expected: i64,
    /// The closed form, evaluated by the checker, rendered as a Poly with the
    /// variable names matching `var_names`.
    pub closed_form: Poly,
    pub var_names: Vec<String>,
}

/// Evidence kinds and their checker routing (PART 6.1, APPENDIX F.6):
/// * `PolynomialIdentity` → Poly coeff-zero (F.1/F.4/F.5)
/// * `Telescoper`         → poly identity (F.2) | Lean operator induction (stub)
/// * `Gf2LinearIdentity`  → basis evaluation over GF(2) (F.3)
/// * `EigenCharpoly`      → Cayley–Hamilton ring identity (F.5)
/// * `NumericResidual`    → exact modular/int replay (F.6)
/// * `PfaffianHolant`     → Pfaffian/det replay (E.5)
// Evidence variants differ in size (HyperTerm / GF(2) circuit are large), but
// Evidence is never hot-path-copied: it lives inside a `Certificate`, which is
// boxed in `JlirRegion::Origin` and otherwise passed by reference to checkers. So
// boxing every variant would add deref churn for no real benefit.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Evidence {
    /// The difference polynomial that must be identically zero.
    PolynomialIdentity { poly: Poly },
    /// Holonomic telescoper / Gosper certificate (APPENDIX E.1/E.2, F.2). Carries
    /// the hypergeometric term `F`, the telescoper coefficients `a_i(n)` (as `l[i]`,
    /// multiplying `F(n+i,k)/F(n,k)`), and the rational certificate `R(n,k)`. The
    /// checker recomputes `R(n,k+1)·F(n,k+1)/F(n,k) − R(n,k) − Σ a_i·F(n+i,k)/F(n,k)`
    /// from these and confirms its numerator is identically zero — independent of
    /// the collapser (R2/R25). Indefinite Gosper is the special case `l = [1]`.
    Telescoper {
        term: HyperTerm,
        l: Vec<RatFunc>,
        r: RatFunc,
    },
    /// Circuit captured as its linear action `M·x ⊕ b`. The checker re-evaluates
    /// the original circuit on `{0, e_i}` (carried in `circuit`) and confirms it
    /// reconstructs `(m, b)` — sound given structural linearity (E.3 note).
    Gf2LinearIdentity {
        circuit: gf2circuit::LinearCircuit,
        m: Gf2Matrix,
        b: Gf2Vec,
    },
    /// The matrix whose characteristic polynomial must annihilate it (F.5).
    EigenCharpoly { matrix: RatMatrix },
    /// Exact recompute task (F.6).
    NumericResidual { replay: ReplayKind },
    /// Planar #CSP / matching witness (E.5).
    PfaffianHolant { witness: HolantWitness },

    // ---- Stage 3 Tier-S numeric kernels (exact-ring certificates preferred) ----
    /// Sherman–Morrison / Woodbury: the claimed inverse `inv` of `m` is checked
    /// **exactly** over ℚ by confirming `m · inv = I` (APPENDIX 10.2; the structural
    /// precondition rank-k ≪ N is checked by the collapser, not here).
    MatrixInverse { m: RatMatrix, inv: RatMatrix },
    /// Cholesky as sqrt-free LDLᵀ: `A = L · diag(d) · Lᵀ` checked **exactly** over ℚ,
    /// and SPD confirmed by `d[i] > 0` for all i. Non-SPD is refused by the collapser.
    LdltSpd {
        a: RatMatrix,
        l: RatMatrix,
        d: Vec<BigRational>,
    },
    /// Strassen product verified by **exact** Freivalds over ℤ: for each recorded
    /// {0,1} vector `x`, `C·x = A·(B·x)`. `r` recorded rounds bound the false-accept
    /// probability by `2^-r` (Freivalds' lemma over an integral domain). Matrices are
    /// row-major `dim × dim`.
    FreivaldsProduct {
        a: Vec<BigInt>,
        b: Vec<BigInt>,
        c: Vec<BigInt>,
        dim: usize,
        seeds: Vec<Vec<u8>>,
    },
    /// Float residual certificate with an **explicit tolerance** (the discipline for
    /// approximate kernels): the claimed inverse `inv` of `m` satisfies
    /// `‖m·inv − I‖∞ ≤ tol`. Soundness is *relative to* `tol` (stated, not hidden).
    FloatResidual {
        m: Vec<f64>,
        inv: Vec<f64>,
        dim: usize,
        tol: f64,
    },

    // ---- Stage 3 Tier-A approximate kernels (residual ≤ tol; exact is impossible) ----
    /// Randomized low-rank approximation: `‖A − Â‖_F ≤ tol` (Frobenius), under the
    /// numerically-low-rank assumption. The construction is probabilistic (HMT 2011,
    /// Thm 10.5: `E‖A−QQᵀA‖_F ≤ (1 + k/(p−1))^{1/2}(Σ_{j>k}σ_j²)^{1/2}`), but the
    /// certificate is the **measured** residual, so correctness does not depend on
    /// that bound. Matrices are row-major `rows × cols`.
    LowRankResidual {
        a: Vec<f64>,
        approx: Vec<f64>,
        rows: usize,
        cols: usize,
        tol: f64,
    },
    /// N-body far-field residual: `‖φ_fast − φ_exact‖∞ ≤ tol`. The checker recomputes
    /// `φ_exact` by the **exact O(N²) direct sum** (ground truth), so a wrong fast
    /// potential is caught. Valid only for a decaying kernel (checked by the collapser).
    FmmResidual {
        points: Vec<f64>,
        charges: Vec<f64>,
        kernel: jeff_math::nbody::KernelKind,
        phi: Vec<f64>,
        tol: f64,
    },
    /// Krylov (CG) residual: `‖A x − b‖₂ ≤ tol` (deterministic). `A` is sparse
    /// symmetric, given as `(i,j,value)` entries on a `dim`-dimensional space.
    LinSolveResidual {
        entries: Vec<(usize, usize, f64)>,
        dim: usize,
        b: Vec<f64>,
        x: Vec<f64>,
        tol: f64,
    },
    /// Sinkhorn (entropic OT): certifies the **regularized** plan's marginal
    /// feasibility `‖P𝟙−a‖₁ + ‖Pᵀ𝟙−b‖₁ ≤ tol` for the Gibbs-form plan
    /// `P_ij = exp((f_i+g_j−C_ij)/eps)`. This is NOT exact (ε→0) Wasserstein — the
    /// obligation states the regularization `eps`. The checker reconstructs `P` from
    /// the dual potentials and recomputes the marginals.
    SinkhornPlan {
        cost: Vec<f64>,
        a: Vec<f64>,
        b: Vec<f64>,
        eps: f64,
        f: Vec<f64>,
        g: Vec<f64>,
        tol: f64,
    },
    /// CARE stabilizing solution: certifies `‖AᵀX+XA−XBR⁻¹BᵀX+Q‖_F ≤ tol` AND `X`
    /// PSD AND the closed loop `A−BR⁻¹BᵀX` Hurwitz. The residual alone is insufficient
    /// (CARE has many solutions); all three are checked, else the collapser refuses.
    /// `a,q,x` are `n×n`, `b` is `n×m`, `r` is `m×m` (row-major).
    AreStabilizing {
        a: Vec<f64>,
        b: Vec<f64>,
        q: Vec<f64>,
        r: Vec<f64>,
        x: Vec<f64>,
        n: usize,
        m: usize,
        tol: f64,
    },

    // ---- Stage 4 Barvinok/Ehrhart lattice counting (exact, no tol) ----
    /// Exact lattice-point count: the Ehrhart quasi-polynomial `qp` equals the true
    /// count for the parametric polytope `cs`. Checked à la APPENDIX F.4 — the
    /// residue classes partition the parameter line (`qp.period`), and at every
    /// sampled `n ∈ [n_lo, n_hi]` the checker's **exact brute-force enumeration**
    /// equals `qp.eval(n)`. Integers only — no tolerance.
    LatticeCount {
        cs: jeff_math::lattice::ConstraintSystem,
        qp: jeff_math::lattice::QuasiPoly,
        n_lo: i64,
        n_hi: i64,
    },
}

/// A small captured GF(2) linear circuit so the GF(2) certificate is self-contained
/// (R16) and replayable (R25).
pub mod gf2circuit {
    use serde::{Deserialize, Serialize};

    /// A gate in a linear GF(2) region. Only linear/affine gates appear here;
    /// `partition` (Layer 2) cuts at nonlinear gates (AND/OR/MUX), which become a
    /// `nonlinearity` barrier (APPENDIX 10.4 / P.3).
    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    pub enum Gate {
        /// output wire = input wire (identity / copy)
        Input(usize),
        /// XOR of two previously-defined wires
        Xor(usize, usize),
        /// NOT of a wire (affine: contributes to b)
        Not(usize),
        /// constant 0/1
        Const(bool),
    }

    /// A straight-line GF(2) circuit: `n` inputs, a list of gates (each defines the
    /// next wire id), and which wires are the outputs.
    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    pub struct LinearCircuit {
        pub n_inputs: usize,
        pub gates: Vec<Gate>,
        pub outputs: Vec<usize>,
    }

    impl LinearCircuit {
        /// Evaluate on a concrete input bit-vector (LSB = input 0). Returns the
        /// output bits. Used by both the folder (to build M,b) and the checker (to
        /// confirm M,b on the basis) — same code, so the check is honest.
        pub fn eval(&self, input: &[bool]) -> Vec<bool> {
            let mut wires: Vec<bool> = Vec::with_capacity(self.n_inputs + self.gates.len());
            // wire ids 0..n_inputs are the inputs
            for i in 0..self.n_inputs {
                wires.push(input.get(i).copied().unwrap_or(false));
            }
            for g in &self.gates {
                let v = match *g {
                    Gate::Input(w) => wires[w],
                    Gate::Xor(a, b) => wires[a] ^ wires[b],
                    Gate::Not(a) => !wires[a],
                    Gate::Const(c) => c,
                };
                wires.push(v);
            }
            self.outputs.iter().map(|&w| wires[w]).collect()
        }

        pub fn n_outputs(&self) -> usize {
            self.outputs.len()
        }
    }
}

/// A certificate: self-contained (R16) — source, collapsed form, obligation,
/// evidence, boundaries, and a fallback that always equals the source.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Certificate {
    /// `Cow<'static, str>` (DR6 representational note): keeps `&'static str`
    /// ergonomics (`CollapserId.into()`) while still deserializing for cert-replay.
    pub collapser_id: Cow<'static, str>,
    pub source: IrRef,
    pub collapsed: IrRef,
    pub obligation: Obligation,
    pub evidence: Evidence,
    pub boundaries: Vec<Boundary>,
    /// Always equals `source` (P0 fallback target).
    pub fallback: IrRef,
}

impl Certificate {
    /// Emit the human + machine JSON record for `--emit-certificates` (APPENDIX
    /// H.3). Deterministic (R11): field order fixed, values canonical.
    pub fn to_json(&self, verified_checker: &str) -> serde_json::Value {
        serde_json::json!({
            "collapser_id": self.collapser_id.as_ref(),
            "source": { "node": self.source.id, "span": self.source.span.to_string() },
            "collapsed": { "node": self.collapsed.id, "span": self.collapsed.span.to_string() },
            "obligation": self.obligation.claim,
            "evidence": self.evidence,
            "boundaries": self.boundaries.iter().map(|b| b.description.clone()).collect::<Vec<_>>(),
            "verified": { "checker": verified_checker, "result": "valid" },
            "fallback": { "node": self.fallback.id },
        })
    }
}

/// Result of a checker (PART 6.2). `Unknown` = timeout / incapable — **not** valid
/// (R31): treated as a fallback trigger, never as success (DR8).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VerifyResult {
    Valid,
    Invalid,
    Unknown,
}

/// A checker discharges one or more evidence kinds (PART 6.2). Implemented in
/// `jeff-verify`. Must terminate (R23): exact checks here are inherently bounded;
/// any external solver must carry its own timeout + fallback.
pub trait Checker {
    fn check(&self, ev: &Evidence, ob: &Obligation, boundaries: &[Boundary]) -> VerifyResult;
}

/// A certificate that has actually passed a checker. The inner field is private:
/// the only constructor is [`verify_with`]. This is the type-level P0/P2 gate.
///
/// # Tripwire: `unverified_collapse_is_unconstructible`
///
/// A `VerifiedCertificate` cannot be forged from outside this crate — the tuple
/// field is private, so the only way to obtain one is [`verify_with`] (which
/// returns `Some` only on `Valid`). Therefore a [`Collapsed`] (which *requires* a
/// `VerifiedCertificate`) cannot be built around an unverified certificate. The
/// following does not compile (private constructor):
///
/// ```compile_fail
/// use jeff_cert::{Certificate, VerifiedCertificate};
/// fn forge(c: Certificate) -> VerifiedCertificate {
///     VerifiedCertificate(c) // ERROR: cannot construct — field is private
/// }
/// ```
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VerifiedCertificate(Certificate);

impl VerifiedCertificate {
    pub fn certificate(&self) -> &Certificate {
        &self.0
    }
    pub fn into_certificate(self) -> Certificate {
        self.0
    }
}

/// The single constructor of [`VerifiedCertificate`]. Runs `checker`; produces
/// `Some` **iff** the result is `Valid` (R31: Unknown/Invalid → `None`). This is
/// the choke point through which every collapse must pass (P2).
pub fn verify_with(c: Certificate, checker: &dyn Checker) -> Option<VerifiedCertificate> {
    match checker.check(&c.evidence, &c.obligation, &c.boundaries) {
        VerifyResult::Valid => Some(VerifiedCertificate(c)),
        VerifyResult::Invalid | VerifyResult::Unknown => None,
    }
}

/// A collapsed program fragment: a sublinear/closed residual plus the verified
/// certificate proving it equivalent to the source. Cannot be built without a
/// `VerifiedCertificate` (P0/P2).
#[derive(Clone, Debug, PartialEq)]
pub struct Collapsed {
    pub residual: IrRef,
    cert: VerifiedCertificate,
}

impl Collapsed {
    pub fn new(residual: IrRef, cert: VerifiedCertificate) -> Self {
        Collapsed { residual, cert }
    }
    pub fn certificate(&self) -> &VerifiedCertificate {
        &self.cert
    }
}

/// An honest deferral: a named barrier (PART 12), a user diagnostic (R4/R27), and
/// the preserved original (P0).
#[derive(Clone, Debug)]
pub struct Defer {
    pub tag: BarrierTag,
    pub diagnostic: Diagnostic,
    pub original: IrRef,
}

impl Defer {
    pub fn new(tag: BarrierTag, original: IrRef) -> Self {
        let diagnostic = tag.diagnostic(original.span);
        Defer {
            tag,
            diagnostic,
            original,
        }
    }
}

/// The result of attempting a collapse: either a verified `Collapsed` (whole, R32:
/// no half-collapse) or an honest `Defer` (R4). A collapser never returns a partial
/// or unverified result (E.7).
// Returned by value once per collapse attempt (not bulk-stored), so the variant
// size spread (Collapsed carries the certificate) is not worth boxing churn.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug)]
pub enum CollapseOutcome {
    Collapsed(Collapsed),
    Defer(Defer),
}

#[cfg(test)]
mod tests {
    use super::*;
    use jeff_span::Span;

    struct AlwaysValid;
    impl Checker for AlwaysValid {
        fn check(&self, _: &Evidence, _: &Obligation, _: &[Boundary]) -> VerifyResult {
            VerifyResult::Valid
        }
    }
    struct AlwaysUnknown;
    impl Checker for AlwaysUnknown {
        fn check(&self, _: &Evidence, _: &Obligation, _: &[Boundary]) -> VerifyResult {
            VerifyResult::Unknown
        }
    }

    fn dummy_cert() -> Certificate {
        Certificate {
            collapser_id: "test/dummy".into(),
            source: IrRef::new(1, Span::dummy()),
            collapsed: IrRef::new(2, Span::dummy()),
            obligation: Obligation::new("x == x"),
            evidence: Evidence::PolynomialIdentity { poly: Poly::zero() },
            boundaries: vec![],
            fallback: IrRef::new(1, Span::dummy()),
        }
    }

    #[test]
    fn valid_yields_verified_certificate() {
        let vc = verify_with(dummy_cert(), &AlwaysValid);
        assert!(vc.is_some());
        let collapsed = Collapsed::new(IrRef::new(2, Span::dummy()), vc.unwrap());
        assert_eq!(collapsed.residual.id, 2);
    }

    #[test]
    fn unknown_yields_none_then_fallback() {
        // R31: Unknown is not Valid → no VerifiedCertificate → caller must fall back.
        let vc = verify_with(dummy_cert(), &AlwaysUnknown);
        assert!(vc.is_none());
    }

    #[test]
    fn certificate_json_is_deterministic() {
        let c = dummy_cert();
        let a = c.to_json("z3");
        let b = c.to_json("z3");
        assert_eq!(a, b);
        assert_eq!(a["verified"]["result"], "valid");
    }
}
