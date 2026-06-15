//! Stage 16.4 — PRECISION axis: accuracy-cost Pareto with **verified error bounds**.
//!
//! Requested precision becomes an input. Each precision level gets a minimum cost (term
//! count) and a *rigorous* error bound — here EXACT over ℚ, so the bound is not merely
//! claimed but provably attained. An anytime evaluation: more terms ⇒ a provably tighter
//! bound, never a wrong answer. The exemplar is the geometric series Σ rᵏ; the same shape
//! (partial + certified tail bound) is the template for the axis.

use num_rational::BigRational;
use num_traits::{One, Signed, Zero};

fn pow_rat(r: &BigRational, k: usize) -> BigRational {
    let mut acc = BigRational::one();
    for _ in 0..k {
        acc *= r;
    }
    acc
}

/// The exact closed-form geometric sum `1/(1-r)` for `|r| < 1`.
pub fn geometric_sum(r: &BigRational) -> Option<BigRational> {
    if r.abs() >= BigRational::one() {
        return None;
    }
    Some(BigRational::one() / (BigRational::one() - r))
}

/// `k`-term partial sum `Σ_{j=0}^{k-1} rʲ = (1 - rᵏ)/(1 - r)` and the EXACT tail bound
/// `|Σ_{j≥k} rʲ| = |rᵏ/(1-r)|`. For `|r| < 1`, `|true_sum − partial| = bound` exactly.
pub fn geometric_partial(r: &BigRational, k: usize) -> Option<(BigRational, BigRational)> {
    if r.abs() >= BigRational::one() {
        return None;
    }
    let one = BigRational::one();
    let denom = &one - r; // ∈ (0, 2), never zero for r ∈ (-1, 1)
    let rk = pow_rat(r, k);
    let partial = (&one - &rk) / &denom;
    let bound = (rk / &denom).abs();
    Some((partial, bound))
}

/// The precision→cost map: the smallest term count `k` whose exact error bound is `≤ eps`.
pub fn terms_for_precision(r: &BigRational, eps: &BigRational) -> Option<usize> {
    if r.abs() >= BigRational::one() || eps <= &BigRational::zero() {
        return None;
    }
    let mut k = 0usize;
    loop {
        let (_, bound) = geometric_partial(r, k)?;
        if &bound <= eps {
            return Some(k);
        }
        k += 1;
        if k > 1_000_000 {
            return None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use num_bigint::BigInt;

    fn r(n: i64, d: i64) -> BigRational {
        BigRational::new(BigInt::from(n), BigInt::from(d))
    }

    #[test]
    fn error_bound_verified() {
        // The stated bound is the ACTUAL error, exactly (verified precision-cost trade).
        let rr = r(1, 2); // Σ (1/2)^k = 2
        let truth = geometric_sum(&rr).unwrap();
        assert_eq!(truth, BigRational::from(BigInt::from(2)));
        for k in 0..12 {
            let (partial, bound) = geometric_partial(&rr, k).unwrap();
            let actual = (&truth - &partial).abs();
            assert_eq!(actual, bound, "stated bound must equal the true error at k={k}");
        }
        // a negative ratio too (alternating): r = -1/3, sum = 3/4.
        let rn = r(-1, 3);
        let truth = geometric_sum(&rn).unwrap();
        assert_eq!(truth, r(3, 4));
        let (partial, bound) = geometric_partial(&rn, 7).unwrap();
        assert_eq!((&truth - &partial).abs(), bound);
    }

    #[test]
    fn precision_cost_curve_sound() {
        // Higher precision ⇒ (weakly) more cost; the chosen k attains the bound and is
        // minimal (one fewer term would exceed eps).
        let rr = r(1, 2);
        let mut last = 0usize;
        for p in 1..=8 {
            let eps = pow_rat(&r(1, 10), p); // 10^-p
            let k = terms_for_precision(&rr, &eps).unwrap();
            assert!(k >= last, "cost must not decrease as precision tightens");
            last = k;
            // chosen k achieves ≤ eps ...
            let (_, bound) = geometric_partial(&rr, k).unwrap();
            assert!(bound <= eps);
            // ... and is minimal.
            if k > 0 {
                let (_, prev) = geometric_partial(&rr, k - 1).unwrap();
                assert!(prev > eps, "k must be the minimal term count");
            }
        }
    }

    #[test]
    fn divergent_ratio_has_no_bound() {
        assert!(geometric_partial(&BigRational::from(BigInt::from(2)), 5).is_none());
        assert!(terms_for_precision(&r(3, 2), &r(1, 100)).is_none());
    }
}
