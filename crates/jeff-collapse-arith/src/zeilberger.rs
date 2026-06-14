//! Zeilberger's creative telescoping for definite hypergeometric sums (CLAUDE.md
//! APPENDIX E.2, F.2).
//!
//! Goal: for `F(n,k)`, find a telescoper `L = Σ_{i=0}^{J} a_i(n) N^i` (not all
//! `a_i` zero) and a rational certificate `R(n,k)` with
//!
//! ```text
//!   R(n,k+1)·ρ_k − R(n,k) − Σ_i a_i(n)·F(n+i,k)/F(n,k) ≡ 0.
//! ```
//!
//! Method (undetermined coefficients + exact null space): we write `a_i(n)` and the
//! numerator of `R(n,k)` with *unknown* coefficients (fresh polynomial variables
//! `u0,u1,…`), compute the telescoper-certificate numerator (a polynomial **linear**
//! in the unknowns via `jeff_math::hyper`), and require it to vanish identically in
//! `n,k`. Each `(n,k)`-monomial gives one homogeneous linear equation in the
//! unknowns; the null space yields candidate `(L,R)`. The denominator of `R` is
//! taken from the term's `n`-shift structure (a small candidate set is tried).
//!
//! Safety (P0/R2): the search is *untrusted*. Every candidate is re-checked by
//! `jeff_math::hyper::telescoper_holds`; only a verified telescoper is returned. A
//! search miss is a (safe) HONEST_DEFER, never a wrong answer.

use jeff_math::hyper::{telescoper_holds, HyperTerm};
use jeff_math::{Poly, RatFunc};
use num_bigint::BigInt;
use num_rational::BigRational;

const N: &str = "n";
const K: &str = "k";

/// A found telescoper: operator coefficients `a_i(n)` and certificate `R(n,k)`.
pub struct Telescoper {
    pub l: Vec<RatFunc>,
    pub r: RatFunc,
}

/// Search bounds (kept small; the fixtures need J≤1, low degrees). Larger searches
/// are deferred rather than run unboundedly (R23).
pub struct Bounds {
    pub max_order: usize,
    pub max_rnum_deg_k: usize,
    pub max_coeff_deg_n: usize,
}

impl Default for Bounds {
    fn default() -> Self {
        Bounds {
            max_order: 2,
            max_rnum_deg_k: 4,
            max_coeff_deg_n: 3,
        }
    }
}

fn uvar(i: usize) -> String {
    format!("u{i}")
}
fn is_uvar(s: &str) -> Option<usize> {
    s.strip_prefix('u').and_then(|r| r.parse::<usize>().ok())
}

/// Build `Σ_{d=0}^{deg} u_{start+d} · n^d` and advance the unknown counter.
fn poly_in_n(start: usize, deg: usize) -> (Poly, usize) {
    let mut p = Poly::zero();
    for d in 0..=deg {
        let mut mono = Poly::var(&uvar(start + d));
        if d > 0 {
            mono = mono.mul(&Poly::var(N).pow(d as u32));
        }
        p = p.add(&mono);
    }
    (p, start + deg + 1)
}

/// Build `Σ_{dk=0}^{degk} Σ_{dn=0}^{degn} u · n^dn k^dk`.
fn poly_in_nk(start: usize, degk: usize, degn: usize) -> (Poly, usize) {
    let mut p = Poly::zero();
    let mut idx = start;
    for dk in 0..=degk {
        for dn in 0..=degn {
            let mut mono = Poly::var(&uvar(idx));
            if dn > 0 {
                mono = mono.mul(&Poly::var(N).pow(dn as u32));
            }
            if dk > 0 {
                mono = mono.mul(&Poly::var(K).pow(dk as u32));
            }
            p = p.add(&mono);
            idx += 1;
        }
    }
    (p, idx)
}

/// Try to find a telescoper for `term` within `bounds`.
pub fn zeilberger(term: &HyperTerm, bounds: &Bounds) -> Option<Telescoper> {
    let rho_n = term.ratio_n();
    let rho_k = term.ratio_k();
    // Candidate denominators for R(n,k), drawn from the term's shift structure.
    let den_candidates = vec![
        rho_n.den.clone(),
        rho_n.den.mul(&rho_k.den),
        rho_n.den.mul(&rho_n.den),
        Poly::from_i64(1),
    ];
    for order in 1..=bounds.max_order {
        for rden in &den_candidates {
            if rden.is_zero() {
                continue;
            }
            for drk in 0..=bounds.max_rnum_deg_k {
                for dcn in 0..=bounds.max_coeff_deg_n {
                    if let Some(t) = try_ansatz(term, order, rden, drk, dcn) {
                        // P0: never trust the search — re-check independently.
                        if telescoper_holds(term, &t.l, &t.r) {
                            return Some(t);
                        }
                    }
                }
            }
        }
    }
    None
}

fn try_ansatz(
    term: &HyperTerm,
    order: usize,
    rden: &Poly,
    drk: usize,
    dcn: usize,
) -> Option<Telescoper> {
    // Unknowns: a_0..a_order (each degree dcn in n), then Rnum (degk=drk, degn=dcn).
    let mut next = 0usize;
    let mut a_polys = Vec::new();
    let mut a_unknown_ranges = Vec::new();
    for _ in 0..=order {
        let lo = next;
        let (p, nn) = poly_in_n(next, dcn);
        next = nn;
        a_unknown_ranges.push((lo, next)); // [lo, next) are this a_i's unknowns
        a_polys.push(p);
    }
    let (rnum, total_unknowns) = poly_in_nk(next, drk, dcn);

    let l: Vec<RatFunc> = a_polys.iter().cloned().map(RatFunc::from_poly).collect();
    let r = RatFunc::new(rnum, rden.clone());

    // Symbolic certificate numerator: a Poly in {n, k, u0..u_{M-1}}, linear in u's.
    let total = jeff_math::hyper::telescoper_certificate(term, &l, &r);
    if total.den_is_zero() {
        return None;
    }
    let numer = &total.num;

    // Build the homogeneous linear system: for each (n,k)-monomial, the sum of
    // (coeff · u_j) must be zero.
    use std::collections::BTreeMap;
    let mut rows: BTreeMap<Vec<(String, u32)>, Vec<BigRational>> = BTreeMap::new();
    for (mono, coeff) in numer.monomials() {
        // split into nk-part and the (single) u-variable
        let mut nk_part: Vec<(String, u32)> = Vec::new();
        let mut uvar_idx: Option<usize> = None;
        let mut udeg = 0u32;
        for (v, e) in mono {
            if let Some(idx) = is_uvar(v) {
                uvar_idx = Some(idx);
                udeg += *e;
            } else {
                nk_part.push((v.clone(), *e));
            }
        }
        if udeg == 0 {
            // u-free term: must be zero for any u ⇒ this ansatz cannot work.
            if !coeff_is_zero(coeff) {
                return None;
            }
            continue;
        }
        if udeg != 1 {
            return None; // non-linear in unknowns (should not happen)
        }
        let row = rows
            .entry(nk_part)
            .or_insert_with(|| vec![BigRational::from(BigInt::from(0)); total_unknowns]);
        let j = uvar_idx.unwrap();
        row[j] += coeff;
    }

    let matrix: Vec<Vec<BigRational>> = rows.into_values().collect();
    if matrix.is_empty() {
        return None;
    }
    let basis = jeff_math::linsolve::nullspace(&matrix);

    // Pick a null-space vector with NOT all a_i unknowns zero.
    for sol in &basis {
        if a_unknown_ranges
            .iter()
            .any(|&(lo, hi)| (lo..hi).any(|j| !coeff_is_zero(&sol[j])))
        {
            return Some(substitute(&a_polys, &r, sol, total_unknowns));
        }
    }
    None
}

fn coeff_is_zero(c: &BigRational) -> bool {
    use num_traits::Zero;
    c.is_zero()
}

/// Substitute a solution vector for the unknowns `u0..u_{M-1}` into the ansatz.
fn substitute(
    a_polys: &[Poly],
    r: &RatFunc,
    sol: &[BigRational],
    m: usize,
) -> Telescoper {
    let subst = |p: &Poly| -> Poly {
        let mut out = p.clone();
        for (j, val) in sol.iter().enumerate().take(m) {
            out = out.subst_var(&uvar(j), val);
        }
        out
    };
    let l = a_polys.iter().map(|p| RatFunc::from_poly(subst(p))).collect();
    let r2 = RatFunc::new(subst(&r.num), r.den.clone());
    Telescoper { l, r: r2 }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jeff_math::hyper::LinForm;

    fn binom_nk() -> HyperTerm {
        HyperTerm {
            coeff: BigRational::from(BigInt::from(1)),
            z_k: BigRational::from(BigInt::from(1)),
            poly: Poly::from_i64(1),
            gammas: vec![
                (LinForm::new(1, 0, 1), 1),
                (LinForm::new(0, 1, 1), -1),
                (LinForm::new(1, -1, 1), -1),
            ],
        }
    }

    fn binom_nk_squared() -> HyperTerm {
        binom_nk().pow(2)
    }

    #[test]
    fn finds_telescoper_for_sum_binomial() {
        // Σ_k C(n,k) = 2^n : Zeilberger must find an order-1 telescoper (checker-validated).
        let term = binom_nk();
        let t = zeilberger(&term, &Bounds::default()).expect("telescoper found");
        assert!(telescoper_holds(&term, &t.l, &t.r));
        assert_eq!(t.l.len(), 2); // first order
    }

    #[test]
    fn finds_telescoper_for_sum_binomial_squared() {
        // Σ_k C(n,k)^2 = C(2n,n) : order-1 telescoper (n+1)S(n+1)=(4n+2)S(n).
        let term = binom_nk_squared();
        let t = zeilberger(&term, &Bounds::default()).expect("telescoper found");
        assert!(telescoper_holds(&term, &t.l, &t.r));
    }
}
