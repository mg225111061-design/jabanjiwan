// JEFF dense FFT (fft_radix2) + sublinear HIKP sparse-FFT, timed, on a k-sparse signal.
use jeff_math::complex::Complex;
use jeff_math::sparsefft::{fft_radix2, hikp_sparse_fft};
use std::time::Instant;

fn signal(n: usize, tones: &[(usize, f64)]) -> Vec<f64> {
    (0..n).map(|t| tones.iter().map(|&(f,a)| a*(std::f64::consts::TAU*f as f64*t as f64/n as f64).cos()).sum()).collect()
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let n: usize = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(65536);
    let reps: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(20);
    let tones = [(5usize, 1.0), (777, 0.5), (10001usize.min(n/2-1), 0.25)];
    let x = signal(n, &tones);

    // dense fft_radix2
    let inp: Vec<Complex> = x.iter().map(|&v| Complex::new(v,0.0)).collect();
    let mut best = f64::MAX; let mut spec = Vec::new();
    for _ in 0..reps { let t=Instant::now(); spec = fft_radix2(&inp); let e=t.elapsed().as_secs_f64(); if e<best {best=e;} }
    println!("JEFF_DENSE_FFT n={n} time_s={best:.9}");
    // magnitude at first tone bin (accuracy anchor)
    let f0 = tones[0].0;
    println!("JEFF_DENSE_mag_bin{f0}={:.6}", spec[f0].abs());

    // sublinear HIKP
    let mut bh = f64::MAX; let mut sup = None;
    for _ in 0..reps { let t=Instant::now(); sup = hikp_sparse_fft(&x, 2*tones.len(), 1e-6); let e=t.elapsed().as_secs_f64(); if e<bh {bh=e;} }
    match sup {
        Some(s) => {
            println!("JEFF_HIKP n={n} time_s={bh:.9} recovered={}", s.len());
            // magnitude at first tone bin from recovered support
            if let Some(&(_,re,im)) = s.iter().find(|&&(fr,_,_)| fr==f0) {
                println!("JEFF_HIKP_mag_bin{f0}={:.6}", (re*re+im*im).sqrt());
            }
        }
        None => println!("JEFF_HIKP n={n} declined(None) -> dense fallback"),
    }
}
