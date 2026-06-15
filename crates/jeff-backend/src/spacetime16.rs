//! Stage 21 — Stage-16 axes not yet built: compile-time total evaluation (the WHEN axis's
//! "past" / Futamura specialization) and **compressed computation** (cost ∝ compressed size,
//! one of the floor's five open doors). The other axes (marginal-cost RMQ, certificate-keyed
//! memoization, fold composition, accuracy-cost Pareto, the unified four-axis verdict) were
//! built earlier; this completes the set. Everything bit-exact (P0); speedups self-relative.

// ---- WHEN axis: compile-time total evaluation (Futamura) ----

/// A computation whose inputs are fully known is evaluated **once** (at "specialization /
/// compile time") and replaced by its constant result — the runtime cost of that part is
/// zero (it is a stored value, not a recomputation). `verify` re-runs the general computation
/// and confirms the specialized constant matches it (verified specialization, P0).
pub struct Specialized<T> {
    value: T,
}

impl<T: Clone + PartialEq> Specialized<T> {
    /// Specialize: run the general computation on its known inputs once.
    pub fn specialize<F: Fn() -> T>(general: F) -> Self {
        Specialized { value: general() }
    }
    /// Runtime access: O(1), returns the precomputed constant (no recomputation).
    pub fn get(&self) -> &T {
        &self.value
    }
    /// Verify the specialized constant equals re-running the general computation.
    pub fn verify<F: Fn() -> T>(&self, general: F) -> bool {
        self.value == general()
    }
}

// ---- compressed computation: total work ∝ compressed size, not N ----

/// A run-length-encoded integer sequence. Computing on it directly costs `O(#runs)` — the
/// compressed size — not `O(N)`, while producing the bit-exact answer of the decompressed
/// computation (one of the five doors the Ω(N) floor leaves open: work ∝ compressed size).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rle {
    /// `(value, run_length)` pairs.
    pub runs: Vec<(i64, usize)>,
}

impl Rle {
    /// Compress a raw sequence (the only `O(N)` step — done once / at ingest).
    pub fn compress(data: &[i64]) -> Rle {
        let mut runs = Vec::new();
        for &x in data {
            match runs.last_mut() {
                Some((v, c)) if *v == x => *c += 1,
                _ => runs.push((x, 1)),
            }
        }
        Rle { runs }
    }

    /// Decompress (for the oracle / when an engine provably needs raw data).
    pub fn decompress(&self) -> Vec<i64> {
        let mut out = Vec::new();
        for &(v, c) in &self.runs {
            out.extend(std::iter::repeat_n(v, c));
        }
        out
    }

    /// Logical length `N`.
    pub fn len(&self) -> usize {
        self.runs.iter().map(|&(_, c)| c).sum()
    }
    pub fn is_empty(&self) -> bool {
        self.runs.is_empty()
    }
    /// Compressed size (#runs) — the cost parameter.
    pub fn compressed_size(&self) -> usize {
        self.runs.len()
    }

    /// Sum, computed on the compressed form in `O(#runs)` (bit-exact vs the raw sum).
    pub fn sum(&self) -> i64 {
        self.runs.iter().map(|&(v, c)| v * c as i64).sum()
    }

    /// Inner product of two RLE sequences of equal logical length, in `O(#runs_a + #runs_b)`
    /// by a run merge (bit-exact vs the decompressed dot product).
    pub fn dot(&self, other: &Rle) -> Option<i64> {
        if self.len() != other.len() {
            return None;
        }
        let (mut i, mut j) = (0usize, 0usize);
        let (mut ra, mut rb) = (0usize, 0usize); // consumed within current run
        let mut acc = 0i64;
        while i < self.runs.len() && j < other.runs.len() {
            let (va, ca) = self.runs[i];
            let (vb, cb) = other.runs[j];
            let take = (ca - ra).min(cb - rb);
            acc += va * vb * take as i64;
            ra += take;
            rb += take;
            if ra == ca {
                i += 1;
                ra = 0;
            }
            if rb == cb {
                j += 1;
                rb = 0;
            }
        }
        Some(acc)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ctfe_zero_runtime_verified() {
        // a fully-known power sum is evaluated once at "compile time"; the runtime value is a
        // stored constant that matches the general loop (verified specialization).
        let n = 10_000u64;
        let general = || (0..=n).map(|i| i * i).sum::<u64>();
        let spec = Specialized::specialize(general);
        assert_eq!(*spec.get(), n * (n + 1) * (2 * n + 1) / 6); // closed form
        assert!(spec.verify(general), "specialized constant must equal the general computation");
    }

    #[test]
    fn compressed_compute_bit_exact() {
        // computing on the compressed form equals computing on the raw data (P0).
        let raw: Vec<i64> = [vec![7i64; 500], vec![3; 300], vec![-2; 200], vec![7; 100]].concat();
        let rle = Rle::compress(&raw);
        assert_eq!(rle.decompress(), raw);
        assert_eq!(rle.sum(), raw.iter().sum::<i64>());
        let raw2: Vec<i64> = [vec![2i64; 400], vec![5; 700]].concat();
        let rle2 = Rle::compress(&raw2);
        let naive_dot: i64 = raw.iter().zip(&raw2).map(|(a, b)| a * b).sum();
        assert_eq!(rle.dot(&rle2).unwrap(), naive_dot);
    }

    #[test]
    fn compressed_compute_cost_scales_with_compressed_size() {
        // self-relative: at large N with few runs, the compressed sum touches O(#runs), not
        // O(N) — far cheaper than the raw sum, same answer.
        let n = 4_000_000usize;
        let raw: Vec<i64> = (0..n).map(|i| if i < n / 2 { 9 } else { -4 }).collect(); // 2 runs
        let rle = Rle::compress(&raw);
        assert!(rle.compressed_size() <= 2);
        let t0 = std::time::Instant::now();
        let cs = rle.sum();
        let comp = t0.elapsed().as_secs_f64().max(1e-12);
        let t1 = std::time::Instant::now();
        let rs: i64 = raw.iter().sum();
        let rawt = t1.elapsed().as_secs_f64();
        assert_eq!(cs, rs, "compressed sum must equal raw sum (bit-exact)");
        assert!(
            comp * 100.0 < rawt,
            "compressed O(#runs) sum must be ≫ cheaper than O(N) (comp {comp:e}s, raw {rawt:e}s)"
        );
    }
}
