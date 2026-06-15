//! ML-KEM (Kyber) polynomial arithmetic — the **incomplete NTT** (Stage 14.1).
//!
//! Kyber works in `Z_q[x]/(x^256 + 1)` with `q = 3329`. Because `q - 1 = 2^8 · 13`, the
//! ring has a primitive 256th root of unity `ζ = 17` (`ζ^128 = -1`) but **no** 512th root,
//! so `x^256 + 1` does not split into linear factors. It splits into 128 quadratics
//!
//! ```text
//!   x^256 + 1 = ∏_{i=0}^{127} (x^2 - ζ^{2·brv7(i)+1})
//! ```
//!
//! The transform is therefore a **7-level** NTT (lengths 128,64,…,2) taking a degree-255
//! polynomial to 128 degree-1 residues; multiplication in the NTT domain is a `basemul`
//! of degree-1 pairs modulo `(x^2 - γ)`:
//!
//! ```text
//!   (a0 + a1 x)(b0 + b1 x) ≡ (a0 b0 + γ a1 b1) + (a0 b1 + a1 b0) x   (mod x^2 - γ)
//! ```
//!
//! Everything here is **exact** arithmetic mod `q` (R33). Correctness is not asserted by
//! recollection of the reference constants: the whole pipeline
//! `inv_ntt(basemul(ntt a, ntt b))` is checked against [`crate::pqc::schoolbook_negacyclic`],
//! the Θ(n²) definition, which is the oracle (AR-4 / P0). The NTT is the structure JEFF
//! collapses; the underlying Module-LWE hardness is an assumption it never touches
//! (see `crate::pqc`).

use crate::modular::ModInt;

/// Kyber modulus.
pub const Q: u64 = 3329;
/// Kyber ring degree.
pub const N: usize = 256;
/// A primitive 256th root of unity mod `Q` (`ZETA^128 ≡ -1`).
pub const ZETA: u64 = 17;
/// `128^{-1} mod Q` — the normalisation applied by the inverse transform (128 = 2^7 levels).
/// `128 · 3303 = 422784 = 127·3329 + 1`.
const N_INV_128: u64 = 3303;

#[inline]
fn m(v: u64) -> ModInt {
    ModInt::new(v, Q)
}

/// Bit-reversal of the low 7 bits of `i` (the NTT factor ordering).
fn brv7(i: usize) -> usize {
    let mut r = 0usize;
    for b in 0..7 {
        r |= ((i >> b) & 1) << (6 - b);
    }
    r
}

/// `zetas[k] = ζ^{brv7(k)} mod Q` for `k = 0..128` (the standard Kyber twiddle table, in
/// the exact/normal domain rather than Montgomery form).
fn zetas() -> [ModInt; 128] {
    let z = m(ZETA);
    std::array::from_fn(|k| z.pow(brv7(k) as u64))
}

/// Forward incomplete NTT, in place (Cooley–Tukey, 7 levels). After this, `f` holds 128
/// degree-1 residues `(f[2i], f[2i+1])` modulo the 128 quadratic factors.
pub fn ntt(f: &mut [u64]) {
    assert_eq!(f.len(), N, "Kyber NTT operates on degree-255 polynomials");
    let z = zetas();
    let mut k = 1usize;
    let mut len = 128usize;
    while len >= 2 {
        let mut start = 0usize;
        while start < N {
            let zeta = z[k];
            k += 1;
            for j in start..start + len {
                let t = zeta * m(f[j + len]);
                f[j + len] = (m(f[j]) - t).val;
                f[j] = (m(f[j]) + t).val;
            }
            start += 2 * len;
        }
        len >>= 1;
    }
}

/// Inverse incomplete NTT, in place (Gentleman–Sande, 7 levels) with the final `128^{-1}`
/// normalisation. `inv_ntt(ntt(f)) == f` exactly.
pub fn inv_ntt(f: &mut [u64]) {
    assert_eq!(f.len(), N, "Kyber NTT operates on degree-255 polynomials");
    let z = zetas();
    let mut k = 127usize;
    let mut len = 2usize;
    while len <= 128 {
        let mut start = 0usize;
        while start < N {
            let zeta = z[k];
            k = k.wrapping_sub(1);
            for j in start..start + len {
                let t = m(f[j]);
                let fl = m(f[j + len]);
                f[j] = (t + fl).val;
                f[j + len] = (zeta * (fl - t)).val;
            }
            start += 2 * len;
        }
        len <<= 1;
    }
    let ninv = m(N_INV_128);
    for x in f.iter_mut() {
        *x = (m(*x) * ninv).val;
    }
}

/// One degree-1 product modulo `(x^2 - γ)`.
#[inline]
fn basemul_pair(a0: u64, a1: u64, b0: u64, b1: u64, gamma: ModInt) -> (u64, u64) {
    let (a0, a1, b0, b1) = (m(a0), m(a1), m(b0), m(b1));
    let r0 = a0 * b0 + (a1 * b1) * gamma;
    let r1 = a0 * b1 + a1 * b0;
    (r0.val, r1.val)
}

/// Pointwise multiplication in the NTT domain: the 128 `basemul`s with `γ = ±ζ^{2brv7(i)+1}`.
pub fn basemul(a: &[u64], b: &[u64]) -> Vec<u64> {
    assert_eq!(a.len(), N);
    assert_eq!(b.len(), N);
    let z = zetas();
    let mut r = vec![0u64; N];
    for i in 0..64 {
        let gamma = z[64 + i];
        let (r0, r1) = basemul_pair(a[4 * i], a[4 * i + 1], b[4 * i], b[4 * i + 1], gamma);
        r[4 * i] = r0;
        r[4 * i + 1] = r1;
        let neg = m(0) - gamma;
        let (r2, r3) =
            basemul_pair(a[4 * i + 2], a[4 * i + 3], b[4 * i + 2], b[4 * i + 3], neg);
        r[4 * i + 2] = r2;
        r[4 * i + 3] = r3;
    }
    r
}

/// Negacyclic polynomial product in `Z_q[x]/(x^256+1)` via the incomplete NTT — the
/// O(n log n) path Kyber actually runs. Equals [`crate::pqc::schoolbook_negacyclic`]
/// exactly (the certificate obligation).
pub fn poly_mul(a: &[u64], b: &[u64]) -> Vec<u64> {
    let mut fa = a.to_vec();
    let mut fb = b.to_vec();
    ntt(&mut fa);
    ntt(&mut fb);
    let mut fc = basemul(&fa, &fb);
    inv_ntt(&mut fc);
    fc
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fmat::Rng;
    use crate::pqc::schoolbook_negacyclic;

    #[test]
    fn zeta_is_primitive_256th_root() {
        // ζ^128 = -1, ζ^256 = 1, and ζ^k ≠ 1 for 0 < k < 256.
        assert_eq!(m(ZETA).pow(128).val, Q - 1);
        assert_eq!(m(ZETA).pow(256).val, 1);
        assert_eq!(m(N_INV_128).val * 128 % Q, 1);
    }

    #[test]
    fn ntt_roundtrips_to_identity() {
        let mut rng = Rng::new(0xC0FFEE);
        for _ in 0..20 {
            let f: Vec<u64> = (0..N).map(|_| rng.next_u64() % Q).collect();
            let mut g = f.clone();
            ntt(&mut g);
            inv_ntt(&mut g);
            assert_eq!(g, f, "inv_ntt(ntt(f)) must be the identity");
        }
    }

    #[test]
    fn incomplete_ntt_matches_schoolbook() {
        // The whole NTT pipeline equals the Θ(n²) negacyclic definition, EXACTLY, for the
        // real Kyber parameters (q=3329, n=256). This is the incomplete-NTT certificate.
        let mut rng = Rng::new(0x5EED);
        for _ in 0..16 {
            let a: Vec<u64> = (0..N).map(|_| rng.next_u64() % Q).collect();
            let b: Vec<u64> = (0..N).map(|_| rng.next_u64() % Q).collect();
            let fast = poly_mul(&a, &b);
            let slow = schoolbook_negacyclic(&a, &b, Q);
            assert_eq!(fast, slow, "incomplete NTT must equal schoolbook negacyclic");
        }
    }

    #[test]
    fn incomplete_ntt_wraparound_sign() {
        // x^255 · x^255 = x^510 = x^{510-512}·(-1)^? : 510 = 256+254 ⇒ x^254 with sign -1.
        let mut a = vec![0u64; N];
        a[255] = 1;
        let c = poly_mul(&a, &a);
        let mut want = vec![0u64; N];
        want[254] = Q - 1; // -1 mod q
        assert_eq!(c, want);
        assert_eq!(c, schoolbook_negacyclic(&a, &a, Q));
    }
}
