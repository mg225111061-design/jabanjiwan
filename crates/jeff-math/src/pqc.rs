//! Post-quantum-cryptography polynomial arithmetic (Stage 5).
//!
//! Lattice PQC (ML-KEM / ML-DSA) multiplies polynomials in `Z_q[x]/(x^n + 1)`
//! (negacyclic convolution). This module provides:
//!   - the exact `schoolbook_cyclic` / `schoolbook_negacyclic` definitions (Θ(n²)),
//!     which are the *oracles* a collapse certificate is checked against (AR-4), and
//!   - `negacyclic_convolve`, the NTT-based O(n log n) product, which is what a
//!     collapser would emit.
//!
//! All arithmetic is exact mod `q` (R33: no floats on the collapse path).
//!
//! # LWE is an assumption, not a structure to collapse (PART 18 #6, hidden-structure #36)
//!
//! The security of lattice PQC rests on the hardness of Learning-With-Errors / Module-LWE.
//! That hardness is a **cryptographic assumption** — a wall, not exploitable structure.
//! JEFF compiles the *operations* of PQC (NTT, modular reduction) exactly and (with the
//! jeff-types audit) constant-time, but it never analyzes, weakens, or "solves" the LWE
//! instance. A compiler that collapsed LWE would not be optimizing; it would be breaking
//! the cryptosystem. The boundary is deliberate: NTT-convolution = structure we collapse;
//! LWE hardness = assumption we preserve untouched.
//!
//! # NTT scope (honest, R24/R30)
//!
//! `negacyclic_convolve` requires a primitive `2n`-th root of unity, i.e. `2n | (q-1)`.
//! This holds for many NTT-friendly primes (e.g. q=7681, n=4). It does **not** hold for
//! Kyber's exact parameters q=3329, n=256: `512 ∤ 3328`, so Kyber uses an *incomplete*
//! NTT (factor `x^256+1` into 128 quadratics, multiply degree-1 residues). That variant
//! is tracked for a later step (R24); we do not pretend it is implemented here. What is
//! implemented is exact and certificate-checked against the schoolbook oracle.

use crate::modular::{ModInt, NttCtx};

/// Cyclic convolution `c = a * b mod (x^n - 1)` over `Z_q`, by definition (Θ(n²)).
/// Exact. This is the oracle for cyclic-NTT collapse certificates.
pub fn schoolbook_cyclic(a: &[u64], b: &[u64], q: u64) -> Vec<u64> {
    let n = a.len();
    assert_eq!(n, b.len(), "operand lengths must match");
    let mut c = vec![0u64; n];
    for (i, &ai) in a.iter().enumerate() {
        for (j, &bj) in b.iter().enumerate() {
            let k = (i + j) % n;
            c[k] = (ModInt::new(c[k], q) + ModInt::new(ai, q) * ModInt::new(bj, q)).val;
        }
    }
    c
}

/// Negacyclic convolution `c = a * b mod (x^n + 1)` over `Z_q`, by definition (Θ(n²)).
/// Wrap-around terms (`i + j >= n`) get a sign flip because `x^n = -1`. Exact. This is
/// the oracle for negacyclic-NTT (PQC poly_mul) collapse certificates.
pub fn schoolbook_negacyclic(a: &[u64], b: &[u64], q: u64) -> Vec<u64> {
    let n = a.len();
    assert_eq!(n, b.len(), "operand lengths must match");
    let mut c = vec![ModInt::zero(q); n];
    for (i, &ai) in a.iter().enumerate() {
        for (j, &bj) in b.iter().enumerate() {
            let prod = ModInt::new(ai, q) * ModInt::new(bj, q);
            let s = i + j;
            if s < n {
                c[s] = c[s] + prod;
            } else {
                c[s - n] = c[s - n] - prod; // x^n = -1
            }
        }
    }
    c.iter().map(|x| x.val).collect()
}

/// NTT-based negacyclic convolution (the O(n log n) PQC poly_mul), via the twisting
/// trick: premultiply by `ψ^i`, cyclic-NTT, pointwise, inverse-NTT, postmultiply by
/// `ψ^{-i}`, where `ψ` is a primitive `2n`-th root of unity (`ψ^n = -1`).
///
/// Returns `None` unless a valid `ψ` exists for the given `(q, n, g)` (requires
/// `2n | (q-1)` and `g` a generator whose `(q-1)/(2n)` power has order exactly `2n`).
/// Exact mod `q`.
pub fn negacyclic_convolve(a: &[u64], b: &[u64], q: u64, g: u64) -> Option<Vec<u64>> {
    let n = a.len();
    if n != b.len() || n == 0 || (n & (n - 1)) != 0 {
        return None;
    }
    let two_n = 2 * n as u64;
    if !(q - 1).is_multiple_of(two_n) {
        return None; // no primitive 2n-th root of unity
    }
    let psi = ModInt::new(g, q).pow((q - 1) / two_n);
    // ψ must be a *primitive* 2n-th root: ψ^n == -1 (≡ q-1).
    if psi.pow(n as u64).val != q - 1 {
        return None;
    }
    let psi_inv = psi.inv()?;
    let ctx = NttCtx::new(q, n, g)?; // n-th root = g^((q-1)/n) = ψ^2, consistent

    // twist
    let mut fa: Vec<ModInt> = a
        .iter()
        .enumerate()
        .map(|(i, &x)| ModInt::new(x, q) * psi.pow(i as u64))
        .collect();
    let mut fb: Vec<ModInt> = b
        .iter()
        .enumerate()
        .map(|(i, &x)| ModInt::new(x, q) * psi.pow(i as u64))
        .collect();
    ctx.transform(&mut fa, false);
    ctx.transform(&mut fb, false);
    let mut fc: Vec<ModInt> = fa.iter().zip(&fb).map(|(x, y)| *x * *y).collect();
    ctx.transform(&mut fc, true);
    // untwist
    let out = fc
        .iter()
        .enumerate()
        .map(|(i, &x)| (x * psi_inv.pow(i as u64)).val)
        .collect();
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ntt_roundtrip_is_identity() {
        // iNTT(NTT(x)) == x exactly (the involution the user asked us to certify).
        let ctx = NttCtx::new(7681, 8, 17).expect("ntt-friendly");
        let x: Vec<u64> = [1u64, 2, 3, 4, 5, 6, 7, 8].to_vec();
        let mut f: Vec<ModInt> = x.iter().map(|&v| ModInt::new(v, 7681)).collect();
        ctx.transform(&mut f, false);
        ctx.transform(&mut f, true);
        let back: Vec<u64> = f.iter().map(|m| m.val).collect();
        assert_eq!(back, x);
    }

    #[test]
    fn negacyclic_ntt_matches_schoolbook() {
        // AR-4: the fast negacyclic NTT product must equal the Θ(n²) definition exactly.
        let q = 7681u64;
        let g = 17u64;
        let a = [1u64, 2, 3, 4];
        let b = [5u64, 6, 7, 8];
        let fast = negacyclic_convolve(&a, &b, q, g).expect("2n | q-1 for n=4");
        let slow = schoolbook_negacyclic(&a, &b, q);
        assert_eq!(fast, slow, "NTT negacyclic must match schoolbook");
    }

    #[test]
    fn negacyclic_handles_wraparound_sign() {
        // x^4 = -1: (x^3)*(x^3) = x^6 = -x^2 ⇒ coefficient of x^2 is -1 = q-1.
        let q = 7681u64;
        let a = [0u64, 0, 0, 1];
        let b = [0u64, 0, 0, 1];
        let c = schoolbook_negacyclic(&a, &b, q);
        assert_eq!(c, vec![0, 0, q - 1, 0]);
        // and the NTT agrees
        assert_eq!(negacyclic_convolve(&a, &b, q, 17).unwrap(), c);
    }

    #[test]
    fn negacyclic_rejects_bad_parameters() {
        // Kyber's q=3329 has no primitive 8th root for... actually 2*4=8 | 3328? 3328/8=416,
        // yes — but 3329's relevant wall is 512 ∤ 3328 for n=256. For small n it can work;
        // the honest check is the order test. Use a prime where 2n ∤ q-1 to force None:
        // q=17, n=4 ⇒ 2n=8, q-1=16, 8|16 ok. Use q=13, n=4 ⇒ 2n=8, q-1=12, 8∤12 ⇒ None.
        assert!(negacyclic_convolve(&[1, 2, 3, 4], &[1, 1, 1, 1], 13, 2).is_none());
    }
}
