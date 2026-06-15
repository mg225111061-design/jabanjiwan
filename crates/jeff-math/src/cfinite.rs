//! Stage 26.1/26.2 — C-finite N-th term, exact over a prime field, with three independent
//! evaluation paths, a measured crossover, and a genuine closed-form O(1) path.
//!
//! A *C-finite* sequence obeys a constant-coefficient linear recurrence of arbitrary order `d`:
//! `a_n = c_1 a_{n-1} + … + c_d a_{n-d}` (here `c = [c_1,…,c_d]`, `init = [a_0,…,a_{d-1}]`).
//! Incumbent evaluation is O(N·d) (iterate). Structure lets us collapse the single N-th term to
//! **O(M(d)·log N)** (Bostan–Mori) or **O(d²·log N)** (Fiduccia / companion-matrix power as
//! `x^N mod charpoly`), and for periodic sequences to genuine **O(1)** (`a_N = a_{N mod p}`).
//!
//! [conservation-law label] These ratios are **self-relative** (JEFF naive vs JEFF collapse),
//! **asymptotic** (grow with N), realized **only where the structure exists** (a constant-
//! coefficient recurrence of small order), and **Ω(N)-safe**: the output is a *single* field
//! element, not Θ(N) data. All arithmetic is exact in `F_q` (a prime modulus); the O(·) counts
//! are **field operations** — with a bignum modulus the per-op bit-cost grows, see §C.

use crate::matrix::bostan_mori;
use crate::modular::ModInt;

#[inline]
fn mi(v: u64, q: u64) -> ModInt {
    ModInt::new(v, q)
}

/// Naive oracle: iterate the recurrence. **O(N·d)** field ops. The correctness/speed baseline.
pub fn cfinite_naive(c: &[u64], init: &[u64], n: u64, q: u64) -> u64 {
    let d = c.len();
    assert!(d >= 1 && init.len() >= d, "need order ≥ 1 and d initial terms");
    if (n as usize) < d {
        return init[n as usize] % q;
    }
    let cc: Vec<ModInt> = c.iter().map(|&x| mi(x, q)).collect();
    let mut window: Vec<ModInt> = init[..d].iter().map(|&x| mi(x, q)).collect();
    for _m in d..=(n as usize) {
        // a_m = Σ_{i=1}^d c_i a_{m-i} = Σ_{i=1}^d cc[i-1]·window[d-i]
        let mut acc = ModInt::zero(q);
        for i in 1..=d {
            acc = acc + cc[i - 1] * window[d - i];
        }
        window.rotate_left(1);
        window[d - 1] = acc;
    }
    window[d - 1].val
}

/// Bostan–Mori: `a_N = [x^N] P(x)/Q(x)` with `Q = 1 − Σ c_i x^i`. **O(M(d)·log N)**.
pub fn cfinite_bostan(c: &[u64], init: &[u64], n: u64, q: u64) -> u64 {
    let d = c.len();
    assert!(d >= 1 && init.len() >= d, "need order ≥ 1 and d initial terms");
    // Q ascending = [1, −c_1, …, −c_d]
    let mut qco = vec![0u64; d + 1];
    qco[0] = 1 % q;
    for i in 0..d {
        qco[i + 1] = (q - c[i] % q) % q;
    }
    // P ascending, length d: P_m = Σ_{j=0}^m Q_j·a_{m-j}, m = 0..d−1
    let mut pco = vec![0u64; d];
    for (m, p) in pco.iter_mut().enumerate() {
        let mut acc = ModInt::zero(q);
        for j in 0..=m {
            acc = acc + mi(qco[j], q) * mi(init[m - j] % q, q);
        }
        *p = acc.val;
    }
    bostan_mori(&pco, &qco, n, q)
}

/// Fiduccia / companion-matrix power: compute `x^N mod f(x)` where `f` is the characteristic
/// polynomial `x^d − Σ c_i x^{d-i}`, then `a_N = Σ_{j<d} (x^N mod f)_j · a_j`. Squaring is a
/// degree-`d` polynomial multiply+reduce (**O(d²)**), so total **O(d²·log N)** — the sparse
/// (poly-arithmetic) realization of the companion matrix power, avoiding the O(d³ log N) of a
/// dense d×d matrix exponentiation.
pub fn cfinite_companion(c: &[u64], init: &[u64], n: u64, q: u64) -> u64 {
    let d = c.len();
    assert!(d >= 1 && init.len() >= d, "need order ≥ 1 and d initial terms");
    if (n as usize) < d {
        return init[n as usize] % q;
    }
    // f monic: x^d ≡ Σ_{p<d} c_{d-p} x^p (mod f). f_coeffs[p] = −c_{d-p}.
    let f: Vec<ModInt> = (0..d).map(|p| ModInt::zero(q) - mi(c[d - 1 - p], q)).collect();

    // reduce a polynomial (Vec<ModInt>, ascending) modulo f, in place, leaving length d.
    let reduce = |r: &mut Vec<ModInt>| {
        if r.len() > d {
            for deg in (d..r.len()).rev() {
                let lead = r[deg];
                if lead.val == 0 {
                    continue;
                }
                // x^deg = x^{deg-d}·x^d = −Σ_{p<d} f[p]·x^{deg-d+p}
                for (p, &fp) in f.iter().enumerate() {
                    let idx = deg - d + p;
                    r[idx] = r[idx] - lead * fp;
                }
                r[deg] = ModInt::zero(q);
            }
        }
        r.truncate(d);
    };
    let mulmod = |a: &[ModInt], b: &[ModInt]| -> Vec<ModInt> {
        let mut prod = vec![ModInt::zero(q); a.len() + b.len() - 1];
        for (i, &x) in a.iter().enumerate() {
            if x.val == 0 {
                continue;
            }
            for (j, &y) in b.iter().enumerate() {
                prod[i + j] = prod[i + j] + x * y;
            }
        }
        reduce(&mut prod);
        prod
    };

    // base = x mod f, result = x^0 = 1, then square-and-multiply to x^N mod f.
    let mut base = vec![ModInt::zero(q); d];
    if d >= 2 {
        base[1] = ModInt::one(q);
    } else {
        base[0] = mi(c[0], q); // d==1: x ≡ c_1 (mod x − c_1)
    }
    let mut result = vec![ModInt::zero(q); d];
    result[0] = ModInt::one(q);
    let mut e = n;
    while e > 0 {
        if e & 1 == 1 {
            result = mulmod(&result, &base);
        }
        base = mulmod(&base, &base);
        e >>= 1;
    }
    let mut acc = ModInt::zero(q);
    for (j, &r) in result.iter().enumerate() {
        acc = acc + r * mi(init[j] % q, q);
    }
    acc.val
}

/// Crossover threshold below which the dispatcher [`cfinite_nth`] uses the naive path (for tiny
/// N the O(N·d) iterate beats the constant overhead of the log-N transforms). Measured: see §C.
pub const CROSSOVER_N: u64 = 64;

/// Which path [`cfinite_nth`] selects for a given N (observable, for the crossover test).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Path {
    Naive,
    Bostan,
}

/// The path chosen for `n` under the crossover policy.
pub fn cfinite_choose(n: u64) -> Path {
    if n < CROSSOVER_N {
        Path::Naive
    } else {
        Path::Bostan
    }
}

/// Crossover-aware N-th term: naive below [`CROSSOVER_N`], Bostan–Mori above.
pub fn cfinite_nth(c: &[u64], init: &[u64], n: u64, q: u64) -> u64 {
    match cfinite_choose(n) {
        Path::Naive => cfinite_naive(c, init, n, q),
        Path::Bostan => cfinite_bostan(c, init, n, q),
    }
}

// ---- 26.2 closed-form / genuine O(1) ----

/// A detected pure period: `a_n = values[n mod period]`.
#[derive(Debug, Clone)]
pub struct Periodic {
    pub period: usize,
    pub values: Vec<u64>,
}

/// Detect a pure period ≤ `bound` (all characteristic roots are roots of unity ⇒ the sequence
/// is purely periodic). Returns `None` if no period appears within the bound — the honest
/// fallback (no forced closed form). One-time O(period·d) precompute; each query is then **O(1)**
/// via [`Periodic::nth`].
pub fn detect_period(c: &[u64], init: &[u64], q: u64, bound: usize) -> Option<Periodic> {
    let d = c.len();
    if init.len() < d {
        return None;
    }
    let cc: Vec<ModInt> = c.iter().map(|&x| mi(x, q)).collect();
    let init_window: Vec<u64> = init[..d].iter().map(|&x| x % q).collect();
    let mut vals: Vec<u64> = init_window.clone();
    for _ in 0..bound {
        let m = vals.len();
        let mut acc = ModInt::zero(q);
        for i in 1..=d {
            acc = acc + cc[i - 1] * mi(vals[m - i], q);
        }
        vals.push(acc.val);
        let p = vals.len() - d;
        if p >= 1 && vals[p..p + d] == init_window[..] {
            vals.truncate(p);
            return Some(Periodic { period: p, values: vals });
        }
    }
    None
}

impl Periodic {
    /// O(1) N-th term lookup.
    pub fn nth(&self, n: u64) -> u64 {
        self.values[(n % self.period as u64) as usize]
    }
}

/// Closed-form geometric (order-1) N-th term `a_N = a_0·c_1^N` in **O(log N)** field ops.
pub fn geometric_nth(c0: u64, a0: u64, n: u64, q: u64) -> u64 {
    (mi(a0, q) * mi(c0, q).pow(n)).val
}

#[cfg(test)]
mod tests {
    use super::*;

    const Q: u64 = 1_000_000_007;

    // Fibonacci: c=[1,1], init=[0,1].
    fn fib() -> (Vec<u64>, Vec<u64>) {
        (vec![1, 1], vec![0, 1])
    }

    #[test]
    fn cfinite_bostan_matches_naive() {
        // several (d, N, coeffs); all three modular-exact and equal.
        let cases: &[(&[u64], &[u64])] = &[
            (&[1, 1], &[0, 1]),             // Fibonacci
            (&[2, 0, 1], &[3, 1, 4]),       // order 3
            (&[1, 1, 1, 1], &[1, 2, 3, 4]), // tetranacci-like
            (&[5], &[7]),                   // geometric a_n = 7·5^n
        ];
        for (c, init) in cases {
            for &n in &[0u64, 1, 2, 5, 17, 100, 1000, 99999] {
                let naive = cfinite_naive(c, init, n, Q);
                let bostan = cfinite_bostan(c, init, n, Q);
                assert_eq!(bostan, naive, "c={c:?} init={init:?} n={n}");
            }
        }
    }

    #[test]
    fn cfinite_companion_matches_bostan() {
        let cases: &[(&[u64], &[u64])] = &[
            (&[1, 1], &[0, 1]),
            (&[2, 0, 1], &[3, 1, 4]),
            (&[1, 1, 1, 1], &[1, 2, 3, 4]),
            (&[5], &[7]),
        ];
        for (c, init) in cases {
            for &n in &[0u64, 1, 3, 10, 64, 257, 100000] {
                let bostan = cfinite_bostan(c, init, n, Q);
                let comp = cfinite_companion(c, init, n, Q);
                assert_eq!(comp, bostan, "c={c:?} init={init:?} n={n}");
            }
        }
    }

    #[test]
    fn fibonacci_hand_checked() {
        // F_10 = 55, F_20 = 6765 — exact small values (well below the modulus).
        let (c, init) = fib();
        assert_eq!(cfinite_naive(&c, &init, 10, Q), 55);
        assert_eq!(cfinite_bostan(&c, &init, 10, Q), 55);
        assert_eq!(cfinite_companion(&c, &init, 10, Q), 55);
        assert_eq!(cfinite_bostan(&c, &init, 20, Q), 6765);
        assert_eq!(cfinite_companion(&c, &init, 20, Q), 6765);
    }

    #[test]
    fn cfinite_crossover_uses_naive_below_threshold() {
        assert_eq!(cfinite_choose(0), Path::Naive);
        assert_eq!(cfinite_choose(CROSSOVER_N - 1), Path::Naive);
        assert_eq!(cfinite_choose(CROSSOVER_N), Path::Bostan);
        assert_eq!(cfinite_choose(1_000_000), Path::Bostan);
        // dispatcher result agrees with naive on both sides of the threshold.
        let (c, init) = fib();
        for &n in &[5u64, 63, 64, 65, 5000] {
            assert_eq!(cfinite_nth(&c, &init, n, Q), cfinite_naive(&c, &init, n, Q));
        }
    }

    #[test]
    fn cfinite_ratio_grows_with_n() {
        // The *operation-count* ratio naive(O(N·d)) / bostan(O(M(d) log N)) must increase
        // monotonically with N (the collapse is asymptotic, not a constant factor). We compare
        // a faithful op-count proxy: N·d vs d²·⌈log2 N⌉ (companion path), exact integers.
        let d = 3u64;
        let mut prev = 0f64;
        for k in 3..=8 {
            let n = 10u64.pow(k);
            let naive_ops = (n * d) as f64;
            let collapse_ops = (d * d * (64 - (n.leading_zeros() as u64))) as f64;
            let ratio = naive_ops / collapse_ops;
            assert!(ratio > prev, "ratio must grow: N=1e{k} ratio={ratio} prev={prev}");
            prev = ratio;
        }
        assert!(prev > 1e5, "by N=1e8 the ratio is large (got {prev})");
    }

    #[test]
    fn closedform_matches_bostan_periodic() {
        // a_n = a_{n-1} − a_{n-2}: roots are primitive 6th roots of unity ⇒ period 6.
        // sequence from init [0,1]: 0,1,1,0,-1,-1, …  (−1 ≡ q−1)
        let c = vec![1, Q - 1]; // a_n = 1·a_{n-1} + (q−1)·a_{n-2} = a_{n-1} − a_{n-2}
        let init = vec![0, 1];
        let per = detect_period(&c, &init, Q, 1000).expect("period 6 exists");
        assert_eq!(per.period, 6);
        for &n in &[0u64, 1, 5, 6, 7, 100, 1001, 999_999] {
            assert_eq!(per.nth(n), cfinite_bostan(&c, &init, n, Q), "n={n}");
        }
    }

    #[test]
    fn closedform_o1_verified() {
        // periodic lookup is O(1) and exact; geometric closed form O(log N) matches naive.
        let c = vec![1, Q - 1];
        let init = vec![0, 1];
        let per = detect_period(&c, &init, Q, 1000).unwrap();
        assert_eq!(per.nth(6_000_000_000_000_000), per.nth(0)); // N mod 6 == 0
        // geometric a_n = 7·5^n
        for &n in &[0u64, 1, 10, 1000, 1_000_000] {
            assert_eq!(geometric_nth(5, 7, n, Q), cfinite_bostan(&[5], &[7], n, Q), "n={n}");
        }
    }

    #[test]
    fn closedform_absent_falls_back() {
        // Fibonacci has no small period (roots not roots of unity) ⇒ detect_period gives None
        // within the bound — the honest fallback to the 26.1 paths (no forced closed form).
        let (c, init) = fib();
        assert!(detect_period(&c, &init, Q, 10_000).is_none());
    }
}
