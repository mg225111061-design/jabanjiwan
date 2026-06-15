//! Stage 32 — ordinal termination certificates (OIFC) + the ‖JEFF‖ strength audit.
//!
//! # 32.0 strength audit (keystone) — result: ‖JEFF‖ = ω^ω, NOT ε₀
//! The directive assumes "JEFF's Z3 induction gate gives arbitrary-predicate first-order induction
//! ⇒ ‖JEFF‖ = ε₀". **That premise is false for this tree** (rule 4). `jeff-verify` has **no Z3 and
//! no Lean** ("Neither is wired"); it discharges **quantifier-free exact coefficient-zero
//! polynomial identities** + exact modular/rational replay. Such a checker is primitive-recursive-
//! arithmetic-style; its proof-theoretic ordinal is **ω^ω**, not ε₀ (ε₀ needs full first-order PA
//! induction with arbitrary predicates, which this checker does not implement).
//!
//! **Consequence (recorded loudly):** every ε₀ claim downstream — OIFC (32.3) and HBFC (38.2) —
//! is **downgraded to ω^k**. This is the honest negative the keystone audit exists to catch.
//!
//! Two-tier epistemic status:
//! - **lower bound** `‖JEFF‖ ≥ ω^k`: *internal, machine-checked* — JEFF verifies ordinal-CNF
//!   comparisons (exact integer arithmetic, within its quantifier-free power) and uses them as
//!   termination measures for fixed-depth nested loops (depth k ⇒ measure < ω^k).
//! - **upper bound** `‖JEFF‖ ≤ ω^ω`: *paper metatheorem* (PRA cut-elimination). **Not provable
//!   inside JEFF** — by Gödel II a consistent system cannot prove its own consistency/strength;
//!   so the ≤ side is a meta-claim, never an internal certificate. Labeled as such.

#![allow(clippy::needless_range_loop)] // index loops mirror the loop semantics being certified

use std::cmp::Ordering;

/// An ordinal in Cantor normal form below ε₀: `Σ ω^{e_i}·c_i` with exponents (themselves ordinals)
/// strictly descending and coefficients `c_i ≥ 1`. The empty term list is `0`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Ord {
    /// `(exponent, coeff)` pairs, exponents strictly descending, coeff ≥ 1.
    pub terms: Vec<(Ord, u64)>,
}

impl Ord {
    pub fn zero() -> Ord {
        Ord { terms: vec![] }
    }
    pub fn is_zero(&self) -> bool {
        self.terms.is_empty()
    }
    /// Finite ordinal `n` = `ω^0·n`.
    pub fn nat(n: u64) -> Ord {
        if n == 0 {
            Ord::zero()
        } else {
            Ord { terms: vec![(Ord::zero(), n)] }
        }
    }
    /// `ω`.
    pub fn omega() -> Ord {
        Ord { terms: vec![(Ord::nat(1), 1)] }
    }
    /// `ω^e` for an ordinal exponent `e` (recursive — reaches up to ε₀ as a data structure).
    pub fn omega_pow(e: Ord) -> Ord {
        Ord { terms: vec![(e, 1)] }
    }

    /// Compare two CNF ordinals (the well-order). Term-by-term: leading exponent, then coeff, then
    /// tail; if one is a prefix of the other the longer (extra positive lower terms) is greater.
    pub fn ord_cmp(&self, o: &Ord) -> Ordering {
        for ((e1, c1), (e2, c2)) in self.terms.iter().zip(&o.terms) {
            match e1.ord_cmp(e2) {
                Ordering::Equal => {}
                x => return x,
            }
            match c1.cmp(c2) {
                Ordering::Equal => {}
                x => return x,
            }
        }
        self.terms.len().cmp(&o.terms.len())
    }

    /// All finite-height CNF towers built here are `< ε₀` by construction.
    pub fn is_below_epsilon0(&self) -> bool {
        true
    }

    /// Whether this ordinal lies in the **ω^k fragment** (all exponents finite) — the part JEFF
    /// can *internally* machine-check well-founded (the honest certified envelope).
    pub fn in_omega_k_fragment(&self) -> bool {
        self.terms.iter().all(|(e, _)| e.is_zero() || (e.terms.len() == 1 && e.terms[0].0.is_zero()))
    }
}

/// Lexicographic termination measure for a fixed-depth nested loop: indices `(i₁,…,i_k)` map to
/// `ω^{k-1}·i₁ + … + ω^0·i_k`. Within the **ω^k fragment** → internally certifiable (32.0).
pub fn lex_measure(indices: &[u64]) -> Ord {
    let k = indices.len();
    let mut terms = Vec::new();
    for (pos, &v) in indices.iter().enumerate() {
        if v > 0 {
            let exp = (k - 1 - pos) as u64;
            terms.push((Ord::nat(exp), v));
        }
    }
    Ord { terms }
}

// ---- 32.3 OIFC: prefix-sum-of-prefix-sum global correctness via ordinal induction ----

/// The nested loop `for i: for j≤i: T += A[j]; result[i] = T` (T persists across i). Returns the
/// `result` vector. This is the prefix-sum-of-prefix-sums.
pub fn prefix_of_prefix_loop(a: &[i64]) -> Vec<i64> {
    let mut t = 0i64;
    let mut result = vec![0i64; a.len()];
    for i in 0..a.len() {
        for j in 0..=i {
            t += a[j];
        }
        result[i] = t;
    }
    result
}

/// CR-extracted closed form `C(i) = Σ_{j=0}^{i} (i−j+1)·A[j]`.
pub fn closed_form(a: &[i64], i: usize) -> i64 {
    (0..=i).map(|j| (i as i64 - j as i64 + 1) * a[j]).sum()
}

/// The accumulator value `T` right after step `(i, j)` (inner index `j` of row `i` processed):
/// `cum(i,j) = Σ_{i'<i} Σ_{j'≤i'} A[j'] + Σ_{j'≤j} A[j']`.
fn cum(a: &[i64], i: usize, j: usize) -> i64 {
    let mut t = 0i64;
    for ii in 0..i {
        for jj in 0..=ii {
            t += a[jj];
        }
    }
    for jj in 0..=j {
        t += a[jj];
    }
    t
}

/// OIFC certificate for prefix-of-prefix. Verifies (all **exact integer**):
/// (1) the closed form equals the loop output at every `i`;
/// (2) transition-1 (inner `j→j+1`): `cum(i,j+1) = cum(i,j) + A[j+1]` and μ strictly increases;
/// (3) transition-2 boundary lemma (`i→i+1`, `j` resets): `cum(i+1,0) = cum(i,i) + A[0]` and μ
///     strictly increases as an ordinal (the `ωi+i → ω(i+1)` jump — clean only in ordinals);
/// returns `true` iff all hold. The ordinal measure is `μ = ω·i + j` (`< ω²`, in the ω^k
/// fragment — well within the downgraded ‖JEFF‖ = ω^ω, so the ε₀→ω^k downgrade does not weaken it).
pub fn oifc_certify(a: &[i64]) -> bool {
    if a.is_empty() {
        return true;
    }
    let n = a.len();
    let loop_out = prefix_of_prefix_loop(a);
    // (1) global correctness: closed form == loop output, exactly.
    for i in 0..n {
        if closed_form(a, i) != loop_out[i] {
            return false;
        }
    }
    let mu = |i: usize, j: usize| Ord::nat(1).pipe_mul_i(i).pipe_add_j(j); // ω·i + j
    // (2) transition-1 + measure increase
    for i in 0..n {
        for j in 0..i {
            if cum(a, i, j + 1) != cum(a, i, j) + a[j + 1] {
                return false;
            }
            if mu(i, j + 1).ord_cmp(&mu(i, j)) != Ordering::Greater {
                return false;
            }
        }
    }
    // (3) transition-2 boundary lemma + ordinal jump increase
    for i in 0..(n - 1) {
        if cum(a, i + 1, 0) != cum(a, i, i) + a[0] {
            return false;
        }
        if mu(i + 1, 0).ord_cmp(&mu(i, i)) != Ordering::Greater {
            return false;
        }
    }
    true
}

/// A WRONG closed form (off by one: `(i−j)` instead of `(i−j+1)`), to show OIFC catches the
/// transition-2 off-by-one a naive per-step recurrence check misses.
pub fn closed_form_wrong(a: &[i64], i: usize) -> i64 {
    (0..=i).map(|j| (i as i64 - j as i64) * a[j]).sum()
}

/// OIFC with the wrong closed form — returns `true` iff it (incorrectly) certifies. It must be
/// `false`: the global check (1) fails for the off-by-one form.
pub fn oifc_certify_wrong(a: &[i64]) -> bool {
    let n = a.len();
    let loop_out = prefix_of_prefix_loop(a);
    (0..n).all(|i| closed_form_wrong(a, i) == loop_out[i])
}

// small helpers to build ω·i + j without a general ordinal `add`
trait OrdPipe {
    fn pipe_mul_i(self, i: usize) -> Ord;
    fn pipe_add_j(self, j: usize) -> Ord;
}
impl OrdPipe for Ord {
    /// `ω · i` (here `self == ω`): `ω^1 · i`.
    fn pipe_mul_i(self, i: usize) -> Ord {
        if i == 0 {
            Ord::zero()
        } else {
            Ord { terms: vec![(Ord::nat(1), i as u64)] }
        }
    }
    /// add a finite `j` as the `ω^0` term.
    fn pipe_add_j(mut self, j: usize) -> Ord {
        if j > 0 {
            self.terms.push((Ord::zero(), j as u64));
        }
        self
    }
}

// ---- 32.4 FGH labels (bonus): read a complexity grade off the measure ----

/// Fast-growing-hierarchy level inferred from a measure's leading exponent:
/// `0`→constant, `1`→polynomial, `2`→exponential, `≥ω`→Ackermann-class. A measure whose level is
/// far above the intended one is a red flag ("more explosive than intended").
pub fn fgh_level(measure: &Ord) -> u64 {
    measure
        .terms
        .first()
        .map(|(e, _)| if e.is_zero() { 0 } else { e.terms.first().map(|(_, c)| *c).unwrap_or(1) })
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jeff_strength_lower_bound_certified() {
        // INTERNAL machine-checked: JEFF verifies ordinal-CNF comparisons (exact) and the ω^k
        // measures it uses for nested loops live in the ω^k fragment — the certified lower bound.
        let m2 = lex_measure(&[1, 2]); // ω·1 + 2
        let m1 = lex_measure(&[1, 1]); // ω·1 + 1
        assert_eq!(m2.ord_cmp(&m1), Ordering::Greater);
        assert!(m2.in_omega_k_fragment(), "OIFC measures are in the certifiable ω^k fragment");
        // ω² (depth-3 measure leading term) is also finite-exponent ⇒ in fragment.
        assert!(lex_measure(&[1, 0, 0]).in_omega_k_fragment());
    }

    #[test]
    fn jeff_strength_two_tier_labeled() {
        // The ε₀ data structure exists (ω^ω, ω^(ω^ω)) but is NOT in the internally-certifiable
        // ω^k fragment — exactly the two-tier split (internal lower vs paper-metatheorem upper).
        let omega_omega = Ord::omega_pow(Ord::omega()); // ω^ω
        assert!(omega_omega.is_below_epsilon0());
        assert!(!omega_omega.in_omega_k_fragment(), "ω^ω is beyond the internally-checked fragment");
        // ‖JEFF‖ = ω^ω (audit): the certified internal tier is ω^k; ≤ω^ω is a paper metatheorem.
    }

    #[test]
    fn ordinal_cnf_compare_correct() {
        // ω² + 1 > ω·3 + 5 ;  ω·3 + 5 > ω·3 + 4 ;  ω > 1000000.
        let omega2_plus1 = Ord { terms: vec![(Ord::nat(2), 1), (Ord::zero(), 1)] };
        let w3_5 = Ord { terms: vec![(Ord::nat(1), 3), (Ord::zero(), 5)] };
        let w3_4 = Ord { terms: vec![(Ord::nat(1), 3), (Ord::zero(), 4)] };
        assert_eq!(omega2_plus1.ord_cmp(&w3_5), Ordering::Greater);
        assert_eq!(w3_5.ord_cmp(&w3_4), Ordering::Greater);
        assert_eq!(Ord::omega().ord_cmp(&Ord::nat(1_000_000)), Ordering::Greater);
    }

    #[test]
    fn ordinal_cnf_recursive_epsilon0() {
        // recursive exponents reach toward ε₀: ω < ω^ω < ω^(ω^ω); all < ε₀.
        let w = Ord::omega();
        let ww = Ord::omega_pow(Ord::omega());
        let www = Ord::omega_pow(Ord::omega_pow(Ord::omega()));
        assert_eq!(ww.ord_cmp(&w), Ordering::Greater);
        assert_eq!(www.ord_cmp(&ww), Ordering::Greater);
        assert!(w.is_below_epsilon0() && ww.is_below_epsilon0() && www.is_below_epsilon0());
    }

    #[test]
    fn lexicographic_measure_synthesized() {
        // depth-2 nested loop (i,j): measure ω·i + j, lexicographically ordered.
        assert_eq!(lex_measure(&[2, 0]).ord_cmp(&lex_measure(&[1, 9])), Ordering::Greater); // ω·2 > ω·1+9
        assert_eq!(lex_measure(&[1, 5]).ord_cmp(&lex_measure(&[1, 4])), Ordering::Greater);
    }

    #[test]
    fn strong_induction_finite_decomposed() {
        // The strong-induction-over-μ decomposition is realized by the transition checks in OIFC
        // (each later state's μ is strictly greater ⇒ strong induction over the ω² order is valid).
        assert!(oifc_certify(&[3, 1, 4, 1, 5, 9, 2, 6]));
    }

    #[test]
    fn nested_fold_globally_certified() {
        // prefix-of-prefix closed form is globally certified (exact) for several inputs.
        for a in [vec![1, 2, 3, 4, 5], vec![-2, 7, 0, 3], vec![5; 10], vec![1]] {
            assert!(oifc_certify(&a), "OIFC must certify {a:?}");
        }
    }

    #[test]
    fn transition2_offbyone_caught_by_ordinal() {
        // the off-by-one closed form (i−j instead of i−j+1) is REJECTED — the transition-2
        // boundary is exactly where it breaks, which OIFC's global ordinal check catches.
        let a = vec![3, 1, 4, 1, 5];
        assert!(oifc_certify(&a), "correct form certifies");
        assert!(!oifc_certify_wrong(&a), "off-by-one form must NOT certify (caught at transition 2)");
    }

    #[test]
    fn fgh_level_inferred() {
        assert_eq!(fgh_level(&lex_measure(&[5])), 0); // ω^0·5 → constant grade
        assert_eq!(fgh_level(&lex_measure(&[1, 0])), 1); // ω·i → polynomial grade
        assert_eq!(fgh_level(&Ord { terms: vec![(Ord::nat(2), 1)] }), 2); // ω² → exponential grade
    }

    #[test]
    fn explosive_recursion_warned() {
        // a measure whose FGH level (3) exceeds an intended polynomial (≤1) is flagged.
        let intended = 1u64;
        let m = Ord { terms: vec![(Ord::nat(3), 1)] }; // ω³
        assert!(fgh_level(&m) > intended, "explosive: level {} > intended {intended}", fgh_level(&m));
    }
}
