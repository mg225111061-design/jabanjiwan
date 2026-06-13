//! Certificate checkers and the verification entry point.
//!
//! Authority: CLAUDE.md PART 6.2 (checker routing), PART 11, APPENDIX F (proof
//! walkthroughs), R2 (checker before collapser), R23/R31 (timeout/Unknown →
//! fallback), DR1 (certificates must *actually* pass a real check).
//!
//! # Honesty note on solvers (DR2/DR3)
//!
//! The constitution names Z3 as the default checker and Lean for holonomic operator
//! induction. Neither is wired in this environment. Instead every evidence kind is
//! discharged by an **exact, in-house, deterministic** check — the quantifier-free
//! coefficient-zero variant the constitution itself endorses as "more robust"
//! (APPENDIX F.1), GF(2) basis evaluation (F.3), Cayley–Hamilton ring identity
//! (F.5), and exact modular/integer replay (F.6). These are *sound* (they only
//! return `Valid` when the identity provably holds) and *terminating* (no external
//! process, no hang — R23). Where a Z3/Lean path would add nothing over the exact
//! check, we say so rather than pretend a solver ran (DR2). See `checker_name`.

use jeff_cert::{
    verify_with, Boundary, Certificate, Checker, Evidence, Obligation, ReplayKind, VerifiedCertificate,
    VerifyResult,
};
use jeff_math::modular::ModInt;
use jeff_math::{ModMatrix, RatMatrix};
use num_bigint::BigInt;
use num_rational::BigRational;
use std::collections::BTreeMap;

/// Safety cap for exact replay loops, so a checker can never hang (R23). Beyond
/// this, replay returns `Unknown` → the collapse falls back (R31) rather than
/// blocking. Test fixtures stay well under this.
const REPLAY_CAP: u64 = 5_000_000;

/// Exact polynomial-identity checker (the "Z3Checker" role, F.1/F.4/F.5).
/// Discharges `PolynomialIdentity`, `Telescoper` (identity part), and
/// `EigenCharpoly` (Cayley–Hamilton).
pub struct PolyChecker;

impl Checker for PolyChecker {
    fn check(&self, ev: &Evidence, _ob: &Obligation, _b: &[Boundary]) -> VerifyResult {
        match ev {
            Evidence::PolynomialIdentity { poly } => {
                // The difference polynomial must be identically zero (every
                // coefficient cancels). This is exact and sound (F.1).
                if poly.is_zero() {
                    VerifyResult::Valid
                } else {
                    VerifyResult::Invalid
                }
            }
            Evidence::Telescoper { identity, .. } => {
                // The creative-telescoping identity, cleared of denominators, is a
                // polynomial that must vanish (F.2). The operator `l` and rational
                // `r` are carried for the record; soundness rests on `identity≡0`.
                if identity.is_zero() {
                    VerifyResult::Valid
                } else {
                    VerifyResult::Invalid
                }
            }
            Evidence::EigenCharpoly { matrix } => {
                // Cayley–Hamilton: charpoly(A) annihilates A, exactly (F.5).
                if matrix.satisfies_charpoly() {
                    VerifyResult::Valid
                } else {
                    VerifyResult::Invalid
                }
            }
            _ => VerifyResult::Unknown, // not my evidence kind
        }
    }
}

/// GF(2) linearity checker (F.3). Re-evaluates the captured circuit on the affine
/// basis `{0, e_1, ..., e_n}` and confirms it reconstructs the certified `(M, b)`.
/// Sound because the circuit is structurally linear (only XOR/NOT/const gates), so
/// agreement on an affine-spanning set implies agreement everywhere (E.3 note).
pub struct Gf2Checker;

impl Checker for Gf2Checker {
    fn check(&self, ev: &Evidence, _ob: &Obligation, _b: &[Boundary]) -> VerifyResult {
        let Evidence::Gf2LinearIdentity { circuit, m, b } = ev else {
            return VerifyResult::Unknown;
        };
        let n = circuit.n_inputs;
        if m.cols_n != n || m.rows != circuit.n_outputs() || b.n != circuit.n_outputs() {
            return VerifyResult::Invalid;
        }
        // b must be circuit(0)
        let zero_in = vec![false; n];
        let c0 = circuit.eval(&zero_in);
        for (i, bit) in c0.iter().enumerate() {
            if *bit != b.get(i) {
                return VerifyResult::Invalid;
            }
        }
        // for each input i: circuit(e_i) must equal col_i XOR b
        for col in 0..n {
            let mut ei = vec![false; n];
            ei[col] = true;
            let out = circuit.eval(&ei);
            let column = &m.cols[col];
            for (row, bit) in out.iter().enumerate() {
                let expected = column.get(row) ^ b.get(row);
                if *bit != expected {
                    return VerifyResult::Invalid;
                }
            }
        }
        VerifyResult::Valid
    }
}

/// Exact numeric/Pfaffian replay checker (F.6, E.5). Recomputes the claimed result
/// by an independent exact method and compares. Bounded by [`REPLAY_CAP`] (R23).
pub struct ReplayChecker;

impl Checker for ReplayChecker {
    fn check(&self, ev: &Evidence, _ob: &Obligation, _b: &[Boundary]) -> VerifyResult {
        match ev {
            Evidence::NumericResidual { replay } => self.check_replay(replay),
            Evidence::PfaffianHolant { witness } => {
                // Replay: det(skew) == pfaffian^2 (FKT, E.5). Exact over Q.
                let m = RatMatrix {
                    rows: witness.dim,
                    cols: witness.dim,
                    data: witness
                        .skew
                        .iter()
                        .map(|&v| BigRational::from(BigInt::from(v)))
                        .collect(),
                };
                let det = m.det();
                let pf = BigRational::from(BigInt::from(witness.claimed_pfaffian));
                if det == &pf * &pf {
                    VerifyResult::Valid
                } else {
                    VerifyResult::Invalid
                }
            }
            _ => VerifyResult::Unknown,
        }
    }
}

impl ReplayChecker {
    fn check_replay(&self, replay: &ReplayKind) -> VerifyResult {
        match replay {
            ReplayKind::MatrixPowerMod {
                matrix,
                dim,
                q,
                exp,
                claimed,
            } => {
                if *exp > REPLAY_CAP {
                    return VerifyResult::Unknown; // would hang → fallback (R31)
                }
                let a = ModMatrix::from_u64(*dim, *q, matrix);
                let got = a.pow_naive(*exp); // independent of fast pow
                let want: Vec<u64> = claimed.iter().map(|&v| v % q).collect();
                if got.data == want {
                    VerifyResult::Valid
                } else {
                    VerifyResult::Invalid
                }
            }
            ReplayKind::LinearRecTerm {
                rec,
                init,
                modulus,
                index,
                claimed,
            } => {
                if *index > REPLAY_CAP {
                    return VerifyResult::Unknown;
                }
                let got = unroll_linrec(rec, init, *modulus, *index);
                if got == claimed % modulus {
                    VerifyResult::Valid
                } else {
                    VerifyResult::Invalid
                }
            }
            ReplayKind::Convolution {
                a,
                b,
                q,
                claimed,
                ..
            } => {
                // Independent check: naive cyclic convolution (does not use NTT).
                let n = a.len();
                if n == 0 || b.len() != n || claimed.len() != n {
                    return VerifyResult::Invalid;
                }
                let mut want = vec![0u64; n];
                for (i, &ai) in a.iter().enumerate() {
                    for (j, &bj) in b.iter().enumerate() {
                        let k = (i + j) % n;
                        want[k] = ((want[k] as u128 + ai as u128 * bj as u128) % *q as u128) as u64;
                    }
                }
                let got: Vec<u64> = claimed.iter().map(|&v| v % q).collect();
                if got == want {
                    VerifyResult::Valid
                } else {
                    VerifyResult::Invalid
                }
            }
            ReplayKind::SampleAgreement { samples } => {
                for s in samples {
                    let mut env: BTreeMap<String, BigRational> = BTreeMap::new();
                    for (name, val) in s.var_names.iter().zip(&s.inputs) {
                        env.insert(name.clone(), BigRational::from(BigInt::from(*val)));
                    }
                    let Some(v) = s.closed_form.eval(&env) else {
                        return VerifyResult::Unknown;
                    };
                    if v != BigRational::from(BigInt::from(s.expected)) {
                        return VerifyResult::Invalid;
                    }
                }
                VerifyResult::Valid
            }
        }
    }
}

/// Unroll a linear recurrence `a_n = Σ_i rec[i] * a_{n-1-i}` (mod m) with `init`
/// as `a_0, a_1, ...`. Exact modular arithmetic.
fn unroll_linrec(rec: &[i64], init: &[i64], modulus: u64, index: u64) -> u64 {
    let m = modulus;
    let red = |x: i64| -> u64 { ((x % m as i64 + m as i64) % m as i64) as u64 };
    if (index as usize) < init.len() {
        return red(init[index as usize]);
    }
    let mut window: Vec<u64> = init.iter().map(|&x| red(x)).collect();
    // ensure window length >= rec.len()
    for i in init.len()..=(index as usize) {
        let mut acc = ModInt::zero(m);
        for (j, &c) in rec.iter().enumerate() {
            if i > j {
                let term = ModInt::new(red(c), m) * ModInt::new(window[i - 1 - j], m);
                acc = acc + term;
            }
        }
        window.push(acc.val);
    }
    window[index as usize]
}

/// Routes evidence to the right checker (PART 6.2 / APPENDIX F.6). Implements
/// [`Checker`] so it plugs straight into `verify_with`.
pub struct DefaultRegistry;

impl Checker for DefaultRegistry {
    fn check(&self, ev: &Evidence, ob: &Obligation, b: &[Boundary]) -> VerifyResult {
        match ev {
            Evidence::PolynomialIdentity { .. }
            | Evidence::Telescoper { .. }
            | Evidence::EigenCharpoly { .. } => PolyChecker.check(ev, ob, b),
            Evidence::Gf2LinearIdentity { .. } => Gf2Checker.check(ev, ob, b),
            Evidence::NumericResidual { .. } | Evidence::PfaffianHolant { .. } => {
                ReplayChecker.check(ev, ob, b)
            }
        }
    }
}

/// Which checker discharged a given evidence kind — for the certificate record
/// (APPENDIX H.3 `verified.checker`). Honest naming (DR2): we report the *actual*
/// exact checker, not "z3" when no solver ran.
pub fn checker_name(ev: &Evidence) -> &'static str {
    match ev {
        Evidence::PolynomialIdentity { .. } => "exact-coeff-zero",
        Evidence::Telescoper { .. } => "exact-poly-identity",
        Evidence::EigenCharpoly { .. } => "cayley-hamilton",
        Evidence::Gf2LinearIdentity { .. } => "gf2-basis",
        Evidence::NumericResidual { .. } => "exact-replay",
        Evidence::PfaffianHolant { .. } => "pfaffian-replay",
    }
}

/// The single public verification entry point (PART 6.2). Produces a
/// `VerifiedCertificate` **iff** the routed checker returns `Valid` (R31).
pub fn verify(c: Certificate) -> Option<VerifiedCertificate> {
    verify_with(c, &DefaultRegistry)
}

/// Same, but reports the routed result without consuming on failure — for
/// diagnostics / `--collapse-report` (does not bypass the gate).
pub fn check_result(c: &Certificate) -> VerifyResult {
    DefaultRegistry.check(&c.evidence, &c.obligation, &c.boundaries)
}

#[cfg(test)]
mod tests {
    use super::*;
    use jeff_cert::gf2circuit::{Gate, LinearCircuit};
    use jeff_cert::{IrRef, Obligation};
    use jeff_math::{Gf2Matrix, Gf2Vec, Poly, RatMatrix};
    use jeff_span::Span;
    use num_bigint::BigInt;
    use num_rational::BigRational;

    fn cert(ev: Evidence) -> Certificate {
        Certificate {
            collapser_id: "test".into(),
            source: IrRef::new(1, Span::dummy()),
            collapsed: IrRef::new(2, Span::dummy()),
            obligation: Obligation::new("test"),
            evidence: ev,
            boundaries: vec![],
            fallback: IrRef::new(1, Span::dummy()),
        }
    }

    #[test]
    fn polynomial_identity_zero_is_valid() {
        // 6*S(n) - n(n+1)(2n+1) with S the i^2 sum closed form ≡ 0 (F.1).
        let n = Poly::var("n");
        let one = Poly::from_i64(1);
        let two = Poly::from_i64(2);
        let sixth = BigRational::new(BigInt::from(1), BigInt::from(6));
        let s = n
            .mul(&n.add(&one))
            .mul(&two.mul(&n).add(&one))
            .scale(&sixth);
        // diff = S(n) - S(n-1) - n^2
        let s_shift = {
            let m = n.sub(&one);
            m.mul(&m.add(&one)).mul(&two.mul(&m).add(&one)).scale(&sixth)
        };
        let diff = s.sub(&s_shift).sub(&n.mul(&n));
        assert!(verify(cert(Evidence::PolynomialIdentity { poly: diff })).is_some());
    }

    #[test]
    fn wrong_polynomial_identity_is_rejected() {
        // A nonzero "identity" must NOT verify (DR7).
        let nonzero = Poly::var("n"); // ≠ 0
        assert!(verify(cert(Evidence::PolynomialIdentity { poly: nonzero })).is_none());
    }

    #[test]
    fn eigen_charpoly_valid() {
        let a = RatMatrix::from_i64(2, 2, &[1, 1, 1, 0]);
        assert!(verify(cert(Evidence::EigenCharpoly { matrix: a })).is_some());
    }

    #[test]
    fn gf2_linear_identity_valid_and_tamper_rejected() {
        // circuit: out0 = in0 ^ in1, out1 = in1. Build (M,b) correctly.
        let circuit = LinearCircuit {
            n_inputs: 2,
            gates: vec![Gate::Xor(0, 1)], // wire 2
            outputs: vec![2, 1],
        };
        // columns = circuit(e_i) (b=0 here since no NOT/const)
        let col0 = {
            let mut v = Gf2Vec::zeros(2);
            v.set(0, true); // e0 -> out0 = 1^0 =1, out1 = 0
            v
        };
        let col1 = {
            let mut v = Gf2Vec::zeros(2);
            v.set(0, true); // e1 -> out0 = 0^1 =1, out1 = 1
            v.set(1, true);
            v
        };
        let m = Gf2Matrix::from_columns(2, vec![col0, col1.clone()]);
        let b = Gf2Vec::zeros(2);
        assert!(verify(cert(Evidence::Gf2LinearIdentity {
            circuit: circuit.clone(),
            m: m.clone(),
            b: b.clone()
        }))
        .is_some());

        // Tamper M -> reject (DR1/DR7).
        let mut bad = m;
        bad.cols[0] = col1;
        assert!(verify(cert(Evidence::Gf2LinearIdentity {
            circuit,
            m: bad,
            b
        }))
        .is_none());
    }

    #[test]
    fn replay_linrec_fibonacci() {
        // a_n = a_{n-1}+a_{n-2}; F0=0,F1=1; F(10)=55.
        let ev = Evidence::NumericResidual {
            replay: ReplayKind::LinearRecTerm {
                rec: vec![1, 1],
                init: vec![0, 1],
                modulus: 1_000_000_007,
                index: 10,
                claimed: 55,
            },
        };
        assert!(verify(cert(ev)).is_some());
        // wrong claim rejected
        let ev_bad = Evidence::NumericResidual {
            replay: ReplayKind::LinearRecTerm {
                rec: vec![1, 1],
                init: vec![0, 1],
                modulus: 1_000_000_007,
                index: 10,
                claimed: 56,
            },
        };
        assert!(verify(cert(ev_bad)).is_none());
    }

    #[test]
    fn checker_names_are_honest() {
        assert_eq!(
            checker_name(&Evidence::PolynomialIdentity { poly: Poly::zero() }),
            "exact-coeff-zero"
        );
        assert_eq!(
            checker_name(&Evidence::EigenCharpoly {
                matrix: RatMatrix::identity(2)
            }),
            "cayley-hamilton"
        );
    }
}
