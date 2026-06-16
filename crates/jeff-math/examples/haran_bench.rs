//! haran_bench — honest wall-clock measurement for HARAN v4 (Stage Y3/Y4).
//! Built in RELEASE for fair numbers. Median of K runs (noise removal). Two modes:
//!   foldsum <n>   — CLOSED collapse O(1) closed form  vs  O(n) naive loop, for Σ i² (num-bigint,
//!                   same bignum both sides → fair). The *fold* case: orders of magnitude at large n.
//!   unstruct <n>  — data-dependent reduction naive vs 4-way unrolled (the SIMD/backend ceiling).
//!                   The *unstructured* case: constant-factor only (Ω(N) is a theorem).
//! output (one line, machine-parseable): "<mode> n=.. ..ns.. ratio=.."

use num_bigint::BigInt;
use num_traits::Zero;
use std::hint::black_box;
use std::time::Instant;

fn closed_form_sq(n: u64) -> BigInt {
    let n = BigInt::from(n);
    (&n * (&n + 1u32) * (2u32 * &n + 1u32)) / 6u32
}
fn naive_sq(n: u64) -> BigInt {
    let mut acc = BigInt::zero();
    for i in 1..=n {
        let bi = BigInt::from(i);
        acc += &bi * &bi;
    }
    acc
}
fn median_ns<T, F: FnMut() -> T>(mut f: F, runs: usize) -> u128 {
    let mut ts = Vec::with_capacity(runs);
    for _ in 0..runs {
        let t = Instant::now();
        black_box(f());          // prevent dead-code elimination of the result
        ts.push(t.elapsed().as_nanos());
    }
    ts.sort_unstable();
    ts[ts.len() / 2]
}

fn data_sum_naive(a: &[u64]) -> u64 {
    let mut s = 0u64;
    for &x in a {
        s = s.wrapping_add(x);
    }
    s
}
fn data_sum_unrolled(a: &[u64]) -> u64 {
    let (mut s0, mut s1, mut s2, mut s3) = (0u64, 0u64, 0u64, 0u64);
    let chunks = a.chunks_exact(4);
    let rem = chunks.remainder();
    for c in chunks {
        s0 = s0.wrapping_add(c[0]);
        s1 = s1.wrapping_add(c[1]);
        s2 = s2.wrapping_add(c[2]);
        s3 = s3.wrapping_add(c[3]);
    }
    let mut s = s0.wrapping_add(s1).wrapping_add(s2).wrapping_add(s3);
    for &x in rem {
        s = s.wrapping_add(x);
    }
    s
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let mode = a.get(1).map(String::as_str).unwrap_or("");
    let n: u64 = a.get(2).and_then(|s| s.parse().ok()).unwrap_or(1000);
    match mode {
        "foldsum" => {
            let runs = if n <= 100_000 { 9 } else { 5 };
            let naive_runs = if n > 1_000_000 { 1 } else { runs };
            let match_ok = n > 200_000 || closed_form_sq(n) == naive_sq(n);
            let cf = median_ns(|| closed_form_sq(black_box(n)), runs);
            let nv = median_ns(|| naive_sq(black_box(n)), naive_runs);
            let ratio = nv as f64 / cf.max(1) as f64;
            println!("foldsum n={n} closed_ns={cf} naive_ns={nv} ratio={ratio:.2} match={match_ok}");
        }
        "unstruct" => {
            let data: Vec<u64> = (0..n).map(|i| i.wrapping_mul(2654435761) & 0xffff).collect();
            let runs = if n <= 2_000_000 { 9 } else { 5 };
            let nv = median_ns(|| data_sum_naive(black_box(&data)), runs);
            let ur = median_ns(|| data_sum_unrolled(black_box(&data)), runs);
            let ratio = nv as f64 / ur.max(1) as f64;
            println!("unstruct n={n} naive_ns={nv} unrolled_ns={ur} speedup={ratio:.2}");
        }
        _ => {
            eprintln!("usage: haran_bench foldsum|unstruct <n>");
            std::process::exit(2);
        }
    }
}
