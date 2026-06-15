//! Stage 29 measurement: SOS basis size C(n+d, d) and exact-certification timing across the
//! practical envelope (≤ 6 vars, deg ≤ 4). Verification-power, not a speed collapse.

use jeff_math::sos::{monomials, poly_from, prove_nonneg};
use std::time::Instant;

fn main() {
    println!("== SOS monomial basis size C(n+half_deg, n) ==");
    println!("{:>6} {:>9} {:>12}", "nvars", "half_deg", "basis_size");
    for nvars in 1..=6 {
        for hd in 1..=2 {
            println!("{:>6} {:>9} {:>12}", nvars, hd, monomials(nvars, hd).len());
        }
    }

    println!("\n== certification timing (exact rational LDLᵀ) ==");
    // p1 = x⁴−2x³+4x²+2 (1 var), p2 = 2x²+2xy+2y² (2 var).
    let p1 = poly_from(&[(&[4], 1), (&[3], -2), (&[2], 4), (&[0], 2)]);
    let p2 = poly_from(&[(&[2, 0], 2), (&[1, 1], 2), (&[0, 2], 2)]);
    for (name, run) in [
        ("p1 (1var deg4)", Box::new(move || prove_nonneg(&p1, 1, 2)) as Box<dyn Fn() -> _>),
        ("p2 (2var deg2)", Box::new(move || prove_nonneg(&p2, 2, 1))),
    ] {
        let mut bt = f64::INFINITY;
        let mut cert = false;
        for _ in 0..1000 {
            let t = Instant::now();
            let r = run();
            bt = bt.min(t.elapsed().as_secs_f64());
            cert = r.is_certified();
        }
        println!("  {name}: certified={cert}  best={:.2} µs", bt * 1e6);
    }
}
