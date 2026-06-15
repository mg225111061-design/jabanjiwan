//! Stage 12.1 — cache-blocked GEMM: correctness always, timing on demand.
//!
//! Correctness (bit-exact vs the scalar oracle) is asserted unconditionally. The timing
//! sweep is `#[ignore]` (run with `--ignored --release`) and only reports the measured
//! blocked-vs-naive ratio on THIS machine — no incumbent (OpenBLAS/MKL) comparison is made
//! because none is installed here (R8/DR2: measure, never fabricate).

use jeff_backend::measure::compare;
use jeff_backend::simd::{gemm_blocked, gemm_scalar, igemm_scalar, igemm_tiled};
use jeff_math::fmat::Rng;

fn mats(m: usize, k: usize, n: usize, seed: u64) -> (Vec<f64>, Vec<f64>) {
    let mut r = Rng::new(seed);
    (
        (0..m * k).map(|_| r.gaussian()).collect(),
        (0..k * n).map(|_| r.gaussian()).collect(),
    )
}

#[test]
fn blocked_gemm_is_bit_exact() {
    for n in [16usize, 64, 100, 129] {
        let (a, b) = mats(n, n, n, 0xA11CE + n as u64);
        assert_eq!(
            gemm_blocked(&a, &b, n, n, n),
            gemm_scalar(&a, &b, n, n, n),
            "blocked GEMM must equal the scalar oracle bit-for-bit at n={n}"
        );
    }
}

#[test]
fn register_tiled_integer_gemm_bit_exact() {
    let mut r = Rng::new(0x17_2B);
    for n in [16usize, 64, 100, 129] {
        let a: Vec<i64> = (0..n * n).map(|_| (r.next_u64() % 11) as i64 - 5).collect();
        let b: Vec<i64> = (0..n * n).map(|_| (r.next_u64() % 11) as i64 - 5).collect();
        assert_eq!(
            igemm_tiled(&a, &b, n, n, n),
            igemm_scalar(&a, &b, n, n, n),
            "register-tiled integer GEMM must equal the scalar oracle at n={n}"
        );
    }
}

#[test]
#[ignore = "timing sweep; run with --ignored --release (ideally -C target-cpu=native)"]
fn register_tiled_integer_timing_sweep() {
    for &n in &[64usize, 128, 256, 512] {
        let mut r = Rng::new(7);
        let a: Vec<i64> = (0..n * n).map(|_| (r.next_u64() % 7) as i64).collect();
        let b: Vec<i64> = (0..n * n).map(|_| (r.next_u64() % 7) as i64).collect();
        assert_eq!(igemm_tiled(&a, &b, n, n, n), igemm_scalar(&a, &b, n, n, n));
        let res = compare(
            3,
            "naive integer GEMM",
            || {
                std::hint::black_box(igemm_tiled(&a, &b, n, n, n));
            },
            || {
                std::hint::black_box(igemm_scalar(&a, &b, n, n, n));
            },
        );
        eprintln!("{}", res.report(&format!("register-tiled igemm n={n}")));
    }
}

#[test]
#[ignore = "timing sweep; run with --ignored --release"]
fn blocked_gemm_timing_sweep() {
    for &n in &[64usize, 128, 256, 512] {
        let (a, b) = mats(n, n, n, 7);
        assert_eq!(gemm_blocked(&a, &b, n, n, n), gemm_scalar(&a, &b, n, n, n));
        let r = compare(
            3,
            "naive i-p-j GEMM",
            || {
                std::hint::black_box(gemm_blocked(&a, &b, n, n, n));
            },
            || {
                std::hint::black_box(gemm_scalar(&a, &b, n, n, n));
            },
        );
        eprintln!("{}", r.report(&format!("blocked gemm n={n}")));
    }
}
