//! Stage 26.3 — definite holonomic sum evaluation via creative telescoping.
//!
//! For `S(n) = Σ_{k} F(n,k)` with `F` hypergeometric, Zeilberger's telescoper is an operator
//! `Σ_{i=0}^{J} a_i(n)·S(n+i) = 0` (polynomial coefficients `a_i`). The *search* lives in the
//! collapser ([`jeff_collapse_arith::zeilberger`]) and the *certificate checker* in
//! [`crate::hyper`] (a rational-function identity, no SMT). This module is the **evaluation**
//! side: a naive `Σ_k` oracle and the telescoper-recurrence evaluator, so the certified
//! recurrence can be checked against the direct sum and measured.
//!
//! Collapse picture: the naive table `S(0..M)` costs `Σ_m Θ(m) = Θ(M²)`; the order-`J`
//! recurrence produces it in `Θ(M)` (each new term from the previous `J`). When the telescoper
//! is *constant-coefficient* (C-finite, e.g. `Σ_k C(n,k)=2^n` ⇒ `S(n+1)=2S(n)`), a single
//! `S(n)` collapses to `O(log n)` via [`crate::cfinite`]. Both ratios diverge with the size
//! argument and the output is small (a value, or the Θ(M) table computed optimally) — Ω(N)-safe.

use crate::cfinite::cfinite_bostan;
use crate::hyper::HyperTerm;
use crate::ratfun::RatFunc;
use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::{One, ToPrimitive, Zero};
use std::collections::BTreeMap;

fn factorial(m: i64) -> BigInt {
    let mut f = BigInt::one();
    let mut i = 2i64;
    while i <= m {
        f *= BigInt::from(i);
        i += 1;
    }
    f
}

fn pow_rat(b: &BigRational, e: u32) -> BigRational {
    let mut acc = BigRational::one();
    for _ in 0..e {
        acc *= b;
    }
    acc
}

/// Evaluate a hypergeometric term `F(n,k)` at integer `(n,k)` exactly over ℚ. A `Γ` at a
/// non-positive integer in a *denominator* contributes a zero term (the binomial support
/// boundary, e.g. `C(n,k)=0` for `k>n`); a `Γ` pole in a *numerator* is ill-defined → `None`.
pub fn eval_hyperterm(term: &HyperTerm, n: i64, k: i64) -> Option<BigRational> {
    if k < 0 {
        return None;
    }
    let mut env = BTreeMap::new();
    env.insert("n".to_string(), BigRational::from(BigInt::from(n)));
    env.insert("k".to_string(), BigRational::from(BigInt::from(k)));
    let polyval = term.poly.eval(&env)?;
    let mut val = &term.coeff * &polyval;
    val *= pow_rat(&term.z_k, k as u32);
    for (l, e) in &term.gammas {
        let m = l.n * n + l.k * k + l.c; // Γ(m) = (m−1)!
        if m < 1 {
            if *e < 0 {
                return Some(BigRational::zero()); // 1/Γ(pole) = 0  (out of support)
            }
            return None; // Γ(pole) in numerator — undefined here
        }
        let fr = BigRational::from(factorial(m - 1));
        if *e >= 0 {
            val *= pow_rat(&fr, *e as u32);
        } else {
            val /= pow_rat(&fr, (-*e) as u32);
        }
    }
    Some(val)
}

/// Naive definite sum `S(n) = Σ_{k=0}^{n} F(n,k)` over ℚ. **O(n)** terms (the incumbent).
pub fn naive_sum(term: &HyperTerm, n: i64) -> Option<BigRational> {
    let mut acc = BigRational::zero();
    for k in 0..=n.max(0) {
        acc += eval_hyperterm(term, n, k)?;
    }
    Some(acc)
}

/// Evaluate a rational-function operator coefficient `a_i(n)` at an integer `n`.
fn eval_ratfunc_n(r: &RatFunc, n: i64) -> Option<BigRational> {
    let mut env = BTreeMap::new();
    env.insert("n".to_string(), BigRational::from(BigInt::from(n)));
    let num = r.num.eval(&env)?;
    let den = r.den.eval(&env)?;
    if den.is_zero() {
        return None;
    }
    Some(num / den)
}

/// Produce the table `S(0..=m)` from the telescoper operator `l = [a_0,…,a_J]`
/// (`Σ_i a_i(n) S(n+i) = 0`): the first `J` values come from the naive oracle (base cases),
/// the rest from the recurrence `S(n+J) = −(Σ_{i<J} a_i(n) S(n+i)) / a_J(n)`. **O(m·J)** field
/// ops vs the naive table's **O(m²)**.
pub fn telescoper_table(term: &HyperTerm, l: &[RatFunc], m: usize) -> Option<Vec<BigRational>> {
    let order = l.len().checked_sub(1)?;
    if order == 0 {
        return None;
    }
    let mut s: Vec<BigRational> = Vec::with_capacity(m + 1);
    for nn in 0..order {
        if s.len() > m {
            break;
        }
        s.push(naive_sum(term, nn as i64)?);
    }
    let mut n = 0i64;
    while s.len() <= m {
        let a_top = eval_ratfunc_n(&l[order], n)?;
        if a_top.is_zero() {
            return None;
        }
        let mut acc = BigRational::zero();
        for (i, li) in l.iter().enumerate().take(order) {
            acc += eval_ratfunc_n(li, n)? * &s[n as usize + i];
        }
        s.push(-acc / a_top);
        n += 1;
    }
    s.truncate(m + 1);
    Some(s)
}

/// If the telescoper is *constant-coefficient* (each `a_i(n)` a constant), return the C-finite
/// recurrence `(c, init)` with `S(m) = Σ_j c_j S(m-j)` modulo `q`, enabling an `O(log n)`
/// single-term evaluation via [`cfinite_bostan`]. `None` if any coefficient is non-constant
/// (genuinely P-finite — evaluate via [`telescoper_table`] instead; honest, no forcing).
pub fn telescoper_as_cfinite(
    term: &HyperTerm,
    l: &[RatFunc],
    q: u64,
) -> Option<(Vec<u64>, Vec<u64>)> {
    let order = l.len().checked_sub(1)?;
    if order == 0 {
        return None;
    }
    // each coefficient must be a constant rational (degree 0 in n, constant denominator).
    let mut consts = Vec::with_capacity(l.len());
    for li in l {
        let c = eval_ratfunc_n(li, 0)?;
        // verify it is genuinely constant by checking another point.
        if eval_ratfunc_n(li, 1)? != c {
            return None;
        }
        consts.push(c);
    }
    let to_modu = |r: &BigRational| -> Option<u64> {
        // r must be an integer mod q: num * den^{-1} mod q.
        let qn = BigInt::from(q);
        let num = ((r.numer() % &qn) + &qn) % &qn;
        let den = ((r.denom() % &qn) + &qn) % &qn;
        let den_u = den.to_u64()?;
        let inv = crate::modular::ModInt::new(den_u, q).inv()?;
        let num_u = num.to_u64()?;
        Some((crate::modular::ModInt::new(num_u, q) * inv).val)
    };
    // S(m) = Σ_{j=1}^J c_j S(m-j), c_j = −a_{J-j}/a_J
    let a_top = &consts[order];
    let mut c = Vec::with_capacity(order);
    for j in 1..=order {
        let cj = -(&consts[order - j]) / a_top;
        c.push(to_modu(&cj)?);
    }
    let mut init = Vec::with_capacity(order);
    for nn in 0..order {
        let s = naive_sum(term, nn as i64)?;
        init.push(to_modu(&s)?);
    }
    Some((c, init))
}

/// `S(n) mod q` via the constant-coefficient telescoper collapsed to C-finite (`O(log n)`),
/// or `None` if the telescoper is not constant-coefficient.
pub fn telescoper_nth_mod(term: &HyperTerm, l: &[RatFunc], n: u64, q: u64) -> Option<u64> {
    let (c, init) = telescoper_as_cfinite(term, l, q)?;
    Some(cfinite_bostan(&c, &init, n, q))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hyper::LinForm;
    use crate::Poly;

    // C(n,k) = Γ(n+1)/(Γ(k+1)Γ(n−k+1)).
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
    fn ints(v: i64) -> BigRational {
        BigRational::from(BigInt::from(v))
    }

    #[test]
    fn naive_sum_hand_checked() {
        // Σ_k C(n,k) = 2^n.
        for n in 0..=12i64 {
            assert_eq!(naive_sum(&binom_nk(), n).unwrap(), ints(1i64 << n), "2^{n}");
        }
        // Σ_k k·C(n,k) = n·2^{n-1}: term k·C(n,k) (poly factor k).
        let mut kc = binom_nk();
        kc.poly = Poly::var("k");
        for n in 1..=10i64 {
            assert_eq!(naive_sum(&kc, n).unwrap(), ints(n * (1i64 << (n - 1))));
        }
    }

    #[test]
    fn telescoper_recurrence_matches_naive_sum() {
        // Σ_k C(n,k): telescoper S(n+1) − 2 S(n) = 0  ⇒ l = [−2, 1].
        let term = binom_nk();
        let l = vec![RatFunc::from_i64(-2), RatFunc::from_i64(1)];
        let tab = telescoper_table(&term, &l, 30).unwrap();
        for (n, s) in tab.iter().enumerate() {
            assert_eq!(*s, naive_sum(&term, n as i64).unwrap(), "n={n}");
        }
    }

    #[test]
    fn telescoper_recurrence_matches_naive_sum_binom_squared() {
        // Σ_k C(n,k)^2 = C(2n,n): telescoper (n+1)S(n+1) − (4n+2)S(n) = 0 (polynomial coeffs).
        // a_0(n) = −(4n+2), a_1(n) = n+1.
        let term = binom_nk().pow(2);
        let l = vec![
            RatFunc::from_poly(Poly::var("n").scale(&ints(-4)).add(&Poly::from_i64(-2))),
            RatFunc::from_poly(Poly::var("n").add(&Poly::from_i64(1))),
        ];
        let tab = telescoper_table(&term, &l, 20).unwrap();
        for (n, s) in tab.iter().enumerate() {
            assert_eq!(*s, naive_sum(&term, n as i64).unwrap(), "Σ C({n},k)^2");
        }
    }

    #[test]
    fn constant_coeff_telescoper_collapses_to_cfinite() {
        // Σ_k C(n,k)=2^n single value via C-finite O(log n), checked vs naive mod q.
        const Q: u64 = 1_000_000_007;
        let term = binom_nk();
        let l = vec![RatFunc::from_i64(-2), RatFunc::from_i64(1)];
        for &n in &[0u64, 1, 5, 12, 30] {
            let viacf = telescoper_nth_mod(&term, &l, n, Q).unwrap();
            let naive = (crate::modular::ModInt::new(2, Q).pow(n)).val;
            assert_eq!(viacf, naive, "2^{n} mod q via cfinite");
        }
    }

    #[test]
    fn polynomial_coeff_telescoper_is_not_cfinite() {
        // Σ_k C(n,k)^2 has polynomial-coefficient telescoper ⇒ not C-finite (honest: no forced
        // O(log n); the P-finite table path is used instead).
        let term = binom_nk().pow(2);
        let l = vec![
            RatFunc::from_poly(Poly::var("n").scale(&ints(-4)).add(&Poly::from_i64(-2))),
            RatFunc::from_poly(Poly::var("n").add(&Poly::from_i64(1))),
        ];
        assert!(telescoper_as_cfinite(&term, &l, 1_000_000_007).is_none());
    }
}
