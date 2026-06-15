//! Stage 28 measurement.
//!  28.1 dense FFT: Stockham (no bit-reversal) vs old radix-2 (self-relative) + a checksum the
//!       companion Python run cross-checks against numpy.fft (pocketfft, the tuned incumbent).
//!  28.2 2D sparse FFT: hikp_sparse_fft_2d vs dense fft2d — n/k crossover, ratio ~ N/k (diverges,
//!       structure-only).

use jeff_math::complex::Complex;
use jeff_math::fft::stockham_fft;
use jeff_math::sparsefft::fft_radix2;
use jeff_math::sparsefft2d::{fft2d, hikp_sparse_fft_2d};
use std::f64::consts::TAU;
use std::time::Instant;

fn best<T, F: FnMut() -> T>(reps: usize, mut f: F) -> (T, f64) {
    let mut bt = f64::INFINITY;
    let mut v = None;
    for _ in 0..reps {
        let t = Instant::now();
        v = Some(std::hint::black_box(f()));
        bt = bt.min(t.elapsed().as_secs_f64());
    }
    (v.unwrap(), bt)
}

fn signal_1d(n: usize) -> Vec<Complex> {
    (0..n).map(|t| Complex::new(((t * 7 + 3) % 11) as f64 - 5.0, ((t * 5 + 1) % 13) as f64 - 6.0)).collect()
}

fn main() {
    println!("== 28.1 dense FFT: Stockham (no bit-reversal) vs old radix-2 ==");
    println!("{:>8} {:>12} {:>12} {:>10} {:>18}", "n", "stockham_us", "radix2_us", "ratio", "checksum(|X[5]|)");
    for e in [12u32, 14, 16, 18] {
        let n = 1usize << e;
        let x = signal_1d(n);
        let (sc, ts) = best(20, || stockham_fft(&x));
        let (_rc, tr) = best(20, || fft_radix2(&x));
        println!("{:>8} {:>12.2} {:>12.2} {:>10.2} {:>18.6}", n, ts * 1e6, tr * 1e6, tr / ts, sc[5].abs());
    }

    println!("\n== 28.2 2D sparse FFT: sparse O(k log k) vs dense fft2d O(n² log n) ==");
    // fixed k, grow n: ratio must grow ~ N²/(k log k).
    let k = 4usize;
    let spikes: Vec<(usize, usize, f64, f64)> =
        vec![(1, 2, 1.0, 0.0), (5, 9, 0.7, -0.3), (20, 3, 0.4, 0.2), (33, 17, 0.5, 0.1)];
    println!("[fixed k={k}] grow n:");
    println!("{:>6} {:>14} {:>14} {:>10} {:>8}", "n", "sparse_us", "dense_us", "ratio", "k_found");
    let mut prev = 0f64;
    for e in [6u32, 7, 8, 9, 10] {
        let n = 1usize << e;
        let mut x = vec![Complex::zero(); n * n];
        for t in 0..n {
            for s in 0..n {
                let mut acc = Complex::zero();
                for &(fx, fy, re, im) in &spikes {
                    let a = TAU * (fx as f64 * t as f64 + fy as f64 * s as f64) / n as f64;
                    acc = acc.add(Complex::new(re, im).mul(Complex::new(a.cos(), a.sin())));
                }
                x[t * n + s] = acc;
            }
        }
        let (sup, tsp) = best(10, || hikp_sparse_fft_2d(&x, n, k, 1e-6).unwrap());
        let (_d, td) = best(if n <= 256 { 5 } else { 2 }, || fft2d(&x, n));
        let ratio = td / tsp;
        println!("{:>6} {:>14.3} {:>14.3} {:>10.1} {:>8}  {}", n, tsp * 1e6, td * 1e6, ratio, sup.len(),
            if ratio > prev { "DIVERGING (≈N²/k log k)" } else { "" });
        prev = ratio;
    }

    // fixed n, grow k: find the crossover where dense overtakes sparse.
    println!("\n[fixed n=512] grow k (crossover where dense wins):");
    println!("{:>6} {:>14} {:>14} {:>10} {:>10}", "k", "sparse_us", "dense_us", "ratio", "verdict");
    let n = 512usize;
    let (_d512, td512) = best(2, || {
        let x = vec![Complex::zero(); n * n];
        fft2d(&x, n)
    });
    for k in [1usize, 2, 4, 8, 16, 32, 64, 128] {
        let mut sp = Vec::new();
        for j in 0..k {
            sp.push(((j * 7 + 1) % (n / 2), (j * 11 + 2) % (n / 2), 1.0 / (j as f64 + 1.0), 0.0));
        }
        let mut x = vec![Complex::zero(); n * n];
        for t in 0..n {
            for s in 0..n {
                let mut acc = Complex::zero();
                for &(fx, fy, re, im) in &sp {
                    let a = TAU * (fx as f64 * t as f64 + fy as f64 * s as f64) / n as f64;
                    acc = acc.add(Complex::new(re, im).mul(Complex::new(a.cos(), a.sin())));
                }
                x[t * n + s] = acc;
            }
        }
        match hikp_sparse_fft_2d(&x, n, k, 1e-9) {
            Some(_) => {
                let (_s, tsp) = best(5, || hikp_sparse_fft_2d(&x, n, k, 1e-9).unwrap());
                let ratio = td512 / tsp;
                println!("{:>6} {:>14.3} {:>14.3} {:>10.2} {:>10}", k, tsp * 1e6, td512 * 1e6, ratio,
                    if ratio > 1.0 { "sparse wins" } else { "dense wins" });
            }
            None => println!("{:>6} {:>14} {:>14.3} {:>10} {:>10}", k, "DEFER", td512 * 1e6, "-", "→ dense"),
        }
    }
}
