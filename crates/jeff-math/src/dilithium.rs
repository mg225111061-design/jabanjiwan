//! ML-DSA (Dilithium) arithmetic foundation — the complete NTT and the rounding
//! primitives (Stage 14.2). The signing scheme is built on these in [`crate::mldsa`].
//!
//! Dilithium works in `Z_q[x]/(x^256+1)` with `q = 8380417 = 2^23 - 2^13 + 1`. Here
//! `512 | (q-1)`, so `x^256+1` splits completely (ζ = 1753 is a primitive 512th root) and
//! the NTT is an ordinary fully-split negacyclic transform (pointwise multiply, no
//! basemul). All arithmetic is exact (R33). The NTT is checked against the Θ(n²)
//! [`crate::pqc::schoolbook_negacyclic`] oracle; the rounding primitives are checked by
//! their defining reconstruction / hint-correctness identities.

use crate::modular::ModInt;

/// Dilithium modulus.
pub const Q: u64 = 8380417;
/// Ring degree.
pub const N: usize = 256;
/// A primitive 512th root of unity mod `Q`.
pub const ZETA: u64 = 1753;
/// Dropped low-order bits in `power2round`.
pub const D: u32 = 13;
/// `256^{-1} mod Q`, the inverse-NTT normalisation.
const N_INV: u64 = 8347681;

#[inline]
fn m(v: u64) -> ModInt {
    ModInt::new(v, Q)
}

fn brv8(i: usize) -> usize {
    let mut r = 0usize;
    for b in 0..8 {
        r |= ((i >> b) & 1) << (7 - b);
    }
    r
}

fn zetas() -> [ModInt; 256] {
    let z = m(ZETA);
    std::array::from_fn(|k| z.pow(brv8(k) as u64))
}

/// Forward complete NTT, in place (Cooley–Tukey, 8 levels).
pub fn ntt(f: &mut [u64]) {
    assert_eq!(f.len(), N);
    let z = zetas();
    let mut k = 1usize;
    let mut len = 128usize;
    while len >= 1 {
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

/// Inverse complete NTT, in place (Gentleman–Sande, 8 levels) with `256^{-1}`.
pub fn inv_ntt(f: &mut [u64]) {
    assert_eq!(f.len(), N);
    let z = zetas();
    let mut k = 255usize;
    let mut len = 1usize;
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
    let ninv = m(N_INV);
    for x in f.iter_mut() {
        *x = (m(*x) * ninv).val;
    }
}

/// Negacyclic product in `Z_q[x]/(x^256+1)` via the complete NTT (pointwise multiply).
pub fn poly_mul(a: &[u64], b: &[u64]) -> Vec<u64> {
    let mut fa = a.to_vec();
    let mut fb = b.to_vec();
    ntt(&mut fa);
    ntt(&mut fb);
    let mut fc: Vec<u64> = fa.iter().zip(&fb).map(|(&x, &y)| (m(x) * m(y)).val).collect();
    inv_ntt(&mut fc);
    fc
}

// ---- rounding primitives (FIPS 204 §7.4 / Dilithium round-2 §5.1) ----

/// Centered representative of `r mod q` in `(-q/2, q/2]`.
#[inline]
pub fn center(r: u64) -> i64 {
    let r = (r % Q) as i64;
    if r > (Q as i64) / 2 {
        r - Q as i64
    } else {
        r
    }
}

/// `power2round(r) = (r1, r0)` with `r ≡ r1·2^D + r0 (mod q)`, `r0 ∈ (-2^{D-1}, 2^{D-1}]`.
pub fn power2round(r: u64) -> (i64, i64) {
    let r = r % Q;
    let r0c = {
        let lo = (r & ((1 << D) - 1)) as i64;
        if lo > (1 << (D - 1)) {
            lo - (1 << D)
        } else {
            lo
        }
    };
    let r1 = (r as i64 - r0c) >> D;
    (r1, r0c)
}

/// `decompose(r, α)` with `α = 2·γ2`. Returns `(r1, r0)`, `r ≡ r1·α + r0 (mod q)`,
/// `r0 ∈ (-α/2, α/2]`, with the Dilithium boundary special case (`r1 = 0` near `q-1`).
pub fn decompose(r: u64, gamma2: u64) -> (i64, i64) {
    let alpha = 2 * gamma2 as i64;
    let r = (r % Q) as i64;
    let mut r0 = r % alpha;
    if r0 > alpha / 2 {
        r0 -= alpha;
    }
    if r - r0 == Q as i64 - 1 {
        (0, r0 - 1)
    } else {
        ((r - r0) / alpha, r0)
    }
}

/// High bits of `r` (the `r1` from [`decompose`]).
pub fn highbits(r: u64, gamma2: u64) -> i64 {
    decompose(r, gamma2).0
}
/// Low bits of `r` (the `r0` from [`decompose`]).
pub fn lowbits(r: u64, gamma2: u64) -> i64 {
    decompose(r, gamma2).1
}

/// Number of possible high-bit values, `m = (q-1)/α`.
fn hint_modulus(gamma2: u64) -> i64 {
    (Q as i64 - 1) / (2 * gamma2 as i64)
}

/// `makehint(z, r)`: 1 iff adding `z` changes the high bits of `r`.
pub fn make_hint(z: u64, r: u64, gamma2: u64) -> bool {
    highbits(r, gamma2) != highbits((r + z) % Q, gamma2)
}

/// `usehint(h, r)`: reconstruct the high bits of `r+z` from `r` and the hint bit.
pub fn use_hint(h: bool, r: u64, gamma2: u64) -> i64 {
    let mm = hint_modulus(gamma2);
    let (r1, r0) = decompose(r, gamma2);
    if !h {
        r1
    } else if r0 > 0 {
        (r1 + 1).rem_euclid(mm)
    } else {
        (r1 - 1).rem_euclid(mm)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fmat::Rng;
    use crate::pqc::schoolbook_negacyclic;

    const GAMMA2: u64 = (Q - 1) / 88; // ML-DSA-44

    #[test]
    fn zeta_is_primitive_512th_root() {
        assert_eq!(m(ZETA).pow(256).val, Q - 1); // ζ^256 = -1
        assert_eq!(m(ZETA).pow(512).val, 1);
        assert_eq!(m(N_INV).val * 256 % Q, 1);
    }

    #[test]
    fn ntt_roundtrips_and_matches_schoolbook() {
        let mut rng = Rng::new(0xD11);
        for _ in 0..12 {
            let a: Vec<u64> = (0..N).map(|_| rng.next_u64() % Q).collect();
            let b: Vec<u64> = (0..N).map(|_| rng.next_u64() % Q).collect();
            let mut ra = a.clone();
            ntt(&mut ra);
            inv_ntt(&mut ra);
            assert_eq!(ra, a, "inv_ntt(ntt) = id");
            assert_eq!(poly_mul(&a, &b), schoolbook_negacyclic(&a, &b, Q), "NTT = schoolbook");
        }
    }

    #[test]
    fn power2round_reconstructs() {
        let mut rng = Rng::new(0x9);
        for _ in 0..5000 {
            let r = rng.next_u64() % Q;
            let (r1, r0) = power2round(r);
            assert!(r0 > -(1 << (D - 1)) && r0 <= (1 << (D - 1)));
            let recon = (m((r1.rem_euclid(Q as i64)) as u64) * m(1 << D)
                + m(r0.rem_euclid(Q as i64) as u64))
            .val;
            assert_eq!(recon, r, "r1·2^D + r0 ≡ r");
        }
    }

    #[test]
    fn decompose_reconstructs_and_bounds() {
        let alpha = 2 * GAMMA2 as i64;
        let mut rng = Rng::new(0x7);
        for _ in 0..5000 {
            let r = rng.next_u64() % Q;
            let (r1, r0) = decompose(r, GAMMA2);
            assert!(r0 > -alpha / 2 && r0 <= alpha / 2, "low bits centered");
            let recon = (r1 * alpha + r0).rem_euclid(Q as i64);
            assert_eq!(recon as u64, r, "r1·α + r0 ≡ r");
        }
    }

    #[test]
    fn hint_correctness_identity() {
        // usehint(makehint(z, r), r) == highbits(r + z) for small ‖z‖∞ ≤ γ2 (the lemma
        // signing relies on). Holds exactly.
        let mut rng = Rng::new(0x4A);
        for _ in 0..5000 {
            let r = rng.next_u64() % Q;
            // z with |z| ≤ γ2
            let zc = (rng.next_u64() % (GAMMA2 + 1)) as i64 * if rng.next_u64() & 1 == 0 { 1 } else { -1 };
            let z = (zc.rem_euclid(Q as i64)) as u64;
            let h = make_hint(z, r, GAMMA2);
            assert_eq!(use_hint(h, r, GAMMA2), highbits((r + z) % Q, GAMMA2));
        }
    }
}
