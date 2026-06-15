//! Stage 26.3 measurement (field ops, exact mod q — the clean algorithmic comparison; over ℚ
//! the 2^n / C(2n,n) bignum growth would dominate, see §C).
//!  (A) P-finite table S(0..M)=Σ_k C(n,k)² (telescoper (n+1)S(n+1)=(4n+2)S(n), polynomial
//!      coeffs): naive O(M²) vs recurrence O(M)  → ratio ~ M (diverges).
//!  (B) C-finite single value S(n)=Σ_k C(n,k)=2^n: naive Σ_k O(n) vs telescoper→C-finite O(log n).
//! Self-relative; output small (Θ(M) table built optimally / one value) — Ω(N)-safe.

use jeff_math::cfinite::cfinite_bostan;
use jeff_math::modular::ModInt;
use std::time::Instant;

const Q: u64 = 1_000_000_007;
fn mi(v: u64) -> ModInt {
    ModInt::new(v % Q, Q)
}

/// naive Σ_{k=0}^{n} C(n,k)^2 mod q via incremental binomials. O(n).
fn naive_central_binom(n: u64) -> u64 {
    let mut acc = ModInt::zero(Q);
    let mut b = ModInt::one(Q); // C(n,0)
    for k in 0..=n {
        acc = acc + b * b;
        if k < n {
            b = b * mi(n - k) * mi(k + 1).inv().unwrap();
        }
    }
    acc.val
}

fn main() {
    println!("== 26.3 (A) table S(0..M)=Σ_k C(n,k)² : naive O(M²) vs telescoper O(M), mod q ==");
    println!("{:>6} {:>14} {:>14} {:>10}", "M", "naive_us", "telesc_us", "ratio");
    let mut prev = 0f64;
    for &m in &[500u64, 1000, 2000, 4000] {
        // naive table
        let t0 = Instant::now();
        let naive: Vec<u64> = (0..=m).map(naive_central_binom).collect();
        let tn = t0.elapsed().as_secs_f64();
        // telescoper recurrence: S(0)=1; (n+1)S(n+1)=(4n+2)S(n)
        let t1 = Instant::now();
        let mut tele = vec![0u64; (m + 1) as usize];
        let mut s = ModInt::one(Q);
        tele[0] = s.val;
        for n in 0..m {
            s = s * mi(4 * n + 2) * mi(n + 1).inv().unwrap();
            tele[(n + 1) as usize] = s.val;
        }
        let tt = t1.elapsed().as_secs_f64();
        assert_eq!(naive, tele, "table mismatch M={m}");
        let ratio = tn / tt;
        println!("{:>6} {:>14.1} {:>14.1} {:>10.1}  {}", m, tn * 1e6, tt * 1e6, ratio,
            if ratio > prev { "DIVERGING (≈M)" } else { "" });
        prev = ratio;
    }

    println!("\n== 26.3 (B) single S(n)=Σ_k C(n,k)=2^n : naive O(n) vs C-finite O(log n), mod q ==");
    println!("{:>10} {:>14} {:>14} {:>12}", "n", "naive_us", "cfinite_us", "ratio");
    let mut prevb = 0f64;
    let c = [2u64];
    let init = [1u64]; // S(0)=1, S(n+1)=2 S(n)
    for k in 3..=7 {
        let n = 10u64.pow(k);
        let reps = if n <= 100_000 { 20 } else { 3 };
        let mut tn = f64::INFINITY;
        let mut vnaive = 0u64;
        for _ in 0..reps {
            let t = Instant::now();
            let mut acc = ModInt::zero(Q);
            let mut b = ModInt::one(Q);
            for j in 0..=n {
                acc = acc + b;
                if j < n {
                    b = b * mi(n - j) * mi(j + 1).inv().unwrap();
                }
            }
            vnaive = std::hint::black_box(acc.val);
            tn = tn.min(t.elapsed().as_secs_f64());
        }
        let t1 = Instant::now();
        let vcf = cfinite_bostan(&c, &init, n, Q);
        let tt = t1.elapsed().as_secs_f64();
        assert_eq!(vnaive, vcf, "single value mismatch n={n}");
        let ratio = tn / tt;
        println!("{:>10} {:>14.3} {:>14.3} {:>12.1}  {}", n, tn * 1e6, tt * 1e6, ratio,
            if ratio > prevb { "DIVERGING (≈n/log n)" } else { "" });
        prevb = ratio;
    }
}
