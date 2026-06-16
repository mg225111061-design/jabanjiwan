//! JEFF exact identity backend for Mr. Jeffrey (Stage 3).
//! Takes two univariate polynomials as ascending rational coefficient lists and uses JEFF's
//! **exact coefficient-zero** mechanism (`UniPoly::from_coeffs` over `BigRational`, `is_zero`) to
//! decide `candidate ≡ reference` for ALL inputs — no floats, no fuzzing.
//!
//! usage:  jeff_identity "<cand_coeffs>" "<ref_coeffs>"
//!   coeffs = comma-separated rationals, ascending (e.g. "1,2,1" = 1 + 2x + x²; "1/2,0,3").
//! output: "PROVEN_EQUAL"  or  "REFUTED degree=<k> residual=<rational>".

use jeff_math::poly::UniPoly;
use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::Zero;

fn parse(s: &str) -> Vec<BigRational> {
    s.split(',')
        .filter(|t| !t.trim().is_empty())
        .map(|t| {
            let t = t.trim();
            if let Some((n, d)) = t.split_once('/') {
                BigRational::new(n.trim().parse::<BigInt>().unwrap(), d.trim().parse::<BigInt>().unwrap())
            } else {
                BigRational::from(t.parse::<BigInt>().unwrap())
            }
        })
        .collect()
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.len() < 3 {
        eprintln!("usage: jeff_identity \"<cand_coeffs>\" \"<ref_coeffs>\"");
        std::process::exit(2);
    }
    let cand = parse(&a[1]);
    let refr = parse(&a[2]);
    let n = cand.len().max(refr.len());
    // exact coefficient-wise difference over the rationals (JEFF's exact arithmetic).
    let diff: Vec<BigRational> = (0..n)
        .map(|i| {
            let c = cand.get(i).cloned().unwrap_or_else(BigRational::zero);
            let r = refr.get(i).cloned().unwrap_or_else(BigRational::zero);
            c - r
        })
        .collect();
    // JEFF's exact coefficient-zero check: from_coeffs trims trailing zeros; is_zero iff identical.
    let dpoly = UniPoly::from_coeffs(diff.clone());
    if dpoly.is_zero() {
        println!("PROVEN_EQUAL");
    } else {
        // witness: lowest-degree nonzero residual coefficient (an exact disagreement).
        let (k, res) = diff.iter().enumerate().find(|(_, c)| !c.is_zero()).unwrap();
        println!("REFUTED degree={k} residual={res}");
    }
}
