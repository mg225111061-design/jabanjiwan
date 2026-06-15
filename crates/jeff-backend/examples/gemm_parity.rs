//! Stage 27.3 — dense f64 GEMM vs OpenBLAS (numpy backend). Integer-valued matrices (so the
//! cross-check vs OpenBLAS is exact). Prints JEFF (new packed AVX-512) and the old blocked path
//! timings + verification entries; a companion Python run does numpy `@` (OpenBLAS) on the
//! identical matrices. We report fraction-of-OpenBLAS, never "beat" (dense = parity ceiling).

use jeff_backend::gemm::{gemm, Blocking};
use jeff_backend::simd::gemm_blocked;
use std::time::Instant;

fn mat_a(n: usize) -> Vec<f64> {
    (0..n * n).map(|t| (((t / n) * 7 + (t % n) * 3) % 11) as f64 - 5.0).collect()
}
fn mat_b(n: usize) -> Vec<f64> {
    (0..n * n).map(|t| (((t / n) * 5 + (t % n) * 2) % 13) as f64 - 6.0).collect()
}

fn best<F: FnMut() -> Vec<f64>>(reps: usize, mut f: F) -> (Vec<f64>, f64) {
    let mut bt = f64::INFINITY;
    let mut v = Vec::new();
    for _ in 0..reps {
        let t = Instant::now();
        v = std::hint::black_box(f());
        bt = bt.min(t.elapsed().as_secs_f64());
    }
    (v, bt)
}

fn main() {
    println!("[gemm] avx512 path available: {}", jeff_backend::gemm::avx512_available());
    println!("[gemm] blocking @ n=1024: {:?}", Blocking::derive(&jeff_backend::cpuprobe::probe()));
    println!("{:>6} {:>14} {:>14} {:>12} {:>22}", "n", "jeff_new_ms", "jeff_old_ms", "old/new", "verify C0,Cmid,Clast");
    for &n in &[256usize, 512, 1024, 2048] {
        let a = mat_a(n);
        let b = mat_b(n);
        let reps = if n <= 512 { 10 } else { 3 };
        let (cnew, tnew) = best(reps, || gemm(&a, &b, n, n, n));
        let (cold, told) = best(reps, || gemm_blocked(&a, &b, n, n, n));
        assert_eq!(cnew, cold, "new vs old GEMM must agree (integer inputs) at n={n}");
        println!(
            "{:>6} {:>14.4} {:>14.4} {:>12.2} {:>8} {:>8} {:>8}",
            n, tnew * 1e3, told * 1e3, told / tnew,
            cnew[0], cnew[n * n / 2 + n / 2], cnew[n * n - 1]
        );
        // GFLOP/s for the new path: 2 n^3 flops.
        let gf = 2.0 * (n as f64).powi(3) / tnew / 1e9;
        println!("        jeff_new = {gf:.1} GFLOP/s  (n={n})");
    }
}
