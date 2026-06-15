//! Stage 30 measurement: reproduce the reference k4 behavior at n=300, m=200, γ=0.667 —
//! MP edge / BBP threshold / TW scale, the detection-probability transition through θ*, and the
//! empirical false-positive rate vs ε. Verification-power (a meta-gate), not a speed collapse.

use jeff_math::bbp::{
    bbp_threshold, calibrate_threshold, mp_top_edge, noise_plus_spike, soundness_gate, top_singular_value, tw_scale,
};
use jeff_math::fmat::Rng;

fn main() {
    let (n, m, sigma, eps) = (300usize, 200usize, 1.0, 0.01);
    let gamma = m as f64 / n as f64;
    println!("=== #4 BBP soundness gate: n={n}, m={m}, gamma={gamma:.3}, sigma={sigma} ===");
    println!("  MP top edge (1+sqrt(gamma))        = {:.4}  (ref 1.8165)", mp_top_edge(gamma));
    println!("  BBP threshold theta* = gamma^(1/4) = {:.4}  (ref 0.9036)", bbp_threshold(gamma));
    println!("  TW null scale n^(-2/3)             = {:.4}  (ref 0.0223)", tw_scale(n));

    let thr = calibrate_threshold(n, m, sigma, eps, 300, 0);
    println!("  calibrated decision threshold (1-eps={} quantile) = {thr:.4}", 1.0 - eps);

    println!("\n=== BBP transition: detection probability vs spike strength theta ===");
    println!("  {:>7} {:>11} {:>13} {:>7}", "theta", "P(detect)", "mean_top_sv", ">edge?");
    let theta_star = bbp_threshold(gamma);
    let edge = mp_top_edge(gamma);
    for &theta in &[0.0, 0.3, 0.5, theta_star * 0.9, theta_star, theta_star * 1.1, 0.9, 1.3, 2.0] {
        let trials = 80;
        let mut det = 0;
        let mut sum_top = 0.0;
        for t in 0..trials {
            let a = noise_plus_spike(n, m, theta, 1, 1000 + t as u64);
            let an: Vec<f64> = a.iter().map(|x| x / (n as f64).sqrt()).collect();
            let stop = top_singular_value(&an, n, m, 100, 5);
            sum_top += stop;
            if stop > thr {
                det += 1;
            }
        }
        let mean_top = sum_top / trials as f64;
        let marker = if (theta - theta_star).abs() < 1e-9 { "  <- theta*" } else { "" };
        println!("  {:>7.3} {:>11.3} {:>13.4} {:>7}{}", theta, det as f64 / trials as f64, mean_top, mean_top > edge, marker);
    }

    println!("\n=== gate decisions on concrete inputs ===");
    let mut rng = Rng::new(3);
    let noise: Vec<f64> = (0..n * m).map(|_| rng.gaussian()).collect();
    let r = soundness_gate(&noise, n, m, sigma, thr, gamma, eps, 5);
    println!("  pure noise     : {:?}", r);
    let structured = noise_plus_spike(n, m, 2.5, 3, 9);
    let r = soundness_gate(&structured, n, m, sigma, thr, gamma, eps, 5);
    println!("  noise + rank-3 : {:?}", r);

    println!("\n=== false-positive rate: does the gate honor eps? ===");
    let trials = 1000;
    let mut fp = 0;
    for t in 0..trials {
        let mut r = Rng::new(900_000 + t as u64);
        let a: Vec<f64> = (0..n * m).map(|_| r.gaussian()).collect();
        let an: Vec<f64> = a.iter().map(|x| x / (n as f64).sqrt()).collect();
        if top_singular_value(&an, n, m, 100, 5) > thr {
            fp += 1;
        }
    }
    println!("  empirical FP rate on pure noise = {:.4}  (target eps={eps}, ref 0.018)", fp as f64 / trials as f64);
}
