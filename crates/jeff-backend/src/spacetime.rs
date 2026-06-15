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
}
