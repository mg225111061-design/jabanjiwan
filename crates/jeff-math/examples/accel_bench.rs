//! accel_bench — HARAN v4.5 unstructured constant-factor acceleration, MEASURED (RELEASE, median,
//! black_box-hardened). Constant-factor only — Ω(N) is never broken (order improvements are fold/approx).
//! General kernels (no PQC). Modes added per stage W1..W5.
//!
//! W1 noalias: HARAN verifies buffer disjointness (own/&) ⇒ can emit `noalias` SAFELY, which a C
//! compiler can't prove (it needs the unchecked `restrict` promise). We measure the gap by comparing
//! a Rust &mut/& kernel (LLVM gets noalias) against the SAME kernel through raw pointers (LLVM must
//! assume possible aliasing → conservative codegen), both run on DISJOINT buffers.

use std::hint::black_box;
use std::time::Instant;

fn med<T, F: FnMut() -> T>(mut f: F, runs: usize) -> u128 {
    let mut ts = Vec::with_capacity(runs);
    for _ in 0..runs {
        let t = Instant::now();
        black_box(f());
        ts.push(t.elapsed().as_nanos());
    }
    ts.sort_unstable();
    ts[ts.len() / 2]
}

// c[i] = a[i]*b[i] + c[i] : streaming (memory-bound) — possible aliasing rarely matters here.
#[inline(never)]
fn fma_noalias(a: &[f64], b: &[f64], c: &mut [f64]) {
    for i in 0..c.len() {
        c[i] = a[i] * b[i] + c[i];
    }
}
#[inline(never)]
unsafe fn fma_aliased(a: *const f64, b: *const f64, c: *mut f64, n: usize) {
    for i in 0..n {
        *c.add(i) = *a.add(i) * *b.add(i) + *c.add(i);
    }
}

// y[i] = x[i] * *s : a value reused behind a pointer. If *s may alias y, it must be RELOADED every
// iteration (the store y[i] could change *s) ⇒ scalar. Verified-disjoint ⇒ hoist *s, vectorize.
// This is the textbook `restrict` win — and exactly what HARAN's verified own/& enables SAFELY.
#[inline(never)]
fn scale_noalias(x: &[f64], s: &f64, y: &mut [f64]) {
    for i in 0..y.len() {
        y[i] = x[i] * *s;
    }
}
#[inline(never)]
unsafe fn scale_aliased(x: *const f64, s: *const f64, y: *mut f64, n: usize) {
    for i in 0..n {
        *y.add(i) = *x.add(i) * *s;
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mode = args.get(1).map(String::as_str).unwrap_or("");
    let n: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(1 << 16);
    match mode {
        "noalias" => {
            let a: Vec<f64> = (0..n).map(|i| (i as f64).sin()).collect();
            let b: Vec<f64> = (0..n).map(|i| (i as f64).cos()).collect();
            let runs = if n <= (1 << 18) { 50 } else { 15 };
            // (1) streaming FMA — possible aliasing usually doesn't matter (memory-bound)
            let mut c1 = vec![1.0f64; n];
            let na = med(|| { fma_noalias(black_box(&a), black_box(&b), black_box(&mut c1)); }, runs);
            let mut c2 = vec![1.0f64; n];
            let al = med(|| unsafe {
                fma_aliased(black_box(a.as_ptr()), black_box(b.as_ptr()), black_box(c2.as_mut_ptr()), black_box(n));
            }, runs);
            // (2) reused scalar behind a pointer — possible aliasing forces reload (textbook restrict)
            let s = 1.0000001f64;
            let mut y1 = vec![0.0f64; n];
            let sna = med(|| { scale_noalias(black_box(&a), black_box(&s), black_box(&mut y1)); }, runs);
            let mut y2 = vec![0.0f64; n];
            let sal = med(|| unsafe {
                scale_aliased(black_box(a.as_ptr()), black_box(&s as *const f64), black_box(y2.as_mut_ptr()), black_box(n));
            }, runs);
            println!("noalias n={n} fma_noalias_ns={na} fma_aliased_ns={al} fma_speedup={:.2} \
scale_noalias_ns={sna} scale_aliased_ns={sal} scale_speedup={:.2}",
                     al as f64 / na as f64, sal as f64 / sna as f64);
        }
        _ => {
            eprintln!("usage: accel_bench noalias <n>");
            std::process::exit(2);
        }
    }
}
