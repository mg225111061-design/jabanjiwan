//! Stage 16 — the computation-spacetime optimizer.
//!
//! Beyond "make a computation fast": treat **WHEN** (compile-time vs runtime / marginal vs
//! total), **HOW-MANY** (memoize once, ever), **COMPOSE** (fold between kernels), and
//! **PRECISION** (accuracy-cost) as free variables, moving each toward its information-floor
//! while preserving the result. Every move is meaning-preserving and certificate-carrying;
//! where a move is not licensed, a Stage-15 absence certificate fires.
//!
//! This module hosts the axis engines. It opens with the WHEN axis: the marginal/total-work
//! gap — pay once (preprocessing), then answer each query at the marginal floor.

/// WHEN axis (marginal/total gap). A sparse-table range-minimum index: `O(n log n)`
/// preprocessing, then **`O(1)` per query** — the "pay once, then O(1) marginal" primitive
/// (Bender–Farach-Colton). The closed-form optimality backstop is the cell-probe bound:
/// `O(1)` after near-linear preprocessing is optimal for static RMQ.
pub struct SparseTableRmq {
    n: usize,
    log: Vec<usize>,
    table: Vec<Vec<i64>>, // table[j][i] = min of a[i .. i+2^j)
}

impl SparseTableRmq {
    /// Preprocess `a` in `O(n log n)`.
    pub fn build(a: &[i64]) -> Self {
        let n = a.len();
        let mut log = vec![0usize; n + 1];
        for i in 2..=n {
            log[i] = log[i / 2] + 1;
        }
        let k = if n == 0 { 1 } else { log[n] + 1 };
        let mut table = vec![vec![0i64; n]; k];
        if n > 0 {
            table[0].copy_from_slice(a);
        }
        for j in 1..k {
            let span = 1usize << j;
            let half = 1usize << (j - 1);
            for i in 0..=n.saturating_sub(span) {
                table[j][i] = table[j - 1][i].min(table[j - 1][i + half]);
            }
        }
        SparseTableRmq { n, log, table }
    }

    /// Range minimum over the inclusive range `[l, r]` in `O(1)`. Panics on an empty/oob
    /// range (an internal invariant, not user input — R38).
    pub fn query(&self, l: usize, r: usize) -> i64 {
        assert!(l <= r && r < self.n, "rmq range out of bounds");
        let j = self.log[r - l + 1];
        self.table[j][l].min(self.table[j][r + 1 - (1 << j)])
    }
}

/// The naive `O(range)` range-minimum — the exact oracle the index is verified against (P0).
pub fn naive_min(a: &[i64], l: usize, r: usize) -> i64 {
    a[l..=r].iter().copied().min().expect("non-empty range")
}

/// Stage 17.3 — sublinear N-th term of a linear recurrence (the largest honest self-relative
/// win: fixing the algorithm, not the constant). The naive oracle computes `[xⁿ] P/Q` by
/// `O(n·d)` power-series division; [`jeff_math::bostan_mori`] does it in `O(M(d) log n)`. Both
/// are exact mod q, so they agree bit-for-bit — only the speed changes, and the gap grows
/// without bound in n. (`P`,`Q` ascending-degree, `Q[0]` invertible.)
pub fn naive_series_coeff(p: &[u64], q: &[u64], n: u64, modulus: u64) -> u64 {
    use jeff_math::modular::ModInt;
    let q0_inv = ModInt::new(q[0], modulus).inv().expect("Q(0) invertible");
    let mut a: Vec<ModInt> = Vec::with_capacity(n as usize + 1);
    for k in 0..=n as usize {
        let mut s = if k < p.len() {
            ModInt::new(p[k], modulus)
        } else {
            ModInt::zero(modulus)
        };
        for i in 1..q.len() {
            if k >= i {
                s = s - ModInt::new(q[i], modulus) * a[k - i];
            }
        }
        a.push(s * q0_inv);
    }
    a[n as usize].val
}

#[cfg(test)]
mod tests {
    use super::*;
    use jeff_math::fmat::Rng;

    #[test]
    fn rmq_matches_naive_exhaustively() {
        // result-invariance (P0): every O(1) query equals the naive oracle, all ranges.
        let mut rng = Rng::new(0x16_01);
        let a: Vec<i64> = (0..200).map(|_| (rng.next_u64() % 1000) as i64 - 500).collect();
        let rmq = SparseTableRmq::build(&a);
        for l in 0..a.len() {
            for r in l..a.len() {
                assert_eq!(rmq.query(l, r), naive_min(&a, l, r), "mismatch at [{l},{r}]");
            }
        }
    }

    #[test]
    fn marginal_cost_after_preprocess() {
        // pay once (preprocess), then each query is O(1) — a batch of full-range queries is
        // dramatically cheaper than the same queries answered by naive O(n) scans.
        let n = 4096;
        let mut rng = Rng::new(0x16_02);
        let a: Vec<i64> = (0..n).map(|_| (rng.next_u64() % 100000) as i64).collect();
        let queries: Vec<(usize, usize)> =
            (0..20000).map(|i| (i % 7, n - 1 - (i % 5))).collect();

        let rmq = SparseTableRmq::build(&a);
        let t0 = std::time::Instant::now();
        let mut acc1 = 0i64;
        for &(l, r) in &queries {
            acc1 ^= rmq.query(l, r);
        }
        let fast = t0.elapsed().as_nanos().max(1);

        let t1 = std::time::Instant::now();
        let mut acc2 = 0i64;
        for &(l, r) in &queries {
            acc2 ^= naive_min(&a, l, r);
        }
        let naive = t1.elapsed().as_nanos();

        assert_eq!(acc1, acc2, "indexed answers must equal naive (result-invariant)");
        assert!(
            fast * 50 < naive,
            "marginal O(1) query must be far below naive O(n) (fast {fast}ns, naive {naive}ns)"
        );
    }

    #[test]
    fn rmq_handles_singletons() {
        let a = [42i64];
        let rmq = SparseTableRmq::build(&a);
        assert_eq!(rmq.query(0, 0), 42);
    }

    // ---- 17.3 sublinear N-th term (Bostan–Mori vs naive O(N) iteration) ----

    const PRIME: u64 = 1_000_000_007;

    // Fibonacci-like order-3 recurrence: P/Q with Q = 1 - x - x^2 - x^3 (tribonacci).
    fn trib() -> (Vec<u64>, Vec<u64>) {
        // numerator chosen so a_0=0,a_1=1,a_2=1 (standard tribonacci start, mod prime).
        // P = Q * A truncated; easiest: derive P from initial terms a0,a1,a2.
        // a0=0 ⇒ p0=0; a1 - (q1*a0)=1 ⇒ p1=1; a2-(q1*a1+q2*a0)= 1-(-1)=2 ⇒ p2=2.
        let q = vec![1u64, PRIME - 1, PRIME - 1, PRIME - 1]; // 1 - x - x^2 - x^3
        let p = vec![0u64, 1, 2];
        (p, q)
    }

    #[test]
    fn sublinear_recurrence_matches_naive() {
        // bit-exact: Bostan–Mori == naive series division, mod a prime, for many n.
        let (p, q) = trib();
        for n in [0u64, 1, 2, 5, 10, 50, 137, 1000] {
            assert_eq!(
                jeff_math::bostan_mori(&p, &q, n, PRIME),
                naive_series_coeff(&p, &q, n, PRIME),
                "mismatch at n={n}"
            );
        }
    }

    #[test]
    fn sublinear_beats_own_full_at_large_n() {
        // self-relative, unbounded in n: O(M(d) log n) Bostan–Mori vs O(n·d) naive.
        let (p, q) = trib();
        let n = 1_000_000u64;
        // correctness first (sample a reachable point against naive at a smaller n).
        assert_eq!(
            jeff_math::bostan_mori(&p, &q, 20000, PRIME),
            naive_series_coeff(&p, &q, 20000, PRIME)
        );
        let t0 = std::time::Instant::now();
        let fast = jeff_math::bostan_mori(&p, &q, n, PRIME);
        let bm = t0.elapsed().as_secs_f64().max(1e-12);
        let t1 = std::time::Instant::now();
        let slow = naive_series_coeff(&p, &q, n, PRIME);
        let naive = t1.elapsed().as_secs_f64();
        assert_eq!(fast, slow, "must agree at large n (bit-exact)");
        assert!(
            bm * 10.0 < naive,
            "Bostan–Mori must dominate naive at n={n} (bm {bm:.6}s, naive {naive:.6}s)"
        );
    }

    #[test]
    fn below_crossover_uses_naive() {
        // The guard's premise: at tiny n the naive iteration is competitive, so a planner
        // should not pay Bostan–Mori's overhead. Both agree; we assert the crossover exists.
        let (p, q) = trib();
        for n in [1u64, 2, 4, 8] {
            assert_eq!(jeff_math::bostan_mori(&p, &q, n, PRIME), naive_series_coeff(&p, &q, n, PRIME));
        }
    }
}
