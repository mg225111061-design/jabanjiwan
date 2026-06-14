//! Gosper's algorithm for indefinite hypergeometric summation (CLAUDE.md APPENDIX
//! E.1, "A=B" Ch. 5).
//!
//! Given a hypergeometric term `t_k` (via its ratio `ρ(k) = t(k+1)/t(k)`), find a
//! rational `R(k)` such that `S(k) = R(k)·t(k)` is an antidifference, i.e.
//! `R(k+1)·ρ(k) − R(k) − 1 ≡ 0` — or report it is not Gosper-summable.
//!
//! Safety (P0): the *search* here is only trusted insofar as its output passes the
//! independent checker `jeff_math::hyper::gosper_holds`. A bug in the search can
//! therefore only cause a (safe) HONEST_DEFER, never a wrong antidifference. We also
//! self-check the Gosper–Petkovšek normal form (`p·b·c(k) = q·a·c(k+1)`).

use jeff_math::hyper::HyperTerm;
use jeff_math::{Poly, RatFunc, UniPoly};
use num_rational::BigRational;
use num_traits::{One, Signed, ToPrimitive, Zero};

const K: &str = "k";

fn exact_div(a: &UniPoly, b: &UniPoly) -> Option<UniPoly> {
    let (q, r) = a.divmod(b)?;
    if r.is_zero() {
        Some(q)
    } else {
        None
    }
}

/// Cauchy-style bound on |roots| of `p`, as a rational.
fn root_bound(p: &UniPoly) -> BigRational {
    let Some(d) = p.degree() else {
        return BigRational::one();
    };
    if d == 0 {
        return BigRational::one();
    }
    let lead = p.leading().abs();
    let mut m = BigRational::zero();
    for i in 0..d {
        let r = p.coeff(i).abs() / &lead;
        if r > m {
            m = r;
        }
    }
    m + BigRational::one()
}

/// Dispersion search bound: an integer `j ≥ 0` with `gcd(a(k), b(k+j))` nonconstant
/// must satisfy `j ≤ root_bound(a) + root_bound(b)`.
fn dispersion_bound(a: &UniPoly, b: &UniPoly) -> i64 {
    let s = root_bound(a) + root_bound(b);
    let c = s.ceil().to_integer().to_i64().unwrap_or(64);
    c.clamp(0, 4096)
}

/// Gosper–Petkovšek normal form of `r = p/q`: `(a, b, c)` with
/// `p/q = (a/b)·c(k+1)/c(k)` and `gcd(a(k), b(k+j)) = 1` for all `j ≥ 0`.
/// Returns `None` if the self-check fails (defensive — a wrong normal form would be
/// caught by the final checker anyway).
fn gp_normal_form(p: &UniPoly, q: &UniPoly) -> Option<(UniPoly, UniPoly, UniPoly)> {
    let mut a = p.clone();
    let mut b = q.clone();
    let mut c = UniPoly::constant(BigRational::one());
    let jmax = dispersion_bound(&a, &b);
    for j in 0..=jmax {
        loop {
            let b_shift_j = b.shift(j); // b(k+j)
            let g = a.gcd(&b_shift_j);
            if g.degree().unwrap_or(0) == 0 {
                break; // constant gcd: nothing to peel at this j
            }
            a = exact_div(&a, &g)?;
            b = exact_div(&b, &g.shift(-j))?; // remove g(k-j) from b
            // c *= Π_{i=1}^{j} g(k-i)
            for i in 1..=j {
                c = c.mul(&g.shift(-i));
            }
        }
    }
    // self-check: p·b·c(k) == q·a·c(k+1)
    let lhs = p.mul(&b).mul(&c);
    let rhs = q.mul(&a).mul(&c.shift(1));
    if lhs.sub(&rhs).is_zero() {
        Some((a, b, c))
    } else {
        None
    }
}

/// Gosper degree bound for `a(k)x(k+1) − b̃(k)x(k) = c(k)` where `b̃(k) = b(k−1)`.
fn degree_bound(a: &UniPoly, bshift: &UniPoly, c: &UniPoly) -> Option<i64> {
    let da = a.degree().unwrap_or(0) as i64;
    let db = bshift.degree().unwrap_or(0) as i64;
    let dc = c.degree().unwrap_or(0) as i64;
    let deg_x = if da != db {
        dc - da.max(db)
    } else {
        let lead_a = a.leading();
        let lead_b = bshift.leading();
        if lead_a != lead_b {
            dc - da
        } else {
            // Equal degree AND equal leading coeff: the top term of
            // a(k)x(k+1) − b̃(k)x(k) cancels, so the operator lowers degree by one
            // ⇒ base = deg c − ℓ + 1 (DR6: the constitution's E.1 pseudocode shows
            // `deg c − ℓ` here, an off-by-one — verified wrong on t(k)=k, whose
            // antidifference k(k-1)/2 has degree 2). `ell` is the extra candidate
            // from the next-order cancellation `(β' − α')/α`.
            let a1 = a.coeff((da - 1).max(0) as usize);
            let b1 = bshift.coeff((da - 1).max(0) as usize);
            let ell = (b1 - a1) / &lead_a;
            let base = dc - da + 1;
            if ell.is_integer() && !ell.is_negative() {
                let elli = ell.to_integer().to_i64().unwrap_or(-1);
                base.max(elli)
            } else {
                base
            }
        }
    };
    if deg_x < 0 {
        None
    } else {
        Some(deg_x)
    }
}

/// Solve `a(k)x(k+1) − bshift(k)x(k) = c(k)` for a polynomial `x` of degree ≤
/// `deg_x` (undetermined coefficients → exact linear system). `None` if no solution.
fn solve_x(a: &UniPoly, bshift: &UniPoly, c: &UniPoly, deg_x: i64) -> Option<UniPoly> {
    let nx = (deg_x + 1) as usize;
    // contribution of unknown x_i: a(k)·(k+1)^i − bshift(k)·k^i
    let mut contrib: Vec<UniPoly> = Vec::with_capacity(nx);
    let kp1 = UniPoly::from_coeffs(vec![BigRational::one(), BigRational::one()]); // k+1
    let kk = UniPoly::x();
    for i in 0..nx {
        let term = a.mul(&kp1.pow(i as u32)).sub(&bshift.mul(&kk.pow(i as u32)));
        contrib.push(term);
    }
    // system degree
    let maxdeg = contrib
        .iter()
        .filter_map(|p| p.degree())
        .max()
        .unwrap_or(0)
        .max(c.degree().unwrap_or(0));
    // rows: one per degree 0..=maxdeg ; columns: x_0..x_{nx-1}
    let mut rows: Vec<Vec<BigRational>> = Vec::new();
    let mut rhs: Vec<BigRational> = Vec::new();
    for d in 0..=maxdeg {
        let row: Vec<BigRational> = (0..nx).map(|i| contrib[i].coeff(d)).collect();
        rows.push(row);
        rhs.push(c.coeff(d));
    }
    let sol = jeff_math::linsolve::solve(&rows, &rhs)?;
    Some(UniPoly::from_coeffs(sol))
}

/// Run Gosper on a hypergeometric term in `k` (no `n`). Returns the rational
/// certificate `R(k)` with `R(k+1)ρ(k) − R(k) = 1`, or `None` (not summable / not a
/// rational ratio). The caller MUST still verify with `gosper_holds` (P0).
pub fn gosper_indefinite(term: &HyperTerm) -> Option<RatFunc> {
    let rho = term.ratio_k();
    let p = rho.num.to_unipoly(K)?; // ratio numerator (k-univariate)
    let q = rho.den.to_unipoly(K)?;
    if p.is_zero() || q.is_zero() {
        return None;
    }
    let (a, b, c) = gp_normal_form(&p, &q)?;
    let bshift = b.shift(-1); // b(k-1)
    let deg_x = degree_bound(&a, &bshift, &c)?;
    let x = solve_x(&a, &bshift, &c, deg_x)?;
    if x.is_zero() {
        return None; // trivial solution ⇒ no antidifference of this form
    }
    // R(k) = b(k-1)·x(k) / c(k)
    let num: Poly = bshift.mul(&x).to_multivar(K);
    let den: Poly = c.to_multivar(K);
    Some(RatFunc::new(num, den))
}

#[cfg(test)]
mod tests {
    use super::*;
    use jeff_math::hyper::{gosper_holds, LinForm};

    /// Term `t(k) = k · k!`  →  HyperTerm: poly factor k, gamma Γ(k+1)^1.
    fn k_times_kfact() -> HyperTerm {
        HyperTerm {
            coeff: BigRational::one(),
            z_k: BigRational::one(),
            poly: Poly::var("k"),
            gammas: vec![(LinForm::new(0, 1, 1), 1)], // Γ(k+1)
        }
    }

    /// Term `t(k) = k` → poly factor k, no gammas.
    fn just_k() -> HyperTerm {
        HyperTerm {
            coeff: BigRational::one(),
            z_k: BigRational::one(),
            poly: Poly::var("k"),
            gammas: vec![],
        }
    }

    /// Harmonic term `t(k) = 1/k` = Γ(k)/Γ(k+1) → gamma (k,+? ) ... represent 1/k as
    /// poly 1 over... simplest: gammas Γ(k)^1, Γ(k+1)^{-1} gives Γ(k)/Γ(k+1)=1/k.
    fn one_over_k() -> HyperTerm {
        HyperTerm {
            coeff: BigRational::one(),
            z_k: BigRational::one(),
            poly: Poly::from_i64(1),
            gammas: vec![(LinForm::new(0, 1, 0), 1), (LinForm::new(0, 1, 1), -1)],
        }
    }

    #[test]
    fn gosper_sum_k_times_kfact_is_kfact() {
        // antidifference of k·k! is k! ; R(k) = 1/k.
        let term = k_times_kfact();
        let r = gosper_indefinite(&term).expect("summable");
        assert!(gosper_holds(&term, &r), "found R must pass the checker");
    }

    #[test]
    fn gosper_sum_k_is_polynomial() {
        let term = just_k();
        let r = gosper_indefinite(&term).expect("summable");
        assert!(gosper_holds(&term, &r));
    }

    #[test]
    fn gosper_harmonic_is_not_summable() {
        // 1/k has no hypergeometric antidifference (A16): Gosper must return None.
        let term = one_over_k();
        assert!(gosper_indefinite(&term).is_none(), "harmonic must not be summable");
    }

    // ----- E.1 degree-bound regression guard (the off-by-one we fixed) -----
    // In the cancellation case (deg a = deg b̃ AND lc a = lc b̃) the operator
    // a·x(k+1) − b̃·x(k) drops degree by 1, so deg x = deg c − ℓ + 1. With a=b̃=1
    // (ℓ=0) the antidifference of a degree-p polynomial summand has degree p+1.
    // The constitution's E.1 pseudocode gives deg c − ℓ (one too small) and would
    // make these return None. We trust the oracle, not the doc.
    #[test]
    fn degree_bound_cancellation_case_adds_one() {
        let one = UniPoly::constant(BigRational::one());
        let k = UniPoly::x();
        assert_eq!(degree_bound(&one, &one, &k), Some(2)); // antidiff of k is degree 2
        assert_eq!(degree_bound(&one, &one, &k.pow(2)), Some(3)); // k^2 -> degree 3
        assert_eq!(degree_bound(&one, &one, &k.pow(3)), Some(4)); // k^3 -> degree 4
    }

    fn poly_term(p: Poly) -> HyperTerm {
        HyperTerm {
            coeff: BigRational::one(),
            z_k: BigRational::one(),
            poly: p,
            gammas: vec![],
        }
    }

    #[test]
    fn gosper_polynomial_summands_are_summable() {
        // t(k) = k, k^2, k^3 — all polynomial, all Gosper-summable (the cancellation
        // case). With the old off-by-one these would (wrongly) report not-summable.
        use jeff_math::hyper::gosper_holds;
        for p in 1..=3u32 {
            let term = poly_term(Poly::var("k").pow(p));
            let r = gosper_indefinite(&term).unwrap_or_else(|| panic!("k^{p} must be summable"));
            assert!(gosper_holds(&term, &r), "antidifference of k^{p} must verify");
        }
    }
}
