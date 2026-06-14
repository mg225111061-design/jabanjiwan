//! Multivariate rational functions over `Q`, as a `Poly` numerator over a `Poly`
//! denominator.
//!
//! Used by the holonomic checker (APPENDIX E.1/E.2, F.2): the telescoper / Gosper
//! identity is a *rational* identity; combining everything into one `num/den` and
//! checking `num ≡ 0` is exactly "clear denominators → polynomial identity →
//! coefficient-zero" — exact and solver-free (no Z3). Fractions are kept unreduced
//! (no multivariate gcd needed): `a/b ± c/d = (ad ± cb)/(bd)`, and the identity
//! holds iff the combined numerator is the zero polynomial (given a nonzero
//! denominator).

use crate::Poly;
use num_bigint::BigInt;
use num_rational::BigRational;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RatFunc {
    pub num: Poly,
    pub den: Poly,
}

impl RatFunc {
    pub fn new(num: Poly, den: Poly) -> Self {
        RatFunc { num, den }
    }

    pub fn zero() -> Self {
        RatFunc {
            num: Poly::zero(),
            den: Poly::constant(BigRational::from(BigInt::from(1))),
        }
    }

    pub fn one() -> Self {
        RatFunc {
            num: Poly::from_i64(1),
            den: Poly::from_i64(1),
        }
    }

    pub fn from_poly(p: Poly) -> Self {
        RatFunc {
            num: p,
            den: Poly::from_i64(1),
        }
    }

    pub fn from_i64(v: i64) -> Self {
        RatFunc::from_poly(Poly::from_i64(v))
    }

    pub fn mul(&self, o: &RatFunc) -> RatFunc {
        RatFunc {
            num: self.num.mul(&o.num),
            den: self.den.mul(&o.den),
        }
    }

    pub fn add(&self, o: &RatFunc) -> RatFunc {
        // a/b + c/d = (ad + cb)/(bd)
        RatFunc {
            num: self.num.mul(&o.den).add(&o.num.mul(&self.den)),
            den: self.den.mul(&o.den),
        }
    }

    pub fn sub(&self, o: &RatFunc) -> RatFunc {
        RatFunc {
            num: self.num.mul(&o.den).sub(&o.num.mul(&self.den)),
            den: self.den.mul(&o.den),
        }
    }

    pub fn neg(&self) -> RatFunc {
        RatFunc {
            num: self.num.neg(),
            den: self.den.clone(),
        }
    }

    /// `self ** e` for non-negative `e`.
    pub fn pow(&self, e: u32) -> RatFunc {
        let mut acc = RatFunc::one();
        for _ in 0..e {
            acc = acc.mul(self);
        }
        acc
    }

    pub fn recip(&self) -> RatFunc {
        RatFunc {
            num: self.den.clone(),
            den: self.num.clone(),
        }
    }

    pub fn shift_var(&self, name: &str, s: i64) -> RatFunc {
        RatFunc {
            num: self.num.shift_var(name, s),
            den: self.den.shift_var(name, s),
        }
    }

    /// `true` iff this rational function is identically zero (numerator ≡ 0). The
    /// caller is expected to also confirm the denominator is not identically zero.
    pub fn is_zero(&self) -> bool {
        self.num.is_zero()
    }

    pub fn den_is_zero(&self) -> bool {
        self.den.is_zero()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_sub_combine_to_zero() {
        // k/(k+1) - k/(k+1) = 0
        let k = Poly::var("k");
        let kp1 = k.add(&Poly::from_i64(1));
        let f = RatFunc::new(k.clone(), kp1.clone());
        let g = RatFunc::new(k, kp1);
        assert!(f.sub(&g).is_zero());
    }

    #[test]
    fn telescoping_rational_identity() {
        // 1/k - 1/(k+1) = 1/(k(k+1)) ; check LHS - RHS == 0 as rational identity.
        let k = Poly::var("k");
        let kp1 = k.add(&Poly::from_i64(1));
        let lhs = RatFunc::new(Poly::from_i64(1), k.clone())
            .sub(&RatFunc::new(Poly::from_i64(1), kp1.clone()));
        let rhs = RatFunc::new(Poly::from_i64(1), k.mul(&kp1));
        assert!(lhs.sub(&rhs).is_zero());
    }

    #[test]
    fn shift_changes_value() {
        // f = 1/k ; shift k->k+1 gives 1/(k+1) ; difference is nonzero
        let k = Poly::var("k");
        let f = RatFunc::new(Poly::from_i64(1), k.clone());
        let fs = f.shift_var("k", 1);
        assert!(!f.sub(&fs).is_zero());
    }
}
