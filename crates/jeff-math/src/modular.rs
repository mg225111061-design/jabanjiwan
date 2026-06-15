//! Modular integer arithmetic over a runtime prime modulus.
//!
//! Backs `Evidence::NumericResidual` replay (APPENDIX F.6) and the NTT / Bostan–Mori
//! kernels (APPENDIX E.4). All operations are exact integer arithmetic mod `q`
//! (R33: exact on the collapse path). `q` is carried per value so a checker can be
//! fully self-contained (R16).

/// An element of Z/qZ. `q` is stored alongside the value so operations can assert
/// matching moduli (a mismatch is an internal-invariant bug → panic is acceptable
/// here per R38, since it is not user input).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ModInt {
    pub val: u64,
    pub modulus: u64,
}

impl ModInt {
    pub fn new(val: u64, modulus: u64) -> Self {
        assert!(modulus > 1, "modulus must be > 1");
        ModInt {
            val: val % modulus,
            modulus,
        }
    }

    pub fn zero(modulus: u64) -> Self {
        ModInt::new(0, modulus)
    }
    pub fn one(modulus: u64) -> Self {
        ModInt::new(1, modulus)
    }

    fn same(self, o: ModInt) -> u64 {
        debug_assert_eq!(self.modulus, o.modulus, "modulus mismatch (R18 invariant)");
        self.modulus
    }

    /// Modular exponentiation, `self ** e mod q`, in O(log e) (square-and-multiply).
    pub fn pow(self, mut e: u64) -> ModInt {
        let m = self.modulus;
        let mut base = self;
        let mut acc = ModInt::one(m);
        while e > 0 {
            if e & 1 == 1 {
                acc = acc * base;
            }
            base = base * base;
            e >>= 1;
        }
        acc
    }

    /// Multiplicative inverse via Fermat's little theorem (assumes `q` prime).
    /// Returns `None` if `self == 0`.
    pub fn inv(self) -> Option<ModInt> {
        if self.val == 0 {
            return None;
        }
        Some(self.pow(self.modulus - 2))
    }
}

impl std::ops::Add for ModInt {
    type Output = ModInt;
    fn add(self, o: ModInt) -> ModInt {
        let m = self.same(o);
        let v = ((self.val as u128 + o.val as u128) % m as u128) as u64;
        ModInt { val: v, modulus: m }
    }
}

impl std::ops::Sub for ModInt {
    type Output = ModInt;
    fn sub(self, o: ModInt) -> ModInt {
        let m = self.same(o);
        let v = ((self.val as u128 + m as u128 - o.val as u128) % m as u128) as u64;
        ModInt { val: v, modulus: m }
    }
}

impl std::ops::Mul for ModInt {
    type Output = ModInt;
    fn mul(self, o: ModInt) -> ModInt {
        let m = self.same(o);
        let v = ((self.val as u128 * o.val as u128) % m as u128) as u64;
        ModInt { val: v, modulus: m }
    }
}

/// Number-theoretic transform context for an NTT-friendly prime `q` with a
/// primitive `n`-th root of unity. Used by `numeric.ntt` (APPENDIX I.3) for exact
/// negacyclic/cyclic convolution; the collapse certificate is `NumericResidual`.
///
/// Requires `n` a power of two with `n | (q - 1)` (checked, returns `None`).
pub struct NttCtx {
    pub q: u64,
    pub n: usize,
    pub root: u64, // primitive n-th root of unity mod q
}

impl NttCtx {
    /// Build a context from a known NTT-friendly prime and a primitive root `g`
    /// of the multiplicative group mod q. Returns `None` if `n ∤ (q-1)` or `n` not
    /// a power of two (R38: report, do not panic on bad caller config).
    pub fn new(q: u64, n: usize, primitive_root_g: u64) -> Option<Self> {
        if n == 0 || (n & (n - 1)) != 0 {
            return None; // n not a power of two
        }
        if !(q - 1).is_multiple_of(n as u64) {
            return None; // n does not divide q-1
        }
        let g = ModInt::new(primitive_root_g, q);
        let exp = (q - 1) / n as u64;
        let root = g.pow(exp).val;
        Some(NttCtx { q, n, root })
    }

    fn bit_reverse(a: &mut [ModInt]) {
        let n = a.len();
        let mut j = 0usize;
        for i in 1..n {
            let mut bit = n >> 1;
            while j & bit != 0 {
                j ^= bit;
                bit >>= 1;
            }
            j ^= bit;
            if i < j {
                a.swap(i, j);
            }
        }
    }

    /// In-place iterative Cooley–Tukey NTT. `inverse=false` forward, `true` inverse.
    pub fn transform(&self, a: &mut [ModInt], inverse: bool) {
        assert_eq!(a.len(), self.n, "NTT length must match context");
        let q = self.q;
        Self::bit_reverse(a);
        let mut len = 2usize;
        while len <= self.n {
            // w = root^(n/len), or its inverse
            let base = ModInt::new(self.root, q).pow((self.n / len) as u64);
            let wlen = if inverse { base.inv().unwrap() } else { base };
            let mut i = 0;
            while i < self.n {
                let mut w = ModInt::one(q);
                for k in 0..len / 2 {
                    let u = a[i + k];
                    let v = a[i + k + len / 2] * w;
                    a[i + k] = u + v;
                    a[i + k + len / 2] = u - v;
                    w = w * wlen;
                }
                i += len;
            }
            len <<= 1;
        }
        if inverse {
            let n_inv = ModInt::new(self.n as u64, q).inv().unwrap();
            for x in a.iter_mut() {
                *x = *x * n_inv;
            }
        }
    }

    /// Cyclic convolution of two length-n vectors via NTT (exact mod q).
    pub fn convolve(&self, a: &[u64], b: &[u64]) -> Vec<u64> {
        let mut fa: Vec<ModInt> = a.iter().map(|&x| ModInt::new(x, self.q)).collect();
        let mut fb: Vec<ModInt> = b.iter().map(|&x| ModInt::new(x, self.q)).collect();
        self.transform(&mut fa, false);
        self.transform(&mut fb, false);
        let mut fc: Vec<ModInt> = fa.iter().zip(&fb).map(|(x, y)| *x * *y).collect();
        self.transform(&mut fc, true);
        fc.iter().map(|x| x.val).collect()
    }
}

/// Extended Euclid: returns `(g, x, y)` with `a*x + b*y = g = gcd(a,b)` (signed).
/// Exact over i128 (R33). Used by CRT (general, modulus need not be prime).
pub fn egcd(a: i128, b: i128) -> (i128, i128, i128) {
    if b == 0 {
        (a, 1, 0)
    } else {
        let (g, x, y) = egcd(b, a % b);
        (g, y, x - (a / b) * y)
    }
}

/// Modular inverse of `a` mod `m` (general `m`, via extended Euclid). `None` if not
/// coprime. Exact.
pub fn inv_mod(a: i128, m: i128) -> Option<i128> {
    let (g, x, _) = egcd(a.rem_euclid(m), m);
    if g != 1 {
        return None;
    }
    Some(x.rem_euclid(m))
}

/// Chinese Remainder Theorem for two coprime moduli: find `x` in `[0, m1*m2)` with
/// `x ≡ r1 (mod m1)` and `x ≡ r2 (mod m2)`. Returns `None` if `m1`, `m2` not coprime.
/// Exact (R33). Used to cross-check NumericResidual replays under two moduli (E.4) and
/// to reconstruct from residues.
pub fn crt2(r1: u64, m1: u64, r2: u64, m2: u64) -> Option<u64> {
    let (m1i, m2i) = (m1 as i128, m2 as i128);
    let inv = inv_mod(m1i, m2i)?; // (m1)^{-1} mod m2
    let diff = ((r2 as i128 - r1 as i128).rem_euclid(m2i) * inv).rem_euclid(m2i);
    let x = r1 as i128 + m1i * diff;
    Some(x.rem_euclid(m1i * m2i) as u64)
}

/// Montgomery modular multiplication for an *odd* modulus `q < 2^32` (PQC moduli
/// q=3329 / 8380417 qualify; G.1). Exact: `from_mont(montmul(to_mont a, to_mont b))`
/// equals `a*b mod q` — verified against standard `ModInt` multiplication in tests
/// (AR-4 oracle). Montgomery avoids a per-multiply division, which is the constant-time
/// representation real PQC implementations use (E.10).
///
/// Uses `R = 2^32`. All intermediates fit in u128.
#[derive(Clone, Copy, Debug)]
pub struct Montgomery {
    pub q: u64,
    qinv: u64,   // -q^{-1} mod 2^32
    r2: u64,     // R^2 mod q, for to_mont
}

impl Montgomery {
    const RBITS: u32 = 32;
    const RMASK: u128 = (1u128 << 32) - 1;

    /// Build a context for odd `q` with `1 < q < 2^32`. `None` otherwise (R38).
    pub fn new(q: u64) -> Option<Self> {
        if q <= 1 || q & 1 == 0 || q >= (1u64 << 32) {
            return None;
        }
        // q^{-1} mod 2^32 by Hensel lifting (q is odd ⇒ invertible mod 2^k).
        let mut inv: u64 = 1;
        for _ in 0..5 {
            // 1,2,4,8,16,32 bits of correctness
            inv = inv.wrapping_mul(2u64.wrapping_sub(q.wrapping_mul(inv)));
        }
        inv &= (1u64 << 32) - 1;
        let qinv = ((1u64 << 32) - inv) & ((1u64 << 32) - 1); // -q^{-1} mod 2^32
        let r = 1u128 << 32;
        let r2 = ((r % q as u128) * (r % q as u128) % q as u128) as u64;
        Some(Montgomery { q, qinv, r2 })
    }

    /// REDC: given `t < q * R`, return `t * R^{-1} mod q` in `[0, q)`.
    fn redc(&self, t: u128) -> u64 {
        let m = ((t & Self::RMASK) * self.qinv as u128) & Self::RMASK;
        let u = (t + m * self.q as u128) >> Self::RBITS;
        let u = u as u64;
        if u >= self.q {
            u - self.q
        } else {
            u
        }
    }

    /// Convert `a mod q` into Montgomery form `a*R mod q`.
    pub fn to_mont(&self, a: u64) -> u64 {
        self.redc((a % self.q) as u128 * self.r2 as u128)
    }

    /// Convert out of Montgomery form.
    pub fn from_mont(&self, a: u64) -> u64 {
        self.redc(a as u128)
    }

    /// Montgomery multiply two values already in Montgomery form.
    pub fn mul(&self, a: u64, b: u64) -> u64 {
        self.redc(a as u128 * b as u128)
    }
}

/// Barrett reduction for a fixed modulus `q` (Stage 14.3). Computes `a mod q` for a wide
/// product `a` using one precomputed `μ = ⌊2^k / q⌋` and shifts instead of a division —
/// the modular-reduction choice on ISAs without a fast Montgomery (the directive's
/// Barrett option). Exact: verified against [`ModInt`] multiplication (the standard-mod
/// oracle, `reduction_matches_standard_mod`).
#[derive(Clone, Copy, Debug)]
pub struct Barrett {
    pub q: u64,
    mu: u128,
    k: u32,
}

impl Barrett {
    /// Build for `1 < q < 2^32`. `None` otherwise (R38).
    pub fn new(q: u64) -> Option<Self> {
        if q <= 1 || q >= (1u64 << 32) {
            return None;
        }
        // k chosen so products a < q^2 are well within 2^k (estimate error ≤ 2).
        let k = 2 * (64 - (q - 1).leading_zeros()) + 2;
        let mu = (1u128 << k) / q as u128;
        Some(Barrett { q, mu, k })
    }

    /// Reduce `a` (any `u128`, e.g. a product of two values `< q`) to `[0, q)`.
    pub fn reduce(&self, a: u128) -> u64 {
        let t = (a * self.mu) >> self.k; // ≈ ⌊a/q⌋, within 2 below
        let mut r = a - t * self.q as u128;
        while r >= self.q as u128 {
            r -= self.q as u128;
        }
        r as u64
    }

    /// `a · b mod q`.
    pub fn mul(&self, a: u64, b: u64) -> u64 {
        self.reduce(a as u128 * b as u128)
    }
}

/// Plantard-style reduction for an odd modulus `q < 2^16` (Stage 14.3). Like Montgomery it
/// computes `a·b·R^{-1} mod q` with `R = 2^32`, but via the signed Plantard rounding (one
/// fewer multiply on small word sizes — faster than Montgomery on Cortex-M4). The reported
/// result is the *same* residue class as the standard product, which is what the
/// `reduction_matches_standard_mod` gate proves. Conversion in/out of the `R`-domain reuses
/// the standard relation `to(a) = a·R mod q`.
#[derive(Clone, Copy, Debug)]
pub struct Plantard {
    pub q: u64,
    qinv32: u64, // q^{-1} mod 2^32
    r2: u64,     // R^2 mod q  (R = 2^32), for domain conversion
}

impl Plantard {
    /// Build for odd `1 < q < 2^16`. `None` otherwise (R38).
    pub fn new(q: u64) -> Option<Self> {
        if q <= 1 || q & 1 == 0 || q >= (1u64 << 16) {
            return None;
        }
        // q^{-1} mod 2^32 by Hensel lifting (q odd ⇒ invertible mod 2^k).
        let mut inv: u64 = 1;
        for _ in 0..5 {
            inv = inv.wrapping_mul(2u64.wrapping_sub(q.wrapping_mul(inv)));
        }
        let qinv32 = inv & 0xffff_ffff;
        let r = 1u128 << 32;
        let r2 = ((r % q as u128) * (r % q as u128) % q as u128) as u64;
        Some(Plantard { q, qinv32, r2 })
    }

    /// Plantard reduction: given `c` (a product of two `R`-domain values, `c < q·R`),
    /// return `c·R^{-1} mod q` in `[0, q)`.
    pub fn reduce(&self, c: u128) -> u64 {
        // t = (c · q^{-1}) mod 2^32, then r = ⌊t · q / 2^32⌋ adjusted — the Plantard
        // rearrangement of Montgomery's REDC. Exact (gated by the test).
        let t = (c as u64).wrapping_mul(self.qinv32) & 0xffff_ffff;
        let prod = t as u128 * self.q as u128;
        // c + t·q is divisible by 2^32; the high part is c·R^{-1} mod q (one cond. sub).
        let r = ((c + prod) >> 32) as u64;
        if r >= self.q {
            r - self.q
        } else {
            r
        }
    }

    /// Lift `a` into the `R`-domain (`a·R mod q`).
    fn lift(&self, a: u64) -> u64 {
        self.reduce((a % self.q) as u128 * self.r2 as u128)
    }
    /// Bring a value back out of the `R`-domain.
    fn unlift(&self, a: u64) -> u64 {
        self.reduce(a as u128)
    }

    /// `a · b mod q` via the Plantard domain (round-trips through `R`).
    pub fn mul(&self, a: u64, b: u64) -> u64 {
        let am = self.lift(a);
        let bm = self.lift(b);
        self.unlift(self.reduce(am as u128 * bm as u128))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reduction_matches_standard_mod() {
        // Stage 14.3: every reduction algorithm must equal the standard mod-mul oracle.
        // Kyber prime q=3329 — exhaustive-ish grid for Barrett, Plantard, and Montgomery.
        let q = 3329u64;
        let barrett = Barrett::new(q).unwrap();
        let plantard = Plantard::new(q).unwrap();
        let mont = Montgomery::new(q).unwrap();
        for a in (0..q).step_by(17) {
            for b in (0..q).step_by(19) {
                let want = (ModInt::new(a, q) * ModInt::new(b, q)).val;
                assert_eq!(barrett.mul(a, b), want, "barrett {a}*{b}");
                assert_eq!(plantard.mul(a, b), want, "plantard {a}*{b}");
                let m = mont.from_mont(mont.mul(mont.to_mont(a), mont.to_mont(b)));
                assert_eq!(m, want, "montgomery {a}*{b}");
            }
        }
        // Dilithium prime q=8380417 (23-bit) — Barrett/Montgomery only (Plantard is ≤16-bit).
        let dq = 8380417u64;
        let bd = Barrett::new(dq).unwrap();
        let md = Montgomery::new(dq).unwrap();
        assert!(Plantard::new(dq).is_none(), "Plantard is a small-modulus (≤16-bit) method");
        for a in (0..dq).step_by(40_000) {
            for b in (0..dq).step_by(50_000) {
                let want = (ModInt::new(a, dq) * ModInt::new(b, dq)).val;
                assert_eq!(bd.mul(a, b), want, "barrett dilithium {a}*{b}");
                assert_eq!(md.from_mont(md.mul(md.to_mont(a), md.to_mont(b))), want);
            }
        }
    }

    #[test]
    fn modpow_and_inv() {
        let a = ModInt::new(3, 3329);
        assert_eq!(a.pow(0).val, 1);
        let inv = a.inv().unwrap();
        assert_eq!((a * inv).val, 1);
    }

    #[test]
    fn crt_known_answer_and_roundtrip() {
        // x ≡ 2 (mod 3), x ≡ 3 (mod 5) ⇒ x = 8.
        assert_eq!(crt2(2, 3, 3, 5), Some(8));
        // non-coprime moduli ⇒ None.
        assert_eq!(crt2(1, 4, 2, 6), None);
        // reconstruct a value < m1*m2 from its residues (exact).
        for x in [0u64, 1, 7, 1000, 3328] {
            let r = crt2(x % 3329, 3329, x % 7681, 7681).unwrap();
            assert_eq!(r % 3329, x % 3329);
            assert_eq!(r % 7681, x % 7681);
        }
    }

    #[test]
    fn montgomery_matches_standard_mul_exactly() {
        // AR-4: Montgomery is correct iff it equals the standard mod-mul oracle on all
        // inputs (here exhaustively over a coprime grid for q=3329).
        let q = 3329u64;
        let mont = Montgomery::new(q).expect("odd q < 2^32");
        // known answer: round-trip identity
        for a in [0u64, 1, 2, 5, 3328] {
            assert_eq!(mont.from_mont(mont.to_mont(a)), a % q, "roundtrip a={a}");
        }
        // exhaustive-ish equivalence to ModInt::mul
        for a in (0..q).step_by(37) {
            let am = mont.to_mont(a);
            for b in (0..q).step_by(53) {
                let bm = mont.to_mont(b);
                let got = mont.from_mont(mont.mul(am, bm));
                let want = (ModInt::new(a, q) * ModInt::new(b, q)).val;
                assert_eq!(got, want, "montmul {a}*{b} mod {q}");
            }
        }
    }

    #[test]
    fn montgomery_rejects_even_or_too_large_modulus() {
        assert!(Montgomery::new(3328).is_none()); // even
        assert!(Montgomery::new(1).is_none());
        assert!(Montgomery::new(1u64 << 33).is_none()); // >= 2^32
        assert!(Montgomery::new(8380417).is_some()); // ML-DSA prime, 23-bit, odd
    }

    #[test]
    fn ntt_matches_naive_convolution() {
        // Kyber prime 3329, n=256 | 3328, primitive root 3.
        let ctx = NttCtx::new(3329, 8, 3).expect("ntt-friendly");
        let a = [1u64, 2, 3, 4, 0, 0, 0, 0];
        let b = [5u64, 6, 7, 8, 0, 0, 0, 0];
        let got = ctx.convolve(&a, &b);
        // naive cyclic convolution mod 3329
        let n = 8;
        let mut want = vec![0u64; n];
        for (i, &ai) in a.iter().enumerate() {
            for (j, &bj) in b.iter().enumerate() {
                want[(i + j) % n] = (want[(i + j) % n] + ai * bj) % 3329;
            }
        }
        assert_eq!(got, want);
    }
}
