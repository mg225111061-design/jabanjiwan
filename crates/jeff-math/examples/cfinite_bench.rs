//! Stage 26.1/26.2 measurement: naive O(N·d) vs Bostan–Mori O(M(d)·log N) vs companion
//! O(d²·log N), and periodic O(1). Raw timings; ratios must DIVERGE with N (asymptotic
//! collapse, not a constant factor). Self-relative, exact over F_q.

use std::time::Instant;

const Q: u64 = 1_000_000_007;

fn best_of<F: FnMut() -> u64>(reps: usize, mut f: F) -> (u64, f64) {
    let mut best = f64::INFINITY;
    let mut val = 0;
    for _ in 0..reps {
        let t = Instant::now();
        val = std::hint::black_box(f());
        best = best.min(t.elapsed().as_secs_f64());
    }
    (val, best)
}

fn main() {
    use jeff_math::cfinite::*;
    // order-3 C-finite: a_n = 2 a_{n-1} + 0 a_{n-2} + 1 a_{n-3}
    let c = [2u64, 0, 1];
    let init = [3u64, 1, 4];
    let d = c.len();

    println!("== 26.1 C-finite N-th term (d={d}), exact mod q={Q} ==");
    println!("{:>10} {:>14} {:>14} {:>14} {:>12} {:>12}", "N", "naive_us", "bostan_us", "comp_us", "naive/bos", "naive/comp");
    let mut prev_ratio = 0f64;
    for k in 3..=7 {
        let n = 10u64.pow(k);
        let reps = if n <= 100_000 { 50 } else { 5 };
        let (vn, tn) = best_of(reps, || cfinite_naive(&c, &init, n, Q));
        let (vb, tb) = best_of(200, || cfinite_bostan(&c, &init, n, Q));
        let (vc, tc) = best_of(200, || cfinite_companion(&c, &init, n, Q));
        assert_eq!(vn, vb, "naive vs bostan mismatch at N={n}");
        assert_eq!(vn, vc, "naive vs companion mismatch at N={n}");
        let r_b = tn / tb;
        let r_c = tn / tc;
        println!("{:>10} {:>14.3} {:>14.3} {:>14.3} {:>12.1} {:>12.1}", n, tn*1e6, tb*1e6, tc*1e6, r_b, r_c);
        if k >= 4 {
            println!("       -> naive/bostan ratio {:.1} vs previous {:.1}  ({})",
                r_b, prev_ratio, if r_b > prev_ratio {"DIVERGING"} else {"flat"});
        }
        prev_ratio = r_b;
    }

    println!("\n== crossover: chosen path by N (threshold={}) ==", CROSSOVER_N);
    for n in [0u64, 10, 63, 64, 1000, 1_000_000] {
        println!("   N={:>9} -> {:?}", n, cfinite_choose(n));
    }

    // 26.2 genuine O(1): periodic sequence a_n = a_{n-1} - a_{n-2} (period 6).
    println!("\n== 26.2 periodic closed-form O(1) vs naive O(N) ==");
    let cp = [1u64, Q - 1];
    let ip = [0u64, 1];
    let per = detect_period(&cp, &ip, Q, 1000).expect("period exists");
    println!("   detected period p={}", per.period);
    // measured naive at feasible N, periodic O(1) lookup; extrapolate naive for huge N.
    let mut naive_rate = 0f64; // seconds per term, from a real measurement
    for &n in &[1_000_000u64, 10_000_000] {
        let (_v, t) = best_of(3, || cfinite_naive(&cp, &ip, n, Q));
        naive_rate = t / n as f64;
        let (_vq, tq) = best_of(100000, || per.nth(n));
        println!("   N={:>11}: naive {:>10.3} us | O(1) lookup {:>8.4} us | ratio {:>12.1}", n, t*1e6, tq*1e9/1e3, t/tq);
    }
    println!("   (naive ~ {:.3} ns/term, measured)", naive_rate*1e9);
    for &n in &[1_000_000_000u64, 1_000_000_000_000, 1_000_000_000_000_000] {
        let est_naive = naive_rate * n as f64;
        let (_vq, tq) = best_of(100000, || per.nth(n));
        println!("   N={:>16}: naive≈{:>12.3} s (extrapolated O(N)) | O(1) {:>8.4} us | ratio≈{:.3e}",
            n, est_naive, tq*1e6, est_naive/tq);
    }
}
