//! Stage-6A Batch-5: streaming / sublinear sketches (CLAUDE.md PART B Batch 5).
//!
//! These trade SPACE (sublinear) for an ε-approximate estimate. Honest framing (PART A):
//! the streaming value is small space, and on most end-to-end pipelines the time fraction
//! is MODEST — these earn their place by pattern-coverage breadth, not per-pipeline
//! speedup. Every certificate is checked against the **exact** oracle (which the checker
//! can compute from the data): the sketch estimate must be within its stated error bound.
//! The exact answer still costs Ω(N) (sum/parity is the canonical non-approximable case).

#![allow(clippy::needless_range_loop)]

/// A 2-independent-ish hash: splitmix64 of `x` salted by `seed`.
fn mix(x: u64, seed: u64) -> u64 {
    let mut z = x.wrapping_add(seed).wrapping_add(0x9E3779B97F4A7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
    z ^ (z >> 31)
}

// ---- 5.1 AMS F₂ sketch ----

/// Exact second frequency moment `F₂ = Σ_i f_i²` (the oracle).
pub fn exact_f2(items: &[u64]) -> u64 {
    use std::collections::HashMap;
    let mut f: HashMap<u64, u64> = HashMap::new();
    for &x in items {
        *f.entry(x).or_insert(0) += 1;
    }
    f.values().map(|&c| c * c).sum()
}

/// AMS F₂ estimate via median-of-means of `reps` `±1`-sketch estimators (Tug-of-War).
pub fn ams_f2(items: &[u64], reps: usize) -> f64 {
    let mut ests: Vec<f64> = Vec::with_capacity(reps);
    for r in 0..reps {
        let z: i64 = items
            .iter()
            .map(|&x| if mix(x, 0xA115 + r as u64) & 1 == 0 { 1i64 } else { -1 })
            .sum();
        ests.push((z * z) as f64);
    }
    ests.sort_by(|a, b| a.partial_cmp(b).unwrap());
    ests[reps / 2]
}

// ---- 5.2 Count-Min sketch ----

/// Exact frequency of `key` (the oracle).
pub fn exact_freq(items: &[u64], key: u64) -> u64 {
    items.iter().filter(|&&x| x == key).count() as u64
}

/// Count-Min point estimate of `key`'s frequency with width `w`, depth `d`.
pub fn count_min(items: &[u64], key: u64, w: usize, d: usize) -> u64 {
    let mut table = vec![vec![0u64; w]; d];
    for &x in items {
        for (row, t) in table.iter_mut().enumerate() {
            let h = (mix(x, 0xC114 + row as u64) % w as u64) as usize;
            t[h] += 1;
        }
    }
    (0..d)
        .map(|row| {
            let h = (mix(key, 0xC114 + row as u64) % w as u64) as usize;
            table[row][h]
        })
        .min()
        .unwrap_or(0)
}

// ---- 5.3 HyperLogLog / distinct count ----

/// Exact distinct-element count (the oracle).
pub fn exact_distinct(items: &[u64]) -> u64 {
    use std::collections::HashSet;
    items.iter().copied().collect::<HashSet<_>>().len() as u64
}

/// HyperLogLog distinct-count estimate with `2^b` registers.
pub fn hyperloglog(items: &[u64], b: u32) -> f64 {
    let m = 1usize << b;
    let mut reg = vec![0u32; m];
    for &x in items {
        let h = mix(x, 0x4717);
        let idx = (h >> (64 - b)) as usize;
        let rest = (h << b) | (1 << (b - 1)); // ensure nonzero tail
        let rho = rest.leading_zeros() + 1;
        if rho > reg[idx] {
            reg[idx] = rho;
        }
    }
    let alpha = 0.7213 / (1.0 + 1.079 / m as f64);
    let sum: f64 = reg.iter().map(|&r| 2f64.powi(-(r as i32))).sum();
    let raw = alpha * (m * m) as f64 / sum;
    // small-range correction
    if raw <= 2.5 * m as f64 {
        let zeros = reg.iter().filter(|&&r| r == 0).count();
        if zeros != 0 {
            return m as f64 * (m as f64 / zeros as f64).ln();
        }
    }
    raw
}

// ---- 5.4 Misra–Gries heavy hitters ----

/// Misra–Gries candidate heavy hitters: items that may exceed frequency `n/(k+1)`.
pub fn misra_gries(items: &[u64], k: usize) -> Vec<u64> {
    use std::collections::HashMap;
    let mut counters: HashMap<u64, i64> = HashMap::new();
    for &x in items {
        if counters.contains_key(&x) {
            *counters.get_mut(&x).unwrap() += 1;
        } else if counters.len() < k {
            counters.insert(x, 1);
        } else {
            counters.retain(|_, c| {
                *c -= 1;
                *c > 0
            });
        }
    }
    let mut out: Vec<u64> = counters.keys().copied().collect();
    out.sort_unstable();
    out
}

// ---- 5.5 sublinear mean estimation ----

/// Exact mean (the oracle).
pub fn exact_mean(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.iter().sum::<f64>() / values.len() as f64
}

/// Sublinear mean estimate from `s` **random** samples (Hoeffding needs iid samples;
/// a seeded PRNG keeps it reproducible, R11). Additive error `λ` with confidence
/// `1−2exp(−2sλ²)`.
pub fn sublinear_mean(values: &[f64], s: usize, seed: u64) -> f64 {
    if values.is_empty() || s == 0 {
        return 0.0;
    }
    let n = values.len() as u64;
    let mut acc = 0.0;
    for r in 0..s {
        let idx = (mix(r as u64, seed) % n) as usize;
        acc += values[idx];
    }
    acc / s as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    fn skewed_stream() -> Vec<u64> {
        // a heavy hitter (7) plus a light tail.
        let mut v = vec![7u64; 500];
        for i in 0..200 {
            v.push((i % 50) as u64 + 100);
        }
        v
    }

    #[test]
    fn ams_f2_close_to_exact() {
        let s = skewed_stream();
        let exact = exact_f2(&s) as f64;
        let est = ams_f2(&s, 51);
        assert!((est - exact).abs() <= 0.5 * exact, "AMS F₂ est {est} vs exact {exact}");
    }

    #[test]
    fn count_min_overestimates_within_bound() {
        let s = skewed_stream();
        let eps = 0.01_f64;
        let w = (2.0 / eps).ceil() as usize;
        let est = count_min(&s, 7, w, 5);
        let truth = exact_freq(&s, 7);
        assert!(est >= truth, "CM never underestimates");
        assert!(est as f64 <= truth as f64 + eps * s.len() as f64, "CM within ε‖f‖₁");
    }

    #[test]
    fn hll_estimates_distinct() {
        // 5000 distinct items.
        let s: Vec<u64> = (0..5000).collect();
        let est = hyperloglog(&s, 10); // 1024 registers ⇒ ~3% error
        let truth = exact_distinct(&s) as f64;
        assert!((est - truth).abs() < 0.1 * truth, "HLL est {est} vs {truth}");
    }

    #[test]
    fn misra_gries_finds_heavy_hitter() {
        let s = skewed_stream();
        let hh = misra_gries(&s, 8);
        assert!(hh.contains(&7), "heavy hitter 7 must survive");
    }

    #[test]
    fn sublinear_mean_close() {
        let v: Vec<f64> = (0..10000).map(|i| (i % 10) as f64).collect();
        let est = sublinear_mean(&v, 1000, 0xBEEF);
        let truth = exact_mean(&v);
        assert!((est - truth).abs() < 0.5, "mean est {est} vs {truth}");
    }
}
