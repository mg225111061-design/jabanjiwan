//! Stage 29 — Rational Sum-of-Squares (SOS) nonnegativity certificate (`prove_nonneg`).
//!
//! Port of reference kernel #5 (`kernels/k5_rational_sos.py`). **Not a speed collapse** — the
//! value is *mechanical verifiability*: a sound, machine-checked certificate that a polynomial
//! is nonnegative, so other folds can discharge preconditions ("this denominator is > 0", "this
//! error term ≥ 0") and fire safely. **Sound but incomplete**: by the Positivstellensatz not
//! every nonnegative polynomial is SOS (Motzkin is the classic counterexample), so the honest
//! behavior is "SOS ⇒ exact certificate; nonnegative-but-not-SOS (or can't decide) ⇒ HONEST_DEFER".
//!
//! # SDP path (directive 29.1(b)): path (ii), verification-only — no SDP solver linked.
//! Rust has no native SDP solver and we link none (R5). The numerical SDP is only the *guesser*;
//! the soundness lives entirely in the **exact rational** verification (round → identity-repair →
//! exact LDLᵀ PSD → exact identity check). The guesser here is a built-in **diagonal-placement
//! heuristic** that handles the easy SOS/PD cases (a perfect square, a PD form). Harder SOS
//! polynomials needing a cross-term `Q` can be certified by supplying an external numeric `Q`
//! (e.g. from running the Python reference once) via [`prove_nonneg_with_q`].
//!
//! # Note on this tree
//! The directive frames this as "replace the empty `prove_nonneg` SMT stub". There is **no such
//! stub in this repository** (`grep prove_nonneg` is empty) — so this is a *new* capability
//! added, not a replacement of a flagged stub. Stated plainly per the honesty discipline.

use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::Zero;
use std::collections::{BTreeMap, BTreeSet};

/// A monomial exponent tuple (length = number of variables).
pub type Exp = Vec<usize>;
/// A polynomial as `exponent → rational coefficient`.
pub type SosPoly = BTreeMap<Exp, BigRational>;

fn rat_i(n: i64) -> BigRational {
    BigRational::from(BigInt::from(n))
}

/// Build a polynomial from `(exponent, integer coeff)` pairs.
pub fn poly_from(terms: &[(&[usize], i64)]) -> SosPoly {
    terms.iter().map(|(e, c)| (e.to_vec(), rat_i(*c))).collect()
}

/// All monomials (exponent tuples) in `nvars` variables up to total degree `deg`, stable order.
pub fn monomials(nvars: usize, deg: usize) -> Vec<Exp> {
    let mut out: Vec<Exp> = Vec::new();
    let mut seen: BTreeSet<Exp> = BTreeSet::new();
    // total degree 0..=deg, combinations_with_replacement of variable indices.
    fn rec(nvars: usize, start: usize, left: usize, cur: &mut Exp, acc: &mut Vec<Exp>) {
        if left == 0 {
            acc.push(cur.clone());
            return;
        }
        for v in start..nvars {
            cur[v] += 1;
            rec(nvars, v, left - 1, cur, acc);
            cur[v] -= 1;
        }
    }
    for total in 0..=deg {
        let mut combos: Vec<Exp> = Vec::new();
        let mut cur = vec![0usize; nvars];
        rec(nvars, 0, total, &mut cur, &mut combos);
        for e in combos {
            if seen.insert(e.clone()) {
                out.push(e);
            }
        }
    }
    out
}

/// `exponent → list of (i,j)` index pairs with `basis[i] + basis[j] = exponent`.
fn exp_map(basis: &[Exp], nvars: usize) -> BTreeMap<Exp, Vec<(usize, usize)>> {
    let mut m: BTreeMap<Exp, Vec<(usize, usize)>> = BTreeMap::new();
    for (i, bi) in basis.iter().enumerate() {
        for (j, bj) in basis.iter().enumerate() {
            let e: Exp = (0..nvars).map(|k| bi[k] + bj[k]).collect();
            m.entry(e).or_default().push((i, j));
        }
    }
    m
}

/// The "guesser" (path ii): place each target coefficient into a single slot — a diagonal slot
/// `(i,i)` when one maps to the exponent, else an off-diagonal symmetric pair `(i,j),(j,i)`.
/// Each `(i,j)` contributes to exactly one exponent, so the identity `zᵀQz = p` holds **exactly**
/// by construction (for exponents that have a slot; missing ⇒ identity check fails ⇒ defer).
fn build_candidate(poly: &SosPoly, basis: &[Exp], nvars: usize) -> Vec<Vec<BigRational>> {
    let n = basis.len();
    let mut q = vec![vec![BigRational::zero(); n]; n];
    let em = exp_map(basis, nvars);
    let exps: BTreeSet<Exp> = poly.keys().cloned().chain(em.keys().cloned()).collect();
    for e in &exps {
        let want = poly.get(e).cloned().unwrap_or_else(BigRational::zero);
        if want.is_zero() {
            continue;
        }
        let Some(ijs) = em.get(e) else { continue }; // no slot → identity check will catch it
        if let Some(&(i, _)) = ijs.iter().find(|&&(i, j)| i == j) {
            q[i][i] += &want;
        } else if let Some(&(i, j)) = ijs.iter().find(|&&(i, j)| i < j) {
            let half = &want / rat_i(2);
            q[i][j] += &half;
            q[j][i] += &half;
        }
    }
    q
}

/// Round a numeric `Q` (path ii external SDP output) to a symmetric bounded-denominator rational.
pub fn rational_round_matrix(qnum: &[Vec<f64>], denom: i64) -> Vec<Vec<BigRational>> {
    let n = qnum.len();
    let mut q = vec![vec![BigRational::zero(); n]; n];
    for i in 0..n {
        for j in i..n {
            let v = BigRational::new(BigInt::from((qnum[i][j] * denom as f64).round() as i64), BigInt::from(denom));
            q[i][j] = v.clone();
            q[j][i] = v;
        }
    }
    q
}

/// Minimally repair `q` so `zᵀ q z == poly` exactly, absorbing residuals into a unique-monomial
/// slot (diagonal preferred). Returns whether the repair could place every residual.
fn fix_identity(q: &mut [Vec<BigRational>], poly: &SosPoly, basis: &[Exp], nvars: usize) -> bool {
    let em = exp_map(basis, nvars);
    let exps: BTreeSet<Exp> = poly.keys().cloned().chain(em.keys().cloned()).collect();
    for e in &exps {
        let want = poly.get(e).cloned().unwrap_or_else(BigRational::zero);
        let ijs = em.get(e);
        let have = ijs
            .map(|v| v.iter().fold(BigRational::zero(), |a, &(i, j)| a + &q[i][j]))
            .unwrap_or_else(BigRational::zero);
        let diff = &want - &have;
        if diff.is_zero() {
            continue;
        }
        let Some(ijs) = ijs else { return false }; // residual on an exponent with no slot
        if let Some(&(i, _)) = ijs.iter().find(|&&(i, j)| i == j) {
            q[i][i] += &diff;
        } else if let Some(&(i, j)) = ijs.iter().find(|&&(i, j)| i < j) {
            let half = &diff / rat_i(2);
            q[i][j] += &half;
            q[j][i] += &half;
        } else {
            return false;
        }
    }
    true
}

/// Exact check `zᵀ q z == poly` over the rationals, term by term.
fn verify_identity(q: &[Vec<BigRational>], poly: &SosPoly, basis: &[Exp], nvars: usize) -> bool {
    let mut produced: SosPoly = BTreeMap::new();
    for (i, bi) in basis.iter().enumerate() {
        for (j, bj) in basis.iter().enumerate() {
            let e: Exp = (0..nvars).map(|k| bi[k] + bj[k]).collect();
            *produced.entry(e).or_insert_with(BigRational::zero) += &q[i][j];
        }
    }
    let exps: BTreeSet<Exp> = produced.keys().cloned().chain(poly.keys().cloned()).collect();
    for e in &exps {
        let a = produced.get(e).cloned().unwrap_or_else(BigRational::zero);
        let b = poly.get(e).cloned().unwrap_or_else(BigRational::zero);
        if a != b {
            return false;
        }
    }
    true
}

/// Exact LDLᵀ over the rationals. `(is_psd, pivots)`: PSD iff every pivot ≥ 0 and the
/// factorization completes (a zero pivot with a nonzero entry below it is a saddle ⇒ not PSD).
pub fn exact_ldl_psd(q: &[Vec<BigRational>]) -> (bool, Vec<BigRational>) {
    let n = q.len();
    let mut m: Vec<Vec<BigRational>> = q.to_vec();
    let mut d = vec![BigRational::zero(); n];
    for k in 0..n {
        let dk = m[k][k].clone();
        d[k] = dk.clone();
        if dk.is_zero() {
            // zero pivot: PSD only if the rest of the column is zero (rank deficiency, not saddle).
            if (k + 1..n).any(|i| !m[i][k].is_zero()) {
                return (false, d);
            }
            continue;
        }
        let mut lcol = vec![BigRational::zero(); n];
        for (i, l) in lcol.iter_mut().enumerate().take(n).skip(k + 1) {
            *l = &m[i][k] / &dk;
        }
        for i in (k + 1)..n {
            for j in (k + 1)..n {
                let upd = &lcol[i] * &dk * &lcol[j];
                m[i][j] -= upd;
            }
        }
    }
    let is_psd = d.iter().all(|p| *p >= BigRational::zero());
    (is_psd, d)
}

/// The outcome of an SOS nonnegativity attempt.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SosOutcome {
    /// Exact rational SOS certificate: `p = zᵀ Q z`, `Q ⪰ 0` proven by exact LDLᵀ pivots ≥ 0.
    Certified { pivots: Vec<BigRational>, basis_size: usize },
    /// Honest don't-know (not SOS over this basis, rounding broke the identity, or `Q` not PSD).
    Defer { reason: String },
}

impl SosOutcome {
    pub fn is_certified(&self) -> bool {
        matches!(self, SosOutcome::Certified { .. })
    }
}

/// Verify an exact-rational candidate `Q`: identity-repair → exact identity check → exact LDLᵀ PSD.
fn certify_with_rational_q(
    mut q: Vec<Vec<BigRational>>,
    poly: &SosPoly,
    basis: &[Exp],
    nvars: usize,
) -> SosOutcome {
    if !fix_identity(&mut q, poly, basis, nvars) || !verify_identity(&q, poly, basis, nvars) {
        return SosOutcome::Defer { reason: "rational rounding/repair broke the exact identity".into() };
    }
    let (is_psd, pivots) = exact_ldl_psd(&q);
    if !is_psd {
        return SosOutcome::Defer { reason: "candidate Q is not PSD over the rationals (saddle/negative pivot)".into() };
    }
    SosOutcome::Certified { pivots, basis_size: basis.len() }
}

/// Prove `poly ≥ 0` via the built-in diagonal-placement guesser + exact rational verification.
/// `SOS over the degree-`half_deg` basis ⇒ exact certificate; otherwise HONEST_DEFER`.
pub fn prove_nonneg(poly: &SosPoly, nvars: usize, half_deg: usize) -> SosOutcome {
    let basis = monomials(nvars, half_deg);
    let q = build_candidate(poly, &basis, nvars);
    certify_with_rational_q(q, poly, &basis, nvars)
}

/// Path (ii) external-guesser entry: certify against a *numeric* `Q` (e.g. from an SDP solver run
/// out-of-process). Rounds to rationals, then the same exact verification.
pub fn prove_nonneg_with_q(
    poly: &SosPoly,
    nvars: usize,
    half_deg: usize,
    qnum: &[Vec<f64>],
    denom: i64,
) -> SosOutcome {
    let basis = monomials(nvars, half_deg);
    if qnum.len() != basis.len() {
        return SosOutcome::Defer { reason: "supplied Q size != monomial basis size".into() };
    }
    let q = rational_round_matrix(qnum, denom);
    certify_with_rational_q(q, poly, &basis, nvars)
}

// ---- 29.3 precondition gate: a fold whose precondition is discharged by prove_nonneg ----

/// Whether a dependent fold may fire. A fold that needs `precond ≥ 0` calls [`require_nonneg`];
/// it fires only if the SOS engine certifies, else it HONEST_DEFERs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GateDecision {
    Fire { pivots_len: usize },
    HonestDefer { reason: String },
}

/// Precondition gate: discharge a fold's `precond ≥ 0` obligation via SOS. Returns `Fire` on an
/// exact certificate, `HonestDefer` otherwise — the quantitative, sound gating the directive asks
/// for (a non-SOS precondition never silently fires).
pub fn require_nonneg(precond: &SosPoly, nvars: usize, half_deg: usize) -> GateDecision {
    match prove_nonneg(precond, nvars, half_deg) {
        SosOutcome::Certified { pivots, .. } => GateDecision::Fire { pivots_len: pivots.len() },
        SosOutcome::Defer { reason } => GateDecision::HonestDefer { reason },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // p1 = x⁴ − 2x³ + 4x² + 2 = (x²−x+1)² + (x+1)²   (SOS, degree 4)
    fn p1() -> SosPoly {
        poly_from(&[(&[4], 1), (&[3], -2), (&[2], 4), (&[0], 2)])
    }
    // p2 = 2x² + 2xy + 2y²  (positive definite quadratic, 2 vars)
    fn p2() -> SosPoly {
        poly_from(&[(&[2, 0], 2), (&[1, 1], 2), (&[0, 2], 2)])
    }
    // Motzkin M(x,y) = x⁴y² + x²y⁴ − 3x²y² + 1  (nonneg by AM–GM, famously NOT SOS)
    fn motzkin() -> SosPoly {
        poly_from(&[(&[4, 2], 1), (&[2, 4], 1), (&[2, 2], -3), (&[0, 0], 1)])
    }

    #[test]
    fn sos_poly_certified_exact() {
        let r = prove_nonneg(&p1(), 1, 2);
        assert!(r.is_certified(), "p1 must be certified SOS: {r:?}");
    }

    #[test]
    fn sos_pd_quadratic_certified() {
        assert!(prove_nonneg(&p2(), 2, 1).is_certified(), "p2 (PD) must be certified");
    }

    #[test]
    fn ldl_pivots_nonneg_exact() {
        // p1: exact pivots are [2, 4, 3/4], all ≥ 0 (verifies the exact LDLᵀ output).
        let SosOutcome::Certified { pivots, .. } = prove_nonneg(&p1(), 1, 2) else {
            panic!("p1 should certify");
        };
        assert!(pivots.iter().all(|p| *p >= BigRational::zero()), "all pivots ≥ 0");
        assert_eq!(pivots, vec![rat_i(2), rat_i(4), BigRational::new(BigInt::from(3), BigInt::from(4))]);
    }

    #[test]
    fn motzkin_defers_honestly() {
        // The exact identity Q for Motzkin carries a −3 on the diagonal (the x²y² slot), so exact
        // LDLᵀ proves it is not PSD ⇒ DEFER. (Motzkin is nonnegative but has no SOS decomposition;
        // deferring is the textbook-correct honest negative — it MUST NOT certify.)
        let r = prove_nonneg(&motzkin(), 2, 3);
        assert!(!r.is_certified(), "Motzkin must NOT be certified");
        assert!(matches!(r, SosOutcome::Defer { .. }));
    }

    #[test]
    fn indefinite_defers() {
        // x² − 1 is negative on (−1,1): the identity Q has a −1 pivot ⇒ not PSD ⇒ DEFER.
        let p = poly_from(&[(&[2], 1), (&[0], -1)]);
        assert!(!prove_nonneg(&p, 1, 1).is_certified());
    }

    #[test]
    fn rounding_failure_defers() {
        // When the exact identity cannot be satisfied by any rounding/repair of Q ⇒ DEFER (never a
        // false certificate). half_deg=1 gives basis [1, x] (max degree 2), which cannot produce
        // the x⁴ term of p1, so the identity-repair fails on the unrepresentable exponent.
        let q22 = vec![vec![1.0, 0.0], vec![0.0, 1.0]];
        let r = prove_nonneg_with_q(&p1(), 1, 1, &q22, 1 << 16);
        assert!(matches!(r, SosOutcome::Defer { .. }), "unsatisfiable identity must defer: {r:?}");
    }

    #[test]
    fn prove_nonneg_no_longer_stub() {
        // A real, input-dependent decision procedure (not a constant): it certifies p1/p2 and
        // defers Motzkin/indefinite — distinct outcomes driven by the input.
        assert!(prove_nonneg(&p1(), 1, 2).is_certified());
        assert!(prove_nonneg(&p2(), 2, 1).is_certified());
        assert!(!prove_nonneg(&motzkin(), 2, 3).is_certified());
        assert!(!prove_nonneg(&poly_from(&[(&[2], 1), (&[0], -1)]), 1, 1).is_certified());
    }

    #[test]
    fn precondition_gate_fires_on_sos() {
        // A dependent fold needs "precond ≥ 0"; p2 is SOS ⇒ the gate fires.
        assert!(matches!(require_nonneg(&p2(), 2, 1), GateDecision::Fire { .. }));
    }

    #[test]
    fn precondition_gate_defers_on_nonsos() {
        // Same gate, non-SOS precondition (Motzkin) ⇒ HONEST_DEFER, the fold does not fire.
        assert!(matches!(require_nonneg(&motzkin(), 2, 3), GateDecision::HonestDefer { .. }));
    }

    #[test]
    fn external_numeric_q_path_certifies() {
        // Path (ii): a numeric Q (as an SDP would emit) for p1, rounded then exactly verified.
        let qnum = vec![vec![2.0, 0.0, 0.0], vec![0.0, 4.0, -1.0], vec![0.0, -1.0, 1.0]];
        assert!(prove_nonneg_with_q(&p1(), 1, 2, &qnum, 1 << 16).is_certified());
    }
}
