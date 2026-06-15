//! Integer-relation discovery and **certified exclusion** (Stage 20 — the PSLQ family, in
//! exact bounded form).
//!
//! An integer relation among values `v_1..v_k` is a nonzero integer vector `a` with
//! `Σ a_i v_i = 0`. PSLQ finds such relations from high-precision *real* values and yields a
//! heuristic exclusion bound from its `H` diagonal. Here we do the **exact** version over
//! integer/rational values: a bounded search that either finds the smallest-`‖·‖∞` relation
//! or **proves** none exists with `‖a‖∞ ≤ B` — an exact, labeled absence certificate
//! (class = integer-relation, Θ = (k, B)). This is a stronger guarantee than PSLQ's numeric
//! bound, but limited to small `(k, B)`; full high-precision-real PSLQ (for π, ζ(3), …) needs
//! arbitrary-precision real arithmetic and is the deferred extension.

use num_bigint::BigInt;
use num_traits::Zero;

/// `Σ a_i v_i` over exact integers.
fn dot(a: &[i64], v: &[BigInt]) -> BigInt {
    a.iter().zip(v).fold(BigInt::zero(), |acc, (&ai, vi)| acc + BigInt::from(ai) * vi)
}

/// Search for the smallest-`‖·‖∞` nonzero integer relation `a` (`‖a‖∞ ≤ bound`) with
/// `Σ a_i v_i = 0`, or `None`. Exact (bounded brute force over the coefficient grid; meant
/// for small `k`, `bound`). Among equal `‖·‖∞`, the first in odometer order is returned.
pub fn find_integer_relation(values: &[BigInt], bound: i64) -> Option<Vec<i64>> {
    let k = values.len();
    if k == 0 || bound < 1 {
        return None;
    }
    // search by increasing L∞ radius so the returned relation is minimal in ‖·‖∞.
    for radius in 1..=bound {
        let mut a = vec![-radius; k];
        loop {
            // only consider vectors whose L∞ is exactly `radius` (new this round) and nonzero.
            if a.iter().map(|x| x.abs()).max().unwrap() == radius && dot(&a, values).is_zero() {
                return Some(a.clone());
            }
            // odometer increment over [-radius, radius]^k
            let mut i = 0;
            loop {
                if i == k {
                    // exhausted this radius
                    break;
                }
                a[i] += 1;
                if a[i] <= radius {
                    break;
                }
                a[i] = -radius;
                i += 1;
            }
            if i == k {
                break;
            }
        }
    }
    None
}

/// `true` iff NO nonzero integer relation with `‖a‖∞ ≤ bound` exists among `values` — an
/// exact exclusion proof (the bounded search found nothing).
pub fn excludes_integer_relation(values: &[BigInt], bound: i64) -> bool {
    find_integer_relation(values, bound).is_none()
}

/// Verify a claimed relation exactly: nonzero, within bound, and `Σ a_i v_i = 0`.
pub fn verify_relation(values: &[BigInt], a: &[i64], bound: i64) -> bool {
    a.len() == values.len()
        && a.iter().any(|&x| x != 0)
        && a.iter().all(|&x| x.abs() <= bound)
        && dot(a, values).is_zero()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ints(v: &[i64]) -> Vec<BigInt> {
        v.iter().map(|&x| BigInt::from(x)).collect()
    }

    #[test]
    fn finds_and_verifies_relation() {
        // 2 + 3 − 5 = 0 ⇒ relation [1,1,-1].
        let v = ints(&[2, 3, 5]);
        let a = find_integer_relation(&v, 5).expect("relation exists");
        assert!(verify_relation(&v, &a, 5), "Σ a_i v_i must be exactly 0");
        // a rational-cleared example: 6, 10, 15 ⇒ 5·6 − 3·10 + 0·15 = 0.
        let v2 = ints(&[6, 10, 15]);
        let a2 = find_integer_relation(&v2, 6).expect("relation exists");
        assert!(verify_relation(&v2, &a2, 6));
    }

    #[test]
    fn excludes_when_none_small() {
        // 2,3,7: no relation with ‖a‖∞ ≤ 1 (checked exactly).
        let v = ints(&[2, 3, 7]);
        assert!(excludes_integer_relation(&v, 1), "no ‖·‖∞≤1 relation");
        // but a larger bound finds one (2·7 − 7·2 ... actually 7·2 + (-2)·7 -> needs 3 terms):
        // 2a+3b+7c=0 with bound 7: e.g. a=7,b=-7? 14-21+7=0 ⇒ [7,-7,1]? 14-21+7=0 ✓ within 7.
        assert!(find_integer_relation(&v, 7).is_some());
    }

    #[test]
    fn single_value_nonzero_has_no_relation() {
        assert!(excludes_integer_relation(&ints(&[5]), 100));
        assert!(find_integer_relation(&ints(&[0]), 3).is_some()); // 0 has relation [1]
    }
}
