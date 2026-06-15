//! Stage 17.1 — the self-benchmark spine: probe THIS CPU and measure peak honestly.
//!
//! No external libraries, no incumbent comparison. We read the machine's real capabilities
//! (cores, cache sizes, vector ISA) and measure its *sustained* FMA throughput, so any
//! later "fraction of peak" claim has a real, same-machine denominator. Every number here
//! is measured at runtime — nothing is hardcoded (R8/DR2).

/// What THIS CPU actually offers. Vector-ISA flags are detected at runtime, so optimizations
/// are conditioned on what the silicon supports — never assumed.
#[derive(Clone, Debug)]
pub struct CpuCaps {
    pub logical_cores: usize,
    pub l1d_bytes: Option<usize>,
    pub l2_bytes: Option<usize>,
    pub l3_bytes: Option<usize>,
    /// Detected vector/throughput features (x86: avx512f/avx2/fma/…; empty on other arch).
    pub isa: Vec<&'static str>,
}

fn read_cache_size(index: usize) -> Option<usize> {
    let path = format!("/sys/devices/system/cpu/cpu0/cache/index{index}/size");
    let s = std::fs::read_to_string(path).ok()?;
    let s = s.trim();
    let (num, mult) = if let Some(p) = s.strip_suffix('K') {
        (p, 1024usize)
    } else if let Some(p) = s.strip_suffix('M') {
        (p, 1024 * 1024)
    } else {
        (s, 1)
    };
    num.parse::<usize>().ok().map(|n| n * mult)
}

#[cfg(target_arch = "x86_64")]
fn detect_isa() -> Vec<&'static str> {
    // `is_x86_feature_detected!` requires a direct string literal, so list them explicitly.
    let mut v = Vec::new();
    if std::arch::is_x86_feature_detected!("avx512f") {
        v.push("avx512f");
    }
    if std::arch::is_x86_feature_detected!("avx512dq") {
        v.push("avx512dq");
    }
    if std::arch::is_x86_feature_detected!("avx512bw") {
        v.push("avx512bw");
    }
    if std::arch::is_x86_feature_detected!("avx512vl") {
        v.push("avx512vl");
    }
    if std::arch::is_x86_feature_detected!("avx2") {
        v.push("avx2");
    }
    if std::arch::is_x86_feature_detected!("fma") {
        v.push("fma");
    }
    if std::arch::is_x86_feature_detected!("avx") {
        v.push("avx");
    }
    v
}

#[cfg(not(target_arch = "x86_64"))]
fn detect_isa() -> Vec<&'static str> {
    // Other arch (e.g. aarch64/NEON): keep honest — only report what we can detect.
    Vec::new()
}

/// Read this CPU's real capabilities.
pub fn probe() -> CpuCaps {
    CpuCaps {
        logical_cores: std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1),
        l1d_bytes: read_cache_size(0),
        l2_bytes: read_cache_size(2),
        l3_bytes: read_cache_size(3),
        isa: detect_isa(),
    }
}

impl CpuCaps {
    pub fn has(&self, feature: &str) -> bool {
        self.isa.contains(&feature)
    }
    /// The widest f64 vector width (lanes) the detected ISA supports — for analytical
    /// tuning (8 = AVX-512, 4 = AVX2/AVX, 2 = SSE baseline).
    pub fn f64_lanes(&self) -> usize {
        if self.has("avx512f") {
            8
        } else if self.has("avx2") || self.has("avx") {
            4
        } else {
            2
        }
    }
}

/// Measure this CPU's **sustained** FMA throughput (GFLOP/s) for the current build target —
/// the honest denominator for "fraction of peak". Uses 8 independent accumulators to break
/// the FMA dependency chain; `black_box` defeats dead-code elimination. (To see the AVX-512
/// ceiling, build the bench with `-C target-cpu=native`; this measures whatever the active
/// target emits, apples-to-apples with kernels built the same way.)
pub fn peak_gflops() -> f64 {
    const LANES: usize = 8;
    const ITERS: u64 = 30_000_000;
    let mut acc = [0.0f64; LANES];
    for (i, a) in acc.iter_mut().enumerate() {
        *a = i as f64 * 0.5 + 1.0;
    }
    // opaque inputs so the loop can't be constant-folded; 8 independent chains keep the
    // FMA units busy (throughput-bound, not latency-bound).
    let b = std::hint::black_box(1.000_000_1f64);
    let c = std::hint::black_box(0.000_000_1f64);
    let t = std::time::Instant::now();
    for _ in 0..ITERS {
        for a in acc.iter_mut() {
            *a = a.mul_add(b, c);
        }
    }
    let secs = t.elapsed().as_secs_f64().max(1e-12);
    std::hint::black_box(&acc);
    let flops = ITERS as f64 * LANES as f64 * 2.0; // FMA = 2 flops
    flops / secs / 1e9
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::simd::{gemm_blocked, gemm_scalar};
    use jeff_math::fmat::Rng;

    #[test]
    fn peak_probe_gives_denominator() {
        let g = peak_gflops();
        assert!(g.is_finite() && g > 0.0, "peak must be a positive measured number, got {g}");
    }

    #[test]
    fn cpu_probe_reports_caps() {
        let caps = probe();
        assert!(caps.logical_cores >= 1);
        assert!(caps.f64_lanes() >= 2);
        // The probe must not invent features: f64_lanes is consistent with the ISA list.
        if caps.f64_lanes() == 8 {
            assert!(caps.has("avx512f"));
        }
    }

    #[test]
    fn harness_measures_self_relative() {
        // BenchResult yields a self-relative ratio against our OWN naive oracle (no incumbent).
        let mut rng = Rng::new(0x17_01);
        let n = 96;
        let a: Vec<f64> = (0..n * n).map(|_| rng.gaussian()).collect();
        let b: Vec<f64> = (0..n * n).map(|_| rng.gaussian()).collect();
        let r = crate::measure::compare(
            3,
            "naive gemm",
            || {
                std::hint::black_box(gemm_blocked(&a, &b, n, n, n));
            },
            || {
                std::hint::black_box(gemm_scalar(&a, &b, n, n, n));
            },
        );
        assert!(r.speedup().is_finite() && r.speedup() > 0.0);
    }

    #[test]
    #[ignore = "diagnostic; run with --ignored --nocapture (ideally -C target-cpu=native)"]
    fn print_cpu_report() {
        let caps = probe();
        eprintln!("[cpu] cores={} isa={:?}", caps.logical_cores, caps.isa);
        eprintln!(
            "[cpu] L1d={:?} L2={:?} L3={:?} f64_lanes={}",
            caps.l1d_bytes, caps.l2_bytes, caps.l3_bytes, caps.f64_lanes()
        );
        eprintln!("[cpu] sustained peak ≈ {:.1} GFLOP/s (this build target)", peak_gflops());
    }

    #[test]
    fn optimized_matches_oracle_each_run() {
        // bit-exactness gate (P0): the optimized path equals the scalar oracle every run.
        let mut rng = Rng::new(0x17_02);
        for _ in 0..8 {
            let n = 40;
            let a: Vec<f64> = (0..n * n).map(|_| rng.gaussian()).collect();
            let b: Vec<f64> = (0..n * n).map(|_| rng.gaussian()).collect();
            assert_eq!(gemm_blocked(&a, &b, n, n, n), gemm_scalar(&a, &b, n, n, n));
        }
    }
}
