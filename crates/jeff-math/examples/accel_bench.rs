//! accel_bench — HARAN v4.5 unstructured constant-factor acceleration, MEASURED (RELEASE, median,
//! black_box-hardened). Constant-factor ONLY — Ω(N) never broken (order improvements = fold/approx).
//! General kernels (no PQC). Modes: noalias (W1) · simd (W2) · soa (W3) · algo (W4) · parallel (W5).
//!
//! Honest expectations: memory-bound kernels gain little from SIMD/parallel (bandwidth floor);
//! compute-bound kernels gain more; SIMD and cache OVERLAP (share the memory bottleneck) — not a product.

#![allow(clippy::needless_range_loop)]

use std::arch::x86_64::*;
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

// ===================== W1 noalias =====================
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

// ===================== W2 SIMD =====================
fn sum_scalar(a: &[f64]) -> f64 {
    let mut s = 0.0;
    for &x in a {
        s += x;
    }
    s
}
#[target_feature(enable = "avx2")]
unsafe fn sum_avx2(a: &[f64]) -> f64 {
    let (mut a0, mut a1) = (_mm256_setzero_pd(), _mm256_setzero_pd());
    let mut ch = a.chunks_exact(8);
    for c in &mut ch {
        a0 = _mm256_add_pd(a0, _mm256_loadu_pd(c.as_ptr()));
        a1 = _mm256_add_pd(a1, _mm256_loadu_pd(c.as_ptr().add(4)));
    }
    let acc = _mm256_add_pd(a0, a1);
    let mut buf = [0.0f64; 4];
    _mm256_storeu_pd(buf.as_mut_ptr(), acc);
    buf.iter().sum::<f64>() + ch.remainder().iter().sum::<f64>()
}
fn poly8_scalar(a: &[f64], c: &[f64; 9]) -> f64 {
    let mut s = 0.0;
    for &x in a {
        let mut p = c[8];
        for k in (0..8).rev() {
            p = p * x + c[k];
        }
        s += p;
    }
    s
}
#[target_feature(enable = "avx2,fma")]
unsafe fn poly8_avx2(a: &[f64], c: &[f64; 9]) -> f64 {
    let mut acc = _mm256_setzero_pd();
    let mut ch = a.chunks_exact(4);
    for chunk in &mut ch {
        let x = _mm256_loadu_pd(chunk.as_ptr());
        let mut p = _mm256_set1_pd(c[8]);
        for k in (0..8).rev() {
            p = _mm256_fmadd_pd(p, x, _mm256_set1_pd(c[k]));
        }
        acc = _mm256_add_pd(acc, p);
    }
    let mut buf = [0.0f64; 4];
    _mm256_storeu_pd(buf.as_mut_ptr(), acc);
    buf.iter().sum::<f64>() + poly8_scalar(ch.remainder(), c)
}

// ===================== W3 SoA =====================
#[derive(Clone, Copy)]
struct P {
    x: f64,
    _y: f64,
    _z: f64,
    _w: f64,
}
fn sum_x_aos(p: &[P]) -> f64 {
    let mut s = 0.0;
    for q in p {
        s += q.x;
    } // strides 32 B, uses 8 — ~1/4 of each cache line
    s
}

// ===================== W4 algorithm: radix vs comparison sort (u32) =====================
fn radix_sort_u32(v: &[u32]) -> Vec<u32> {
    let mut a = v.to_vec();
    let mut b = vec![0u32; a.len()];
    for shift in [0u32, 8, 16, 24] {
        let mut cnt = [0usize; 256];
        for &x in &a {
            cnt[((x >> shift) & 0xff) as usize] += 1;
        }
        let mut sum = 0usize;
        for c in cnt.iter_mut() {
            let t = *c;
            *c = sum;
            sum += t;
        }
        for &x in &a {
            let d = ((x >> shift) & 0xff) as usize;
            b[cnt[d]] = x;
            cnt[d] += 1;
        }
        std::mem::swap(&mut a, &mut b);
    }
    a
}

// ===================== W5 parallel =====================
fn sum_par(a: &[f64], threads: usize) -> f64 {
    if threads <= 1 {
        return sum_scalar(a);
    }
    let chunk = a.len().div_ceil(threads);
    std::thread::scope(|sc| {
        let hs: Vec<_> = a.chunks(chunk).map(|ch| sc.spawn(move || sum_scalar(ch))).collect();
        hs.into_iter().map(|h| h.join().unwrap()).sum()
    })
}
#[inline(always)]
fn heavy_elem(x: f64) -> f64 {
    let mut v = x;
    for _ in 0..64 {
        v = v * 0.99999 + 0.00001;
    } // ~128 FLOPs, no extra memory ⇒ compute-bound
    v
}
fn heavy_seq(a: &[f64]) -> f64 {
    a.iter().map(|&x| heavy_elem(x)).sum()
}
fn heavy_par(a: &[f64], threads: usize) -> f64 {
    if threads <= 1 {
        return heavy_seq(a);
    }
    let chunk = a.len().div_ceil(threads);
    std::thread::scope(|sc| {
        let hs: Vec<_> = a.chunks(chunk).map(|ch| sc.spawn(move || heavy_seq(ch))).collect();
        hs.into_iter().map(|h| h.join().unwrap()).sum()
    })
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mode = args.get(1).map(String::as_str).unwrap_or("");
    let n: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(1 << 16);
    let a: Vec<f64> = (0..n).map(|i| (i as f64 * 0.001).sin() + 1.5).collect();
    let has_avx2 = is_x86_feature_detected!("avx2");

    match mode {
        "noalias" => {
            let b: Vec<f64> = (0..n).map(|i| (i as f64).cos()).collect();
            let runs = if n <= (1 << 18) { 50 } else { 15 };
            let mut c1 = vec![1.0f64; n];
            let na = med(|| fma_noalias(black_box(&a), black_box(&b), black_box(&mut c1)), runs);
            let mut c2 = vec![1.0f64; n];
            let al = med(|| unsafe { fma_aliased(black_box(a.as_ptr()), black_box(b.as_ptr()), black_box(c2.as_mut_ptr()), black_box(n)) }, runs);
            let s = 1.0000001f64;
            let mut y1 = vec![0.0f64; n];
            let sna = med(|| scale_noalias(black_box(&a), black_box(&s), black_box(&mut y1)), runs);
            let mut y2 = vec![0.0f64; n];
            let sal = med(|| unsafe { scale_aliased(black_box(a.as_ptr()), black_box(&s as *const f64), black_box(y2.as_mut_ptr()), black_box(n)) }, runs);
            println!("noalias n={n} fma_speedup={:.2} scale_speedup={:.2}", al as f64 / na as f64, sal as f64 / sna as f64);
        }
        "simd" => {
            let runs = if n <= (1 << 18) { 50 } else { 15 };
            let c = [0.1, -0.2, 0.3, -0.05, 0.02, -0.01, 0.004, -0.001, 0.0003];
            let sum_s = med(|| sum_scalar(black_box(&a)), runs);
            let sum_v = if has_avx2 { med(|| unsafe { sum_avx2(black_box(&a)) }, runs) } else { sum_s };
            let pol_s = med(|| poly8_scalar(black_box(&a), black_box(&c)), runs);
            let pol_v = if has_avx2 { med(|| unsafe { poly8_avx2(black_box(&a), black_box(&c)) }, runs) } else { pol_s };
            let (rs, rv) = (sum_scalar(&a), if has_avx2 { unsafe { sum_avx2(&a) } } else { sum_scalar(&a) });
            let (qs, qv) = (poly8_scalar(&a, &c), if has_avx2 { unsafe { poly8_avx2(&a, &c) } } else { poly8_scalar(&a, &c) });
            let correct = (rs - rv).abs() <= 1e-6 * rs.abs().max(1.0) && (qs - qv).abs() <= 1e-6 * qs.abs().max(1.0);
            println!("simd n={n} sum_speedup={:.2} poly8_speedup={:.2} correct={correct}",
                     sum_s as f64 / sum_v.max(1) as f64, pol_s as f64 / pol_v.max(1) as f64);
        }
        "soa" => {
            let runs = if n <= (1 << 18) { 50 } else { 15 };
            let aos: Vec<P> = (0..n).map(|i| P { x: a[i], _y: 0.0, _z: 0.0, _w: 0.0 }).collect();
            let soa: Vec<f64> = a.clone();
            let t_aos = med(|| sum_x_aos(black_box(&aos)), runs);
            let t_soa = med(|| sum_scalar(black_box(&soa)), runs);
            // SIMD on SoA (contiguous) — to show SIMD+cache OVERLAP, not a product
            let t_soa_simd = if has_avx2 { med(|| unsafe { sum_avx2(black_box(&soa)) }, runs) } else { t_soa };
            let correct = (sum_x_aos(&aos) - sum_scalar(&soa)).abs() <= 1e-6 * sum_scalar(&soa).abs().max(1.0);
            println!("soa n={n} soa_speedup={:.2} soa_then_simd_extra={:.2} correct={correct}",
                     t_aos as f64 / t_soa.max(1) as f64, t_soa as f64 / t_soa_simd.max(1) as f64);
        }
        "algo" => {
            let mut seed = 0x2026u64;
            let keys: Vec<u32> = (0..n).map(|_| { seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1); (seed >> 33) as u32 }).collect();
            let runs = if n <= (1 << 18) { 20 } else { 7 };
            let t_std = med(|| { let mut v = keys.clone(); v.sort_unstable(); v }, runs);
            let t_rdx = med(|| radix_sort_u32(black_box(&keys)), runs);
            // correctness: radix == std sort
            let mut s = keys.clone(); s.sort_unstable();
            let ok = radix_sort_u32(&keys) == s;
            println!("algo n={n} std_sort_ns={t_std} radix_ns={t_rdx} speedup={:.2} correct={ok}", t_std as f64 / t_rdx.max(1) as f64);
        }
        "parallel" => {
            let threads: usize = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(4);
            let runs = if n <= (1 << 18) { 30 } else { 9 };
            let mem1 = med(|| sum_par(black_box(&a), 1), runs);
            let memp = med(|| sum_par(black_box(&a), threads), runs);
            let cpu1 = med(|| heavy_par(black_box(&a), 1), runs.min(9));
            let cpup = med(|| heavy_par(black_box(&a), threads), runs.min(9));
            let correct = (sum_par(&a, 1) - sum_par(&a, threads)).abs() <= 1e-6 * sum_par(&a, 1).abs().max(1.0);
            println!("parallel n={n} threads={threads} mem_speedup={:.2} cpu_speedup={:.2} correct={correct}",
                     mem1 as f64 / memp.max(1) as f64, cpu1 as f64 / cpup.max(1) as f64);
        }
        _ => {
            eprintln!("usage: accel_bench noalias|simd|soa|algo|parallel <n> [threads]");
            std::process::exit(2);
        }
    }
}
