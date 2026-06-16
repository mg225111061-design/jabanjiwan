//! cfinite_nth — JEFF C-finite N-th term (Stage 26.1/26.2, cfinite.rs) as a CLI for HARAN closure
//! classification (Stage V2). A linear recurrence a_n = Σ c_i·a_{n-i} collapses the N-th term from
//! O(N·d) naive to O(d²·log N) (companion-matrix power). This certifies the collapse by confirming
//! the O(log N) companion path EQUALS the O(N) naive path (exact in F_q).
//!
//! usage:  cfinite_nth "<c1,…,cd>" "<a0,…,a_{d-1}>" <n> <q>
//! output: MATCH n=<n> value=<v> (O(log n) companion ≡ O(n) naive)   or   MISMATCH …

use jeff_math::cfinite::{cfinite_companion, cfinite_naive};

fn parse(s: &str) -> Vec<u64> {
    s.split(',')
        .filter(|t| !t.trim().is_empty())
        .map(|t| t.trim().parse::<u64>().expect("coefficient must be a u64"))
        .collect()
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.len() < 5 {
        eprintln!("usage: cfinite_nth \"<c1,...,cd>\" \"<a0,...>\" <n> <q>");
        std::process::exit(2);
    }
    let c = parse(&a[1]);
    let init = parse(&a[2]);
    let n: u64 = a[3].parse().expect("n must be a u64");
    let q: u64 = a[4].parse().expect("q must be a u64");
    let comp = cfinite_companion(&c, &init, n, q);
    let naive = cfinite_naive(&c, &init, n, q);
    if comp == naive {
        println!("MATCH n={n} value={comp} (O(log n) companion ≡ O(n) naive)");
    } else {
        println!("MISMATCH n={n} companion={comp} naive={naive}");
    }
}
