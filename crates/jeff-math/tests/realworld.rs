//! Stage 14.4 — real-world end-to-end validation (master directive PART G).
//!
//! Two genuine target workloads exercised end to end with correctness assertions, plus
//! honest wall-clock measurements (printed, never asserted or hard-coded — R8/DR2). No
//! incumbent comparison is claimed here: FFTW/OpenBLAS/reference-Kyber are not present in
//! this environment, so only absolute, this-machine numbers are reported.

use jeff_math::{mldsa, mlkem, prony};
use std::time::Instant;

fn seed(tag: u64) -> [u8; 32] {
    // deterministic, distinct per tag (tests must be reproducible, R11).
    let mut s = [0u8; 32];
    let mut x = tag.wrapping_mul(0x9E3779B97F4A7C15).wrapping_add(1);
    for b in s.iter_mut() {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        *b = (x & 0xff) as u8;
    }
    s
}

/// A full post-quantum handshake: ML-KEM key establishment, authenticated by an ML-DSA
/// signature over the encapsulation key (so a man-in-the-middle who swaps `ek` is caught).
#[test]
fn pqc_handshake_end_to_end() {
    // Alice's long-term signing identity (ML-DSA) and ephemeral KEM key (ML-KEM).
    let t0 = Instant::now();
    let (vpk, vsk) = mldsa::keygen(&seed(1)); // identity key
    let (ek, dk) = mlkem::keygen(&seed(2), &seed(3)); // ephemeral KEM key
    let keygen_us = t0.elapsed().as_micros();

    // Alice authenticates her ek by signing it.
    let sig = mldsa::sign(&vsk, &ek);

    // Bob verifies Alice's identity over ek, then encapsulates a shared secret.
    assert!(mldsa::verify(&vpk, &ek, &sig), "Alice's ek signature must verify");
    let t1 = Instant::now();
    let (k_bob, ct) = mlkem::encaps(&ek, &seed(4));
    let encaps_us = t1.elapsed().as_micros();

    // Alice decapsulates; both sides must hold the same secret.
    let t2 = Instant::now();
    let k_alice = mlkem::decaps(&dk, &ct);
    let decaps_us = t2.elapsed().as_micros();
    assert_eq!(k_alice, k_bob, "KEM shared secret must agree");

    // Active attacker swaps ek for their own — the signature no longer matches → rejected.
    let (ek_mallory, _) = mlkem::keygen(&seed(99), &seed(98));
    assert!(
        !mldsa::verify(&vpk, &ek_mallory, &sig),
        "a swapped ek must fail authentication"
    );

    // a signature must also authenticate the message it signed: tamper ek bytes → reject.
    let mut ek_tampered = ek.clone();
    ek_tampered[0] ^= 0x01;
    assert!(!mldsa::verify(&vpk, &ek_tampered, &sig), "tampered ek must be rejected");

    // Honest, this-machine timings (measurement only; no speedup claim).
    eprintln!(
        "[pqc] keygen(KEM+DSA)={keygen_us}us encaps={encaps_us}us decaps={decaps_us}us \
         ek={}B ct={}B",
        ek.len(),
        ct.len(),
    );
}

/// A signal-processing pipeline: Prony spectral estimation recovers a sparse
/// exponential/sinusoidal model from samples (collapse), but HONESTLY DEFERS on broadband
/// noise — the structure-present vs structure-absent boundary on real-shaped data.
#[test]
fn signal_pipeline_prony_collapse_vs_noise_defer() {
    // two clean decaying modes: x_t = 2·0.9^t + 0.5^t  → order-2 linear recurrence.
    let clean: Vec<f64> = (0..24).map(|t| 2.0 * 0.9_f64.powi(t) + 0.5_f64.powi(t)).collect();
    let t0 = Instant::now();
    let fit = prony::prony_fit(&clean, 2);
    let fit_us = t0.elapsed().as_micros();
    let (coeffs, residual) = fit.expect("two-mode signal is Prony-fittable");
    assert!(residual < 1e-9, "clean modes ⇒ exact recurrence (residual {residual:e})");
    assert!(coeffs.len() >= 2, "an order-≥2 model is recovered");

    // white noise: no low-order recurrence ⇒ the residual stays large ⇒ honest defer.
    let mut x = 0x1234_5678u64;
    let noise: Vec<f64> = (0..24)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            ((x >> 11) as f64 / (1u64 << 53) as f64) * 2.0 - 1.0
        })
        .collect();
    let noisy_residual = match prony::prony_fit(&noise, 2) {
        Some((_, r)) => r,
        None => f64::INFINITY,
    };
    assert!(
        noisy_residual > 1e-3,
        "broadband noise must NOT yield a clean order-2 recurrence (residual {noisy_residual:e})"
    );

    eprintln!(
        "[signal] prony fit {fit_us}us; clean residual≈{residual:e}, noise residual≈{noisy_residual:e}"
    );
}
