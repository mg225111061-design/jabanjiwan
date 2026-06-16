//! prony_check — HARAN v6 / U1: Prony spectral recovery, on the real jeff_math::prony asset.
//! A signal s_t = Σ_{j≤k} c_j·z_j^t satisfies a degree-k linear recurrence; the certificate is the
//! recurrence RESIDUAL (real arithmetic, no root-finding) — exactly 0 for a noiseless k-exponential
//! signal (deterministic), bounded by the noise level otherwise.
//! usage:  prony_check noiseless|noisy [eta]
//! output: PRONY k=.. residual=<r> coeffs=<a0,a1,..> max_noise=<eta>

use jeff_math::prony::{prony_fit, recurrence_residual};

fn main() {
    let mode = std::env::args().nth(1).unwrap_or_else(|| "noiseless".into());
    let eta: f64 = std::env::args().nth(2).and_then(|s| s.parse().ok()).unwrap_or(1e-3);
    let k = 2usize;
    let n = 2 * k + 4;
    // s_t = 1·0.9^t + 0.5·0.7^t  (two real exponentials)
    let mut s: Vec<f64> = (0..n).map(|t| 0.9f64.powi(t as i32) + 0.5 * 0.7f64.powi(t as i32)).collect();
    let mut maxn = 0.0f64;
    if mode == "noisy" {
        let mut seed = 0x2026_0616u64;
        for v in s.iter_mut() {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            let u = ((seed >> 33) as f64 / (1u64 << 31) as f64) - 1.0; // ~[-1,1)
            let e = u * eta;
            *v += e;
            maxn = maxn.max(e.abs());
        }
    }
    match prony_fit(&s, k) {
        Some((a, resid)) => {
            let r2 = recurrence_residual(&s, &a);
            let acoeffs: Vec<String> = a.iter().map(|x| format!("{x:.6}")).collect();
            println!("PRONY k={k} residual={:.3e} residual_check={:.3e} coeffs={} max_noise={:.3e}",
                     resid, r2, acoeffs.join(","), maxn);
        }
        None => println!("PRONY_FAIL"),
    }
}
