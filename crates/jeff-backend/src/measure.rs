//! Honest measurement harness (CLAUDE.md PART C, R8/DR2). Nothing here hardcodes a
//! speedup; every number comes from a runtime measurement. Amdahl's law turns a kernel
//! speedup into an end-to-end speedup given the measured runtime fraction `p`.

use std::time::{Duration, Instant};

/// A simple wall-clock timer over `reps` repetitions (best-of to reduce noise).
pub struct Timer;

impl Timer {
    /// Run `f` `reps` times and return the **minimum** elapsed time (least-noisy sample).
    pub fn best_of<F: FnMut()>(reps: usize, mut f: F) -> Duration {
        let mut best = Duration::MAX;
        for _ in 0..reps.max(1) {
            let t = Instant::now();
            f();
            let e = t.elapsed();
            if e < best {
                best = e;
            }
        }
        best
    }
}

/// End-to-end speedup under Amdahl's law: a kernel that occupies fraction `p` of runtime
/// and is sped up by `kernel_speedup×` gives overall `1 / ((1−p) + p/kernel_speedup)`.
/// This is the honest cap — even `kernel_speedup → ∞` yields only `1/(1−p)`.
pub fn amdahl_speedup(p: f64, kernel_speedup: f64) -> f64 {
    let p = p.clamp(0.0, 1.0);
    let k = kernel_speedup.max(1e-12);
    1.0 / ((1.0 - p) + p / k)
}

/// Result of comparing a fast path to a baseline on the same input.
#[derive(Clone, Debug)]
pub struct BenchResult {
    /// Measured fast-path time.
    pub fast: Duration,
    /// Measured baseline time (an in-house naive oracle unless stated otherwise).
    pub baseline: Duration,
    /// What the baseline actually is (honesty, DR2): e.g. "in-house naive DFT".
    pub baseline_name: String,
}

impl BenchResult {
    /// Measured speedup `baseline / fast` (a ratio of *measured* times only).
    pub fn speedup(&self) -> f64 {
        let f = self.fast.as_secs_f64().max(1e-12);
        self.baseline.as_secs_f64() / f
    }

    /// One-line honest report (no claim beyond the measurement).
    pub fn report(&self, kernel: &str) -> String {
        format!(
            "{kernel}: {:.3}× vs {} (fast {:?}, baseline {:?}) [measured, this machine, single input]",
            self.speedup(),
            self.baseline_name,
            self.fast,
            self.baseline
        )
    }
}

/// Compare two closures on the same input. `baseline_name` MUST describe what the
/// baseline really is — we never label an in-house naive baseline as a tuned incumbent.
pub fn compare<F: FnMut(), G: FnMut()>(
    reps: usize,
    baseline_name: impl Into<String>,
    fast: F,
    baseline: G,
) -> BenchResult {
    let fast_t = Timer::best_of(reps, fast);
    let base_t = Timer::best_of(reps, baseline);
    BenchResult {
        fast: fast_t,
        baseline: base_t,
        baseline_name: baseline_name.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn amdahl_caps_speedup() {
        // p=0.5: infinite kernel speedup caps overall at 2×.
        assert!((amdahl_speedup(0.5, 1e9) - 2.0).abs() < 1e-3);
        // p=0.05: even ∞ kernel speedup ⇒ ≤ 1.053× (the honest small-fraction warning).
        assert!(amdahl_speedup(0.05, 1e9) < 1.06);
        // no speedup when p=0.
        assert!((amdahl_speedup(0.0, 100.0) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn compare_measures_a_real_difference() {
        // a deliberately slower baseline (more work) must measure as slower.
        let n = 2000;
        let data: Vec<f64> = (0..n).map(|i| i as f64).collect();
        let r = compare(
            5,
            "in-house naive (O(n²))",
            || {
                // fast: linear sum
                let _s: f64 = data.iter().sum();
            },
            || {
                // baseline: quadratic busywork on the same input
                let mut s = 0.0;
                for &v in &data {
                    for j in 0..(n / 50) {
                        s += v * (j as f64);
                    }
                }
                std::hint::black_box(s);
            },
        );
        assert!(r.speedup() > 1.0, "fast path should measure faster: {}", r.report("sum"));
    }
}
