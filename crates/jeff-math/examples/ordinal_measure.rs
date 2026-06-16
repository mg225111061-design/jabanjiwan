//! ordinal_measure — JEFF's ordinal termination engine (Stage 32, ordinal.rs) exposed as a CLI for
//! Mr.Jeffrey / HARAN measure synthesis (Stage H3). Builds the lexicographic CNF ordinal measure
//! `ω^{k-1}·i₁ + … + ω^0·i_k` for the BEFORE and AFTER index tuples of a recursive call and decides
//! a STRICT DECREASE via the Cantor-normal-form well-order (`Ord::ord_cmp`).
//!
//! usage:  ordinal_measure "<before>" "<after>"     (each = comma-separated u64 indices)
//! output: "DECREASES omega_k=<bool> fgh=<level> before=<ord> after=<ord>"
//!     or  "NOT_DECREASES before=<ord> after=<ord>"

use jeff_math::ordinal::{fgh_level, lex_measure, Ord};
use std::cmp::Ordering;

fn parse(s: &str) -> Vec<u64> {
    s.split(',')
        .filter(|t| !t.trim().is_empty())
        .map(|t| t.trim().parse::<u64>().expect("index must be a non-negative integer"))
        .collect()
}

/// Render a CNF ordinal in the ω^k fragment as e.g. "ω·2 + 3".
fn render(o: &Ord) -> String {
    if o.terms.is_empty() {
        return "0".to_string();
    }
    o.terms
        .iter()
        .map(|(e, c)| {
            // exponent is a finite ordinal here (ω^k fragment): read its ω^0 coefficient.
            let ek = e.terms.first().map(|(_, k)| *k).unwrap_or(0);
            match ek {
                0 => format!("{c}"),
                1 => format!("ω·{c}"),
                k => format!("ω^{k}·{c}"),
            }
        })
        .collect::<Vec<_>>()
        .join(" + ")
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.len() < 3 {
        eprintln!("usage: ordinal_measure \"<before>\" \"<after>\"");
        std::process::exit(2);
    }
    let before = lex_measure(&parse(&a[1]));
    let after = lex_measure(&parse(&a[2]));
    let frag = before.in_omega_k_fragment() && after.in_omega_k_fragment();
    let fgh = fgh_level(&before);
    match before.ord_cmp(&after) {
        Ordering::Greater => println!(
            "DECREASES omega_k={frag} fgh={fgh} before={} after={}",
            render(&before),
            render(&after)
        ),
        _ => println!("NOT_DECREASES before={} after={}", render(&before), render(&after)),
    }
}
