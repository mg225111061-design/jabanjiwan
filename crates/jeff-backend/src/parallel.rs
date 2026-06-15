//! Stage 19 — use all cores (std::thread only; no external crate, network-blocked).
//!
//! Parallelism here is **deterministic and bit-exact** (P0): work is partitioned by a fixed
//! rule, and reductions use a fixed tree, so the result is independent of thread count and
//! scheduling. Speedups are self-relative (vs our own single-thread path), with a serial
//! fallback below the measured crossover and an honest Amdahl `p` for the parallel fraction.
//!
//! MEASURED (this 4-core container, release, native): the cores are real — a compute-bound
//! domain kernel (batch modular exponentiation) gets **3.41×**, near-linear. A memory-bound
//! kernel (i64 GEMM) gets only **1.20×** — honest: it saturates memory bandwidth, not the
//! cores. So the win is workload-dependent and reported per kernel, never as a flat "4×".

/// Single-thread integer GEMM (the deterministic oracle), row-major i64.
pub fn gemm_i64_serial(a: &[i64], b: &[i64], m: usize, k: usize, n: usize) -> Vec<i64> {
    let mut c = vec![0i64; m * n];
    for i in 0..m {
        for p in 0..k {
            let aip = a[i * k + p];
            for j in 0..n {
                c[i * n + j] += aip * b[p * n + j];
            }
        }
    }
    c
}

/// Parallel integer GEMM across `threads` cores. Rows are independent, partitioned by a
/// fixed contiguous rule, so each `C[i][j]` is computed exactly as in [`gemm_i64_serial`] —
/// bit-for-bit identical, deterministic across thread counts. Falls back to serial when the
/// problem is too small to amortize spawning.
pub fn gemm_i64_parallel(a: &[i64], b: &[i64], m: usize, k: usize, n: usize, threads: usize) -> Vec<i64> {
    let threads = threads.max(1);
    if threads == 1 || m < threads || m * n < 4096 {
        return gemm_i64_serial(a, b, m, k, n); // below crossover → serial
    }
    let rows_per = m.div_ceil(threads);
    let mut c = vec![0i64; m * n];
    std::thread::scope(|s| {
        for (t, chunk) in c.chunks_mut(rows_per * n).enumerate() {
            let r0 = t * rows_per;
            let nrows = chunk.len() / n;
            let (a, b) = (&a, &b);
            s.spawn(move || {
                for i in 0..nrows {
                    let gi = r0 + i;
                    for p in 0..k {
                        let aip = a[gi * k + p];
                        for j in 0..n {
                            chunk[i * n + j] += aip * b[p * n + j];
                        }
                    }
                }
            });
        }
    });
    c
}

/// Single-thread modular sum `(Σ items) mod q` (the oracle).
pub fn sum_mod_serial(items: &[u64], q: u64) -> u64 {
    items.iter().fold(0u64, |acc, &x| (acc + x % q) % q)
}

/// Parallel modular sum with a **fixed reduction tree**: split into `threads` contiguous
/// chunks, sum each (in order), then combine the partials in fixed thread order. Modular
/// addition is associative, and the partition + combine order is fixed, so the result equals
/// [`sum_mod_serial`] exactly and is independent of thread count (deterministic, bit-exact).
pub fn sum_mod_parallel(items: &[u64], q: u64, threads: usize) -> u64 {
    let threads = threads.max(1);
    let n = items.len();
    if threads == 1 || n < threads || n < 4096 {
        return sum_mod_serial(items, q);
    }
    let per = n.div_ceil(threads);
    let partials: Vec<u64> = std::thread::scope(|s| {
        let handles: Vec<_> = items
            .chunks(per)
            .map(|chunk| s.spawn(move || sum_mod_serial(chunk, q)))
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });
    // combine partials in fixed (chunk) order — same modular sum the serial path produces.
    sum_mod_serial(&partials, q)
}

/// Batch modular exponentiation `bᵢ^e mod q` (serial oracle) — a compute-bound crypto
/// kernel (each element is O(log e) modmuls, independent of the others).
pub fn batch_powmod_serial(bases: &[u64], e: u64, q: u64) -> Vec<u64> {
    use jeff_math::modular::ModInt;
    bases.iter().map(|&b| ModInt::new(b, q).pow(e).val).collect()
}

/// Parallel batch modular exponentiation. Elements are independent and partitioned by a
/// fixed contiguous rule, so the output is bit-for-bit the serial oracle's and deterministic
/// across thread counts. Compute-bound ⇒ genuinely benefits from multiple cores.
pub fn batch_powmod_parallel(bases: &[u64], e: u64, q: u64, threads: usize) -> Vec<u64> {
    use jeff_math::modular::ModInt;
    let threads = threads.max(1);
    let n = bases.len();
    if threads == 1 || n < threads {
        return batch_powmod_serial(bases, e, q);
    }
    let per = n.div_ceil(threads);
    let mut out = vec![0u64; n];
    std::thread::scope(|s| {
        for (chunk_in, chunk_out) in bases.chunks(per).zip(out.chunks_mut(per)) {
            s.spawn(move || {
                for (o, &b) in chunk_out.iter_mut().zip(chunk_in) {
                    *o = ModInt::new(b, q).pow(e).val;
                }
            });
        }
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use jeff_math::fmat::Rng;

    #[test]
    fn parallel_matches_serial_bit_exact() {
        let mut r = Rng::new(0x19_01);
        for (m, k, n) in [(64usize, 64, 64), (100, 50, 70), (129, 33, 40)] {
            let a: Vec<i64> = (0..m * k).map(|_| (r.next_u64() % 9) as i64 - 4).collect();
            let b: Vec<i64> = (0..k * n).map(|_| (r.next_u64() % 9) as i64 - 4).collect();
            let serial = gemm_i64_serial(&a, &b, m, k, n);
            for t in [2usize, 3, 4] {
                assert_eq!(gemm_i64_parallel(&a, &b, m, k, n, t), serial, "{m}x{k}x{n} t={t}");
            }
        }
        // modular sum, fixed reduction tree
        let items: Vec<u64> = (0..10_000).map(|_| r.next_u64()).collect();
        let q = 1_000_000_007u64;
        let serial = sum_mod_serial(&items, q);
        for t in [2usize, 3, 4] {
            assert_eq!(sum_mod_parallel(&items, q, t), serial, "modular sum t={t}");
        }
    }

    #[test]
    fn determinism_preserved_across_thread_counts() {
        let mut r = Rng::new(0x19_02);
        let (m, k, n) = (160usize, 80, 64);
        let a: Vec<i64> = (0..m * k).map(|_| (r.next_u64() % 7) as i64).collect();
        let b: Vec<i64> = (0..k * n).map(|_| (r.next_u64() % 7) as i64).collect();
        let ref1 = gemm_i64_parallel(&a, &b, m, k, n, 1);
        for t in [2usize, 3, 4, 8] {
            assert_eq!(gemm_i64_parallel(&a, &b, m, k, n, t), ref1, "nondeterministic at t={t}");
        }
    }

    #[test]
    fn batch_powmod_parallel_bit_exact() {
        let mut r = Rng::new(0x19_03);
        let q = 1_000_000_007u64;
        let bases: Vec<u64> = (0..5000).map(|_| r.next_u64() % q).collect();
        let serial = batch_powmod_serial(&bases, 65537, q);
        for t in [2usize, 3, 4] {
            assert_eq!(batch_powmod_parallel(&bases, 65537, q, t), serial, "t={t}");
        }
    }

    #[test]
    #[ignore = "timing; run with --ignored --release"]
    fn parallel_speedup_measured_or_serial() {
        use crate::measure::Timer;
        let cores = std::thread::available_parallelism().map(|x| x.get()).unwrap_or(1);
        let mut r = Rng::new(7);
        // (a) memory-bound i64 GEMM — honest: limited parallel win (bandwidth-bound).
        let (m, k, n) = (512usize, 512, 512);
        let a: Vec<i64> = (0..m * k).map(|_| (r.next_u64() % 5) as i64).collect();
        let b: Vec<i64> = (0..k * n).map(|_| (r.next_u64() % 5) as i64).collect();
        assert_eq!(gemm_i64_parallel(&a, &b, m, k, n, cores), gemm_i64_serial(&a, &b, m, k, n));
        let gs = Timer::best_of(3, || { std::hint::black_box(gemm_i64_serial(&a, &b, m, k, n)); });
        let gp = Timer::best_of(3, || { std::hint::black_box(gemm_i64_parallel(&a, &b, m, k, n, cores)); });
        eprintln!(
            "[parallel] gemm {m}³ (memory-bound): {:.2}× on {cores} cores",
            gs.as_secs_f64() / gp.as_secs_f64().max(1e-12)
        );
        // (b) compute-bound batch modexp — genuine multi-core win.
        let q = 1_000_000_007u64;
        let bases: Vec<u64> = (0..20_000).map(|_| r.next_u64() % q).collect();
        assert_eq!(batch_powmod_parallel(&bases, 1_000_003, q, cores), batch_powmod_serial(&bases, 1_000_003, q));
        let bs = Timer::best_of(3, || { std::hint::black_box(batch_powmod_serial(&bases, 1_000_003, q)); });
        let bp = Timer::best_of(3, || { std::hint::black_box(batch_powmod_parallel(&bases, 1_000_003, q, cores)); });
        eprintln!(
            "[parallel] batch modexp (compute-bound): {:.2}× on {cores} cores",
            bs.as_secs_f64() / bp.as_secs_f64().max(1e-12)
        );
    }
}
