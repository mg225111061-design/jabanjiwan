//! ML-DSA-44 (Dilithium) — keygen / sign / verify (Stage 14.2).
//!
//! Built on the exact Dilithium NTT and rounding primitives ([`crate::dilithium`]) and the
//! KAT-anchored SHAKE layer ([`crate::keccak`]). The Fiat–Shamir-with-aborts signature is
//! implemented at the algorithm level over `Z_q[x]/(x^256+1)`, `q = 8380417`. Module-SIS/LWE
//! hardness is an assumption JEFF never touches (see `crate::pqc`).
//!
//! # Conformance — VERIFIED against official NIST ACVP (FIPS 204)
//!
//! ML-DSA-44 is validated **byte-for-byte** against the official NIST ACVP vectors
//! (usnistgov/ACVP-Server): **keyGen 25/25**, **sigGen 90/90** (external/pure, internal, and
//! external-μ interfaces; deterministic and hedged), and **sigVer 45/45** (accepts valid,
//! rejects every tampered/forged signature, including the negative tests). Representative
//! vectors are embedded in `mod acvp_kat` for permanent, network-free regression.
//!
//! Getting there required the FIPS 204 byte layer and one sampling fix: ExpandA uses the XOF
//! **SHAKE128** (not SHAKE256), `ρ'' = H(K ‖ rnd ‖ μ)` carries the randomness byte string, the
//! challenge hash consumes `w1Encode` at 6 bits/coeff (`SimpleBitPack`), and pk/sk/sig use the
//! FIPS `BitPack`/`HintBitPack` encodings (see [`encode_pk`], [`encode_sk`], [`encode_sig`] and
//! their decoders). The NTT and rounding primitives ([`crate::dilithium`]) were already
//! identity-checked, and the SHAKE layer is NIST-KAT validated.
//!
//! Not wired: the `preHash` (HashML-DSA) message-formatting variant — that is purely a
//! different `M'` prefix (`0x01 ‖ len(ctx) ‖ ctx ‖ OID ‖ H(M)`); the cryptographic core is the
//! same one proven above. Module-SIS/LWE hardness is an assumption JEFF never touches.

use crate::dilithium::{self, make_hint, use_hint, Q, N, D};
use crate::keccak::{shake128, shake256};
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

/// Rejection-sample a poly in the NTT domain (uniform coeffs in [0,q)).
///
/// FIPS 204 Alg. 30 (RejNTTPoly) uses the XOF **SHAKE128** (`H128`) — not SHAKE256 — to
/// produce the byte stream for `Â`. Getting this XOF right is load-bearing: a wrong XOF
/// makes every entry of `Â` wrong and the public key non-conformant.
fn sample_ntt_dilithium(seed: &[u8]) -> Poly {
    let mut len = 3 * 168;
    loop {
        let buf = shake128(seed, len);
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

/// SampleInBall (FIPS 204 Alg. 29): a poly with exactly τ coefficients in {−1,+1}, rest 0.
/// The first 8 squeezed bytes give the signs; the rest drive rejection sampling of positions.
/// We squeeze incrementally (doubling) so a long rejection run is still handled exactly —
/// the XOF prefix property makes a larger squeeze a superset, never a different stream.
fn sample_in_ball(c_tilde: &[u8; 32]) -> Poly {
    let mut extra = 256usize;
    loop {
        let buf = shake256(c_tilde, 8 + extra);
        let mut signs = u64::from_le_bytes(buf[0..8].try_into().unwrap());
        let mut c = zero();
        let mut pos = 8usize;
        let mut underrun = false;
        for i in (N - TAU)..N {
            let mut jj = 0usize;
            let mut found = false;
            while pos < buf.len() {
                let candidate = buf[pos] as usize;
                pos += 1;
                if candidate <= i {
                    jj = candidate;
                    found = true;
                    break;
                }
            }
            if !found {
                underrun = true;
                break;
            }
            c[i] = c[jj];
            c[jj] = if signs & 1 == 1 { Q - 1 } else { 1 };
            signs >>= 1;
        }
        if !underrun {
            return c;
        }
        extra *= 2;
    }
}

// ---- hashing helpers ----

fn shake256_n(parts: &[&[u8]], n: usize) -> Vec<u8> {
    let mut input = Vec::new();
    for p in parts {
        input.extend_from_slice(p);
    }
    shake256(&input, n)
}

/// FIPS 204 `w1Encode` = `SimpleBitPack(w1, (q−1)/(2γ2)−1)`. For ML-DSA-44, w1 ∈ [0,43] →
/// 6 bits per coeff, 192 bytes per poly. This feeds the challenge hash `c̃ = H(μ ‖ w1Encode)`,
/// so the bit width is load-bearing for KAT conformance (8 bits would change c̃).
fn encode_w1(w1: &[Vec<i64>]) -> Vec<u8> {
    let mut out = Vec::with_capacity(K * 192);
    for poly in w1 {
        let coeffs: Vec<u64> = poly.iter().map(|&c| c as u64).collect();
        out.extend_from_slice(&bit_pack(&coeffs, 6, 192));
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

/// ML-DSA-44 key generation returning the **FIPS 204 byte encodings** `(pk, sk)`
/// (pkEncode / skEncode). This is the form NIST ACVP KAT vectors compare against.
pub fn keygen_encoded(xi: &[u8; 32]) -> (Vec<u8>, Vec<u8>) {
    let (pk, sk) = keygen(xi);
    (encode_pk(&pk), encode_sk(&sk))
}

// ---- FIPS 204 byte encodings (BitPack / SimpleBitPack, §7.1–7.2) ----

/// Pack `coeffs` (each already in `[0, 2^bits)`) into `nbytes` bytes, LSB-first.
/// This is FIPS 204 `SimpleBitPack`/`BitPack` core: coefficient 0 occupies the lowest
/// `bits` bits, coefficient 1 the next `bits`, etc. (little-endian over the bit stream).
fn bit_pack(coeffs: &[u64], bits: usize, nbytes: usize) -> Vec<u8> {
    let mut out = vec![0u8; nbytes];
    let mut pos = 0usize;
    for &c in coeffs {
        for b in 0..bits {
            if (c >> b) & 1 == 1 {
                out[pos / 8] |= 1 << (pos % 8);
            }
            pos += 1;
        }
    }
    out
}

/// `BitPack(s, η, η)` for η = 2: coefficient `c` (stored mod q) is encoded as `(η − c) mod q`
/// in 3 bits, 96 bytes per poly (FIPS 204 skEncode for s1/s2).
fn pack_eta(poly: &Poly) -> Vec<u8> {
    let altered: Vec<u64> = poly.iter().map(|&c| (mi(ETA as u64) - mi(c)).val).collect();
    bit_pack(&altered, 3, 96)
}

/// `BitPack(t0, 2^{d−1}−1, 2^{d−1})`: centered coeff `c0` encoded as `2^{d−1} − c0` in 13 bits,
/// 416 bytes per poly (FIPS 204 skEncode for t0).
fn pack_t0(poly: &Poly) -> Vec<u8> {
    let altered: Vec<u64> = poly
        .iter()
        .map(|&c| ((1i64 << (D - 1)) - dilithium::center(c)) as u64)
        .collect();
    bit_pack(&altered, 13, 416)
}

/// FIPS 204 `pkEncode`: `ρ ‖ SimpleBitPack(t1, 2^{bitlen(q−1)−d}−1)` = ρ ‖ 10-bit pack of t1,
/// 320 bytes per poly (total pk = 32 + k·320 = 1312 for ML-DSA-44).
fn encode_pk(pk: &PublicKey) -> Vec<u8> {
    let mut out = pk.rho.to_vec();
    for poly in &pk.t1 {
        out.extend_from_slice(&bit_pack(poly, 10, 320));
    }
    out
}

/// FIPS 204 `skEncode`: `ρ ‖ K ‖ tr ‖ BitPack(s1) ‖ BitPack(s2) ‖ BitPack(t0)`
/// (total sk = 32 + 32 + 64 + l·96 + k·96 + k·416 = 2560 for ML-DSA-44).
fn encode_sk(sk: &SecretKey) -> Vec<u8> {
    let mut out = sk.rho.to_vec();
    out.extend_from_slice(&sk.k_key);
    out.extend_from_slice(&sk.tr);
    for poly in &sk.s1 {
        out.extend_from_slice(&pack_eta(poly));
    }
    for poly in &sk.s2 {
        out.extend_from_slice(&pack_eta(poly));
    }
    for poly in &sk.t0 {
        out.extend_from_slice(&pack_t0(poly));
    }
    out
}

/// Inverse of [`bit_pack`]: read `n` coefficients of `bits` bits each, LSB-first.
fn bit_unpack(bytes: &[u8], bits: usize, n: usize) -> Vec<u64> {
    let mut coeffs = vec![0u64; n];
    let mut pos = 0usize;
    for c in coeffs.iter_mut() {
        let mut v = 0u64;
        for b in 0..bits {
            if (bytes[pos / 8] >> (pos % 8)) & 1 == 1 {
                v |= 1 << b;
            }
            pos += 1;
        }
        *c = v;
    }
    coeffs
}

/// FIPS 204 `skDecode`: parse a private key byte string back into its components. Inverse of
/// [`encode_sk`]; used to sign from an externally supplied (e.g. ACVP) secret key.
fn decode_sk(sk: &[u8]) -> SecretKey {
    let mut rho = [0u8; 32];
    rho.copy_from_slice(&sk[0..32]);
    let mut k_key = [0u8; 32];
    k_key.copy_from_slice(&sk[32..64]);
    let mut tr = [0u8; 64];
    tr.copy_from_slice(&sk[64..128]);
    let mut off = 128usize;
    let take_eta = |bytes: &[u8]| -> Poly {
        let raw = bit_unpack(bytes, 3, N);
        std::array::from_fn(|i| (mi(ETA as u64) - mi(raw[i])).val) // c = (η − altered) mod q
    };
    let mut s1 = Vec::with_capacity(L);
    for _ in 0..L {
        s1.push(take_eta(&sk[off..off + 96]));
        off += 96;
    }
    let mut s2 = Vec::with_capacity(K);
    for _ in 0..K {
        s2.push(take_eta(&sk[off..off + 96]));
        off += 96;
    }
    let mut t0 = Vec::with_capacity(K);
    for _ in 0..K {
        let raw = bit_unpack(&sk[off..off + 416], 13, N);
        off += 416;
        let p: Poly = std::array::from_fn(|i| {
            let centered = (1i64 << (D - 1)) - raw[i] as i64; // c0 = 2^{d−1} − altered
            centered.rem_euclid(Q as i64) as u64
        });
        t0.push(p);
    }
    SecretKey { rho, k_key, tr, s1, s2, t0 }
}

/// FIPS 204 `HintBitPack` (Alg. 20): pack the hint into `ω + k` bytes — the sorted nonzero
/// positions per polynomial, then `k` running offsets.
fn pack_hint(h: &[[bool; N]]) -> Vec<u8> {
    let mut packed: Vec<u8> = Vec::with_capacity(OMEGA);
    let mut offsets: Vec<u8> = Vec::with_capacity(h.len());
    for poly in h {
        for (i, &b) in poly.iter().enumerate() {
            if b {
                packed.push(i as u8);
            }
        }
        offsets.push(packed.len() as u8);
    }
    while packed.len() < OMEGA {
        packed.push(0);
    }
    packed.extend_from_slice(&offsets);
    packed
}

/// FIPS 204 `sigEncode`: `c̃ ‖ BitPack(z, γ1) ‖ HintBitPack(h)`. z coeff `c` is encoded as
/// `(γ1 − c) mod q` in 18 bits (576 bytes/poly) for ML-DSA-44.
fn encode_sig(c_tilde: &[u8; 32], z: &[Poly], h: &[[bool; N]]) -> Vec<u8> {
    let mut out = c_tilde.to_vec();
    for poly in z {
        let altered: Vec<u64> = poly.iter().map(|&c| (mi(GAMMA1 as u64) - mi(c)).val).collect();
        out.extend_from_slice(&bit_pack(&altered, 18, 576));
    }
    out.extend_from_slice(&pack_hint(h));
    out
}

/// ML-DSA-44 signing from FIPS byte encodings: decode `sk`, sign the formatted message `m_prime`
/// with randomness `rnd`, and return the FIPS `sigEncode` bytes. This is the form NIST ACVP
/// sigGen vectors compare against (`rnd = 0³²` for the deterministic groups).
pub fn sign_encoded(sk_bytes: &[u8], m_prime: &[u8], rnd: &[u8; 32]) -> Vec<u8> {
    let sk = decode_sk(sk_bytes);
    let sig = sign_internal(&sk, m_prime, rnd);
    encode_sig(&sig.c_tilde, &sig.z, &sig.h)
}

/// As [`sign_encoded`] but with a precomputed `μ` (external-μ interface).
pub fn sign_encoded_mu(sk_bytes: &[u8], mu: &[u8], rnd: &[u8; 32]) -> Vec<u8> {
    let sk = decode_sk(sk_bytes);
    let sig = sign_with_mu(&sk, mu, rnd);
    encode_sig(&sig.c_tilde, &sig.z, &sig.h)
}

/// Deterministic ML-DSA signing (struct API): equivalent to `sign_internal` with the message
/// taken verbatim as `M'` and `rnd = 0³²`.
pub fn sign(sk: &SecretKey, msg: &[u8]) -> Signature {
    sign_internal(sk, msg, &[0u8; 32])
}

/// FIPS 204 `ML-DSA.Sign_internal` (Alg. 7): sign the already-formatted message `m_prime`
/// with per-signature randomness `rnd` (all-zero for deterministic signing). `μ = H(tr ‖ M')`
/// and `ρ'' = H(K ‖ rnd ‖ μ)` — the `rnd` byte string is part of `ρ''` and must not be omitted.
pub fn sign_internal(sk: &SecretKey, m_prime: &[u8], rnd: &[u8; 32]) -> Signature {
    let mu = shake256_n(&[&sk.tr, m_prime], 64);
    sign_with_mu(sk, &mu, rnd)
}

/// Sign with a precomputed message representative `μ` (the "external-μ" interface, FIPS 204
/// §6.2): identical to [`sign_internal`] except `μ` is supplied directly rather than derived
/// from `tr ‖ M'`. `ρ'' = H(K ‖ rnd ‖ μ)`.
pub fn sign_with_mu(sk: &SecretKey, mu: &[u8], rnd: &[u8; 32]) -> Signature {
    let a_hat = expand_a(&sk.rho);
    let rho2 = {
        let v = shake256_n(&[&sk.k_key, rnd, mu], 64);
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
        c_tilde.copy_from_slice(&shake256_n(&[mu, &encode_w1(&w1)], 32));
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

/// ML-DSA verification (struct API): `μ = H(tr ‖ M')` with `M' = msg`.
pub fn verify(pk: &PublicKey, msg: &[u8], sig: &Signature) -> bool {
    let tr = shake256(&encode_pk(pk), 64);
    let mu = shake256_n(&[&tr, msg], 64);
    verify_with_mu(pk, &mu, &sig.c_tilde, &sig.z, &sig.h)
}

/// Verification core given a precomputed `μ` (FIPS 204 `ML-DSA.Verify_internal`, Alg. 8):
/// recompute `w₁' = UseHint(h, A·z − c·t1·2^d)` and accept iff `H(μ ‖ w1Encode(w₁')) = c̃`,
/// after the `‖z‖∞ < γ1−β` and hint-weight bounds.
pub fn verify_with_mu(pk: &PublicKey, mu: &[u8], c_tilde: &[u8; 32], z: &[Poly], h: &[[bool; N]]) -> bool {
    let gamma1_minus_beta = GAMMA1 - BETA;
    if z.iter().map(norm_inf).max().unwrap_or(0) >= gamma1_minus_beta {
        return false;
    }
    let weight: usize = h.iter().flat_map(|r| r.iter()).filter(|&&b| b).count();
    if weight > OMEGA {
        return false;
    }

    let a_hat = expand_a(&pk.rho);
    let c = sample_in_ball(c_tilde);
    let c_hat = pntt(&c);

    // w' = A·z − c·t1·2^D
    let z_hat: Vec<Poly> = z.iter().map(pntt).collect();
    let az = matrix_mul_ntt(&a_hat, &z_hat);
    let two_d = 1u64 << D;
    let mut w1p: Vec<Vec<i64>> = Vec::with_capacity(K);
    for (i, az_i) in az.iter().enumerate() {
        let t1_scaled: Poly = std::array::from_fn(|c2| (mi(pk.t1[i][c2]) * mi(two_d)).val);
        let ct1 = pinvntt(&ppoint(&c_hat, &pntt(&t1_scaled)));
        let wp = psub(&pinvntt(az_i), &ct1);
        let row: Vec<i64> = (0..N).map(|c2| use_hint(h[i][c2], wp[c2], GAMMA2)).collect();
        w1p.push(row);
    }

    let mut c_check = [0u8; 32];
    c_check.copy_from_slice(&shake256_n(&[mu, &encode_w1(&w1p)], 32));
    &c_check == c_tilde
}

// ---- FIPS decoders for verification ----

/// FIPS 204 `pkEncode` inverse: `ρ ‖ SimpleBitPack(t1, 10)` → `(ρ, t1)`.
fn decode_pk(pk: &[u8]) -> PublicKey {
    let mut rho = [0u8; 32];
    rho.copy_from_slice(&pk[0..32]);
    let mut t1 = Vec::with_capacity(K);
    let mut off = 32usize;
    for _ in 0..K {
        let raw = bit_unpack(&pk[off..off + 320], 10, N);
        off += 320;
        t1.push(std::array::from_fn(|i| raw[i])); // t1 coeffs already in [0, 1023]
    }
    PublicKey { rho, t1 }
}

/// FIPS 204 `HintBitUnpack` (Alg. 21): returns `⊥` (None) on a malformed hint — offsets must
/// be non-decreasing and ≤ ω, positions strictly increasing within a polynomial, and all
/// padding bytes zero. These checks are what make tampered/forged signatures verify-false.
fn unpack_hint(bytes: &[u8]) -> Option<Vec<[bool; N]>> {
    let mut h = vec![[false; N]; K];
    let mut index = 0usize;
    for i in 0..K {
        let end = bytes[OMEGA + i] as usize;
        if end < index || end > OMEGA {
            return None;
        }
        let first = index;
        while index < end {
            if index > first && bytes[index - 1] >= bytes[index] {
                return None;
            }
            h[i][bytes[index] as usize] = true;
            index += 1;
        }
    }
    for &b in &bytes[index..OMEGA] {
        if b != 0 {
            return None;
        }
    }
    Some(h)
}

/// FIPS 204 `sigDecode`: `c̃ ‖ BitPack(z) ‖ HintBitPack(h)` → `(c̃, z, h)`, or `None` if the
/// length is wrong or the hint is malformed.
#[allow(clippy::type_complexity)]
fn decode_sig(sig: &[u8]) -> Option<([u8; 32], Vec<Poly>, Vec<[bool; N]>)> {
    if sig.len() != 32 + L * 576 + OMEGA + K {
        return None;
    }
    let mut c_tilde = [0u8; 32];
    c_tilde.copy_from_slice(&sig[0..32]);
    let mut off = 32usize;
    let mut z = Vec::with_capacity(L);
    for _ in 0..L {
        let raw = bit_unpack(&sig[off..off + 576], 18, N);
        off += 576;
        z.push(std::array::from_fn(|i| (mi(GAMMA1 as u64) - mi(raw[i])).val)); // z = γ1 − altered
    }
    let h = unpack_hint(&sig[off..off + OMEGA + K])?;
    Some((c_tilde, z, h))
}

/// ML-DSA-44 verification from FIPS byte encodings: `μ = H(tr ‖ M')`. The form NIST ACVP
/// sigVer vectors test (it must accept valid signatures and reject every tampered one).
pub fn verify_encoded(pk_bytes: &[u8], m_prime: &[u8], sig_bytes: &[u8]) -> bool {
    let pk = decode_pk(pk_bytes);
    let Some((c_tilde, z, h)) = decode_sig(sig_bytes) else {
        return false;
    };
    let tr = shake256(&encode_pk(&pk), 64);
    let mu = shake256_n(&[&tr, m_prime], 64);
    verify_with_mu(&pk, &mu, &c_tilde, &z, &h)
}

/// As [`verify_encoded`] but with a precomputed `μ` (external-μ interface).
pub fn verify_encoded_mu(pk_bytes: &[u8], mu: &[u8], sig_bytes: &[u8]) -> bool {
    let pk = decode_pk(pk_bytes);
    let Some((c_tilde, z, h)) = decode_sig(sig_bytes) else {
        return false;
    };
    verify_with_mu(&pk, mu, &c_tilde, &z, &h)
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

#[cfg(test)]
mod acvp_kat {
    //! Official NIST ACVP ML-DSA-44 (FIPS 204) known-answer vectors, embedded for permanent
    //! byte-for-byte regression (network-free). Source: usnistgov/ACVP-Server
    //! (ML-DSA-keyGen / ML-DSA-sigGen / ML-DSA-sigVer FIPS204).
    //! Broader sweeps (keyGen 25/25, sigGen 90/90, sigVer 45/45) are run out-of-tree against
    //! the full vector set; these anchors lock conformance into the in-tree suite.
    // keyGen tcId 1; sigGen tcId 1; sigVer tcId 6(pass)/1(fail)
    use super::*;
    fn unhex(s: &str) -> Vec<u8> {
        (0..s.len() / 2).map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap()).collect()
    }
    fn a32(s: &str) -> [u8; 32] { let mut a = [0u8; 32]; a.copy_from_slice(&unhex(s)); a }

    #[test]
    fn keygen_matches_official_acvp() {
        let (pk, sk) = keygen_encoded(&a32("D71361C000F9A7BC99DFB425BCB6BB27C32C36AB444FF3708B2D93B4E66D5B5B"));
        assert_eq!(pk, unhex("B845FA2881407A59183071629B08223128116014FB58FF6BB4C8C9FE19CF5B0BD77B16648A344FFE486BC3E3CB5FAB9ABC4CC2F1C34901692BEC5D290D815A6CDF7E9710A3388247A7E0371615507A572C9835E6737BF30B92A796FFF3A10A730C7B550924EB1FB6D56195F02DE6D3746F9F330BEBE990C90C4D676AD415F4268D2D6B548A8BCDF27FDD467E6749C0F87B71E85C2797694772BBA88D4F1AC06C7C0E91786472CD76353708D6BBC5C28E9DB891C3940E879052D30C8FD10965CBB8EE1BD79B060D37FB839098552AABDD3A57AB1C6A82B0911D1CF148654AA5613B07014B21E4A1182B4A5501671D112F5975FB0C8A2AC45D575DC42F48977FF37FFF421DB27C45E79F8A9472007023DF0B64205CD9F57C02CE9D1F61F2AE24F7139F5641984EE8DF783B9EA43E997C6E19D09E062AFCA56E4F76AAAB8F66600FC78F6AB4F6785690D185816EE35A939458B60324EEFC60E64B11FA0D20317ACB6CB29AA03C775F151672952689FA4F8F838329CB9E6DC9945B6C7ADE4E7B663578F87D3935F2A1522097AD5042A0D990A628510B6103CB242CD8A3AFC1A5ADA52331F4DF461BC1DA51D1D224094E7ABED3D87D98F0D817084780EE80370F397631ECB75D4264B6B5E2E66C0586B5FB743516399165837A0FDFF7C6134F033BFA69C1B2416965C6E578592F40E258CB6DFB29FB8E0F54355B6E24A65F67ABAE3193D007115CC0B9FF94CB911A93B1A76C0E7662F5E2B20139E0159ED929CB932D4895F89A02E55C59DF2DBB8F6E5DD7D5B1F3CEC37B4A9166B381C5440E23E67368CDE0A29D59AA05A3C9BE24A4DC8DD75BE30E82BC635D36AAC66DE880C6701A987D7E05F0F2FF287828BEC30595089D8AB9AA390ED719CAA6E576CDBBE9B184A322E5E2DABB69C23CC696D54FC32FF57001B6B64E2A837F3062D85AEB50B3510F7EDFC34DF38E083D4D9B94FFAB0DE15D73D9AF30B9F31CC4F41C9C24F2D618B2A7C3C4BDFB745D52D3EB54589C8BDA8AC05DAD14EC744505575A0988EEC651C1715439FDFB29923380A43C1A66A86C982A841F11820A6A0E1E2F2FFF5108ECAE51A6AABC9B949226D228FF84C4E5E5D63114D80359C4931E612DCED1838B7D066AC9182CECFA223A21A4C8E155AEFA780373BCC15098AEE40C033AF22F8E7C67A0D2526DA7475E830308C04AED9D32BCCC72E719EE70A8D13F09AC11E26EA237D5CC8F98B5AE0E54F933BD0507942ED900D056FD32F8E6E81777912FD482746029B71CCE3BA69B8FC2D03EB441027C387BC2F95031A0AE7052215EB24B9EA8FB0A961B0F80BFA80D0D6257C1C22B508C5D31B97FCDFE1D1766E8A9C8771932DD598ADB7E717743F45FC571F21E4A516249F81D747F15329790F0F70A0B8E461A4EDF50504AF03F30DDF8A8818E38761E1681D6DDEF0B1DD326B2EC228CE48570F285B49D29D7C2EF37866D5446DF82B8E43B34CB248962A21A9A3946159740F8AEE8E6A16A4EB2B42D143FE2612E05EF4B5E646D813248444556A2A8BF92CE10BADECB6B8A40B080DD42D53346FEFCC4B9B40B1E4998991EC753C95AA2F2A506F311E710B0F1D36C1DCA6644EE6D1D4AE9CEA5666EF4B3E888DBDBB95A77ECFE1E8B477DE7CB07639D682D53020EC14EA6C7DD7E715389D10938429FAB8A068A1466A4CD891359F8074E0F5A142ADD731B87878D985E4FA6ECB3B73D298553418273E9503AA84092C080E5F2902F90F5C59944D24CA0271D11D0D6734606D039550A37FCA2B735850E63F540F2F06B79144B5C4ED2C700BB51C33D265B3D037389C99EFD597642D829DB1EB58643CFCD07F4DEC60B8F727D97BD7C4B59BDA1"), "pk must match official ACVP byte-for-byte");
        assert_eq!(sk, unhex("B845FA2881407A59183071629B08223128116014FB58FF6BB4C8C9FE19CF5B0B28965F58D99EE0DE7BFB7840F59F65414289E259E05E8A18D47EC06D7900284AD7A0AE404B93B0498945ABD07638920FC7F8C21282031EECEB0CC48D71FB0AD1CE84B9B3DDD1B3A2A40A15D5BFE0BE23FC4757137C6C8FEB7F8055677BA4AF2814242A9AA0108A4046981800042130A2340A99140E01A931E1300A1A136959486601C111140451244206E4C848A1B80008B2490121901224500B03306112025C144912B30049424D14470E93342023898803238CA1C081CC0411184545C1C20862465210300D52B870A3B004DCA20D4A386511214A01B48C43104809461253068161348618A00920878CA0168682208E41188E52A29021014652184521A42902132ED0B8248816710AB245818480942670DC126C1BC16C4016246236008996108B124E1B3721639225E1C88CA346285A32111C856110B9295990910493651CC40063B810C1184A52900C1CB20D2380090A4021D4188043A64D0835060C98650CB4890C8661D804601C0789D09231D8081100881119294E48B661D8962DD2B04121A1051226860349301A246D99483001200C5CC2840128900A298850340E1804640AB409D1968413196DD91031133689808025E300294A06304448109B429008858902A20D13461110120EDC380C4C468842C004131490C1462248B82D48C80D64346C02348109136842A4211B26511C302A6328688B18609C922C22C9000031058AC08C894430D0142189424C4C106400B64DD2A8246306308BB4289B246EC34262A0C00C0C2050E112444A360AD122320C36014836804C00400C16090C9569A19829110602CC982D08C811498445D20290014942C1062E42C000A2902D9810711A054008058608B2651B36441109402107100B3306D298858148088034305C3844DC266ECA947004394A442841E1464898862D113621221509530826213088DA94804A80601425321295641B02120A92241A4422230624CC12818922600036608A32820CC368A1A04D5B286A110744589851C42831A4064EC0C831CAB46810B669132488840462A3C421E0920523303050B6105942285C3005102286CC260E2226450003928A242C9C024E40B20400148C4A086E4BA6444CC84924464D4946821AA4690A316E21408C22964949044618B72C12B864D1C851C3366E5C2008E3328513198564C2019A4822111300A4B64960C06D1944628C086119B761021570EA732640984F68143D81AB95763698DCD02625A980994C19B7D47626409370F87E45D39B76B6762F7827B2779BA44046F54228A7B1BAC9D8E751AC2DD248521564F81D028AD671DB6A3A5BA3A9463597A519AE0C1DA435FEF7C421CDE7F8483782C4334993192923796DDC3E5803F6C86CF2B37E39BBCB53775E5E9D2BDC65E84395B01AF95C4398503A7E63A8D72F7E9BC58D7655CB86BAFA0B679D1D791F09703F6E4B333A51DFB14B36EAC9A3582F795AFCECC2A67A42C7BC43B447B73EC8B73C9164B2D9EF4F767D607E975738E1D871FE61B7BB9570516959B9E7D8155455FC91590D84BD430921568B718241CCD09C989DDF75F529C4F15818969216EC0FBABEC64F184832BFE428BE44849C42F13CA84A5CE6CA628044CB9DA3FD9CB5EBC776BA0E12683B98253B213B286593779EACC489C985BEB734A7506791BE1A6E6E03B557CFEF2069091B846CDBBB72D36A89FD5C45C7C7FBFAED956EC9D8F3025225D6FEF811B1CC3C6705F056388BBFBBA1FEBF4D072281DC792ADA048216D60D0D6044150909B70A4753336F4D5D3FDB8BACA62946DB28332EC778A76F96577BAF868BF896773C200637495098B0F7749835915348D37641EC74D5A78E565D8100627468B0B5D05C2937618A98E5A12476533CE9D699E0F4F23B3127EE1F8D3CDE3851A94BEB8D606D836D47A9F090581B660FF8EC31E3EF4E45DB530DD88A44D48F84B6BD3CED53BE935EFBCE9930DD83BD004F086A064B3260B7211BA0547CB29E56603B922597B31A30A5FFCBD8A7AD2F3F05372DCEA89048C683164410E56A6E5B7EC2EA6E3B0176DB1EBF9258C97C297E752A202B1B4B02DA47235873CC3E4A376B655779AFFFC9AEF2E30B62BE022AEFF5E34B468507C921A00F1110B2CC1BC99B2EE295A31EAC141AB4D87EDC11D7FEE22A07D69B7008C107EBE47DD2F215D074868C8B929A3DF6614B5E95CC8A52F96C57E1697CEE332F45B1980E6CA372574BDB51825654E6FF31D8E755CFC15B602F46A0570D3BD0D65E43BDE8F792E7D526B3D362FE33638B3870E1C29E1030D3DBA43C3D0DB6E5F501EA3441B709E54FCA1FD3E8654B1A9301AD151A36C8FA1513D0229DE6E94D02A97B9D6F2EE8078A5D1228E783B8FDAC46102A19600C531276B447F3900EF8F723FC3B60D7BBBBDA9F949E4460B2F1103315ACE25B3497CA4B4674A6052304609A16E90D064B0F6A9EAE8D8E4BE38E3B781F9173654E876CE6CC00578171FD9CFDBA5704AF2E60CF84FD02EBEF19AED140287D195023000568E8D908EDEADC76BCA3E7FE8FE1F4B201C2A15603C025A61917AE664B4294C57030AC988D7967364A8BB5857E98218C2943E8CF1CB8C719DACCFAE8F42AA7DDE9B4130170068A9CC86B0552D1DD0A48F63DDB8F8A51B3C0E10F3174A87D18F96533CF0C30C0BE33D37C25CD3EBF8F5F10E66464AC2FB8260BA8B5BD443ABC6AE0D5F89DAA42BC71B2A9BC2D10140B117E32F41353FA1ECEC9DBCCB621350806C4E4EBFC966AA7122D85532728EF26AA40E55E2E1739790FFA157EFA4E2A68DA1EA4C9E3273CDE11DB7DA47220123C40D50B73B8460CD93CB449783EAD8A58F6FF272275A4F6C0437DAF3DF1D8D9617743D9302E73B715AA7CAA139CDD07EDAD2DE7B32B3F11E3C2B6A96D4630632C6D1DE21473A287D42B52D210CD4EF808A998148817BB5083BDAB3A3354B084C5773970B15DA56F755E1F25354EFEFF89FDF4E307998D623CED32C7B9E942B6816A87A233867E2BEB84582FFF61AF612BE6E521AB3005E17D471A5C67A0A927211802CA4A0C2FEDC01A12AF5E8A6E8FF7BD08883AF1D35C2F8570091D9E5FCD9D720644F0A45042443D2FAC309FA7A15B2DBBA1517EA19407B17CAF7E9EA81FBCE42F35A0568B2F70B279E12DA48858F0E0FF0B5CB3A3012308AB83DB627F5439E3A424D83F9E0B7750F2CBD9CA4AFB9BA77765A1EA590CE9E37BC7EABE1EF099765CE59EA74C2776ECCBCEB895997F577733C609B9E853CF03010C5B9CC4E819C041ABF11A9B3866502BF6AC0354E6CE1060D5155E6E5A1AC4FADF73080B20569317F9E118B97C78D7273300E329373E5B64A4181C4AC7B8933F82ADE505D52CDDCEBA373A54D072BC04A8D83273586615ADDC4D6F258BFD75AE94EFD120F20B721A1CAC33FF3446E554D6CFE266B05E4CD44DE77D263A9D347F53B282A7BCB1511DD405A67AC05E7233F5EBB40D38F953AC903F90E96F17A6357EB59ED7900FFD82F54C3043D37F7774A0F4DEFAF65887EB53FE156FB26794982B1249BAD27C89C136097AA2268347FC9C5E5BE79E23C923215353A850E63748E766"), "sk must match official ACVP byte-for-byte");
    }

    #[test]
    fn siggen_matches_official_acvp() {
        let sig = sign_encoded(&unhex("60FC3030C9CC67C44C0EFD47C60D02296D0F73B25AE1B5F506EB9521D59A6A500717CB32F9BD049A77B2C5511B777B7422B6FE1EDE80985951F3F22BDCA5F6AD720842C17F42017DAB38E431FAA7878E936C1F23FA61CF1EB7CCDCBDF816F8628C60983C78A70D512BA37EA469CD9E7D0C53F6F0B9196614223FB31B429A6A3B03232C4C00111CC48543468A13090AE0988104316DCA34248090812301201425229B402E92B6911481259846860380248208520AB929A32245111586020648A398401A190E90C8291341111AC36514806484484D5A8031881050DA1412CB180A22C82DD3C8404A828094C44992288E58984919038062C4081CC810102549D320620904451B03261491609116485CA64C51422ACA10861121821B210A53B810490451D4147280802C0A252E214065A12806C3927001410002416E80A0888446900B45929C84695C300511076DD0328D010321090026C198444214720A01281B836941A6311042729B120824852D8A144108C31008A5850CB510023351D130241A811014902D40960DCC8220431885194952D0A6054A384111260EE2285024472063820822A72D4C324159460D4AC42D90224614B16419205292040611C3650098091B460E4BC6301A014C6020081137044444706424001A3602D9A4251093810CB930E0022900868C0CC3451428425B302C03B98581C6296346491C1600CBB071C1228AE4A49151448A934291C9002D1240118B22011B2044020044D0B6205008065A90455B042209C0008288515482485B0081DC9001C8964C181050DA247120223052A04461960C528025D8B888CAC841D8086D12314D49A6800BC150800685A48851123442410832D802620028845A8269613462E1B64823490C5C26411C0890104662A2048902A70903154E00B500D1048C5AC26C1B456C89300A24C505912028C8B000C196481A396C80B849820069DB406850420502440CA1844119400550068901176A09A724A4C21060149003083100450ED8422E102430522066D2946D40124113C164403064A09620184905D2122A1A476ECB164EE30841E3A07118B00419A4809A826109B96842206A41186400148E802405C480888CB40940A8605840458C0212DCC824D4B82912C985404220E4C4302082118B882853C824CA06050C072DC8A22C0938008C2049E08050530022CA94491BA40911984923179152042903456E0AA8241A322CD0144C10B6308C0222201172DAB820CA84811492448A3BA3C98A607F8DADC136AB4891DB9E86BD5E76773B777C3114BAB40C5CACA95E840C73BB3E7B1FE16E8512DFB5A72491CA15FDE8BFBFB1F72A73EB11D3FD6C887D3F9E7122CED067F0AFC4527233B11318527FB236C3704FA5FC16301BB750B6DD79F84865CCD1EF8F1180510D4B56947E3D993AD9200093610E5814F926906C84F9D22787503A5886C78952DDECFD3D7893258C44A59D899420F12A9F405B897A935F423D7041778340A4F41F07935129F4E8B15051924148EAB0548A880FBA223F75031E33706F04896A600EA02BFC12D97AC3BCD626B3DED6F3B2BA46F474DFC1266C4C1C915763C1BB4D71D3374C01CC5E09407F8C77F575BC39B258718C4D85CA42C92A6B3DEE6C7D52D0985B1EECD83AC67C42B2A71710193D95C84EAE2D2CD5DC9A3C9FD3ED22140D8AEE53C69DB30653AF6D14301C8E8721AE002AA67F8CEC7C9CDA6050B4CFDDE3D5D15EB8B28711F4B38762A6FB688BA952299AC83374EB6D03553F243EB3A537C5DEB2C099CC3B5EDF798031D9DD131F9563015FC4D2ED25FE74907AB61E6163795D45A465E9CACDB9ED896750F5A03A77E85FFAB280F22525BC8DC9A29175650D26517938EC3A4707AEDDAAF6C67CDD499B16927EE3B4B435E5FFC29ADBD6FB4F7E73B62058BB5EED6EDF9F7FEDF1495193A6A76C8D475EF5B6605915B07F40E7C10EDD60AFD71384809CBBF94577C6D5E51F8395BB2277A886188680892801B5D718FC4DF915D3778872E44C42BA7E0D6E2D7628105C9630976C50DCD9FC0239F2256F06656E9F44BF8C993AB9EDDDFE61F0C0BD4CC7E7FF3EA3EF0863EBF22B5B5FC0696000BA2A1874C8CDF4D901D831537158BF0E747FC365E83F490640EEB309C1079CACB4D9B4D7FA46321CAFB2D13D101E988D52847854C09934659B62FA1F7230DC94161060B242C07C1DA0B1F1AC9FBAA2BA6276729DD0660C057CBB1544AFF5FBA74B0EA7AF375BD67BC469727BA9D35BDC2233E87E405EDE5C97B1204BFBADCBEE037EE6C6C550243248522B2AB1F195059C0E81ABD02AB579C7BC9400691D715288B22FABA998CC02F7640B32F9836069A888C13A9676C8E1B252779D0B4DE6E4F2D68CBDCCBC15E5345188094756B1CCD4CE0D10F11EE89E3D7377F5C96445BE8676D9FE7CDE9CE497D128F1BA42F40C38C1E0931497BA8B28EAFEA41A5CE203D19E1F44BE11DBB0B287605172690F0988B7427F9083B9D583FAEE3487F6B75F2CED99DE2E67F8D9987EEEF91E51828FBC3128432BAD5BD9B3FF385858B65CCF1C79905BEB655E46DD9210F87C7D43FDE100356984DBD55956632665864380EDE7E1CF4450DD24A463BE1BA934878C6872F60E4AB96ACC44B157A728F0EC32FC6A7F0F1C4348911BB715AC36F88D68555CBCED2B4B1EDC339DC6FD36BCAF72FFD808038146249964FF8C5F1823C32A0EC44F3F97192573BC79D7DF1A473E8D945D8F4D72465307A01FB6B165729CF56FACB03F11062B1A6CBD9B300BD0F4FB6A54CE360F52C378E6E150D8BBE3ECB7DEA65457CD3121914283481D380DE6B527744EAE2BF45FB09ED5FDB7C0F7CD45427540E674306E7B4303CF0825588F606EDD795C31211259860F2913E05061E9E8D78B15DA97A5492532B4EDEBF63C901F620AAF9ECC9F2BE5B5E2F02864ADF1B6A2663E9D654F20ED0FE14BDA106336AC76682E84D66ADC6826176240C3BA7A8D0895ADD55706069B378304FB6A45AD9A0354C4D0DFA13A2F97244AF6C1F03B91A6DF0DCD938BC2DEBC275ADF1B9D706672C02E12B1D2FB129BA4D58FF2F869AB929071E5BC4BC420E2B346F56424FABA5C80E1418CDE6542F74297D62F34D0E84FC8C6297BD95B1B5AB5AEB3113B6B3A27BB9866656BE9572A864A14B2DF2185AFEF09C70EBA43046E4536CF7938BA7E45BED3A580D5AE98074F254D3E1DDB15EB705D8A3482316BAF5A02038D4186EA9FF40AD35D3CA955F5720932DBEF69016773C5AF8BE1D7278BFBC58D4D500FCB6AD9D7ACC9C6EB0F33CDD0D0F49D40505D9020C0332756D35CC5AEB9C873447C90C25B220A3FC1888557692B9F33D1A113C56DBBFB018691B96961B73826F58DB0C411F548395DD3F5BDBE777BBA14152C6910F07E2577B08247645D4BEDF0A13A87F7855CBA4DF7D8687AC0DD1241EBF3B3EA9398E7CFD4CCF1A237AA03D840030206FA7DE3FB35A4CAB4A2A38288F710A16FB0E24B95851A18F11A1C65DECA9A66B9C3B9BF1863436E494ECC82E5A04209F00FB03E0B6B094013ABA55EB3FA9B325521C50DB1844479006EF65B088ED1AD9CA62F4A8A03D3D3EE041D3C44D9B2D15216CF8D0DCE2A92C6BF"), &unhex("006f9b13bf381d9dc5db51f7df2520e41bdbd60cde6c2f175fd92837226843d04fb25d2084bd3ddff2c53af872d7e3a1cc863973f060b3a4391fd0fc180f6300d4c3cec2c6ed6b554ec66d921168372fd64147856818931d74240def128ba65f584b60a906bae2130ca28b847e28c8460e968dcbc54eb2cf5cd36f04bb004eb1700e24915ce0885bda2c5818c1a7e69414aa1968943a5c7a6ae152a2f130a246067162b304590607ff210621c81cb0ae7661345d465522d2c229ea62e8887de14e213471565cf6490860bdb70d1ea355433a2a6e11f0683be3b80ffba20e67bdd914406c71f4e9a0d0464eaf91068bd2248d71504556f58e8ea6663ffcc78f570f4ebe98f0c8e2b48a6fc5077ff7e249f80cc23265760c97ad303454a97310964dcea6735083dee6bfd6c651eea2bfc3fb0fb30c5ddd25b5c20e1cceaea28b51c1b4aa3ebadd59ab191f108a2ccfa6c9ffe3f635c5d24af80c6051a92ff706cf61f321946cbfa6f24478dea7d5311614b84cc6b472ed44e24816fd60d09e24f7c7e6bc42e35e16c69768220ab4920e0ac1214754efcdb61e3604f282853e5a6783933ae43cd736ffa0b582280c7be73a99839e2a05a78b02cbf91a195eb67b1a912f859ccda88d5f4c4dfc7ad08118501e50390158eb960830088dca36824d9d3052a925cb058bd384f6169cb0bb0b58c3509cab54ac6c3a5450a2597b115663e9ac36dbdadca1bafb8dadc426eb59d26209db8de6364a672d83016cc0876131bb3d87098e6d629fde82e29e1399be55bd62e1dc4312a170b6c4483c7b7f3dde2cc15ee540163543e61f9f87a7002b75c07cecfd658df31a23c720c3b048bf7e0a899f3f0f7ed2a9604669362cc438d787f8853bfd4767cb006e6412c095895056cc4be0d7b36e45b7e945cd58f38887dae0583a99154a556a76f27dc48a92a613c22ab54b13b7e0321e51489dd142f3eb289d523b9d12808297f48f94a0630831e39f9ae674b0773394d7ab96cded5e06871e2893c489b35c102afb8bea750ac9e7158a30487a0a503017c5506e91ad927f7909ab2df0476ecfe2607e00a84a59281a277a80dd8090c955830bf7823e71a9fc3335cf33e30f8f0a0dc80bd3a6b323bdd3ad02568fb2afd8de11ffd779ddaa6b52a65c6d17fcc72ad10f902f69a76683e2705fe9ddfca38fa3e62faa39611a9ef7747a0c37052e3296df52324252be0dbc3838937ba2e0ccb39373452f5d092442d28228b422bb1a04da8bbdc85722162b5e0843855c707a9c2e2ff220320d219d1b896469ea6bc47bbdefeb1c7dfbe28594773c7ea740cc37ec121fe737d8f4c6a83ac439a8aba29a87eac528a866af4e162af926d4c9d8e0c30d158d064b65821894832c4af9b08d9e10487f208f45019f25329d5b0c5b2b670f32583841abd069e6b8ed674f7448c754332ff53feff6664dceb11bcf69d082eb7cbe4177e1d1403ebe1086ab4e6ed74e6cc4dddd17e3cd74d01a13"), &[0u8; 32]);
        assert_eq!(sig, unhex("0476B8DA70153726E115877BC45607CE80DE0B02C499D254CA45E095D627928E0E9C38A9BBDCF4D9FBFB1869BB4D4003CB70C46F5B225A6CDEEFCDF96611941A676FA0AFB6C6F9B39C0083948FC90D99BDA6FADD9F508D213BD7D0BAFCD399E20927E7E334B006A52139865C18727EE0CF726578CDF82EFFC8659D2CC6E68EEF3EA037F9F6B411BD88BAE3FA4C28B3DCE901F73F681F9E5F106C843F20DB42FB7DC9D9A4CD7AD5BE7C04425FD5EEB53B98241EEFA30A63455B10C84591C3AAB938C4B5EFB8A12D09805EA5710DA1E20DBF0670F57997482F95CBECAC3C4AE9A6E2602CD46950D2F4339C7CDDE3085F3D38BB0C43B2C5B767C1DEEC2A6E3199730EEDCC6B2DF10C036C2E7AB399D893B3675F236085183E4E3A859533FA7BFF97DBD12710ABE4C813479AEC23AF042F6C6ACCB0592D73E2A56CB8A61F1977F29A93F05B592D7E7AEE8911B2D93645C3A43E6AED6F6FFD84DC49E5DE468C480DDB493E9C8E3DD92E188E39E637F93377F4F0AA2FD6D30BD1DF4DFF3AA02849D20509C3F6C6F23085A792FEA8D4F3ED254EBEB10CF1A2C34206ABF508866CEC9953FBFDA7B9B6BDEB08B5309F68B757BE8ACC22D22E1976DE4C8A97FA6200DAB292E7935C7CC6900B3F1ED8B8990F9F2F18FB23F5FD2ACDED0994D681AE2416D2635159622EBE009137C9C8ACADD41563E850527EF4DD5D0E12866732B13EECED11FB8AC3CB4B657A1980EB8C0EE3A9FA92BA5C82618EC8CF5B2D6514B1C3386A27A6DC0F2D9AA7AFDD4D93296C772D0C9191E26B02DB137DF2ABA473E93C04001EEFF610BE56FCC8DB9E4D717BF2F1ECC3FF9A663F95331882F7176590AFDA22BC697D23CDBE1FC6DC960930220CA063ADBEDCCEA4D6388E4BF05FE343F29F5526AE7DD35252DD2BB67AE0FF9A509D85075A7B8A073DD5D6A73707AC22AC7488A03F04DC6F3334B808BAEE98CA1993CB4BBC21E7CFC5406B2730E6BB31D1E2D2134564C098DAFD158B140670472440D3408948CE3457B2343CF9359B3949BDF148FDB3BC4915063FCE99DDD9467756C8F883D83C8CB35755068B1F9ED99DBBD52842D277CAA5F55E88F4DA8763B4C1D733FD0F81999D858B98BDC2EE8DF9E8F46F7A0C73915AE1C8FABF707335457FC61937993DC99877AB4DF122A8723DAF4B88B1DA92E398FC7FED2084ABF5AD370538001239BF46058DE89A7B90717627A0EAE7C62EA1832EC065A694849AD992B0C27B310A55D6E950F6BC470E5818FC0194F0B1552A9211D9A8EF5C940D1719AC315EA925ECCD1076741A2A1A3333E0021FB5B16F65F58181BA64904F6550945FCBB855D6AEF12BC1E3C9DAC6D6E3E5F3B5CAF2D0F27DAFAA97C1426BC3A740B17653DC809F2C0CFBD6409E6995FE905198CDBFE8987B45E6839B72365FF16E465DCF5E139F02A9538722DAC7E0DD1CBDD462442FD1200928CE291F651617D405978A8A946FE37821C9563D2DD24ACC282CC9FCF7736E228436586F352D856B9009BDC33A834A626B580F8A91003A12C2D30E5B7D49883EA3594DA819CEE2D39D8C3CF691E42E6554B4562FEAA300BE3950C8264E3F431DA17381B11936972C165021285608412836085194AC26A5C5868DD2BB9BFD7B561E7454AB63DAC73C5B1B727B8E1DEA479E47ABA6BA3FE5FAB099CD3E856D0654F01E72FB65C5E85EA48E92B824F550AA0B9329007FF6543C0A5160AFD2C37F3D7975972127610466303E5B8E73CE4E973875DD68411B2DBAA0CB235903BD5C95ABF58562564E37B76BE22CC205EA48652720F1B82C433762C30D6115A6A958C5207B0F2F4D1B6FE34E487D88E8C815BFFF1C43442284F346525863198F4B7B3E0B6E3326829FD1357DED656DEBDE0BD590E737337C839C2C956A863E614FD6B4AA68FEC99F2F34BCCE585FB152F7A56FA96FAD64544097E7716E3EB376141A43E577F8A2E886DB868B19521054E340EA2D65D63F9CAB102D68C37962ED1460F3A4FAB561210F742424F7E63BBE06C3D0501BB8EF30708376142EA5FDE75D0DA78E757C865AC24EB5F0A863DB8D681CE2AA1A645756687F323046E5DB2FDF9363EA809133711A4509FAFEDD4A8020EAFD0214677132B1C604927BFF5FD5BDB57AB8F04D0097476C4B1CB55B2ED0B19607BD95A7F71BD59D9370E4ABFA72AA383ACDB30B9668388F6F9244325E9A6D6F772C547A8D9B36A486F6832C547B2729CE6564B532B232170FB2B452E3C7CEC81BD37D0B5DD44FA05EBE35F03C5EF46143AD2DEBB8EA0E856CDBDF43C97398819F7E49303632D96AC7F8CE3BE4217307CA28504EAF1DF7F2A162240B3DE7D85BABF5B6CFE607DAA439E9773D596A81C60624AE1460A9E41A2843F8BDBC5B7A181A4C59982CC2CB58BA5C9447A4B6E58C75A0B41A6D2A1365F6B44E9271AF423A3888F3A1613DF6E84A7EF05BE50D018A517BB7B312A04D18F6980B281A582B2D78F3FBD89205862171DDA898EA906A0D6B08D33E9B843E75F593E524796E321B4FE6F3659FB001A895D7039B62D54C3A999FEA798A4245F29F6E992DE1B0CB882C1C5ADFE5FDAFB57CC6EE52371A76F65F632C6D0A76992749268F0A59B4951AC1AF7D1CBE3E484161B5940BE065952CCEF341D88FC25AA906FF11C64595CC56EDB0293948EE1898041F26FFB9BB03ACD71207156EBC072B1E0F54E7E68E153FA8036FAAE2BB0D1904552A96CE167C00E2A5123F76FB762B4A2E66E81C35CD667A442B36A09C2A29812E3D7E99D8501E4259690FAFA138AEF08902E3AEE8F57CDCB8F39B20894149EB8E72F919880EFA746C466C01D64A04F1F3379C8EB95A02C5E5180104BF6222FBDF8110AC0EC316F89DC224FFA6973F37DA97DB05D94E503D08DD318C30186BDA2686F46F5B1851CE792217F63E03C6EAD5241365E41A8854E633A43C2671C3650F62874ED76395FC0C627830290BECED03447FA999FB93BD75B0DECD963E3076AAB489D041B44E9F4F4C10AD80FC34B7F3FB89665E3989737750D1E5317B07765D00B75185C253C26DFA8FACB0040B656F449F35B3E84A4949AFF414F3F559357A913C0168E8E6D7930A2E9C6B5C9A7803270D32DC85EF4DEE3BD7E90DFCC51690138B752A9BC2A8CB37A9BE25DCFA16C409519D53FB571CAC6BDB0A1969B6A2F9FA951DAE001401EF3072D3C2BAAB1DAA6487275D5150639652DAF7CFBDAF62F6E4F5BAA4A239094847138102DCEF4F9CE84AEF72AC7EBD6E8D24E8A5D3327FB8F343F14CE0D9CB66CF295E13483C7034EC8A7C7439A9918CC1EB1ADD16C416E326172B3E6A6F7785888B989A9DA7ADAEB4C0D2FE323B54565F88A4D10B1220283335555C7D83A5B2B4BDBEF2181D3F434D5055577E868C9DB8CBE500000000000000000000000000000000000000000000131B2B3A"), "signature must match official ACVP byte-for-byte");
    }

    #[test]
    fn sigver_accepts_and_rejects_official_acvp() {
        // positive vector (testPassed = true) must verify
        assert!(verify_encoded(&unhex("79F984109990D7ED99515D7D0AC2F6447A5D928F353DFD80617D3F94A9076A318A55BD960CFDAB6340343BE6F2E82B0EA208B35FB7C7EE18D4BC29A5D9799728392B0B57388497A15EB2C72DB80695E716F2275B3E97586FB5E47BBC3A93E2B9C1D67BF73DD1846F5DBA968C90EC36DE55AB926BBBE7383EEA70BBC93499B4E09070BB1C538716F60F540A9EF59B119D872B3BC000FFD981B3DF40FBD1BDF7711634ADFDD52E4BD054AB565F735674F37C9543FCFF9AB9B02294E7581D96E5E01C1087392DE72E3314D156576A6D7E8997E77E297824D784FEEBDEAAC88D901627C25CC09B340368D875C1D82E6BD72D475D6D5768B3DDF30CF776A40FD0C851C441A9DD55E71504F61D0E508231C4606CB0F360F7495FC7B8165F4A90AC6D452342F05BEB7DBE0A0ECAE9970A4773DF1EE6646F9D975034866CB5B3F77490F1F0BE7DF8878EBD69E602DA6C892E513F75BABE6148FDF213DFE905D33CF25E6131B93BE1B311C764E37F26E66BE28CAB17D87D1813179B7805CCCADCEF4EC684415730136B6B4F6B654ED44C0405969A1801E48167549DAA12C5A44841CFEF66DFFFF8A62C59706973421396F4810C6A173DCD3C16F65DC110E50FF2B49DB90B93905360936A6258431099CA4C669286A0413C8C4925F51362EDBD0DF82B1B8DFEA8A29BEEC210472C8E466125AE3446F600492C47DE5FC4B63C18DB0403BBEF35B059AAF76275B54F2A46E4535FAD1F75AC4D11C3E907146179F5EA018FD04A860994638BDCB0DA243398217B7661B6EB60E5824724C9A3FB2DBA4E27D0AC393CA3315B7C7154F1F1B703A3F76B345D85F251637CFFDEE281BA77F7E7D9181F4308FB829AF3DE5EDE05A34F9C90315D45FC93D68A5BF5940A7C8578DE13EB7988B99AD0A1EFBEEC79AF6D22932FB0EE7C679EA5D80164E4C10C722E42F09FE680CB221E0B7A4F5B43F260569A6DBE44340D69396166F15593BE97F083FFD36FC08AF0AED398222915FAE98CDFF1414325AC4B748D09D7B1969D661FE20029EAF5C02F6E37C606F83D65E8AA2583719F3C6C49F1A066CE67555C86EDC62F05631CBED22E824E7B883EECCBCED45F874CBD3507F90F1034DA73487BB6557FF8CB2480D2AA51F1A324E77BCBD1F2855B997B49C408B1E87A9BD227879602D283813AD57D88872D35375224D069E1FC3577B646C2EFC3BF75521977F915F8A593A3AEFCEF0E65822DC938BDC30584E2C4BDDCEC37C219E68BA9E1A3C032DD53C2AEDA51413FEB360DCFB61DF29973DFAE5193122021034F81B585A68E31E8C7ACF018649767590982C7B37C420A4913315033FAFA5EA90016AA0BDBC4CBA256C75324F7F49918765DA675AEED6479FF63A667B7EFECAE2D26BC9B3259CB120808569862403CCA87F65EFA03C84F0CE5E3DC0BC977F4A85C52802132AF7BE921F9A0FA7A4091C8DFF6AC16C9C610984DA1CF3374FD4B4907F15F07ABDB1F81B4C7EB3EB064D68A665B7216C9C171EAB0354C938AE59A160FEE2E1F9D60C0E8208DD5361CA74A33CF36FBC63E5C700391C5AE60500DA863F1C5BB9AB9D36DB6D7523ACBDF5FD780538CE058AF58A94711F2187512E750BE1C215B53030BADE044010C5A27D8AFFD9978CB46B10F140407AA3B80B7995BF39EFB6FEE46ECE7303CC7F2B6B3563CB46C2DB00752B1A7396D55B3AF8627C19B3CD8F4B24613589DDDCE767EB3F693344F00775AFAE048C307975C7D5C3407F6501318626D857ECA835D5FD4C4B18FCBCBE19F6CFBD7DB620B225B552A163B6FC7499F342090E8E4E2A91440F0C39931FA99F2D52830D9213C7C48DCF07CE587711E8192FEF186254CEE19"), &unhex("00b4f7104c4229a429b641dfa3e7bc912d04007d23181e1d1a6977116b6aa91713460800f8b0acc3552f90e90fc6996c5f87d154f79889b3e9efc1f9067ed3ba9f50a53c531c19ff59f816accc080cdc627d8f3f244868b9835c1c11b006bc6cdb5cf11ca2fbeaefc6f69d60510c0f060171ffc2eaaf16906d48383e0be0f5f75868b9db0e0d4d61407aee3bd9d501bc9a28f1a6fd15d49538ff2af05d2a6ab531763553c9aa2ef217fcd7bf444f909738956975952794888f9a3630c7a2afb806b3b9d5d8b4f348a6a07fd62c795716d354bd5d858198215127110215692ed08245f83c65059c5a9290fce3e6e5afa19d4b99f67232fefff01edb740f85d8a28465bcdc1af5626d4a2f8f838088402d9c2088ed0bb2353f221232c873e78a21147c6c26134cc22639a5a9a872201122e74f5c99a7ec1ef08cd028dc1b9c13f3d702a391ab277cf8ba30da2aa3c20f4aef4d6486db6124b80e174d90457d5614cf4c6a816b01704bd538fbcba42c08d9606d5972b0e24042840f921b735230b6fc7f57ac9c70b848c015691ba4a8c7a033c0e27533539326f806f2234930613f53a0736bcd3c9a42e38dc5440939487632dc4bd491845670eb24e836b29bd6865a7355ec1ef97d7f9574fa8ac37e4be13da4bf1fb6bbb2a88c92654bbb30b6de1889c3551a861af1fc0290442de3025070a3666979ee0f1dc86f7339ea7a8125b8286ec5cfbb84a40e380ec5d97b18ffcb50c53d556e06864f93119e2a292a0bc869c046e3cda035e0bcd880755eb0da0e9d6215854a8f786e529370f16561e6b983248a55269d2d654a87e5e774d9e30302048b0fe38eedf3156c81fb068068a2bff54e41b4dc122971f81437838edbd38e7671cd03a5aed2663e3239c69f5e5fbe687550ad717a74d18eb59261543248a6479a64df270b3545fbb32e51e4b7278a758bb5d6811b29db51f40edbf3ce1a6c831d7ec6e9d44a1e4cd1c89a55c6c69c90852f6e9fccc3c6604db371298116c44e8417aa54b710a4fa54a345e189290566375c0a9268f19bc7d37ef9794473ef16aa247ecb2cd4ed90c2d690678b99df472db69d712d6f7203e3d629a4a167d1e50bd07b6970321cd5cd3802dbf47e9efc239be01559fae7258deffe48aabac8c4b285e6299502e20dccf66b159400f3e11ed35f692b836af1fa106b533fe8cb394d97dce760bcd082050c99c3ece236f8433e3075f3165b8e6c46e66684e71965ba58f7cbf9361f4f4953808712d738e177ef3b594d85914562d51d800c19d58593ab9b25c4ade87e977efb855bbe648ac7f37f5a6cac10567dbc79e2b4b7ce44d8db457662ffd5f74aa8065edc0e01dbb8d237da5eae60d922fe2499a533269ddcd7945d9a1c5cfecdde09cd853254a1551e15da55aa1c6c38df93bffea3d553758c9130b057facdac8d857a1b99b8b5bd233c7a869c296d0744b494ea098c535824673ee811c592999b767f1823b1ed747dc98af62e77dcca794175ea9369230c9e9ecc862c3286ad33a8fe79f4e749ba9e27532d98f1f7f1ed6b1b9873c64199ef73006d6c8b9cd0f723e82569ca3694d9e8b0d50313809e0b3912c9037dd11bce67432a7b23a34efd56411990032a00341e1544e11001339e7b900fda9926383c782893cb6ab2ef2d91e504471f1a9c49ed622ab60d94f6fb0f952e13d1e0e25a8938aa9f73ee9749cc527506fc4334aaa3e0bb7b81c17522c707b902fbbc7f07eb008a53c53f2aa9deab7783b7be02c9972acf50e8ce5a138c6fdf9e9775f444d52cc5ff0cccd57231a44adcd50292a7a55177f0005534b1ff97209ab25b4d28874d854965a81a0f1bcfa61d7f5aaaad0991f9e59d7c915b762aa827dbf042d7c1c93680af9c399f36146d91b0aadba7befb8bf4330ff1becd34e59e485123896057c85b0169a895862df48f3b5e905cc9f078ff79a22e892538e95831bfcd2ba4a8ef4107a59837fa0597629917f6b78c175e167cc7315098cf009aa0a39a1e908da631151d732707f5c7abf3c75925d28afbccde5db84ad9055e7bcd9f2cc1b08a29f23cc84188e33c7434120b56f64ace39cc654ba9ba7fe3f957a557647b29d9b749bea3f30b5761178718382dd35c04d2321b569b68d4e89630d398eafbc48c6c40deb2fb1d00542201b37da21514cdf090532908961996fcb9c1f3607a62cb05ba4e42a04c94265b23ecb4fed0a1572434134cc6ca47ba827fff8f435b770fb5b24e446d390103dc19f34f519b15fc49054fd889187d54ffbb7ba24f713f4f723e5014cceb5d740c9c65431e4676fd214f0183abea4e06c61b30cbad0d9f415d1295c898374b4a88fac2d71e436f2f8628b0efb39d610807e94c1836e66ece7483878d75fd25d23fb4200cf255ef123366b11cdc14c5e5a00bc6be098bffa907b42d0e56a39638448d0478820213cda755a336907683a292de310b356a5f442da30f06229768d79c419260b657c3d117fc2969cf2d4d16ba4fa6be7a3279d7edbbd046d32ac9d1be7363dd8d3ef1389cab3eaf424dfffeeae330ec68274aff1e209fa890607c55ab6bb6ce2b37f9eb3b0ee6c43298f78d6461a65080743c75418e15872a2d47837071473ada635f9133b160a59c8c8e34a7cad6d7e3edc7f2a590328bc82d9d9cb48251c1cc9f583a388a3e4766e2057319deb5c7cafb05c13213d9329e981b72f9c788a2ffa8125c92a1d3e9aff9067fc3d023e4a096633c255169dffbeadb0c74ed416c62c9433dc3555e2e382b129bf2d51d79a48f5f3b34ea6b43e5d5b1f0cf0b14c195f954e049c874ffd1982791944eb5addaab1888afd765b7aa8d05375430c65fbca7eaa882f825bd180e129aa116d1eb0d0620966bd06980a1d7d86f858d0ee432c8a76639c2b3fb4183591858078e83fd8bc46314f927630e246a35f3674071276ba13099ce77d269358f933377a40b0dccd6c6ff354fd3f84035b1951c30d1168d90fc129b4fd352c49864cc20a343d5ff52243857ffb867b4cbe9a2c99602995cb3d7bb7a0b5ff10e1dfeeeb787c429d77a2b1eca2dca644c761efb41e26fc2861b54e5cd1b4f2b70ef66216304ded2c72a85395889b5b098a4d7c937d0e0753e62f2955a8b535db384a77fde6209234055cb2157ae2479b1ae70207803b0186e44836c271f28934bdce77d02665f09c550cde7e84fb9c2ce1ab5821f9f399b01a9e63e6cce730e3cb4701171f393663190bfb29eb2c25582f03b9b2d6548bcee587095cd909985026c99aba43cb20ab5e2e7675b28f3723fb4ce0553767ad660154e23fd494918f71bdb9224020a51f68f11a83ba8d13d7c6edf17a990a0deffbeadae8ea999c78a220f2c6570e01fcc7896f45479940dc4ddcd9610d43b064841030521d6c98d836621cfb5951a4904f0ece497e81f02c3cf8558361c2dacf2c5c280bae50fb85c2edc737c58dc493603b33e812dcb0ac2bdeb308f26bb5fd99e2792df504db82e6d1163f3e1c0afff147ef17c074a1a57a2d1089d3e266e91d0543c07dc5eb4f52f45cc4723ac93c5896fb5944c2f2eda2f6f3d0543e7743a5986a07485386fca133eac352549c6dfd74b85b360c71770c9c23f4c29c833db7bf191ea8c6a100e22165b64a0f89c6ebb66cd76c3f02bea702cdf964692e16fa24c2ec2b78246a4c148a38cb2889da95986de0a37d528a9b3496cd30fd257c6a30eec3da1f5f49971044ceb9adf33dd00ee699e58857d8e89cfb8ac1bb8a90e343e6ce6659d97c224368fb464b1796bf5888cb9e18c0410f452db39b819d001b9b19d134b29658ca84a2a5d9571e0640870c5ecba63bfd251b80fcb2a400d4e932525451681dcbb74461ca15e56c8041d0579c5676426def7b49527e5dad3c4ec88b35d75fe6a81362d50e141b786e7e0f5a5fe05f9d747839a44810312e9c4275e58e43da8f9c2047ebf3eabaa793b35983ace9efa23446df0066ec16e00f56cfb8bbbf2bc7f1ac444f9283d4438890905f0030855c0b2194aa1bdc3bd07fda6fd83e4338240259c43235fad3d8e0eaec92cc86ba6f2efe0be3e33e21c5ee8a3a665cc53e65235cd5caafb44a071577d83d921a5aaa30cc9d6ca86b80e82463f68001f940c9287ebd1720fb2bcc5aa45f64164629e50a3c6fad9e56e3d06406b5db8094bb74ebfe83cc0f54d6906a3bb39c4bf4feda3f8724bd6e2cc7ac3abc7c079b95f80e409b591520696c2ef9cdda56032e9ce01e516188a2fc003503a4256b83d06e3481b6bad8d51d2ba10fd7b97e61db4060280fe7f9da89c4fb743e224c0a7c06b33c3c21439eb4eff64b3c1277f7846335f03fe79a839a70b30a499dc51376ed224fb0f2c51140a00f9a6b944b4cdf4501f345c2e79e46642f0132943ec48a61ef875f474c96567bc32785d0f7c971f59e78624f75666481970d892b8d03c7857b636dcd426dcd7432bb395c2fed71a83ccf97f659f5c55d44a1b51a0a9936fc73a5be6971aec5267ebc354ab3b9234c1783b4c3f03b3b39ccdf7c07943c207805ed054329993be1938821852f922cb0a949562b013ce978f8e802ad9058bea04164dce069cc957daec600e9b42411df8d0c6052ec61dcd2dd7f1fcf06dab79471e5b94161b9cd09ac329cde1fb6f808a7198847fd2085fab64e9a36d631b04ba410a0ac3cc8e905546d73626c832167b5f600b7265bb51281e61a5abf21772fa50961312eae0eb7d0aea3b23317e0c0a959685d5155450551cf4a3b80b9b3f81fb68a5d53cac11e43621c6b5116d1dc219b14632986d62f56b50befbc4219ed1a04d914e4026bbdcc81e437849bca6dbef68d9a96cdfcf8db7e5fbf788d90279991072b7490d2af25bb31168fb19da674746d4c79a37e11204da5c6a19168fda863e922424ed22c43074e14df2d4681350140d17b388b82b3496a56bd25a673c01043c632d2b7c555e8850536316a9aa462ec5805390dec2a1bf4fa042804d80c745649b7eae42169460a8914b5aeb8436e8ec1fdb2e82659fba116f4111b5071a9bae4e9ae092324ea0afd5f67e7cb32f90b1f888671c7694fe7b11e190b643369f10c80d1a15e8e17486f7a416641bcbf2fff755eba62a63bc69afc873a7d49853ec5a9948a52f8806e42e918032fdae448588654e9722f9a911b3499deec5a31877d714125f517b6446ed0ea0d47570b34db6394bed0f2b006cf99a2194cff3f2ce65f5a2b1fbfbbed3525ec32366a3abd5de6ce2b8727e6c79c50d5a0f4fb7a5b0310677204591ba98f1bd6b7ca4c67954a9704b74a324d5dd15e742b594b091bddb67e9e0f7554529e185dc475b826008c2eec0e07580171be66e758bbb9b1ece04c6ff05cf3c436a51c0f4bb3fbb4b8a72fd50b27e9684da6cb91c3abf97ead6401b03a70dd30613a13e76fa5a84d403a93422a5725fea076d0cd9d54fe4a9c8ba4f3f14bef4bf95a2c211689ef2e6468f6c1e685fa4267f30c19bcc69a9d6b3a715ec41ca646fc89c824a98df1ac909d9427f259750dda79009782ebd5c18ed5af18267fcbf880e92b6b83d4cb789a8f05929819631a42a7ce9015373027e55bf964f68c2ba6202c416573c7b5a90925ceb303cb2d883aa5945225fc24d24e9f23497dc49e6942195db591adbfe709f48c3a098eb71895773984f57dbf7dd1e30323aebf9d955fd841cad3b2ff642a99cfe579f6c541b2d1f91b59ee4183e421789101da70d9d9f630ced079ea9e8293633701a3af4a7404fe1f89678b80a71244ce5171f78dd0d18929414956b0f18272e56ee8a628ffca13096841ab532f8208272adc110578d22fa7ccaf14970e566124caab02bf496faa7277706ff07945c865d0650"), &unhex("B8CB5FE5A9662B215C95D5BA2B0328680F7576A148FF625E23CC5D203E97C62469BDC078CB6438117C1E6669B88A9C2C567894394FC06BD2E4CD754FE9A0C7AA506C004A290306D269059176635E13EAD71D0B50A99F543AEB8AA10449BC3087CAB7420D92020E7693160454229449E0913A3D84DC884BCEBECF6DD93896DA67D036CAB5C57E4671EFA3E21217F45CB4F91299E14F260E9F2260991D286994E6FE96B5B92C6B289B51C1B7F8197778B26DB4208AFEBD2B61ABA5DF09B740D7ACB49B6EE0B488AAF2AC9F2064A924D1ADD6361E19317703CA73024B1521B0247DBFE05199DCA559B3D1037A13015237E2CD5633C27B45140F503FC142E9C16620217CA18B9DD1A2D2ED1C353286426E63DC2AF5E7453299A67AF6E21D87D201CF6069F7196481D6E6FA8424FC5C2D0EB9B203A635472016ADF97810DD4F9CBA8A4B45A5EC4E550643A8349CDE7700E401B62D2B4DF65E08741BB1B79003A850D5E1475B2985DDD770AF073614F510C8C59EE5A8FD785E61BD9AE0299E2A376F2D8727AB02101B6A62195D526E1170643E06D682AFBAC1AD410968FF35E60F5962F07783192498BC7563B9F2F3658816C7901F76F34A76EF8CBD4F05F14181240AFE93BE82EC8E2067EA227AC297FE1BF8A8D456E8EE7766F1F806E55AEB16FA1FB17BB54C382E121FFE67090F40AD5B229AC088A044304833D9E2B05F7DEE98C2B084E2FE066E24963DC66CDD3AB2E58ECDC0718B14D6AF3A58CC7D911B71FA941B03F1DA25EE03D370D9CD440EE31640BCF355DE5C1C44ED382DB6AFD9A1771D355760DD981D939BF045DFD1F08B7850808A0F5B5DABBAC64C927F9C034FA4F66EC183351586E5F7A19F827F0FAD68BBB7C160AF3FBBC59ECC4DF7C9ABDC629051ACC4CAE7E23EAB6BA006D48170AAB6C38FC6E77FA85B30A90034DEB46756C006D5317A044E647BDEA1D017E69304EB3B44344C73893DE13C32E0A272716F38E899D353138BDAE0C5EA63E499C79CB9AF228BEBCC813BA99FBDF0C8F28C238AE5F81E88376F061DC8EF0618BE3A75785921C17FF7AB6EE674FAC5CDF03B69B69878582062D539A41B4B79597CEFF7C7BC8C07A50BCB51577AE64272E5A01EBBA5DC85B2C0C8D736E9EF4F81996CAAE3F12F431044194CE388A510C480D6436244A020D3ECB8F1DB9DA2823DFD510F8F1C3AFEE9A696010E7AC50E794B9E6858657EA7ADB70453DBA7EEF8D5CFD157DC916ABC5EEEACE1CC0EDD4114B4D002904ADA1AC0FFD5C15B5BC07D4F4E33A35DCBBA76573CDB18E288B037A9793D22FBF6F784E87101693581BC905BE02F86AFA6FE9C191BFD52C21122DC17ABA542DA8DD74DF521EE1C0994D50D494B2573BA3811A9CBB8297E2C7CB04914F3EB43BEEDF78A947EC34B96A5EB7CC4DCE75C5B66ED6544699C117D2CC151C3D1DE4D144A63150B6F40DB8B3073802D089938DFE641101D75BF29477F81FDAFE39A7DEDF7B8958CC89C64C03F4E2F370BAFFDBCC12F6900189D6E8C783231E36DCA783C4FE24FB73612A705A1744699EEBC5D15CD49427706BAB668AB933930E280E92A94B70E0C2F874288C140AEE9AFDF91781ECCF6A523DC81612F05BD3129548CD2C723D724AAEADB3768BC5D456E7D65C9FD1CDDD314CDF117FDE6A713DDF3754C66B6ECDA3D24175B2ECC1D793A8745AB25FC8299FA07F256CCCFD5529205568A448181B28BAFAA0369B84FBBE02153615D6C416875C65D3B230651EFADCEA465076C8ECB56A684AB93F7AA71CA6FD6AF595F126D51648CEA184F2220442E16DFAFEA25DDA77BCD2649E5804003E4AE405D186D925FC4B759B8221FC79DF55B4333D63D44D473AF570F42E6F9C8804F7414ACBA273F1F8E72582A88DA767038C3FC345EDBB7A885C46B313C42A30684B04EB6D00DA7DFA2E4CA882FF6B161C485D86E8451DDCE71B15D87FFA504CE3CFC003555E6B15CC4E0667389E524C24522487526DD37C882A4ABCCAD92EE728A742F08CA3A740BBC9BEF476443482483DDB3A46C1B4C030C4BFC08BDE3E835212AB69811267B25840302F4E7B4B7A6A3322A85D93058C32440B25F5FA2660889C20049D558C49910C8575B5820FA70C34D2FC94ABDD14D987DB0EAA377712648B5D976685A4E43FD4622CD3A3AE64C4AEBF3A3C4B29BE6555FE5E39E7F86998073F3714F260089864120BFD8A5A838540432E663BD2F427627BE8504B9DE113D79ECE70F08A4FB25AD3876AB352CCB71F15DEF83F18E33F6B9FC64B9D599E1B6FF0CF7E3C5D9F2ACDC45F10E45EEC9C1341FB952582A58735CC1606A6411CF228F2D003C0952D98068C0BEECCB16C595B37037A50F60A196E9BF8338581043FA15E94F06EA4B1D0B75261F5E87202F59408B1B1E2152459904CDDC06D16C85E1AC48B3A6C8C533665E8CE6B384ECA2454BA7A9D2EE5F28A84D23ED34C73BAACA66D8ABB8348444BA3E6B4F69B7FA68A6BD52871F7DDA19C61FE5E52CF5F6A99266550489A0D366BCEB8BEACAC019ABC6DAE7B45CCFD3EB31782AED29EC8E8CFBA79F89B038AD1BD83BB1F3829C5182AB71AE8CDA16090C50B8EA5DA8B8A09441CAEA026BF5E010236C5291304159C87F0F97C135449F1643B84272A2D6E7D89D1BF5ECBEBAFEAAF27944EBF7F7815B595639735B4260134479A6AE6C65BA4BEB766BB69588FDC50783341DB81B5B0261FD2FFE658A2ED894D17D093A9835FE227FBB4CF4E355C5E404A7BE8A78A257D692081F1C8EA27F5A3D12EAEDA54D7E0BB24C258C6841925ED5FB3BBC299C62E0F781785ADC4AA4E7A948A225DC78DE1A01EC213E1F6F88A7832218DC2FBC20B195602F8B8F7A483C56B8A303180826B4B52C8C7B9FAC5719F15A1006BEC1557C4F83A1B8167F74E2FE552E2E20D5A9B44B8795AE73E7081C5F9FB8C81D52527ADB2F1E082F696E47CF8B258BCA3914369627250CB52699EE0102F402C327713BA0092AF2DE9F06A7D3AA701D6BD1CB3F777CAA2077C3658179ABD36D938D3BA82ED8C9C62C134DF21182843754704C1D3D999120378206DD599CC81D4C367AB0DFE0770DD783873F58F5DE9137A40B721FD619F66D4963E65DDA82DB6554EE73EAC7E3C66612DD55E0DF93496A9FF919F1A3DACF9D85DF3BF4699D66F5627580BCC0B81AD240A7C1EFB29C208F723EF65A2AE11EFD9AEB25FFBF90C597E4A1F5C26CE6867D2FE3659B7196695648CFBD7229A5CFCA21F866FD5F31D0130EC01BE9B5EF66E22D96685DBA3F70BF3F17973CCD157149E00660E99954ADA5A31CEA74005C7A7E9091A3AAADB0B4C9E4E5FDFE000A0C1C213C404563708FAFB1B4BDD9E9EBF9101D213144555A6C6D878996A9BCC5CFD5DFEB050A2730343F68727376838FA1A4E6000000000000000000000010233645")),
            "valid ACVP signature must verify");
        // negative vector (testPassed = false) must be rejected
        assert!(!verify_encoded(&unhex("FAA0FDC4AB2786BAB8236F2769E7654F345E5D1AEA205765FEFA7C965759A4AF6E30AE034A74E688ED29CFC2AC23DC9A4BC97229D4796895E3AFB87E7AE14FF56E316384F4DE951869B5FBD302038F31EDA3B90BF5EC0DBF1DEE970E454D4F4AD52B2B131A4085B3158AAFFB9E6905ADE287BED9A99D750DD7A98572B1A9D3D48CE17DAB3DA826E98F0A14F903460DFBAA7C2031F4D1A722ECB9194CAC4A06BE3C640987158253377FC0715590CA677A625A9CDB661F5E0D9A85526EC4F746A219A825DD0AB0A2A9B16086A778BAAA5EEB420BFE615034E47F4C5E4B7AAF8481B198FEDD1F98BBF3913192D52D41D207D29EABD97E2F9DC66F72DC04AB125860B8EAE7827058D9AD1DF80CAF37E11005A8A036BC9941DBA33D01FFF9B0E858705E9A8E3113CAC1C3C48B1B29A6A2589AC3BC6D1E8651C2B81B7936E91E62B7994E5C5FED5CC99AC164814D2344DDD26BF9065AE5A5D2A8165B5B4280F530762D4589DD75275DE1C5875575C04F0F705E7A00F97885511162A0F6631A27835089758BB941CEB99987CB072CDF74C748C6CF72D48C8F9F3D4D166F1CFFA35E236AAAD346FF30F822C37F3A1CB85E665925F9F8607E57CE8B2B0694F75E17070C8F8B0A652ECF5D627CB411897718C5D04EA79F0DC9E00486589D472597A774874F8EBD12C1095CE8CC8E7EC15D1D890C3414A99AAB50FB35761CA3D0CC83D3AF90984A3B2BEB55FA1964C3E14835CB529909EB7BA0425CF7B3358634E9AEA6B5ED6723C4D177E7D17C4110D9D165EA59C5B30F30EE2575D78119A5C41D7FB34D0063EEB5191A6C138B0FFF18AF4AD46B9C996D6B0269EC0E0667B89C4CEBCE8F4654B60DDFC00500B3E439DA44B8DDE7802F4647851014ECBFB4C42D40993074DE5BF87CB06E07985B8F861B0FD21CC08A543DB7DA30B488A7CF6E0A4A8DE69D84A118A0F296FF4609D73E5967C7E9CA47764C4AF781E2A4C1ABD64CD4357C019C2C6777C839F887DA64DC48B661AB18F243BB738DB365A9A7ABB16F0E183B0414740CFF46A4A36B49A6D5E4D3AF9698C4F9791DD75D8557A2041C0270612B290742BCC9D7E2F4CF6F13618EA529938594B84ED1EF443356C02030E4C74F512C63E544FB73401CACEFA72C8B7511CCC44BCD89A5CFEE566F16333540417153E3AEAC112ED845C606F8C3BEBC6F75399E098C8B4BBC989D2AD48C015B90A919E7ACA2B757371244C68DA1105722804C92C2C3BC5966EE743FF6DCFCDED0126FA53BCA68874874C5FFBFB0CF72196C8AF4C3D6A52F0B9B1BC908292109C57D513BF9BA7DF65CEB26DC65DD0D69BC2E230CD4BE8CFB4875CCA062E0CE7D8DC7AF85E0CD944964DADACCEE2B057829818AC3825A45F3093BD0926C0FEFD379DF3F1C9618FAA4B4E81AA8937FACBD5DEEFCFBEB7891AC71240F7124B7F4D5133FA71ECC596645AB8D9FD959805AE1E1509CDA0339C46CB9D8123C45A3FD5A3FA87BC7804FA5BF2131D0CCFFDA71454030173D8FE2176AAC2BA5E0915BA9AF5657C2CB5D4C0677C0E95A35FB3B9E68091402B45B0AF96DED21220FF750FF303EF4C9DF756A8FBCD742A6A72D4A4977B3D0E315EC14D1688D59534EDD26E6A132E5A881B83DD9E9C35E26EC131E2F56B33FCA390A03047EE62390541DAD5BDCCEBE40D33A411109037BC0D8B5E3F2976F1620BA0EA95E12BB818D9FCB278A091FA02DAAD4205664ABBEA469B5AC7168CB3DDE3B11D366479EB2E3F15ECB000695A079256B2E9F1931FB35A1F12F41A004202BCCF8E9BA1D96A4CC1F9EEC66D78A3342556AE97201D4A351631571117024218A384AE12BFE50B514D5F68F4035C265EB7807"), &unhex("0023ec6da10af5292f80772d21e1bef4a462bf0e1e3d28f6dc03163a9a72927d953932116e59629b0554f06fb0845a1bcabe40a57680cc6e6a27d46e8a060bdbdabb72b55e1cd640dfe2780ed62fdb89ab8b768f205d0195b1f63dde99427e7e63c79f8c0e06a6414e610277167d65a8056eb009846ea2794781bc045d24a1e15a8f8c3bd7e1f85c94fbfec3d648176dfb9b023b23d90df002776d06623b3d4c25d9c02deb604308aad4e3925fa5f1b5881f409a70c7bb1bbc779c1beaa330778d5d69c7fd9f8d3b367303dff4c9689e9258aa92b2caa98ea1bdcc789c7a7cadb3d75bc744830d32b2f26519555dd988f039956651e34fe924ad4b0bd13f2e9c8d583ccd5258d728bffb7602b4cd3cedc99db8cb170906f0a5ad1f1caccc2d29f51ae7914aa226a904ac7057d4f59743caf8d7fe6824b674ea876f7901dcd52f524af74b1106e5a09c21efcd7ef177152721f981fc8073ed65f9983043c3caf47aee484b0abe65c37ad47edc111c6d83b7b33cccd1501e9b96020977ca5f751bfd01eaec742fc863edcad21a1b0aef6f70628fb8dec8f564dbbd7bd322538d4cef9ae7008cdde6476b1b94c6b5f95812d52cca58ab64b5e50276ca5a76337f82908714741dc59b3aba3b1c94adf7dba517f207a1d51ed3030e04c62718a08387b042be909e7004ddcd9f767f67c4eac89ee2fbfabb2fc44f17b2e421a269da31bb835998c0798813ecc3d19a6eebb6f3266991f3d23e0b09da7e092683f1a09f39e46b1db810312fd5ce39c7399fce21d968134308141626a111a8f1a85de1268a0b393c0d8f5c39ac9a07e965ad1b8370b28465172baa722178ad144adec9ddb79f7455223ed45ecd2a6eb387379e4e26f2d0723b98c5ff605b7ae1ee29641a4d8e2104fc41764024c0fddc2b0bce9b599b3eaef8176cd8a37197987ff91ecb0799be55d3f8ea64a1c989a7716871f8f3fada0cf56cb40be4f5fde7a61709ae676f459c03c29634a8331aad2250cea0583db089a7853b4d35c117692d9397bc76acf83f09c935a383829230b9e9cdfc5e2a7a3dd4805a5faf4f65396519b63b63d497f04c2dcad27b840902e5eb9c9bfa7d83e3cefb910d9c446cd49cb1288c84781b5685c879e20e4939658df7a41c2302ea8da53bb37f2b1cb9c60fd91bb4e996f98907c09de50b281f827bbc29550a7cadb86511227e01231722ad840a2dcbb421b64dae98d1ba4f9fc8b9b5a3423e664a1a06e7680b4d4638a0915734aaf1d8371eff68bc5596b39fcd1c70c43ac4a87ae22ffc0603d2211f09042e845a1dbc397eef67ebb9487a160cea3a2c6c503ccf16b252672a562786ea2105d0acc08a0849dd147794bf2c3496275c2b580c400ed4db45e42606169367fcaae57c113d69159339ad5e11ab5b96786d40e98e82950681e1ea29a09c828a73893f1ac3ac046746b9dba14790b6c66f84c289a006f03bc5a92915a5b0cf6fdaf4efc871cb1b3bf6e1f07fbe5ab0abf78fda6f0c5d7f6251b1e6dfe7fa48363899fcf73bae5649f4cee61f6941edab881010eee70809d7ec2a76c714100add0059c0ab0638bb087dc17815346b8dc879faeff1d84990f9440230f7b6d40a6ce41b8af60966a67702e83777024dd91c212d9c1e7ceef98b4fca11acb0bdf1b5387a630e88aff515196bdfd1d4e0c356afe0b5a78943e6472a13a5fe7bbfe3e1ba0c57c6a489fdd6d425626335249ef5c98d6c949fabc1904008988686c93a7bce14f573d58de41b7554534bd3264fc2992bc8b63bb2f6b07f2d5389a4722b2871358984e637219144a746955c41bd93913cf8b1f2acb15899dcabf61e29c3bbeab08c91bae0ff91ddba62ec0b908d2a2471b2682e15d8fdc64365e6c9fe4ca7945126ea6ce20544a527c7caa9789cffc374f46bd0580d109af873bf25a5b9a4850c64a78f519f18dd83c03b2673f63a038a8b82a2d2b4f9f9b53b0bfbe2c86662ed3ea0b29f894ff7973d855206c5d5046ea1ed0b1b91c3ad174719a096b01a091342eb49fba8a478979e3496611041cedba2abfcd179a4d9879b5b4401e695cbd3e7634894b1a6199dbf85c06a4b49728c1b872c7cedf0768a80fba1c242371c5e9fe1ab83cde963ce27eac1ef2f59823d1d863903f46ab492e6157d157441888074efc4cd869013548b21e6a2825ca0fe101603cab8914e63ae3113e03deeef0e8d444ccf5792532151f19543ce63c3faf6bb04644f15c5c0bf1c60d9b164452064b505465696b9147a53e07e520d800e0ac545830f54c3928ba26511a7ce74316fe7514e72cebf3b3f13b812a85163242e07a72696b507b503781416d2f927336688cfb532ae16b2ecdc38160ac70ba0b9380bce01dbba3e9d84fc8880ab187829c3dab3f11c5d39d5a90251f3a43a000095c2ea7151f6403cc0d290e7ec5bee2d2cbb44c70c357b227c1ff81bf6ff6f14e474dc9a46a5e407a1bf279880b5f5a21d47f3144ee575725c301635fee2fbe76a109de1fe0b751fcf93ac81a7d7539355f7fdf1e6da248156daa5bbc4b49b4e41ce6fd3494b991b39d9ef7367cb4c02ecdbea99313cb6dae4e62c9727aef9bf06144df4d93f6e40902e2e9cb508688372cd69be74a1628ec34913b78b2a4a6876a47ccf4fbecf55995e3b94cb7923f89eb8c271166f6383e39cb3a895089fccc2f916d800073b343e802ed724462a76f2a9c02af60d618d7d7dd78af20dd6c73660daf5f015a15fe3b87e78fde000bb26a6b66cd0d52c9def07eea2661eedbcf82bb82502511ca815900023aa50946b1f3da9dfcacff3ee85dc470dbeb7efe3dc29c9924c1ff9d085803bc3c4b27a3614ad595602c9e6c2bd0444fb21a2295154c4d52587a9187994aba9b091b0f3cad869b0e70844a8b24f513b65998d7b2f30cd74b77c52a0c594629aab7b3513f969de889dd2591ec87c5178286eaddc2f5625e5a4a95da860068241270c2504c779aa1275bcd25516711e1524b3a747d58aa7d080fc8ca47c534570277090e691762ccfb68366d627a5e2314ed9fca676aab1b7f3b77af4486195dc93f9faf8a26cee20591ef187419b88faf0d505112fd86921683252809786eda80aa871afa0d30653eb2b7ee0cc1aea4637117a360aa08405a99acba9c0d94cd8ffbda22ce1bf0c6b0b10b1b3de13bc34dc1359e35fec66acd4536fac3e12649cdf5cf4884fc65991f5d923714d585818460abb24762c35f346f1f23b5001442f81fe0c362b033f9737cf1094e0ad9ec68ebfad8241e59beb93208c8e5e9161056e031eeb1ee7d8f48dfd091118c913d28050ccb106b2339fea2b1d03f8cc9541e1285ef3801f175a71b43575858616b124845bf3d2422eccf79e0b364149a9c2728e982f7a3757db449ef4357e2a3df5615f4da18e07d07957d82bd7fa5cf966e9b49fc27b78117125e9fba2bc86a5227505933baf9276115113a88ba87cc53f41b1bb244cb3fbf49bfd330519228d0064d453c09b3b17a4d8ec76776dac6e59ef14c54a860af6c9c62e76c7374ffec78b153817d5b45406fc412e437cc12312c46a5418ea19169deb2975ef4213e6062ff23991be32c4442caf639ca7e2f23b1eb9da1d562ebac4a7f74a99f980b560ddd5256ffd42cb9e88aa46a4ddbd9436c36e70f773c3dc556136d38ad14cef735341593b18b6aa8eb04ee132831cfa7b811ce6e8f309a137113d25a7d17ced69ddcac0d8381552a957640115fbee9387dea506bfb5cd3d141992516a6001f3b263dc2117a4d92087db9cb5311ebaca62a886a9c42e5146ffe165b1599d2d460c951fcfc5513b0e87693510fccc1f79caebee98a60e88d233a27252b26089bceb9882509785b651402db74c0084e871331712e25919de8fe5b7557ca27b3b3fb53594e53e5390ddce48b1859d6e89881e14cdeda03ff2807dc0a328628499f2b255ba8a0d07860104b2224c9a5c4f89dfb8013b6ded9470c0a2ecc454958ff1944ce1edf769401f4f3ada35b0b2226a84f7bdf2762d3e9442b1f34e27d53cdfe7d3ffbdc6d4819daa7768ab82d722b3f0149a907477c49309a8c17806eb97b8e65d0cd266137e1e8832693bc1473d032afa39be8d223cf4a665d045f209d013b229f3bfda4cad29d227b3b16b6af9daeeb382d7fe49e1d91ce71f133d51463ce4d5a5ba212bb7acd138086ffc59dbc5790d973946b4e958e315eb91f87f62db97b00a473072e785ecc88ce8a1c5ef077918939f2ddeedee1cd52c09b1a4745e20aa35ec6154dd0cadc41733a3052d101e9c356ddd8285cd9c6b9b0d7e944904fb2fedcfd50ea4af857022b2ab895ab8e3aa9efba8708d0d7a0bfa0603637ccf77c3bdfdbd11babd98dede6a90ffd896719f71fa64c1c7b92dabe190cb3d8d63dbb2b28420cb5755a502bc7029bab85b9bc54391c757822947ddc915665fc0544e0725495b735cb4303952e6eb0f96f6c048ef8b92db6b01fc0ef87c80888ef7e9a77fc0b179a38a51d56d33597e383f5a4a9df557a9fccdc8d7c2fefead8a9d6cab64234ff1415e3d46358ea933d9be0df62a0a26faa34dcaceb2f3d7e1d186641767323d45368555c4f0faf7c79c7aca9ae0d034eeb4d3dda168857696222d402c5646aea359edc75ef9dbce81ef33542889fa0f12b3ba5d15d69632fde34e9fbe7be6faf0a35494efe2e7a812e8b4389a835ae4c2ecbc13513652f8eddca946a1edb63527ced7836ebcfbcfc60b9468ce840fa72aef8c75b10ec9c827c251eab1aeb3a65182bcaa2a84d44e616d211aff79ba3fc75bf4559308b7e3780208420b5d3ce82593e188c3d6021e45207d68f241ca2f055c47ee96d93a320a7f802da31a6fded6135bcaa9bf9b0f498b2ecd3ac263f830ac5d7a3b13fc805d8a485ee64caa74eeb72b553afadc62d68302925a97bd53de677a4d8e5d96026995c4852271934c1d8965f8154160aafaebd7d215661622106acf7998c7fd54b041979aa5d64dad5617f870bacebdaee013158bb14743012f6a5b6f1fe96416f920c6d3f040bd107789195867468d6cdaef63fddef774761487db63e6c537707060b6473a979d4410f48225bd57890ace4234565ad577b067006b8d85ce05f9aed7b0f1515bae0f45bed256a7db777f156032ebbee647215108a2e9a7e302e24b2a0134d69e6b002ef33ca1f6c9b80ae9127088b15024b00c878fbc5bdd038260c82060e5619185e9e33ce39654a8e1c4fec6aa6562ab6b8ae796682454e36ad7d2f69eab7d08b0bcd6eb5cbcae97a2efb4113e47da555667cbddee114d5cd43ff8912df69895d63c0d84bab8123668d5a7b6802923626900ec37405445bd4090a46ff2166c4aa5343b61a94355439e0b3233cf8eec78aeeaa89a413dca129eb7c70ffbb1429a0877446da2d14d119a2d750b66e37508a0a02035b175c1eaa2dd33f755f0701317981644978d18cf1f591002286cefd2ae1c006cc559321fbbdbfabaa16d6bab309d7755b52425a00de68ee3957dc69c98d564c19ba1b18eeecc69bc2d25b6ae3da57fee0987a713dd9b0e76e821eb249828e0571ba950b08b80dd5c37033b1b5d760581a25ec4782d806aec513d0fcc29a0bf694113254326214ccd00d49348875fb4f7ed14c55791216b41ed77d1d84ede31ca4bc37a116ef29667b6569a98c1576323155daa5a6a0b2cbb249436bc8902249b6d54a0eb82c955976f3063bedd90d523345bdb72799e1eecaefddbc48a059719147828967b987ef0d023f989f8dab80058c0c2211c381395ea1b8301ba25bb69b95b9996902de4f77cf7ab8d34090831fe96ea466f5175ad0eb9c35f8ffbe87bb12e92000cb7964207327291e6224fae7b13fca3a16c984c5ca54d276feb29ef8991af84b88345fb398bb3079dd1535ce0334bb9ea710d61b24b8451b2d0ceba3a1063397f2fe2c6cde714afcd4b97e0f600f73da664ffeccb2fb39a4a12e24e3b815235b24a5dd871f11dea5ed99066e5ce63951e093c8d451684dc0853b7ebd173885eb17caf5a951647e2f144034f0cbbed02fdb4c2c3df04b162a5f2efb8d40f2f62299c22d3d06149e1cd1b91f77829cff70c8cfa00d5b1fbd44f7fbcdff714fe833c7c78dbb3fa8ef7b59775dad901e261d2e6cdd2926bdea278d5cd5e59fe7842f66bf95a8aa2ddd95f8c6cb35373aa1dde15de7056f5ee35d6037e90d4ed84214bc84e831fc076b05723c880a1b5a5f666a14cdd20c30b45ee181b005335fd270c0d68f86a10be2bf2fe34613adc4ef204d1c6d115902fb4c1b893deeb68af8b85025cefa7a9c82c72616d4f78cdfe82f6076903c0822a69fc10a2323d318c604eac2f385e44fa6ed486543ff92cc464ebd11078c9fe37b43bbb34a2179cb3c7a0677b3d31e95306803301f737662431b2bacff0f92dc7cae96022b14b7aa1d67e8ba51e5d771fbac109a691c820155e6d7f1cb97231534686d8b450a4140f26c4ca184dc491b0cf7341214c744ea29eb1c6c65ba165a097d86ab00d4692fce3551adf6bbec72534dabd1f13c82cb136e6fbe3a0f86541a335d714e9c58b8c9255ea78258489999567bfc0e209d66fed547599d32551cece4063f4585c8cf39b9f28e9381905ab7fcf09f1c63381072a307076b9272cbb3e22922a3009699e5985f46310019f5eca2ec000e190f0606a93fec1fbe579900339ead777cfda3fd0ea82b794ec9a28f26c5b32b730f529fa09fa0b493989db3d884fab2991ca25617730d6baeb6a659e47fa85c0b61953e5146942f53cb700c13ca2fe51396fedf11ce8d083da3e634cef47003a4a18e087a1e5997ebc424a48b4b0f84fb754e64734d0d896a0f4caaee8fe378600b57235f2b5a61fb721c1f57f7178a4ed4c4b27ce00efbd4dd23d470451a07d37caba2d55e59b027cb448248d2669de3bce00897c6e50bae3c71b8db822d300464e1be950b473edd5c8961ebbd3856960f56ad851e7cc52b30ae1c8d371cb12d203f8128fc9d3a6a7517149c104c4c490eefe8335a6043e61abb2245a1b3f461258227f9715575e799f7f56262efd662e3ce412b7dd6af5838f28e6284c4527a89777bf6af2007400b660a09c87ce8992da709f42e7de89178c72988a5715d93f1cdef5b790409fdc26f995678282f628b23e3d268543868d2f4fab92dc9bf94cc7221f7ef73cd0189725856d6bb967380575265161a1623b5cedf461593601ac660120eabafb9ed9909ec8e77ee27a23b004c9de485f4de70cda7a961561b65f0c044fe5dc183cce536eb1ae896f15d68309c20f960db1b5e6fc02f7c9f247414c515694b9fe2d731360ebf91533e40fd144895da202b1639908e3098b502e3f67c6e26e7ee61a0c6765"), &unhex("95BBA469B6F6464A4269F47A7766E8F6F066BF3EA9C1B7196E5FB2076D3618C3128571993395F34FE9EF40E4EE4CBF6E795E16AB4998A64178A1631218F238DD8CA3835BF14B2E67A99E9FD14408962816E2D9E9AA86A258202ECA606F3C75E5C81C98D3A01BCB685710F631BC253A173D79AF484B84738C90EC067C5C095FECF4CD9E18BF81567E4A21FA0F8EEC262EBD3256439B718CAE5F5B7FA3B7E42345A6A93AB69B46DDBD7565C541B521E276F0EAB270F547BC37C61108899E0F503296CD6E5497FBF2F9BD9554448E572DC5229CE08077B71DCECAD3F5DADC49F60D32E4471D3B92DA59B3BABE649896A341F703B51227855B2D2AA080F4E325C5DFDC3DB47331EE713A64B0ADEDA4C518D04B1E27FBFD4477D58B6B92F3FCC3503E64980F2E002000FF87AE717928963EA887498A2E2B58B345A6D9EA8C0AC7A7F3E0DF03C358DF04A6ADE7C047B98C3F1DE92E116747DFA5C30714C3A47ABF55BE8043779A1A3810415B4940014EE6CE767D2257F3BB1A03E52A0539B3ABEEF98224F7A75AA7C7B6C0966C769994480C9803BFC94F0D4FF6B9CA70131CAB035BA462165534133761C07C12F1E57C12187DEE14B6D4156A9A81955B2885907B94108BDBBCD95F120C1E93F3B260D359DEC24DEC9C8F64EAC9A9B571B111DDE745FBA6DF48B54F1A445C2198BEAB63D0BB43FDB7C92B79C5620FA12CFE3AA7D132156CA5ABC85E07B55EEE259CCF7764AEDEE054933721402266998A2C0F312C1AD779BD00D2ADF31ADF482226560E453ADEF749118FCA7930E7391460BBAB2800C1614A7296CE89097AA86FA6BC7A6018C9828BA6DE105E87ACD8E157348021AC57A11049B18CCD0195DDAC91E93CF423935917DEE2FD3FCF7EB836BDED70F73CF420D3E8F4567D466F29C51AC715E1648E0F35116DD233B787D404E43819CF4D169FC8438E14349546C86F3D0F33ECFFA7C86F2ED842228764CD8EB436676D2E2419F9B35857EFB78C74647A785895D4EDAF1DD2391656F4DAB6317FE3845CBAC9550E4A72D8D1CB8FA3934947DCC5FB0B2C5677470A418C21944324CFA1B8D9566A07EE01F756747003E329ABD96578793F074959227C735A1257F269388CA865031D6009634DA524154DDC3118F70C864C84A01CC4AFC0A68E72422823ABE627DAD11A42DDF3ACBF9334CA5F1133E833E951E8C92F2D47C7123331CF30CDD160784D34262E8A162D0338B1C18067C83F6EE00EC71542DB46BFA9706F61B06FF517142163BFDC2459DFAD14F9F5CAEBBB7E6BE12A7D4ED709A58E599C175438B96C845E5783DFAD5E26B9131EBBB29FF469265B9ABBC65F763F7CC47E7A0666C6159FE9B65C1E055F34D883E4EF98A05AB1F8AD61A019FDFA4A7B493646C4B4D648C3A6677CC8974F9A2BF74040B750150F17EE5A064D2BE9D5A05C2FF56543F37B6DFCFE1A4697B158A7EF08085E4BAC2A5E5A42F98E76FEB1CD6DADCE8A13C285756C6E0F7A09E2F93E90275940C1F7C92D14A5D74761E1314C717952C3B0506645939021E459DA4C1EC7B37C681FE1642D272FFCDCC64D9DE0AA00AEF29C3B28919C7CD98D0808006EA6A0CA800B520CB39691DB033365056EADAEECA059038B9B2611908DB1AD0A4A1A73183E22FBC4760709878C21558156F04D00BBD24EBD11550B0CA43D6127CFE7B365C3F3D9C4D53E6C2AB7F7919FFEC9AFA30F944D1298282D87309C647A99C6DD50E73E59EF1BFBCC9542C627EEABC7DD75B47BA1CA8F0BDD708AA847C4F9F5D3043606A57ED2A4B52A6C9B02494A43FF411C830D661ED9099D8B80192C816B3EB79804B89F9CC9B746E68AD6A97A851EAC845D7EA3375E99DDDA5D804C1A020D9C942A42224F1AA8E6812EE0BFFCDA33E998DAD5A3E6271950732056C1BE9C6178DB71ED550F27383CC9FC257591FC0DE9163AFD5F8DBE0CE85148FABA556F1DA792A6BD502BD5353B539463770DBB829A7F31E324A1570481215A69F7612111482D224C27F9A32710823F446C33CEC5A721B15BA78239594AD78F2917FEBB064CA0AA9AABDDA40AD7EB353E14BE51980D573161EFE175B346A481A86CCBA3CE068FDF474883A8201EAABEB9D9A0212307D8F6852308F1B2C251E58C4B710031AAFD266CACD1A6A1FC91529452F50DD5C8486A4CD12AA2505B472082E9E60D0B7023F92AC8919BFA63CE6FC5B79A8B3DEF92A089A51CECC055A72C51C6B8D273C105A5E7E143CAB55A5880779275E92DF0D0371393975F4F9A6A59D14484DE3ADAD079036AEEFE6507279E138F11D31779FFC694558F3BEE25D4F817C754C4A3E04D6F1B22058E347A8F1D5535474C68303D94A9B3A8718C5086994FB9AD0DDB908F538699E451355E8EFB2226A7C6542F4F30793B741091DA87A1AB44D4D965444AC6B4251B3C04511065C276C7F9EAFFAECE13DEC8B61DBEE9E5806E458501A357434E258B35285EDD75900B057F763DB9EF893154F6E9C972DDFA134BAD5F5C26590328E080D873DE67F75142556B50F1A53C1DED217CF6D04C522DFA4E9ECEA32073B0ECADCF7B133FE822A46A28AB2478AB4C95B1B9E2728063F7BA9AF0ABF8AFD9FC8687D63E1F35FA8D16601A9CB7972E8659756464D13DEFAAAD4A90B3DC0E713657E15AAAF8D11D88E117412794C5272C452749D3AC6F8EBB80C68501631F1865FA3814F9D7093B8AD0BFDDB9C9D34D5AA78365BACFD8354F537A284437279191715D0E4481D707BF01CE99A7BEF78B5E4227E268A6CF7198288D46C1183D9230C2D175D54FA033A39C046170ABA8A9EA776C31E2ECB163351B374206CED8095F236B769018011FA3453BABC9693280A2812F1B14E040443CC3ACA86C63B4891206037B3072F95B9624440F529FA725C025ECBCF60026C6F0DB6EB39CC392361B16E43EE71F416AA5259F36A55F52E92A5CA923D2D7B8A61CEE69E1145856CC222F68ABC54BD0685D7EA3040270D5FC8F68F0DD79E3272D94F0CB85B3D88B5DA5114DE09A3CDBA24B66BC55C964103E4B0F86A6C324AE4CC12173950F5794CE724C300E60663E5AF75D60933974929BB7253C24EAF3FE1D1EB5B99C7A8CED8B9D250EAC4F2E337750672330389A1BB5DE41C8ABBF45A2BBD5AF10E78730FCF93689F383F64C49066BDAAF56C9209D8343766F30C20D6CACA3ABF975A53BDFBD6E289A10E48407FB547CAD417C50010EB2C93CCAEED8E07892BD1026FBC3A9B0A988F965E0B8AD63E5B94B018EE52EE5E1F9B8E12308D47019C6BDAA6D6547824D694F8B4DFFE14C1BFE291466C365C3191B2338397C9CA9B0DFEBF10F1E384F6A8F939CAABCF4F9212B496D7478A1A2B6BAD7313B3C7794A2B3D1D7DBE8F0FF00000002000000000000000000000000000000000000000000000000000000000C182330")),
            "tampered ACVP signature must be rejected");
    }
}
