//! Stage 12 — find the real Strassen crossover (CLAUDE.md PART D). At n=128 with bignum
//! entries Strassen LOST (0.806×); the asymptotic win must appear at larger n. This sweeps
//! n and reports where Strassen first beats the in-house naive O(n³). All measured
//! (R8/DR2); correctness checked first (Strassen == naive exactly).

use jeff_backend::measure::compare;
use jeff_math::fmat::Rng;
use jeff_math::IntMatrix;
use num_bigint::BigInt;

fn mat(n: usize, seed: u64) -> IntMatrix {
    let mut r = Rng::new(seed);
    IntMatrix { n, data: (0..n * n).map(|_| BigInt::from(r.next_u64() % 7)).collect() }
}

#[test]
#[ignore = "timing sweep; run with --ignored"]
fn strassen_crossover_sweep() {
    let mut crossover = None;
    for &n in &[64usize, 128, 192, 256, 320, 384] {
        let a = mat(n, 1);
        let b = mat(n, 2);
        assert_eq!(a.strassen_mul(&b), a.naive_mul(&b), "n={n}: Strassen must equal naive");
        let r = compare(
            2,
            "in-house naive O(n³)",
            || {
                std::hint::black_box(a.strassen_mul(&b));
            },
            || {
                std::hint::black_box(a.naive_mul(&b));
            },
        );
        eprintln!("{}", r.report(&format!("strassen n={n}")));
        if r.speedup() > 1.0 && crossover.is_none() {
            crossover = Some(n);
        }
    }
    match crossover {
        Some(n) => eprintln!("CROSSOVER: Strassen first wins at n={n}"),
        None => eprintln!("CROSSOVER: beyond tested range (Strassen still slower at n=384)"),
    }
}
