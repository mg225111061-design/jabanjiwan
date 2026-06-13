//! JLIR — the common lowering target where a *collapsed residual* and a *deferred
//! loop* both arrive (CLAUDE.md 6.5, PART 8, D16). The backend (Layer B) and
//! codegen are therefore universal: they see one region type regardless of origin.
//!
//! Stage-0 scope (AR-5): a region carries an executable [`CoreExpr`] body, its
//! [`Origin`] (collapsed-with-cert or deferred-with-tag), and a minimal metadata
//! envelope. The full envelope (dependence, secret-taint bitset, layout hints,
//! lifetimes — 6.5) is populated by absint/backend in Stages 3/6; the fields are
//! present and documented as not-yet-populated rather than silently absent (R24).
//!
//! Crucially, the **fallback harness** lives here (R1/P0): a region built by
//! [`JlirRegion::deferred`] runs the *original* loop, and a region built by
//! [`JlirRegion::collapsed`] runs the *verified* residual. [`eval`] executes either
//! via the exact core-ir evaluator, so collapse and fallback are observably equal.

use jeff_cert::{BarrierTag, VerifiedCertificate};
use jeff_core_ir::{eval as core_eval, CoreExpr, CoreFn, CoreTy, EvalError};
use num_bigint::BigInt;
use std::collections::BTreeMap;

/// Where a region's body came from (6.5 `Origin`).
#[derive(Clone, Debug)]
pub enum Origin {
    /// A verified collapse. `layer` is the collapser layer (1 = arithmetic, ...).
    /// The certificate is boxed: it is much larger than the `Deferred` variant
    /// (clippy::large_enum_variant) and is only touched for reporting/emission.
    Collapsed {
        layer: u8,
        cert: Box<VerifiedCertificate>,
    },
    /// An honest deferral; the body is the original work (P0 fallback).
    Deferred { tag: BarrierTag },
}

/// Minimal metadata envelope (6.5). Populated by later stages; present now so the
/// shape is fixed and the backend can rely on it.
#[derive(Clone, Debug, Default)]
pub struct Meta {
    /// Per-value secret taint (R6/R29). Empty until secret-taint analysis (Stage 5).
    pub secret_taint: Vec<bool>,
    /// `true` once absint has attached dependence info (Stage 3). Backend must not
    /// recompute dependence (R18) — it reads it from here.
    pub has_dependence: bool,
}

/// A JLIR region: one function's worth of executable IR plus origin + metadata.
#[derive(Clone, Debug)]
pub struct JlirRegion {
    pub name: String,
    pub params: Vec<(String, CoreTy)>,
    /// Executable body: the verified residual (collapsed) or the original (deferred).
    pub body: CoreExpr,
    pub origin: Origin,
    pub meta: Meta,
}

impl JlirRegion {
    /// Build a region from a *verified* collapse: the body is the closed-form
    /// residual, and the certificate is retained for reporting / emission.
    pub fn collapsed(f: &CoreFn, residual: CoreExpr, cert: VerifiedCertificate, layer: u8) -> Self {
        JlirRegion {
            name: f.name.clone(),
            params: f.params.iter().map(|p| (p.name.clone(), p.ty.clone())).collect(),
            body: residual,
            origin: Origin::Collapsed {
                layer,
                cert: Box::new(cert),
            },
            meta: Meta::default(),
        }
    }

    /// Build a region from a deferral: the body is the *original* work (P0
    /// fallback — the never-miscompile guarantee at the IR boundary, R1).
    pub fn deferred(f: &CoreFn, tag: BarrierTag) -> Self {
        JlirRegion {
            name: f.name.clone(),
            params: f.params.iter().map(|p| (p.name.clone(), p.ty.clone())).collect(),
            body: f.body.clone(),
            origin: Origin::Deferred { tag },
            meta: Meta::default(),
        }
    }

    pub fn is_collapsed(&self) -> bool {
        matches!(self.origin, Origin::Collapsed { .. })
    }

    pub fn certificate(&self) -> Option<&VerifiedCertificate> {
        match &self.origin {
            Origin::Collapsed { cert, .. } => Some(cert.as_ref()),
            Origin::Deferred { .. } => None,
        }
    }

    /// SSA / metadata well-formedness checks (R18). Minimal at Stage 0: parameter
    /// names are unique and the secret-taint vector, if present, is consistent.
    pub fn verify_invariants(&self) -> Result<(), String> {
        let mut seen = std::collections::HashSet::new();
        for (p, _) in &self.params {
            if !seen.insert(p) {
                return Err(format!("duplicate parameter '{p}' in region '{}'", self.name));
            }
        }
        Ok(())
    }
}

/// Execute a region with integer arguments, via the exact core-ir evaluator. This
/// is the fallback harness AND the reference for differential checks (P0/R1).
pub fn eval(region: &JlirRegion, args: &[(String, BigInt)]) -> Result<BigInt, EvalError> {
    let env: BTreeMap<String, BigInt> = args.iter().cloned().collect();
    core_eval(&region.body, &env)
}

#[cfg(test)]
mod tests {
    use super::*;
    use jeff_core_ir::{CoreExprKind, CoreParam, RedKind, CoreDomain};
    use jeff_span::Span;

    fn triangular_fn() -> CoreFn {
        let span = Span::dummy();
        let body = CoreExpr {
            kind: CoreExprKind::Reduction {
                kind: RedKind::Sum,
                binder: "i".into(),
                domain: CoreDomain::Range {
                    lo: Box::new(CoreExpr {
                        kind: CoreExprKind::Int(0.into()),
                        span,
                    }),
                    hi: Box::new(CoreExpr {
                        kind: CoreExprKind::Var("n".into()),
                        span,
                    }),
                    inclusive: true,
                },
                body: Box::new(CoreExpr {
                    kind: CoreExprKind::Var("i".into()),
                    span,
                }),
            },
            span,
        };
        CoreFn {
            name: "triangular".into(),
            params: vec![CoreParam {
                name: "n".into(),
                ty: CoreTy::Nat,
            }],
            ret: CoreTy::Nat,
            body,
            total: true,
            span,
        }
    }

    #[test]
    fn deferred_region_runs_original_loop() {
        let f = triangular_fn();
        let region = JlirRegion::deferred(&f, BarrierTag::ConstantFactorOnly);
        assert!(!region.is_collapsed());
        let v = eval(&region, &[("n".into(), BigInt::from(100))]).unwrap();
        assert_eq!(v, BigInt::from(5050));
        region.verify_invariants().unwrap();
    }
}
