//! kyber_check — differential correctness of the production PQC kernel (Stage Y2).
//! The HARAN kernel `poly_mul = intt(pointwise(ntt(a), ntt(b)))` is exactly kyber.rs's
//! `inv_ntt(basemul(ntt a, ntt b))`. This checks it equals the Θ(n²) schoolbook negacyclic
//! definition EXACTLY (mod Q) on random polynomials — a real differential proof of the kernel math.
//! output: MATCH trials=<t>  or  MISMATCH ok=<k>/<t>

use jeff_math::kyber::{poly_mul, N, Q};
use jeff_math::pqc::schoolbook_negacyclic;

fn main() {
    let trials: usize = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(200);
    let mut seed = 0x2026_0616_u64;
    let mut rng = || {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (seed >> 33) % Q
    };
    let mut ok = 0usize;
    for _ in 0..trials {
        let a: Vec<u64> = (0..N).map(|_| rng()).collect();
        let b: Vec<u64> = (0..N).map(|_| rng()).collect();
        if poly_mul(&a, &b) == schoolbook_negacyclic(&a, &b, Q) {
            ok += 1;
        }
    }
    if ok == trials {
        println!("MATCH trials={trials} (ntt poly_mul ≡ schoolbook negacyclic, exact mod {Q})");
    } else {
        println!("MISMATCH ok={ok}/{trials}");
    }
}
