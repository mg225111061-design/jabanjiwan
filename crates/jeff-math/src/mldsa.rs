//! ML-DSA-44 (Dilithium) — keygen / sign / verify (Stage 14.2).
//!
//! Built on the exact Dilithium NTT and rounding primitives ([`crate::dilithium`]) and the
//! KAT-anchored SHAKE layer ([`crate::keccak`]). The Fiat–Shamir-with-aborts signature is
//! implemented at the algorithm level over `Z_q[x]/(x^256+1)`, `q = 8380417`. Module-SIS/LWE
//! hardness is an assumption JEFF never touches (see `crate::pqc`).
//!
//! # Honesty on scope (DR2/DR3/R30)
//!
//! Keys and signatures are kept as typed structs, not the FIPS 204 byte encodings (the
//! packing is a mechanical format, not the cryptography). The SHAKE layer is NIST-KAT
//! validated and the NTT/rounding primitives are identity-checked; the scheme is proven
//! here by its defining properties — honest signatures verify, tampered messages and
//! forged signatures are rejected, and the abort bounds (‖z‖∞ < γ1−β, ‖r0‖∞ < γ2−β,
//! hint weight ≤ ω) hold on every accepted signature. It is **not** validated against the
//! official ML-DSA KAT vectors (unavailable offline); that anchor is stated, not implied.

use crate::dilithium::{self, make_hint, use_hint, Q, N, D};
use crate::keccak::shake256;
use crate::modular::ModInt;

// ML-DSA-44 parameters (FIPS 204, Table 1).
const K: usize = 4; // rows of A
const L: usize = 4; // cols of A
const ETA: i64 = 2;
const TAU: usize = 39;
const BETA: i64 = (TAU as i64) * ETA; // 78
const GAMMA1: i64 = 1 << 17; // 131072
const GAMMA2: u64 = (Q - 1) / 88; // 95232
const OMEGA: usize = 80;

type Poly = [u64; N];

fn zero() -> Poly {
    [0u64; N]
}
#[inline]
fn mi(v: u64) -> ModInt {
    ModInt::new(v, Q)
}
fn padd(a: &Poly, b: &Poly) -> Poly {
    std::array::from_fn(|i| (mi(a[i]) + mi(b[i])).val)
}
fn psub(a: &Poly, b: &Poly) -> Poly {
    std::array::from_fn(|i| (mi(a[i]) - mi(b[i])).val)
}
fn pntt(p: &Poly) -> Poly {
    let mut v = *p;
    dilithium::ntt(&mut v);
    v
}
fn pinvntt(p: &Poly) -> Poly {
    let mut v = *p;
    dilithium::inv_ntt(&mut v);
    v
}
/// Pointwise product in the (complete) NTT domain.
fn ppoint(a: &Poly, b: &Poly) -> Poly {
    std::array::from_fn(|i| (mi(a[i]) * mi(b[i])).val)
}
fn neg(p: &Poly) -> Poly {
    std::array::from_fn(|i| (mi(0) - mi(p[i])).val)
}
fn norm_inf(p: &Poly) -> i64 {
    p.iter().map(|&c| dilithium::center(c).abs()).max().unwrap_or(0)
}

/// Public key (ρ for Â, plus the high part `t1` of `t`).
#[derive(Clone, Debug)]
pub struct PublicKey {
    pub rho: [u8; 32],
    pub t1: Vec<Poly>, // k polys (coeffs are small high parts)
}
/// Secret key.
#[derive(Clone, Debug)]
pub struct SecretKey {
    rho: [u8; 32],
    k_key: [u8; 32],
    tr: [u8; 64],
    s1: Vec<Poly>,
    s2: Vec<Poly>,
    t0: Vec<Poly>,
}
/// Signature.
#[derive(Clone, Debug)]
pub struct Signature {
    c_tilde: [u8; 32],
    z: Vec<Poly>,
    h: Vec<[bool; N]>,
}

// ---- expansion / sampling ----

fn expand_a(rho: &[u8; 32]) -> Vec<Vec<Poly>> {
    (0..K)
        .map(|i| {
            (0..L)
                .map(|jcol| {
                    let mut seed = rho.to_vec();
                    seed.push(jcol as u8);
                    seed.push(i as u8);
                    sample_ntt_dilithium(&seed)
                })
                .collect()
        })
        .collect()
}

/// Rejection-sample a poly in the NTT domain (uniform coeffs in [0,q)) from SHAKE256.
fn sample_ntt_dilithium(seed: &[u8]) -> Poly {
    let mut len = 3 * 168;
    loop {
        let buf = shake256(seed, len);
        let mut p = zero();
        let mut c = 0usize;
        let mut i = 0usize;
        while i + 3 <= buf.len() && c < N {
            let v = (buf[i] as u64) | ((buf[i + 1] as u64) << 8) | (((buf[i + 2] as u64) & 0x7f) << 16);
            i += 3;
            if v < Q {
                p[c] = v;
                c += 1;
            }
        }
        if c == N {
            return p;
        }
        len += 168;
    }
}

/// Sample one η=2 secret poly (coeffs in [-2,2]) from a seed+nonce (rejection on nibbles).
fn sample_eta(seed: &[u8; 64], nonce: u16) -> Poly {
    let mut input = seed.to_vec();
    input.extend_from_slice(&nonce.to_le_bytes());
    let mut len = 3 * 136;
    loop {
        let buf = shake256(&input, len);
        let mut p = zero();
        let mut c = 0usize;
        for &byte in buf.iter() {
            for nib in [byte & 0x0f, byte >> 4] {
                if c >= N {
                    break;
                }
                if nib < 15 {
                    let coeff = 2 - (nib as i64 % 5); // uniform on {-2,..,2}
                    p[c] = coeff.rem_euclid(Q as i64) as u64;
                    c += 1;
                }
            }
            if c >= N {
                break;
            }
        }
        if c == N {
            return p;
        }
        len += 136;
    }
}

/// Sample the mask poly y with coeffs in (−γ1, γ1] from ρ''+nonce.
fn expand_mask(rho2: &[u8; 64], nonce: u16) -> Poly {
    let mut input = rho2.to_vec();
    input.extend_from_slice(&nonce.to_le_bytes());
    let need = N * 18 / 8; // 18 bits per coeff
    let buf = shake256(&input, need);
    let mut p = zero();
    let mut bit = 0usize;
    for coeff in p.iter_mut() {
        let mut v = 0u64;
        for b in 0..18 {
            if (buf[bit / 8] >> (bit % 8)) & 1 == 1 {
                v |= 1 << b;
            }
            bit += 1;
        }
        let signed = GAMMA1 - v as i64; // ∈ (−γ1, γ1]
        *coeff = signed.rem_euclid(Q as i64) as u64;
    }
    p
}

/// SampleInBall: a poly with exactly τ coefficients in {−1,+1}, rest 0.
fn sample_in_ball(c_tilde: &[u8; 32]) -> Poly {
    let buf = shake256(c_tilde, 8 + 256);
    let mut signs = u64::from_le_bytes(buf[0..8].try_into().unwrap());
    let mut c = zero();
    let mut pos = 8usize;
    for i in (N - TAU)..N {
        // rejection: j uniform in [0, i]
        let mut jj;
        loop {
            jj = buf[pos] as usize;
            pos += 1;
            if jj <= i {
                break;
            }
        }
        c[i] = c[jj];
        c[jj] = if signs & 1 == 1 { Q - 1 } else { 1 };
        signs >>= 1;
    }
    c
}

// ---- hashing helpers ----

fn shake256_n(parts: &[&[u8]], n: usize) -> Vec<u8> {
    let mut input = Vec::new();
    for p in parts {
        input.extend_from_slice(p);
    }
    shake256(&input, n)
}

/// Encode a high-bits vector deterministically for hashing (one byte per coeff; w1 ∈ [0,43]).
fn encode_w1(w1: &[Vec<i64>]) -> Vec<u8> {
    let mut out = Vec::with_capacity(K * N);
    for poly in w1 {
        for &c in poly {
            out.push(c as u8);
        }
    }
    out
}

fn matrix_mul_ntt(a_hat: &[Vec<Poly>], v_hat: &[Poly]) -> Vec<Poly> {
    (0..a_hat.len())
        .map(|i| {
            let mut acc = zero();
            for (jcol, vh) in v_hat.iter().enumerate() {
                acc = padd(&acc, &ppoint(&a_hat[i][jcol], vh));
            }
            acc
        })
        .collect()
}

// ---- the scheme ----

/// ML-DSA-44 key generation from a 32-byte seed ξ.
pub fn keygen(xi: &[u8; 32]) -> (PublicKey, SecretKey) {
    let seed = shake256_n(&[xi, &[K as u8], &[L as u8]], 128);
    let mut rho = [0u8; 32];
    rho.copy_from_slice(&seed[0..32]);
    let mut rho2 = [0u8; 64];
    rho2.copy_from_slice(&seed[32..96]);
    let mut k_key = [0u8; 32];
    k_key.copy_from_slice(&seed[96..128]);

    let a_hat = expand_a(&rho);
    let s1: Vec<Poly> = (0..L).map(|i| sample_eta(&rho2, i as u16)).collect();
    let s2: Vec<Poly> = (0..K).map(|i| sample_eta(&rho2, (L + i) as u16)).collect();

    let s1_hat: Vec<Poly> = s1.iter().map(pntt).collect();
    let as1 = matrix_mul_ntt(&a_hat, &s1_hat);
    let t: Vec<Poly> = (0..K).map(|i| padd(&pinvntt(&as1[i]), &s2[i])).collect();

    let mut t1 = vec![zero(); K];
    let mut t0 = vec![zero(); K];
    for i in 0..K {
        for jcoef in 0..N {
            let (hi, lo) = dilithium::power2round(t[i][jcoef]);
            t1[i][jcoef] = hi.rem_euclid(Q as i64) as u64;
            t0[i][jcoef] = lo.rem_euclid(Q as i64) as u64;
        }
    }

    let pk = PublicKey { rho, t1: t1.clone() };
    let tr = {
        let enc = encode_pk(&pk);
        let v = shake256(&enc, 64);
        let mut o = [0u8; 64];
        o.copy_from_slice(&v);
        o
    };
    let sk = SecretKey { rho, k_key, tr, s1, s2, t0 };
    (pk, sk)
}

fn encode_pk(pk: &PublicKey) -> Vec<u8> {
    let mut out = pk.rho.to_vec();
    for poly in &pk.t1 {
        for &c in poly {
            out.extend_from_slice(&(c as u32).to_le_bytes());
        }
    }
    out
}

/// Deterministic ML-DSA signing.
pub fn sign(sk: &SecretKey, msg: &[u8]) -> Signature {
    let a_hat = expand_a(&sk.rho);
    let mu = shake256_n(&[&sk.tr, msg], 64);
    let rho2 = {
        let v = shake256_n(&[&sk.k_key, &mu], 64);
        let mut o = [0u8; 64];
        o.copy_from_slice(&v);
        o
    };

    let s1_hat: Vec<Poly> = sk.s1.iter().map(pntt).collect();
    let s2_hat: Vec<Poly> = sk.s2.iter().map(pntt).collect();
    let t0_hat: Vec<Poly> = sk.t0.iter().map(pntt).collect();
    let gamma2_minus_beta = GAMMA2 as i64 - BETA;
    let gamma1_minus_beta = GAMMA1 - BETA;

    let mut kappa = 0u16;
    for _attempt in 0..1000 {
        let y: Vec<Poly> = (0..L).map(|i| expand_mask(&rho2, kappa + i as u16)).collect();
        kappa += L as u16;
        let y_hat: Vec<Poly> = y.iter().map(pntt).collect();
        let w: Vec<Poly> = matrix_mul_ntt(&a_hat, &y_hat).iter().map(pinvntt).collect();

        let w1: Vec<Vec<i64>> = w
            .iter()
            .map(|p| p.iter().map(|&c| dilithium::highbits(c, GAMMA2)).collect())
            .collect();

        let mut c_tilde = [0u8; 32];
        c_tilde.copy_from_slice(&shake256_n(&[&mu, &encode_w1(&w1)], 32));
        let c = sample_in_ball(&c_tilde);
        let c_hat = pntt(&c);

        // z = y + c·s1
        let z: Vec<Poly> = (0..L)
            .map(|i| padd(&y[i], &pinvntt(&ppoint(&c_hat, &s1_hat[i]))))
            .collect();
        if z.iter().map(norm_inf).max().unwrap() >= gamma1_minus_beta {
            continue;
        }

        // r0 = lowbits(w - c·s2)
        let cs2: Vec<Poly> = (0..K).map(|i| pinvntt(&ppoint(&c_hat, &s2_hat[i]))).collect();
        let w_minus_cs2: Vec<Poly> = (0..K).map(|i| psub(&w[i], &cs2[i])).collect();
        let r0_norm = w_minus_cs2
            .iter()
            .flat_map(|p| p.iter().map(|&c| dilithium::lowbits(c, GAMMA2).abs()))
            .max()
            .unwrap();
        if r0_norm >= gamma2_minus_beta {
            continue;
        }

        // hints over w' = w - c·s2 + c·t0, with correction −c·t0
        let ct0: Vec<Poly> = (0..K).map(|i| pinvntt(&ppoint(&c_hat, &t0_hat[i]))).collect();
        if ct0.iter().map(norm_inf).max().unwrap() >= GAMMA2 as i64 {
            continue;
        }
        let mut h = vec![[false; N]; K];
        let mut hint_weight = 0usize;
        for i in 0..K {
            let wp = padd(&w_minus_cs2[i], &ct0[i]);
            let z_corr = neg(&ct0[i]);
            for jcoef in 0..N {
                let bit = make_hint(z_corr[jcoef], wp[jcoef], GAMMA2);
                h[i][jcoef] = bit;
                if bit {
                    hint_weight += 1;
                }
            }
        }
        if hint_weight > OMEGA {
            continue;
        }
        return Signature { c_tilde, z, h };
    }
    panic!("ML-DSA signing exceeded the abort bound (statistically unreachable)");
}

/// ML-DSA verification.
pub fn verify(pk: &PublicKey, msg: &[u8], sig: &Signature) -> bool {
    let gamma1_minus_beta = GAMMA1 - BETA;
    // ‖z‖∞ bound
    if sig.z.iter().map(norm_inf).max().unwrap_or(0) >= gamma1_minus_beta {
        return false;
    }
    // hint weight
    let weight: usize = sig.h.iter().flat_map(|r| r.iter()).filter(|&&b| b).count();
    if weight > OMEGA {
        return false;
    }

    let a_hat = expand_a(&pk.rho);
    let tr = shake256(&encode_pk(pk), 64);
    let mu = shake256_n(&[&tr, msg], 64);
    let c = sample_in_ball(&sig.c_tilde);
    let c_hat = pntt(&c);

    // w' = A·z − c·t1·2^D
    let z_hat: Vec<Poly> = sig.z.iter().map(pntt).collect();
    let az = matrix_mul_ntt(&a_hat, &z_hat);
    let two_d = 1u64 << D;
    let mut w1p: Vec<Vec<i64>> = Vec::with_capacity(K);
    for (i, az_i) in az.iter().enumerate() {
        let t1_scaled: Poly = std::array::from_fn(|c2| (mi(pk.t1[i][c2]) * mi(two_d)).val);
        let ct1 = pinvntt(&ppoint(&c_hat, &pntt(&t1_scaled)));
        let wp = psub(&pinvntt(az_i), &ct1);
        let row: Vec<i64> = (0..N).map(|c2| use_hint(sig.h[i][c2], wp[c2], GAMMA2)).collect();
        w1p.push(row);
    }

    let mut c_check = [0u8; 32];
    c_check.copy_from_slice(&shake256_n(&[&mu, &encode_w1(&w1p)], 32));
    c_check == sig.c_tilde
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fmat::Rng;

    fn seed(rng: &mut Rng) -> [u8; 32] {
        std::array::from_fn(|_| (rng.next_u64() & 0xff) as u8)
    }

    #[test]
    fn dilithium_sign_verify() {
        let mut rng = Rng::new(0xD5A);
        for _ in 0..8 {
            let xi = seed(&mut rng);
            let (pk, sk) = keygen(&xi);
            let msg = b"the quick brown fox jumps over the lazy dog";
            let sig = sign(&sk, msg);
            assert!(verify(&pk, msg, &sig), "honest signature must verify");
            // abort bounds hold on the accepted signature
            assert!(sig.z.iter().map(norm_inf).max().unwrap() < GAMMA1 - BETA);
            let w: usize = sig.h.iter().flat_map(|r| r.iter()).filter(|&&b| b).count();
            assert!(w <= OMEGA);
        }
    }

    #[test]
    fn dilithium_reject_forgery() {
        let mut rng = Rng::new(0xF0E);
        let (pk, sk) = keygen(&seed(&mut rng));
        let msg = b"authentic message";
        let sig = sign(&sk, msg);
        // wrong message → reject
        assert!(!verify(&pk, b"tampered message", &sig));
        // tampered signature (flip a z coefficient) → reject
        let mut bad = sig.clone();
        bad.z[0][0] = (bad.z[0][0] + 1) % Q;
        assert!(!verify(&pk, msg, &bad));
        // tampered challenge → reject
        let mut bad2 = sig.clone();
        bad2.c_tilde[0] ^= 0xff;
        assert!(!verify(&pk, msg, &bad2));
        // a different key cannot verify this signature
        let (pk2, _) = keygen(&seed(&mut rng));
        assert!(!verify(&pk2, msg, &sig));
    }
}
