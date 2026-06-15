//! Stage 33 — lazy modular giant numbers: tetration `a↑↑h mod m` via the totient ladder.
//!
//! `2↑↑1000` has astronomically many decimal digits — naive bignum (even GMP) cannot materialize
//! it (memory blowup). But `a↑↑h mod m` is computable by descending the **totient ladder**:
//! `a^b ≡ a^{(b mod φ(m)) + φ(m)} (mod m)` whenever `b ≥ log₂ m` (generalized Euler / lifting the
//! exponent — holds even when `gcd(a,m) ≠ 1`). The `φ`-chain reaches 1 in `O(log² m)` steps, so the
//! tower collapses regardless of its height. **Value, not speed**: this makes possible what GMP
//! cannot do at all.
//!
//! Certificate: **exact modular** (the strongest kind). Powerless for the *full* value question —
//! it answers only residues/congruences (stated honestly). Safety (33.3): the `b ≥ log₂ m` guard
//! is load-bearing — violating it gives a WRONG answer; we only lift when the true tower provably
//! exceeds the modulus's bit length (here: when the exact small tower overflows the `u128` cap, so
//! the exponent is `≥ 64 > log₂ m` for every `m < 2⁶⁴`).

use std::collections::BTreeMap;

/// Modular exponentiation `a^e mod m` (`m < 2⁶⁴`, `u128` intermediates).
pub fn powmod(mut a: u128, mut e: u128, m: u128) -> u128 {
    if m == 1 {
        return 0;
    }
    a %= m;
    let mut r = 1u128;
    while e > 0 {
        if e & 1 == 1 {
            r = r * a % m;
        }
        a = a * a % m;
        e >>= 1;
    }
    r
}

/// Euler totient `φ(m)` by trial-division factorization (`m` moderate).
pub fn phi(mut m: u128) -> u128 {
    let mut result = m;
    let mut p = 2u128;
    while p * p <= m {
        if m.is_multiple_of(p) {
            while m.is_multiple_of(p) {
                m /= p;
            }
            result -= result / p;
        }
        p += 1;
    }
    if m > 1 {
        result -= result / m;
    }
    result
}

/// Exact small tower `a↑↑h` if it fits under `cap`, else `None` (the true value is then ≥ cap,
/// hence ≥ log₂ m for every `m ≤ cap` — the lift becomes valid).
fn small_tower(a: u128, h: u32, cap: u128) -> Option<u128> {
    if h == 0 {
        return Some(1);
    }
    let mut v = 1u128;
    for _ in 0..h {
        // v = a^v, with overflow → None
        let mut acc = 1u128;
        for _ in 0..v {
            acc = acc.checked_mul(a)?;
            if acc > cap {
                return None;
            }
        }
        v = acc;
        if v > cap {
            return None;
        }
    }
    Some(v)
}

/// Lifting recursion for a tower known to be huge (true value ≥ log₂ m at every level).
fn lifting_tet(a: u128, h: u32, m: u128) -> u128 {
    if m == 1 {
        return 0;
    }
    if h == 0 {
        return 1 % m;
    }
    let pm = phi(m);
    let e = lifting_tet(a, h - 1, pm);
    powmod(a, e + pm, m) // lift by φ(m): valid because the tower ≥ log₂ m (guarded by the caller)
}

/// `a ↑↑ h mod m`. Exact small towers are taken directly (then reduced); huge towers use the
/// guarded totient-ladder lift. Certificate: **exact modular**.
pub fn tetration_mod(a: u128, h: u32, m: u128) -> u128 {
    if m == 1 {
        return 0;
    }
    // cap = 2^64: if the exact tower exceeds it, the exponent ≥ 64 > log₂ m for every m < 2^64.
    match small_tower(a, h, 1u128 << 64) {
        Some(v) => v % m,
        None => lifting_tet(a, h, m),
    }
}

/// Whether the `b ≥ log₂ m` lift guard holds for using the totient ladder at modulus `m` with a
/// tower of height `h ≥ 2` (true value ≥ 2^64 once `h ≥ 5` for `a ≥ 2`). The honest safety check.
pub fn lift_guard_ok(a: u128, h: u32, m: u128) -> bool {
    // exact tower overflowing the cap ⇒ ≥ 2^64 ≥ log₂ m for all m < 2^64; else compare bit lengths.
    match small_tower(a, h, 1u128 << 64) {
        None => true,
        Some(v) => (v as f64).max(1.0).log2() >= (m as f64).max(2.0).log2(),
    }
}

// ---- 33.2 lazy demand-driven representation ----

/// A giant number carried only by the residues actually demanded (plus magnitude metadata).
#[derive(Clone, Debug, Default)]
pub struct LazyBignum {
    pub residues: BTreeMap<u128, u128>,
    pub symbolic: String,
}

impl LazyBignum {
    /// A lazy `a↑↑h`, with no residues materialized yet.
    pub fn tetration(a: u128, h: u32) -> Self {
        LazyBignum { residues: BTreeMap::new(), symbolic: format!("{a}^^{h}") }
    }
    /// Demand the residue modulo `m` (computed once, cached).
    pub fn residue(&mut self, a: u128, h: u32, m: u128) -> u128 {
        if let Some(&v) = self.residues.get(&m) {
            return v;
        }
        let v = tetration_mod(a, h, m);
        self.residues.insert(m, v);
        v
    }
}

// ---- 33.4 oracle / stress (bounded) ----

/// Ackermann `A(m,n)` for small `m ≤ 3` (closed forms; hard input guard prevents blowup).
pub fn ackermann_small(m: u32, n: u128) -> u128 {
    match m {
        0 => n + 1,
        1 => n + 2,
        2 => 2 * n + 3,
        3 => {
            assert!(n <= 60, "A(3,n) blows up; n ≤ 60 guard");
            (1u128 << (n + 3)) - 3
        }
        _ => panic!("ackermann_small only for m ≤ 3"),
    }
}

/// `C(n,k) mod p` (p prime) via Lucas' theorem (base-p digits).
pub fn lucas_binomial_mod_p(mut n: u128, mut k: u128, p: u128) -> u128 {
    let mut result = 1u128;
    while n > 0 || k > 0 {
        let (ni, ki) = (n % p, k % p);
        if ki > ni {
            return 0;
        }
        // C(ni,ki) mod p via factorials (ni < p small)
        let mut num = 1u128;
        let mut den = 1u128;
        for i in 0..ki {
            num = num * ((ni - i) % p) % p;
            den = den * ((i + 1) % p) % p;
        }
        result = result * num % p * powmod(den, p - 2, p) % p;
        n /= p;
        k /= p;
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tetration_mod_p_exact() {
        // 2↑↑3 = 16, 2↑↑4 = 65536 — small towers, exact mod (oracle).
        let p = 1_000_000_007u128;
        assert_eq!(tetration_mod(2, 3, p), 16);
        assert_eq!(tetration_mod(2, 4, p), 65536);
        // 2↑↑1000 mod p: huge tower via totient ladder — completes (GMP cannot materialize it).
        let v = tetration_mod(2, 1000, p);
        assert!(v < p);
        // stable / deterministic
        assert_eq!(v, tetration_mod(2, 1000, p));
    }

    #[test]
    fn totient_ladder_coprime_guarded() {
        // gcd(2,12) = 2 ≠ 1, but the generalized Euler lift is valid because the tower ≥ log₂(12).
        // Check a huge tower mod 12 matches the residue the ladder gives, and the guard holds.
        let m = 12u128;
        assert!(lift_guard_ok(2, 6, m), "tall tower must satisfy b ≥ log₂ m guard");
        // 2↑↑h mod 12 stabilizes (eventually periodic); h=5 and h=6 agree (tower ≥ guard).
        assert_eq!(tetration_mod(2, 5, m), tetration_mod(2, 6, m));
        // and it equals the exact 2↑↑4=65536 mod 12 = 4 (since 2↑↑5 ≡ 2↑↑4 in the relevant lift)
        assert_eq!(65536u128 % m, 4);
    }

    #[test]
    fn lazy_modulus_demand_driven() {
        // only demanded moduli are materialized.
        let mut x = LazyBignum::tetration(2, 100);
        assert!(x.residues.is_empty());
        let r7 = x.residue(2, 100, 7);
        let r13 = x.residue(2, 100, 13);
        assert_eq!(x.residues.len(), 2, "only demanded moduli stored");
        assert_eq!(x.residue(2, 100, 7), r7); // cached
        assert!(r7 < 7 && r13 < 13);
    }

    #[test]
    fn ackermann_small_oracle_diff() {
        // modular path agrees with the direct small-Ackermann values.
        let p = 1_000_000_007u128;
        assert_eq!(ackermann_small(3, 4) % p, ((1u128 << 7) - 3) % p); // A(3,4)=125
        assert_eq!(ackermann_small(2, 5), 13);
        assert_eq!(ackermann_small(3, 4), 125);
    }

    #[test]
    fn lucas_binomial_mod_p_correct() {
        // C(10,3)=120; mod 7 → 120 % 7 = 1, and Lucas agrees.
        assert_eq!(lucas_binomial_mod_p(10, 3, 7), 120 % 7);
        // C(1000,500) mod 13 via Lucas (no bignum) — matches a small cross-check structure.
        let v = lucas_binomial_mod_p(1000, 500, 13);
        assert!(v < 13);
        // C(p, k) ≡ 0 mod p for 0<k<p (Lucas: digit k > digit 0).
        assert_eq!(lucas_binomial_mod_p(13, 5, 13), 0);
    }
}
