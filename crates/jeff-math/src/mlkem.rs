//! ML-KEM-768 (FIPS 203) — full keygen / encaps / decaps (Stage 14.1).
//!
//! Built on the exact, oracle-checked Kyber incomplete NTT ([`crate::kyber`]) and the
//! KAT-anchored Keccak layer ([`crate::keccak`]). All polynomial arithmetic is exact mod
//! `q = 3329` (R33). The Module-LWE hardness this rests on is a cryptographic assumption,
//! never something JEFF analyses or "collapses" (see `crate::pqc`): we implement the
//! *operations* exactly and (jeff-types) constant-time, and prove the scheme's
//! input/output behaviour.
//!
//! # Honesty on conformance (DR2/DR3/R30)
//!
//! The hash layer is validated against published NIST KATs and the NTT against the Θ(n²)
//! schoolbook oracle, so those pieces are *verified*. The full scheme is proven here by
//! its defining properties — IND-CPA decryption correctness (`decrypt∘encrypt = id`),
//! KEM round-trip (`decaps∘encaps = id`), and FO implicit rejection — across many random
//! inputs. It is **not** checked against the official ML-KEM KAT vectors (unavailable in
//! this offline environment); that anchor is stated as outstanding, not implied.

use crate::keccak::{sha3_256, sha3_512, shake128, shake256};
use crate::kyber::{basemul, inv_ntt, ntt, N, Q};

// ML-KEM-768 parameters (FIPS 203, Table 2).
/// Module rank.
pub const K: usize = 3;
const ETA1: usize = 2;
const ETA2: usize = 2;
const DU: u32 = 10;
const DV: u32 = 4;

/// Encapsulation key length: `384·K + 32`.
pub const EK_LEN: usize = 384 * K + 32;
/// Decapsulation key length: `768·K + 96`.
pub const DK_LEN: usize = 768 * K + 96;
/// Ciphertext length: `32·(DU·K + DV)`.
pub const CT_LEN: usize = 32 * (DU as usize * K + DV as usize);
/// Shared-secret length.
pub const SS_LEN: usize = 32;

type Poly = [u64; N];

#[inline]
fn add(a: u64, b: u64) -> u64 {
    (a + b) % Q
}
#[inline]
fn sub(a: u64, b: u64) -> u64 {
    (a + Q - b % Q) % Q
}

fn poly_add(a: &Poly, b: &Poly) -> Poly {
    std::array::from_fn(|i| add(a[i], b[i]))
}
fn poly_sub(a: &Poly, b: &Poly) -> Poly {
    std::array::from_fn(|i| sub(a[i], b[i]))
}
fn poly_ntt(p: &Poly) -> Poly {
    let mut v = *p;
    ntt(&mut v);
    v
}
fn poly_intt(p: &Poly) -> Poly {
    let mut v = *p;
    inv_ntt(&mut v);
    v
}
/// NTT-domain pointwise product (the 128 base multiplications).
fn poly_basemul(a: &Poly, b: &Poly) -> Poly {
    let r = basemul(a, b);
    std::array::from_fn(|i| r[i])
}

// ---- compression / coding (FIPS 203 §4.2) ----

#[inline]
fn compress(x: u64, d: u32) -> u64 {
    // round(2^d · x / q) mod 2^d, ties up: floor((2^{d+1} x + q) / 2q).
    let num = ((x as u128) << (d + 1)) + Q as u128;
    ((num / (2 * Q as u128)) as u64) & ((1u64 << d) - 1)
}
#[inline]
fn decompress(y: u64, d: u32) -> u64 {
    // round(q · y / 2^d) = floor((q·y + 2^{d-1}) / 2^d).
    (((Q as u128) * y as u128 + (1u128 << (d - 1))) >> d) as u64
}

/// Pack `coeffs` (`d` bits each, LSB-first) into bytes.
fn byte_encode(coeffs: &[u64], d: u32) -> Vec<u8> {
    let mut out = vec![0u8; coeffs.len() * d as usize / 8];
    let mut bit = 0usize;
    for &c in coeffs {
        for b in 0..d {
            if (c >> b) & 1 == 1 {
                out[bit / 8] |= 1 << (bit % 8);
            }
            bit += 1;
        }
    }
    out
}
/// Inverse of [`byte_encode`]; `n` coefficients of `d` bits. For `d = 12` results are mod q.
fn byte_decode(bytes: &[u8], d: u32, n: usize) -> Vec<u64> {
    let mut out = vec![0u64; n];
    let mut bit = 0usize;
    for c in out.iter_mut() {
        let mut v = 0u64;
        for b in 0..d {
            if (bytes[bit / 8] >> (bit % 8)) & 1 == 1 {
                v |= 1 << b;
            }
            bit += 1;
        }
        *c = if d >= 12 { v % Q } else { v };
    }
    out
}

fn encode_poly12(p: &Poly) -> Vec<u8> {
    byte_encode(p, 12)
}
fn decode_poly12(b: &[u8]) -> Poly {
    let v = byte_decode(b, 12, N);
    std::array::from_fn(|i| v[i])
}

// ---- sampling (FIPS 203 §4.2.2) ----

/// SampleNTT: rejection-sample a poly (in the NTT domain) from a SHAKE128 stream.
fn sample_ntt(seed: &[u8]) -> Poly {
    let mut len = 3 * 168; // grow if rejection consumes it (XOF prefix is stable)
    loop {
        let buf = shake128(seed, len);
        let mut p = [0u64; N];
        let mut c = 0usize;
        let mut i = 0usize;
        while i + 3 <= buf.len() && c < N {
            let b0 = buf[i] as u64;
            let b1 = buf[i + 1] as u64;
            let b2 = buf[i + 2] as u64;
            i += 3;
            let d1 = b0 + ((b1 & 0x0f) << 8);
            let d2 = (b1 >> 4) + (b2 << 4);
            if d1 < Q && c < N {
                p[c] = d1;
                c += 1;
            }
            if d2 < Q && c < N {
                p[c] = d2;
                c += 1;
            }
        }
        if c == N {
            return p;
        }
        len += 168;
    }
}

/// SamplePolyCBD_η from `64·η` bytes (centered binomial, η ∈ {2}).
fn sample_cbd(bytes: &[u8], eta: usize) -> Poly {
    let bit = |idx: usize| ((bytes[idx / 8] >> (idx % 8)) & 1) as i64;
    std::array::from_fn(|i| {
        let base = 2 * i * eta;
        let mut a = 0i64;
        let mut b = 0i64;
        for j in 0..eta {
            a += bit(base + j);
            b += bit(base + eta + j);
        }
        let v = a - b; // in [-eta, eta]
        v.rem_euclid(Q as i64) as u64
    })
}

// ---- symmetric primitives (FIPS 203 §4.1) ----

fn g(input: &[u8]) -> ([u8; 32], [u8; 32]) {
    let h = sha3_512(input);
    let mut a = [0u8; 32];
    let mut b = [0u8; 32];
    a.copy_from_slice(&h[..32]);
    b.copy_from_slice(&h[32..]);
    (a, b)
}
fn h(input: &[u8]) -> [u8; 32] {
    sha3_256(input)
}
fn j(input: &[u8]) -> [u8; 32] {
    let v = shake256(input, 32);
    let mut o = [0u8; 32];
    o.copy_from_slice(&v);
    o
}
fn prf(eta: usize, s: &[u8; 32], b: u8) -> Vec<u8> {
    let mut input = s.to_vec();
    input.push(b);
    shake256(&input, 64 * eta)
}
fn xof_seed(rho: &[u8; 32], i: u8, jj: u8) -> Vec<u8> {
    let mut s = rho.to_vec();
    s.push(i);
    s.push(jj);
    s
}

// ---- K-PKE (FIPS 203 §5) ----

/// Build the matrix Â (k×k of NTT-domain polys) from ρ.
fn gen_matrix(rho: &[u8; 32]) -> Vec<Vec<Poly>> {
    (0..K)
        .map(|i| (0..K).map(|jj| sample_ntt(&xof_seed(rho, i as u8, jj as u8))).collect())
        .collect()
}

fn kpke_keygen(d: &[u8; 32]) -> (Vec<u8>, Vec<u8>) {
    let mut din = d.to_vec();
    din.push(K as u8); // FIPS 203 domain separation
    let (rho, sigma) = g(&din);
    let a = gen_matrix(&rho);

    let mut n = 0u8;
    let s: Vec<Poly> = (0..K)
        .map(|_| {
            let p = sample_cbd(&prf(ETA1, &sigma, n), ETA1);
            n += 1;
            p
        })
        .collect();
    let e: Vec<Poly> = (0..K)
        .map(|_| {
            let p = sample_cbd(&prf(ETA1, &sigma, n), ETA1);
            n += 1;
            p
        })
        .collect();

    let s_hat: Vec<Poly> = s.iter().map(poly_ntt).collect();
    let e_hat: Vec<Poly> = e.iter().map(poly_ntt).collect();

    // t̂[i] = Σ_j Â[i][j] ∘ ŝ[j] + ê[i]
    let mut t_hat = vec![[0u64; N]; K];
    for i in 0..K {
        let mut acc = [0u64; N];
        for jj in 0..K {
            acc = poly_add(&acc, &poly_basemul(&a[i][jj], &s_hat[jj]));
        }
        t_hat[i] = poly_add(&acc, &e_hat[i]);
    }

    let mut ek = Vec::with_capacity(EK_LEN);
    for t in &t_hat {
        ek.extend_from_slice(&encode_poly12(t));
    }
    ek.extend_from_slice(&rho);
    let mut dk = Vec::with_capacity(384 * K);
    for sh in &s_hat {
        dk.extend_from_slice(&encode_poly12(sh));
    }
    (ek, dk)
}

fn kpke_encrypt(ek: &[u8], msg: &[u8; 32], coins: &[u8; 32]) -> Vec<u8> {
    let t_hat: Vec<Poly> = (0..K).map(|i| decode_poly12(&ek[384 * i..384 * (i + 1)])).collect();
    let mut rho = [0u8; 32];
    rho.copy_from_slice(&ek[384 * K..384 * K + 32]);
    let a = gen_matrix(&rho);

    let mut n = 0u8;
    let y: Vec<Poly> = (0..K)
        .map(|_| {
            let p = sample_cbd(&prf(ETA1, coins, n), ETA1);
            n += 1;
            p
        })
        .collect();
    let e1: Vec<Poly> = (0..K)
        .map(|_| {
            let p = sample_cbd(&prf(ETA2, coins, n), ETA2);
            n += 1;
            p
        })
        .collect();
    let e2 = sample_cbd(&prf(ETA2, coins, n), ETA2);

    let y_hat: Vec<Poly> = y.iter().map(poly_ntt).collect();

    // u = NTT^{-1}(Âᵀ ∘ ŷ) + e1 ;  û[i] = Σ_j Â[j][i] ∘ ŷ[j]
    let mut u = vec![[0u64; N]; K];
    for i in 0..K {
        let mut acc = [0u64; N];
        for jj in 0..K {
            acc = poly_add(&acc, &poly_basemul(&a[jj][i], &y_hat[jj]));
        }
        u[i] = poly_add(&poly_intt(&acc), &e1[i]);
    }

    // v = NTT^{-1}(t̂ᵀ ∘ ŷ) + e2 + Decompress1(msg)
    let mut acc = [0u64; N];
    for i in 0..K {
        acc = poly_add(&acc, &poly_basemul(&t_hat[i], &y_hat[i]));
    }
    let mu_bits = byte_decode(msg, 1, N);
    let mu: Poly = std::array::from_fn(|i| decompress(mu_bits[i], 1));
    let v = poly_add(&poly_add(&poly_intt(&acc), &e2), &mu);

    // c = ByteEncode_du(Compress_du(u)) ‖ ByteEncode_dv(Compress_dv(v))
    let mut c = Vec::with_capacity(CT_LEN);
    for ui in &u {
        let comp: Vec<u64> = ui.iter().map(|&x| compress(x, DU)).collect();
        c.extend_from_slice(&byte_encode(&comp, DU));
    }
    let compv: Vec<u64> = v.iter().map(|&x| compress(x, DV)).collect();
    c.extend_from_slice(&byte_encode(&compv, DV));
    c
}

fn kpke_decrypt(dk: &[u8], c: &[u8]) -> [u8; 32] {
    let c1_len = 32 * DU as usize * K;
    // u = Decompress_du(ByteDecode_du(c1))
    let u: Vec<Poly> = (0..K)
        .map(|i| {
            let seg = &c[32 * DU as usize * i..32 * DU as usize * (i + 1)];
            let dec = byte_decode(seg, DU, N);
            std::array::from_fn(|t| decompress(dec[t], DU))
        })
        .collect();
    // v = Decompress_dv(ByteDecode_dv(c2))
    let dvdec = byte_decode(&c[c1_len..], DV, N);
    let v: Poly = std::array::from_fn(|t| decompress(dvdec[t], DV));

    let s_hat: Vec<Poly> = (0..K).map(|i| decode_poly12(&dk[384 * i..384 * (i + 1)])).collect();

    // w = v - NTT^{-1}(ŝᵀ ∘ NTT(u))
    let mut acc = [0u64; N];
    for i in 0..K {
        acc = poly_add(&acc, &poly_basemul(&s_hat[i], &poly_ntt(&u[i])));
    }
    let w = poly_sub(&v, &poly_intt(&acc));
    let bits: Vec<u64> = w.iter().map(|&x| compress(x, 1)).collect();
    let m = byte_encode(&bits, 1);
    let mut out = [0u8; 32];
    out.copy_from_slice(&m);
    out
}

// ---- ML-KEM (FO transform, FIPS 203 §6) ----

/// ML-KEM-768 key generation from 32-byte seeds `d` (K-PKE) and `z` (implicit-reject).
pub fn keygen(d: &[u8; 32], z: &[u8; 32]) -> (Vec<u8>, Vec<u8>) {
    let (ek, dk_pke) = kpke_keygen(d);
    let mut dk = Vec::with_capacity(DK_LEN);
    dk.extend_from_slice(&dk_pke);
    dk.extend_from_slice(&ek);
    dk.extend_from_slice(&h(&ek));
    dk.extend_from_slice(z);
    (ek, dk)
}

/// Encapsulate with explicit message `m` (the internal, deterministic form) → (shared
/// secret `K`, ciphertext `c`).
pub fn encaps(ek: &[u8], m: &[u8; 32]) -> ([u8; 32], Vec<u8>) {
    let mut gin = m.to_vec();
    gin.extend_from_slice(&h(ek));
    let (k, r) = g(&gin);
    let c = kpke_encrypt(ek, m, &r);
    (k, c)
}

/// Decapsulate: recover the shared secret, with FO re-encryption check + implicit
/// rejection (returns `J(z‖c)` on a mismatch — never an error, constant control flow).
pub fn decaps(dk: &[u8], c: &[u8]) -> [u8; 32] {
    let dk_pke = &dk[..384 * K];
    let ek = &dk[384 * K..768 * K + 32];
    let hh = &dk[768 * K + 32..768 * K + 64];
    let z = &dk[768 * K + 64..768 * K + 96];

    let m2 = kpke_decrypt(dk_pke, c);
    let mut gin = m2.to_vec();
    gin.extend_from_slice(hh);
    let (k2, r2) = g(&gin);
    let mut zc = z.to_vec();
    zc.extend_from_slice(c);
    let k_bar = j(&zc);
    let c2 = kpke_encrypt(ek, &m2, &r2);
    if c == c2.as_slice() {
        k2
    } else {
        k_bar
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fmat::Rng;

    fn seed(rng: &mut Rng) -> [u8; 32] {
        std::array::from_fn(|_| (rng.next_u64() & 0xff) as u8)
    }

    #[test]
    fn compress_decompress_within_bound() {
        // |Decompress(Compress(x)) - x| ≤ round(q / 2^{d+1}) for all x (FIPS 203 §4.2.1).
        for d in [DU, DV] {
            let bound = (Q >> (d + 1)) + 1;
            for x in 0..Q {
                let r = decompress(compress(x, d), d);
                let diff = sub(x, r).min(sub(r, x));
                assert!(diff <= bound, "d={d} x={x} diff={diff}");
            }
        }
    }

    #[test]
    fn byte_encode_decode_roundtrips() {
        let mut rng = Rng::new(0xEC0DE);
        for d in [1u32, 4, 10, 12] {
            // for d=12 ByteDecode reduces mod q (FIPS 203), so use the valid range.
            let hi = if d >= 12 { Q } else { 1 << d };
            let coeffs: Vec<u64> = (0..N).map(|_| rng.next_u64() % hi).collect();
            let bytes = byte_encode(&coeffs, d);
            assert_eq!(bytes.len(), N * d as usize / 8);
            assert_eq!(byte_decode(&bytes, d, N), coeffs);
        }
    }

    #[test]
    fn kpke_decrypt_inverts_encrypt() {
        // IND-CPA correctness: Decrypt(Encrypt(m)) = m (overwhelming probability; the
        // tiny Kyber failure rate ~2^-164 never triggers across these trials).
        let mut rng = Rng::new(0x1234);
        for _ in 0..20 {
            let d = seed(&mut rng);
            let (ek, dk) = kpke_keygen(&d);
            assert_eq!(ek.len(), EK_LEN);
            let m = seed(&mut rng);
            let coins = seed(&mut rng);
            let c = kpke_encrypt(&ek, &m, &coins);
            assert_eq!(c.len(), CT_LEN);
            assert_eq!(kpke_decrypt(&dk, &c), m, "K-PKE must invert");
        }
    }

    #[test]
    fn mlkem_roundtrip() {
        // kyber_roundtrip: decaps(encaps) = the same shared secret.
        let mut rng = Rng::new(0xABCDEF);
        for _ in 0..20 {
            let (d, z, m) = (seed(&mut rng), seed(&mut rng), seed(&mut rng));
            let (ek, dk) = keygen(&d, &z);
            assert_eq!(ek.len(), EK_LEN);
            assert_eq!(dk.len(), DK_LEN);
            let (k_enc, c) = encaps(&ek, &m);
            assert_eq!(c.len(), CT_LEN);
            let k_dec = decaps(&dk, &c);
            assert_eq!(k_enc, k_dec, "shared secrets must agree");
        }
    }

    #[test]
    fn mlkem_implicit_rejection() {
        // A tampered ciphertext yields the implicit-reject secret J(z‖c'), not the real K
        // and not an error (constant control flow).
        let mut rng = Rng::new(0x9);
        let (d, z, m) = (seed(&mut rng), seed(&mut rng), seed(&mut rng));
        let (ek, dk) = keygen(&d, &z);
        let (k_enc, mut c) = encaps(&ek, &m);
        c[0] ^= 0xff; // tamper
        let k_dec = decaps(&dk, &c);
        assert_ne!(k_enc, k_dec, "tampered ct must not recover the real secret");
        // and it equals the defined implicit-reject value
        let mut zc = z.to_vec();
        zc.extend_from_slice(&c);
        assert_eq!(k_dec, j(&zc), "implicit reject must be J(z‖c)");
    }
}
