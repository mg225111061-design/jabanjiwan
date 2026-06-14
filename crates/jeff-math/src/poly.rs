//! Polynomials over the rationals.
//!
//! Two flavours:
//!   * [`Poly`]  — multivariate, the workhorse for *identity checking*. A
//!     `PolynomialIdentity` certificate (CLAUDE.md PART 11 / APPENDIX F.1) is
//!     discharged by building the difference polynomial, normalising to canonical
//!     form, and checking every coefficient is zero. This is the quantifier-free,
//!     "more robust" variant the constitution explicitly endorses in F.1 — sound
//!     and deterministic (R11), no external SMT solver, no hang (R23).
//!   * [`UniPoly`] — univariate, with division / gcd / resultant, used by the
//!     Gosper & Zeilberger algorithms (APPENDIX E.1/E.2).
//!
//! All arithmetic is exact over `BigRational` (R33: no float on the collapse path).

use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::{One, Zero};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::str::FromStr;

/// Parse a `BigRational` from its canonical string ("a" or "a/b"). Used by the
/// serde round-trip so emitted certificates are replayable from disk (R25).
fn parse_rational(s: &str) -> BigRational {
    BigRational::from_str(s).unwrap_or_else(|_| {
        // num-rational only parses "a/b"; integers like "5" parse via BigInt.
        BigRational::from(BigInt::from_str(s).expect("invalid rational in certificate"))
    })
}

/// A monomial: a sorted list of `(variable, exponent)` with `exponent > 0`.
/// The empty vector is the constant monomial `1`. Canonical ordering (by the
/// `BTreeMap` key) gives deterministic output (R11).
pub type Monomial = Vec<(String, u32)>;

fn norm_monomial(mut m: Monomial) -> Monomial {
    m.retain(|(_, e)| *e > 0);
    m.sort_by(|a, b| a.0.cmp(&b.0));
    // merge duplicate variables
    let mut out: Monomial = Vec::with_capacity(m.len());
    for (v, e) in m {
        if let Some(last) = out.last_mut() {
            if last.0 == v {
                last.1 += e;
                continue;
            }
        }
        out.push((v, e));
    }
    out
}

/// Multivariate polynomial over `BigRational`, stored as a canonical map from
/// monomial to nonzero coefficient.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(into = "PolySer", from = "PolySer")]
pub struct Poly {
    terms: BTreeMap<Monomial, BigRational>,
}

/// Serde shadow for [`Poly`]: a sequence of `(monomial, coeff-as-string)` so the
/// JSON is deterministic (R11) and replayable (R25) without needing string map
/// keys. Rationals are rendered as canonical strings.
#[derive(Serialize, Deserialize)]
struct PolySer {
    terms: Vec<(Monomial, String)>,
}

impl From<Poly> for PolySer {
    fn from(p: Poly) -> Self {
        PolySer {
            terms: p
                .terms
                .into_iter()
                .map(|(m, c)| (m, c.to_string()))
                .collect(),
        }
    }
}

impl From<PolySer> for Poly {
    fn from(s: PolySer) -> Self {
        let mut p = Poly::zero();
        for (m, c) in s.terms {
            p.add_term(m, parse_rational(&c));
        }
        p
    }
}

impl Poly {
    pub fn zero() -> Self {
        Poly {
            terms: BTreeMap::new(),
        }
    }

    pub fn constant(c: BigRational) -> Self {
        let mut p = Poly::zero();
        if !c.is_zero() {
            p.terms.insert(Vec::new(), c);
        }
        p
    }

    pub fn from_i64(c: i64) -> Self {
        Poly::constant(BigRational::from(BigInt::from(c)))
    }

    /// The polynomial `x` for a single variable name.
    pub fn var(name: &str) -> Self {
        let mut p = Poly::zero();
        p.terms
            .insert(vec![(name.to_string(), 1)], BigRational::one());
        p
    }

    /// Insert (add) a single term, keeping canonical form: a coefficient that
    /// cancels to zero is removed so that `PartialEq` and `is_zero` stay reliable.
    fn add_term(&mut self, mono: Monomial, coeff: BigRational) {
        if coeff.is_zero() {
            return;
        }
        let mono = norm_monomial(mono);
        let became_zero = {
            let entry = self
                .terms
                .entry(mono.clone())
                .or_insert_with(BigRational::zero);
            *entry += coeff;
            entry.is_zero()
        };
        if became_zero {
            self.terms.remove(&mono);
        }
    }

    fn prune(&mut self) {
        self.terms.retain(|_, c| !c.is_zero());
    }

    pub fn is_zero(&self) -> bool {
        self.terms.values().all(Zero::is_zero)
    }

    /// Iterate `(monomial, coefficient)` pairs in canonical order. Used by the
    /// Zeilberger ansatz to extract a linear system from a certificate numerator.
    pub fn monomials(&self) -> impl Iterator<Item = (&Monomial, &BigRational)> {
        self.terms.iter()
    }

    /// Number of distinct variables actually present.
    pub fn vars(&self) -> std::collections::BTreeSet<String> {
        let mut s = std::collections::BTreeSet::new();
        for m in self.terms.keys() {
            for (v, _) in m {
                s.insert(v.clone());
            }
        }
        s
    }

    pub fn neg(&self) -> Poly {
        let mut out = Poly::zero();
        for (m, c) in &self.terms {
            out.terms.insert(m.clone(), -c.clone());
        }
        out.prune();
        out
    }

    pub fn add(&self, other: &Poly) -> Poly {
        let mut out = self.clone();
        for (m, c) in &other.terms {
            out.add_term(m.clone(), c.clone());
        }
        out.prune();
        out
    }

    pub fn sub(&self, other: &Poly) -> Poly {
        self.add(&other.neg())
    }

    pub fn mul(&self, other: &Poly) -> Poly {
        let mut out = Poly::zero();
        for (m1, c1) in &self.terms {
            for (m2, c2) in &other.terms {
                let mut mono = m1.clone();
                mono.extend(m2.iter().cloned());
                out.add_term(mono, c1 * c2);
            }
        }
        out.prune();
        out
    }

    pub fn scale(&self, k: &BigRational) -> Poly {
        if k.is_zero() {
            return Poly::zero();
        }
        let mut out = Poly::zero();
        for (m, c) in &self.terms {
            out.terms.insert(m.clone(), c * k);
        }
        out
    }

    /// `self ** e` (e small non-negative).
    pub fn pow(&self, e: u32) -> Poly {
        let mut out = Poly::constant(BigRational::one());
        for _ in 0..e {
            out = out.mul(self);
        }
        out
    }

    /// Substitute a single variable by a rational and evaluate partially.
    pub fn subst_var(&self, name: &str, value: &BigRational) -> Poly {
        let mut out = Poly::zero();
        for (m, c) in &self.terms {
            let mut newmono = Monomial::new();
            let mut factor = c.clone();
            for (v, e) in m {
                if v == name {
                    // value ** e
                    let mut acc = BigRational::one();
                    for _ in 0..*e {
                        acc *= value;
                    }
                    factor *= acc;
                } else {
                    newmono.push((v.clone(), *e));
                }
            }
            out.add_term(newmono, factor);
        }
        out.prune();
        out
    }

    /// Fully evaluate, given values for all variables that appear. Returns `None`
    /// if a variable is missing (caller error; never panics — R38).
    pub fn eval(&self, env: &BTreeMap<String, BigRational>) -> Option<BigRational> {
        let mut acc = BigRational::zero();
        for (m, c) in &self.terms {
            let mut term = c.clone();
            for (v, e) in m {
                let val = env.get(v)?;
                for _ in 0..*e {
                    term *= val;
                }
            }
            acc += term;
        }
        Some(acc)
    }

    /// Substitute a variable by an arbitrary polynomial (`name := repl`). Enables
    /// shifting an argument by a polynomial, the workhorse of the holonomic
    /// telescoper checker (shift k→k+1, n→n+j).
    pub fn subst_var_poly(&self, name: &str, repl: &Poly) -> Poly {
        let mut out = Poly::zero();
        for (m, c) in &self.terms {
            let mut e_name = 0u32;
            let mut rest = Monomial::new();
            for (v, e) in m {
                if v == name {
                    e_name += e;
                } else {
                    rest.push((v.clone(), *e));
                }
            }
            let mut term = Poly::zero();
            term.add_term(rest, c.clone());
            if e_name > 0 {
                term = term.mul(&repl.pow(e_name));
            }
            out = out.add(&term);
        }
        out
    }

    /// Shift a variable by an integer constant: returns `self[name := name + s]`.
    pub fn shift_var(&self, name: &str, s: i64) -> Poly {
        let repl = Poly::var(name).add(&Poly::constant(BigRational::from(BigInt::from(s))));
        self.subst_var_poly(name, &repl)
    }

    /// Project to a univariate polynomial in `var`, or `None` if any other variable
    /// appears (used by the Gosper machinery, which works in `k` alone).
    pub fn to_unipoly(&self, var: &str) -> Option<UniPoly> {
        let mut coeffs: Vec<BigRational> = Vec::new();
        for (m, c) in &self.terms {
            let mut deg = 0usize;
            for (v, e) in m {
                if v == var {
                    deg = *e as usize;
                } else {
                    return None; // another variable present
                }
            }
            if coeffs.len() <= deg {
                coeffs.resize(deg + 1, BigRational::zero());
            }
            coeffs[deg] += c;
        }
        Some(UniPoly::from_coeffs(coeffs))
    }

    /// Pretty representation (deterministic order), used in certificate JSON
    /// (APPENDIX H.3) and diagnostics.
    pub fn to_canonical_string(&self) -> String {
        if self.is_zero() {
            return "0".to_string();
        }
        let mut parts = Vec::new();
        for (m, c) in &self.terms {
            let mut s = String::new();
            s.push_str(&c.to_string());
            for (v, e) in m {
                if *e == 1 {
                    s.push_str(&format!("*{v}"));
                } else {
                    s.push_str(&format!("*{v}^{e}"));
                }
            }
            parts.push(s);
        }
        parts.join(" + ")
    }
}

/// Univariate polynomial over `BigRational`, coefficients indexed by degree
/// (`coeffs[i]` is the coefficient of `x^i`). Trailing zeros are trimmed so that
/// the leading coefficient is nonzero (except the zero polynomial = empty).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(into = "UniPolySer", from = "UniPolySer")]
pub struct UniPoly {
    pub coeffs: Vec<BigRational>,
}

#[derive(Serialize, Deserialize)]
struct UniPolySer {
    coeffs: Vec<String>,
}

impl From<UniPoly> for UniPolySer {
    fn from(p: UniPoly) -> Self {
        UniPolySer {
            coeffs: p.coeffs.iter().map(|c| c.to_string()).collect(),
        }
    }
}

impl From<UniPolySer> for UniPoly {
    fn from(s: UniPolySer) -> Self {
        UniPoly::from_coeffs(s.coeffs.iter().map(|c| parse_rational(c)).collect())
    }
}

impl UniPoly {
    pub fn zero() -> Self {
        UniPoly { coeffs: vec![] }
    }

    pub fn from_coeffs(mut coeffs: Vec<BigRational>) -> Self {
        while coeffs.last().map(Zero::is_zero).unwrap_or(false) {
            coeffs.pop();
        }
        UniPoly { coeffs }
    }

    pub fn constant(c: BigRational) -> Self {
        UniPoly::from_coeffs(vec![c])
    }

    pub fn from_i64(c: i64) -> Self {
        UniPoly::constant(BigRational::from(BigInt::from(c)))
    }

    /// The monomial `x`.
    pub fn x() -> Self {
        UniPoly::from_coeffs(vec![BigRational::zero(), BigRational::one()])
    }

    pub fn is_zero(&self) -> bool {
        self.coeffs.is_empty()
    }

    /// Degree, or `None` for the zero polynomial.
    pub fn degree(&self) -> Option<usize> {
        if self.coeffs.is_empty() {
            None
        } else {
            Some(self.coeffs.len() - 1)
        }
    }

    pub fn leading(&self) -> BigRational {
        self.coeffs.last().cloned().unwrap_or_else(BigRational::zero)
    }

    pub fn coeff(&self, i: usize) -> BigRational {
        self.coeffs.get(i).cloned().unwrap_or_else(BigRational::zero)
    }

    pub fn add(&self, other: &UniPoly) -> UniPoly {
        let n = self.coeffs.len().max(other.coeffs.len());
        let mut c = vec![BigRational::zero(); n];
        for (i, slot) in c.iter_mut().enumerate() {
            *slot = self.coeff(i) + other.coeff(i);
        }
        UniPoly::from_coeffs(c)
    }

    pub fn sub(&self, other: &UniPoly) -> UniPoly {
        let n = self.coeffs.len().max(other.coeffs.len());
        let mut c = vec![BigRational::zero(); n];
        for (i, slot) in c.iter_mut().enumerate() {
            *slot = self.coeff(i) - other.coeff(i);
        }
        UniPoly::from_coeffs(c)
    }

    pub fn neg(&self) -> UniPoly {
        UniPoly::from_coeffs(self.coeffs.iter().map(|c| -c.clone()).collect())
    }

    pub fn mul(&self, other: &UniPoly) -> UniPoly {
        if self.is_zero() || other.is_zero() {
            return UniPoly::zero();
        }
        let mut c = vec![BigRational::zero(); self.coeffs.len() + other.coeffs.len() - 1];
        for (i, a) in self.coeffs.iter().enumerate() {
            for (j, b) in other.coeffs.iter().enumerate() {
                c[i + j] += a * b;
            }
        }
        UniPoly::from_coeffs(c)
    }

    pub fn scale(&self, k: &BigRational) -> UniPoly {
        if k.is_zero() {
            return UniPoly::zero();
        }
        UniPoly::from_coeffs(self.coeffs.iter().map(|c| c * k).collect())
    }

    /// `self ** e` for a small non-negative exponent.
    pub fn pow(&self, e: u32) -> UniPoly {
        let mut acc = UniPoly::constant(BigRational::one());
        for _ in 0..e {
            acc = acc.mul(self);
        }
        acc
    }

    pub fn eval(&self, x: &BigRational) -> BigRational {
        // Horner
        let mut acc = BigRational::zero();
        for c in self.coeffs.iter().rev() {
            acc = acc * x + c;
        }
        acc
    }

    /// Shift the argument: returns `p(x + s)` for integer `s`.
    pub fn shift(&self, s: i64) -> UniPoly {
        // p(x+s) = sum_i c_i (x+s)^i
        let xs = UniPoly::from_coeffs(vec![BigRational::from(BigInt::from(s)), BigRational::one()]);
        let mut acc = UniPoly::zero();
        let mut pw = UniPoly::constant(BigRational::one());
        for c in &self.coeffs {
            acc = acc.add(&pw.scale(c));
            pw = pw.mul(&xs);
        }
        acc
    }

    /// Polynomial long division: returns `(quotient, remainder)` with
    /// `self = q*divisor + r`, `deg r < deg divisor`. `None` if divisor is zero.
    pub fn divmod(&self, divisor: &UniPoly) -> Option<(UniPoly, UniPoly)> {
        if divisor.is_zero() {
            return None;
        }
        let mut rem = self.clone();
        let dlc = divisor.leading();
        let ddeg = divisor.degree().unwrap();
        let mut quot = vec![BigRational::zero(); self.coeffs.len().saturating_sub(ddeg) + 1];
        while let Some(rdeg) = rem.degree() {
            if rdeg < ddeg {
                break;
            }
            let shift = rdeg - ddeg;
            let factor = rem.leading() / &dlc;
            quot[shift] = factor.clone();
            // rem -= factor * x^shift * divisor
            let mut sub = vec![BigRational::zero(); shift + divisor.coeffs.len()];
            for (i, dc) in divisor.coeffs.iter().enumerate() {
                sub[shift + i] = dc * &factor;
            }
            rem = rem.sub(&UniPoly::from_coeffs(sub));
        }
        Some((UniPoly::from_coeffs(quot), rem))
    }

    /// `true` if `divisor` divides `self` exactly.
    pub fn divides(&self, dividend: &UniPoly) -> bool {
        match dividend.divmod(self) {
            Some((_, r)) => r.is_zero(),
            None => false,
        }
    }

    /// Monic gcd via the Euclidean algorithm over `Q[x]`.
    pub fn gcd(&self, other: &UniPoly) -> UniPoly {
        let mut a = self.clone();
        let mut b = other.clone();
        while !b.is_zero() {
            let (_, r) = a.divmod(&b).expect("divisor nonzero in gcd loop");
            a = b;
            b = r;
        }
        // make monic
        if let Some(_d) = a.degree() {
            let lc = a.leading();
            a = a.scale(&(BigRational::one() / lc));
        }
        a
    }

    /// Exact Lagrange interpolation through `(x_j, y_j)` with distinct `x_j`.
    /// Returns the unique polynomial of degree `< points.len()` over `Q`. Used to
    /// derive a closed form from naively-sampled values (AR-4): sample the sum at
    /// enough points, interpolate, then *prove* the result by a polynomial identity.
    pub fn interpolate(points: &[(BigRational, BigRational)]) -> UniPoly {
        let mut result = UniPoly::zero();
        for (j, (xj, yj)) in points.iter().enumerate() {
            let mut term = UniPoly::constant(yj.clone());
            for (m, (xm, _)) in points.iter().enumerate() {
                if m == j {
                    continue;
                }
                let denom = xj - xm; // nonzero since x's are distinct
                // (x - x_m) / (x_j - x_m)
                let factor =
                    UniPoly::from_coeffs(vec![-xm.clone(), BigRational::one()]).scale(&(BigRational::one() / denom));
                term = term.mul(&factor);
            }
            result = result.add(&term);
        }
        result
    }

    /// Convert to a multivariate [`Poly`] in the given variable name (for plugging
    /// univariate results into the identity checker).
    pub fn to_multivar(&self, var: &str) -> Poly {
        let mut out = Poly::zero();
        for (i, c) in self.coeffs.iter().enumerate() {
            let mono = if i == 0 {
                Vec::new()
            } else {
                vec![(var.to_string(), i as u32)]
            };
            out.add_term(mono, c.clone());
        }
        out.prune();
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(n: i64) -> BigRational {
        BigRational::from(BigInt::from(n))
    }

    #[test]
    fn shift_var_expands_binomially() {
        // p = n^2 ; p[n := n+1] = n^2 + 2n + 1
        let n = Poly::var("n");
        let p = n.mul(&n);
        let s = p.shift_var("n", 1);
        let expect = n.mul(&n).add(&n.scale(&r(2))).add(&Poly::from_i64(1));
        assert_eq!(s, expect);
    }

    #[test]
    fn subst_var_poly_two_vars() {
        // p = n*k ; p[k := k+1] = n*k + n
        let n = Poly::var("n");
        let k = Poly::var("k");
        let p = n.mul(&k);
        let s = p.shift_var("k", 1);
        assert_eq!(s, n.mul(&k).add(&n));
    }

    #[test]
    fn faulhaber_difference_is_zero() {
        // S(n) = n(n+1)(2n+1)/6 ; check S(n) - S(n-1) - n^2 == 0  (PART 11 example 1)
        let n = Poly::var("n");
        let one = Poly::from_i64(1);
        let two = Poly::from_i64(2);
        let sixth = BigRational::new(BigInt::from(1), BigInt::from(6));
        // S(t) as a function we can shift by substituting n -> n-1
        let s = |arg: &Poly| -> Poly {
            let np1 = arg.add(&one);
            let tw015 = two.mul(arg).add(&one);
            arg.mul(&np1).mul(&tw015).scale(&sixth)
        };
        let n_minus_1 = n.sub(&one);
        let diff = s(&n).sub(&s(&n_minus_1)).sub(&n.mul(&n));
        assert!(diff.is_zero(), "got {}", diff.to_canonical_string());
    }

    #[test]
    fn faulhaber_wrong_form_is_nonzero() {
        // A deliberately wrong closed form must NOT verify (DR7: "almost" is wrong).
        let n = Poly::var("n");
        let one = Poly::from_i64(1);
        let half = BigRational::new(BigInt::from(1), BigInt::from(2));
        let s = |arg: &Poly| arg.mul(&arg.add(&one)).scale(&half); // n(n+1)/2 (this is sum i, not i^2)
        let diff = s(&n).sub(&s(&n.sub(&one))).sub(&n.mul(&n));
        assert!(!diff.is_zero());
    }

    #[test]
    fn unipoly_divmod_and_gcd() {
        // (x^2 - 1) = (x-1)(x+1)
        let p = UniPoly::from_coeffs(vec![r(-1), r(0), r(1)]);
        let d = UniPoly::from_coeffs(vec![r(-1), r(1)]); // x - 1
        let (q, rem) = p.divmod(&d).unwrap();
        assert!(rem.is_zero());
        assert_eq!(q, UniPoly::from_coeffs(vec![r(1), r(1)])); // x + 1
        let g = p.gcd(&d);
        // gcd is monic associate of (x-1)
        assert_eq!(g.degree(), Some(1));
    }

    #[test]
    fn interpolate_recovers_triangular_closed_form() {
        // sum_{i=0}^{m} i has closed form m(m+1)/2; values 0,1,3,6 at m=0,1,2,3.
        let pts: Vec<(BigRational, BigRational)> = [(0, 0), (1, 1), (2, 3), (3, 6)]
            .iter()
            .map(|&(x, y)| (r(x), r(y)))
            .collect();
        let s = UniPoly::interpolate(&pts);
        // S(10) must be 55
        assert_eq!(s.eval(&r(10)), r(55));
        // S(n) = (n^2 + n)/2: coeff of n^2 is 1/2, of n is 1/2, const 0
        assert_eq!(s.coeff(2), BigRational::new(BigInt::from(1), BigInt::from(2)));
        assert_eq!(s.coeff(1), BigRational::new(BigInt::from(1), BigInt::from(2)));
        assert_eq!(s.coeff(0), r(0));
    }

    #[test]
    fn unipoly_shift() {
        // p = x^2 ; p(x+1) = x^2 + 2x + 1
        let p = UniPoly::from_coeffs(vec![r(0), r(0), r(1)]);
        let s = p.shift(1);
        assert_eq!(s, UniPoly::from_coeffs(vec![r(1), r(2), r(1)]));
    }
}
