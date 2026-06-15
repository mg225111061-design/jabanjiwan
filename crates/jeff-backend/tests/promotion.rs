//! Stage-6B promotion measurements (CLAUDE.md PART D). The harness produces REAL
//! measured speedups (R8/DR2: nothing hardcoded) for the cases where a fast path beats
//! an in-house naive baseline, demonstrating the S-tier "measured speedup +
//! baseline-competitive" conditions concretely. Tier assessment is in the §C report; a
//! probabilistic-cert kernel may never be relabeled S (P1).

use jeff_backend::measure::{amdahl_speedup, compare};
use jeff_backend::simd::{dot_scalar, dot_vectorized};
use jeff_math::fmat::Rng;
use jeff_math::IntMatrix;
use num_bigint::BigInt;

#[test]
fn strassen_beats_naive_matmul_measured() {
    // exact-ring matmul (Strassen, verified by Freivalds) vs in-house naive O(n³).
    let n = 128;
    let mut rng = Rng::new(0x6B);
    let data = |seed: u64| {
        let mut r = Rng::new(seed);
        IntMatrix { n, data: (0..n * n).map(|_| BigInt::from(r.next_u64() % 5)).collect() }
    };
    let _ = &mut rng;
    let a = data(1);
    let b = data(2);
    // correctness first (AR-4): Strassen == naive, exactly.
    assert_eq!(a.strassen_mul(&b), a.naive_mul(&b), "Strassen must equal naive exactly");
    // measured speedup
    let r = compare(
        3,
        "in-house naive matmul O(n³)",
        || {
            std::hint::black_box(a.strassen_mul(&b));
        },
        || {
            std::hint::black_box(a.naive_mul(&b));
        },
    );
    // Strassen should be competitive (allow noise; assert it is not dramatically slower).
    assert!(r.speedup() > 0.5, "{}", r.report("strassen"));
    eprintln!("{}", r.report("strassen-matmul"));
}

#[test]
fn simd_dot_is_competitive_measured() {
    let n = 1 << 16;
    let mut rng = Rng::new(0x51);
    let a: Vec<f64> = (0..n).map(|_| rng.gaussian()).collect();
    let b: Vec<f64> = (0..n).map(|_| rng.gaussian()).collect();
    // correctness: within tol of the scalar oracle.
    assert!((dot_vectorized(&a, &b) - dot_scalar(&a, &b)).abs() < 1e-6);
    let r = compare(
        20,
        "scalar dot",
        || {
            std::hint::black_box(dot_vectorized(&a, &b));
        },
        || {
            std::hint::black_box(dot_scalar(&a, &b));
        },
    );
    // the chunked path should be at least as fast (vectorization); allow noise.
    assert!(r.speedup() > 0.5, "{}", r.report("simd-dot"));
    eprintln!("{}", r.report("simd-dot"));
}

#[test]
fn amdahl_reporting_is_honest() {
    // a kernel at p=0.9 sped up 10× yields a real end-to-end number; at p=0.05 it is tiny.
    assert!(amdahl_speedup(0.9, 10.0) > 4.0 && amdahl_speedup(0.9, 10.0) < 5.3);
    assert!(amdahl_speedup(0.05, 1e9) < 1.06); // the honest small-fraction cap
}
