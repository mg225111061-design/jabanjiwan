//! jeff_foldsum — JEFF fold-collapse backend for Mr.Jeffrey / HARAN (Stage H4).
//! Collapses a definite fold `Σ_{k=lo}^{n} body(k)` of a POLYNOMIAL summand to a closed form and
//! PROVES it, entirely inside JEFF's exact polynomial engine (poly.rs), no floats / no fuzzing:
//!   1. DERIVE  — sample the partial sum S(m)=Σ_{k=lo}^{m} body(k) at deg+2 points and
//!      `UniPoly::interpolate` the closed form (the documented AR-4 path);
//!   2. PROVE   — telescoping coefficient-zero: `S(n) − S(n−1) − body(n) ≡ 0` (`shift`,`sub`,`is_zero`)
//!      and the base case `S(lo−1) = 0`. Together ⇒ Σ = S(n) for ALL n by induction.
//!
//! usage:  jeff_foldsum "<lo>" "<c0,c1,…,cd>"   (summand body(k)=Σ cᵢ kⁱ, ascending rationals)
//! output: OK telescope=ZERO base=ZERO deg=<D> closed="<S(n)>" coeffs=<c0,…,cD>
//!     or  FAIL telescope=<...> base=<...>

use jeff_math::poly::UniPoly;
use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::{One, Zero};

fn parse_coeffs(s: &str) -> Vec<BigRational> {
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

fn fmt_rat(r: &BigRational) -> String {
    if r.denom() == &BigInt::one() {
        r.numer().to_string()
    } else {
        format!("{}/{}", r.numer(), r.denom())
    }
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.len() < 3 {
        eprintln!("usage: jeff_foldsum \"<lo>\" \"<c0,c1,...>\"");
        std::process::exit(2);
    }
    let lo: i64 = a[1].trim().parse().expect("lo must be an integer");
    let body = UniPoly::from_coeffs(parse_coeffs(&a[2]));
    let d = body.degree().unwrap_or(0);
    let npoints = d + 2; // S has degree d+1 → needs d+2 points

    // (1) DERIVE: sample S(m) = Σ_{k=lo}^{m} body(k), m = lo-1, lo, …  (S(lo-1)=empty sum=0).
    let mut points = Vec::with_capacity(npoints);
    let mut running = BigRational::zero();
    let start = lo - 1;
    for j in 0..npoints {
        let m = start + j as i64;
        if j > 0 {
            running += body.eval(&BigRational::from(BigInt::from(m))); // add body(m), m≥lo
        }
        points.push((BigRational::from(BigInt::from(m)), running.clone()));
    }
    let s = UniPoly::interpolate(&points);

    // (2) PROVE: telescoping S(n) − S(n−1) − body(n) ≡ 0, and base S(lo−1) = 0 — exact coeff-zero.
    let telescope = s.sub(&s.shift(-1)).sub(&body).is_zero();
    let base = s.eval(&BigRational::from(BigInt::from(lo - 1))).is_zero();

    let sdeg = s.degree().unwrap_or(0);
    if telescope && base {
        let coeffs: Vec<String> = (0..=sdeg).map(|i| fmt_rat(&s.coeff(i))).collect();
        let closed = s.to_multivar("n").to_canonical_string();
        println!("OK telescope=ZERO base=ZERO deg={sdeg} closed=\"{closed}\" coeffs={}", coeffs.join(","));
    } else {
        println!(
            "FAIL telescope={} base={}",
            if telescope { "ZERO" } else { "NONZERO" },
            if base { "ZERO" } else { "NONZERO" }
        );
    }
}
