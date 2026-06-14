//! Hypergeometric terms and the holonomic certificate *checker math*.
//!
//! Authority: CLAUDE.md 10.2, APPENDIX E.1 (Gosper), E.2 (Zeilberger), F.2.
//!
//! A proper-hypergeometric term in variables `n`, `k`:
//!
//! ```text
//!   F(n,k) = coeff · z^k · poly(n,k) · Π_j Γ(L_j(n,k))^{e_j}
//! ```
//!
//! where each `L_j = α·n + β·k + γ` is an integer linear form (a factorial/Pochhammer
//! argument) and `e_j ∈ ℤ`. Binomials and factorials are exactly this shape, so the
//! **shift ratios** `F(n,k+1)/F(n,k)` and `F(n+1,k)/F(n,k)` are *rational functions*
//! computed here exactly. The telescoper/Gosper certificate is then a rational
//! identity, discharged by [`crate::ratfun::RatFunc::is_zero`] (coefficient-zero,
//! no SMT). This module is the **independent checker** used by `jeff-verify`; the
//! Gosper/Zeilberger *search* lives in the collapser and is only trusted insofar as
//! its output passes this check (R2/R25).

use crate::ratfun::RatFunc;
use crate::Poly;
use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::Zero;
use serde::{Deserialize, Serialize};

pub const N: &str = "n";
pub const K: &str = "k";

/// An integer linear form `α·n + β·k + γ` (a Γ argument).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinForm {
    pub n: i64,
    pub k: i64,
    pub c: i64,
}

impl LinForm {
    pub fn new(n: i64, k: i64, c: i64) -> Self {
        LinForm { n, k, c }
    }
    pub fn to_poly(self) -> Poly {
        let mut p = Poly::constant(BigRational::from(BigInt::from(self.c)));
        if self.n != 0 {
            p = p.add(&Poly::var(N).scale(&BigRational::from(BigInt::from(self.n))));
        }
        if self.k != 0 {
            p = p.add(&Poly::var(K).scale(&BigRational::from(BigInt::from(self.k))));
        }
        p
    }
}

/// A hypergeometric term (see module docs). Serializable so it can live in a
/// certificate and be replayed (R25).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HyperTerm {
    pub coeff: BigRational,
    /// Base `z` of a `z^k` geometric factor (1 if none).
    pub z_k: BigRational,
    /// Polynomial factor in n,k (1 if none).
    pub poly: Poly,
    /// `Π Γ(L)^e` factorial factors.
    pub gammas: Vec<(LinForm, i64)>,
}

impl HyperTerm {
    pub fn one() -> Self {
        HyperTerm {
            coeff: BigRational::from(BigInt::from(1)),
            z_k: BigRational::from(BigInt::from(1)),
            poly: Poly::from_i64(1),
            gammas: Vec::new(),
        }
    }

    pub fn mul(&self, o: &HyperTerm) -> HyperTerm {
        let mut gammas = self.gammas.clone();
        for (l, e) in &o.gammas {
            gammas.push((*l, *e));
        }
        HyperTerm {
            coeff: &self.coeff * &o.coeff,
            z_k: &self.z_k * &o.z_k,
            poly: self.poly.mul(&o.poly),
            gammas,
        }
    }

    /// `self ** e` for a non-negative integer exponent.
    pub fn pow(&self, e: u32) -> HyperTerm {
        let mut acc = HyperTerm::one();
        for _ in 0..e {
            acc = acc.mul(self);
        }
        acc
    }

    /// `Γ(L + s)/Γ(L)` raised to `e`, as a rational function.
    fn gamma_shift(l: LinForm, e: i64, s: i64) -> RatFunc {
        if s == 0 || e == 0 {
            return RatFunc::one();
        }
        let lp = l.to_poly();
        // base = Γ(L+s)/Γ(L)
        let base = if s > 0 {
            // Π_{j=0}^{s-1} (L + j)
            let mut num = Poly::from_i64(1);
            for j in 0..s {
                num = num.mul(&lp.add(&Poly::constant(BigRational::from(BigInt::from(j)))));
            }
            RatFunc::new(num, Poly::from_i64(1))
        } else {
            // s<0, m=-s : 1 / Π_{j=1}^{m} (L - j)
            let m = -s;
            let mut den = Poly::from_i64(1);
            for j in 1..=m {
                den = den.mul(&lp.sub(&Poly::constant(BigRational::from(BigInt::from(j)))));
            }
            RatFunc::new(Poly::from_i64(1), den)
        };
        if e > 0 {
            base.pow(e as u32)
        } else {
            base.recip().pow((-e) as u32)
        }
    }

    /// `F(n, k+1) / F(n, k)` as a rational function (the `k` shift ratio).
    pub fn ratio_k(&self) -> RatFunc {
        // z^{k+1}/z^k = z
        let mut r = RatFunc::from_poly(Poly::constant(self.z_k.clone()));
        // poly(n,k+1)/poly(n,k)
        r = r.mul(&RatFunc::new(self.poly.shift_var(K, 1), self.poly.clone()));
        // Γ factors: k→k+1 shifts each argument by its k-coefficient.
        for (l, e) in &self.gammas {
            r = r.mul(&Self::gamma_shift(*l, *e, l.k));
        }
        r
    }

    /// `F(n+1, k) / F(n, k)` as a rational function (the `n` shift ratio).
    pub fn ratio_n(&self) -> RatFunc {
        // z^k independent of n → factor 1.
        let mut r = RatFunc::new(self.poly.shift_var(N, 1), self.poly.clone());
        for (l, e) in &self.gammas {
            r = r.mul(&Self::gamma_shift(*l, *e, l.n));
        }
        r
    }
}

/// The telescoper-certificate numerator (CLAUDE.md E.2 / F.2):
///
/// ```text
///   R(n,k+1)·ρ_k(n,k) − R(n,k) − Σ_{i=0}^{J} a_i(n)·[F(n+i,k)/F(n,k)]
/// ```
///
/// where `ρ_k = F(n,k+1)/F(n,k)` and `F(n+i,k)/F(n,k) = Π_{j<i} ρ_n(n+j,k)`. The
/// telescoper relation holds **iff** this combined rational function's numerator is
/// the zero polynomial (with a nonzero denominator). `l[i]` is `a_i(n)` as a
/// polynomial in `n` only (its `k`-coefficients must be zero, enforced by the
/// caller's representation). For indefinite Gosper use `l = [1]` and an `n`-free
/// term, which reduces the third term to `1` (the antidifference relation).
pub fn telescoper_certificate(term: &HyperTerm, l: &[RatFunc], r: &RatFunc) -> RatFunc {
    let rho_k = term.ratio_k();
    let rho_n = term.ratio_n();

    let mut total = r.shift_var(K, 1).mul(&rho_k).sub(r);

    // Σ_i a_i(n) · ρ_n^{(i)}
    let mut rn_power = RatFunc::one(); // ρ_n^{(0)} = 1
    for (i, ai) in l.iter().enumerate() {
        if i > 0 {
            // multiply by ρ_n(n + (i-1), k)
            rn_power = rn_power.mul(&rho_n.shift_var(N, (i - 1) as i64));
        }
        total = total.sub(&ai.mul(&rn_power));
    }
    total
}

/// Convenience: is the candidate telescoper certificate valid? (numerator ≡ 0 and
/// denominator ≢ 0). This is exactly what the `jeff-verify` checker calls.
pub fn telescoper_holds(term: &HyperTerm, l: &[RatFunc], r: &RatFunc) -> bool {
    let total = telescoper_certificate(term, l, r);
    !total.den_is_zero() && total.is_zero()
}

/// The Gosper antidifference relation as a special case (`l = [1]`, `n`-free term):
/// `R(k+1)·ρ_k − R(k) − 1 ≡ 0`.
pub fn gosper_holds(term: &HyperTerm, r: &RatFunc) -> bool {
    let _ = BigRational::is_zero(&BigRational::zero());
    telescoper_holds(term, &[RatFunc::one()], r)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// C(n,k) = Γ(n+1)/(Γ(k+1)Γ(n-k+1)).
    fn binom_nk() -> HyperTerm {
        HyperTerm {
            coeff: BigRational::from(BigInt::from(1)),
            z_k: BigRational::from(BigInt::from(1)),
            poly: Poly::from_i64(1),
            gammas: vec![
                (LinForm::new(1, 0, 1), 1),  // Γ(n+1)
                (LinForm::new(0, 1, 1), -1), // 1/Γ(k+1)
                (LinForm::new(1, -1, 1), -1), // 1/Γ(n-k+1)
            ],
        }
    }

    #[test]
    fn binomial_k_ratio_is_nk_over_kp1() {
        // F(n,k+1)/F(n,k) for C(n,k) is (n-k)/(k+1).
        let rho = binom_nk().ratio_k();
        let expect = RatFunc::new(
            Poly::var(N).sub(&Poly::var(K)),
            Poly::var(K).add(&Poly::from_i64(1)),
        );
        // rho - expect == 0
        assert!(rho.sub(&expect).is_zero(), "rho={:?}", rho.num.to_canonical_string());
    }

    #[test]
    fn binomial_n_ratio_is_np1_over_np1mk() {
        // F(n+1,k)/F(n,k) for C(n,k) is (n+1)/(n+1-k).
        let rho = binom_nk().ratio_n();
        let expect = RatFunc::new(
            Poly::var(N).add(&Poly::from_i64(1)),
            Poly::var(N).add(&Poly::from_i64(1)).sub(&Poly::var(K)),
        );
        assert!(rho.sub(&expect).is_zero());
    }

    #[test]
    fn known_telescoper_for_sum_binomial_eq_2n() {
        // Σ_k C(n,k) = 2^n : telescoper S(n+1) - 2 S(n) = 0 (l = [-2, 1]) with the
        // classical certificate R(n,k) = -k/(n+1-k). Hand-verified (see F.2 method).
        let term = binom_nk();
        let l = vec![RatFunc::from_i64(-2), RatFunc::from_i64(1)];
        let r = RatFunc::new(
            Poly::var(K).neg(),
            Poly::var(N).add(&Poly::from_i64(1)).sub(&Poly::var(K)),
        );
        assert!(telescoper_holds(&term, &l, &r), "known certificate must verify");
    }

    #[test]
    fn wrong_telescoper_is_rejected() {
        // Same term, but a WRONG operator / certificate must NOT verify (DR1/DR7).
        let term = binom_nk();
        // wrong operator coefficient (-3 instead of -2)
        let bad_l = vec![RatFunc::from_i64(-3), RatFunc::from_i64(1)];
        let good_r = RatFunc::new(
            Poly::var(K).neg(),
            Poly::var(N).add(&Poly::from_i64(1)).sub(&Poly::var(K)),
        );
        assert!(!telescoper_holds(&term, &bad_l, &good_r));

        // wrong certificate (denominator n-k instead of n+1-k)
        let good_l = vec![RatFunc::from_i64(-2), RatFunc::from_i64(1)];
        let bad_r = RatFunc::new(Poly::var(K).neg(), Poly::var(N).sub(&Poly::var(K)));
        assert!(!telescoper_holds(&term, &good_l, &bad_r));
    }
}
