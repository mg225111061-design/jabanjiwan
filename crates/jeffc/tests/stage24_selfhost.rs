//! Stage 24 — self-hosting bootstrap (verified max subset).
//!
//! JEFF's executable subset is integer/reduction/match, which cannot express a lexer/parser,
//! so a full compiler self-host is out of scope (it needs the Stage-23-deferred features:
//! strings, ADTs, closures). The strongest available self-application — and what this stage
//! ships — is JEFF compiling+running JEFF-written implementations of its OWN stdlib numeric
//! kernels, with output checked **bit-for-bit** against an independent Rust reference. That
//! boundary is reported honestly, not papered over.

use jeffc::{compile, run, CompileStatus, Options};
use num_bigint::BigInt;

fn src() -> String {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/e2e/selfhost.jeff");
    std::fs::read_to_string(path).expect("fixture present")
}

fn n(v: i64) -> Vec<(String, BigInt)> {
    vec![("n".to_string(), BigInt::from(v))]
}

// independent Rust references (the oracle).
fn ref_sum_id(v: i64) -> BigInt { (0..=v).map(BigInt::from).sum() }
fn ref_sum_sq(v: i64) -> BigInt { (0..=v).map(|i| BigInt::from(i * i)).sum() }
fn ref_sum_cube(v: i64) -> BigInt { (0..=v).map(|i| BigInt::from(i) * BigInt::from(i) * BigInt::from(i)).sum() }
fn ref_sum_binom(v: i64) -> BigInt { BigInt::from(1u64) << v } // Σ_k C(n,k) = 2^n
fn ref_sum_k_binom(v: i64) -> BigInt {
    if v == 0 { BigInt::from(0) } else { BigInt::from(v) * (BigInt::from(1u64) << (v - 1)) }
}

#[test]
fn jeff_in_jeff_subset_compiles() {
    // the JEFF-written stdlib module compiles, and every kernel is recognized + collapsed
    // (the self-hosted stdlib goes through JEFF's own verified collapse pipeline).
    let art = compile(&src(), &Options::default()).expect("self-hosted stdlib compiles");
    for name in ["sum_id", "sum_sq", "sum_cube", "sum_binom", "sum_k_binom"] {
        let f = art.func(name).unwrap_or_else(|| panic!("{name} present"));
        assert!(
            matches!(f.status, CompileStatus::Collapsed { .. }),
            "{name} should collapse through JEFF's own pipeline, got {:?}",
            f.status
        );
    }
}

#[test]
fn self_hosted_output_matches_reference() {
    // JEFF-compiled, JEFF-run output == independent Rust reference, bit-for-bit (P0).
    let art = compile(&src(), &Options::default()).expect("compiles");
    for v in 0..=24i64 {
        assert_eq!(run(&art, "sum_id", &n(v)).unwrap(), ref_sum_id(v), "sum_id n={v}");
        assert_eq!(run(&art, "sum_sq", &n(v)).unwrap(), ref_sum_sq(v), "sum_sq n={v}");
        assert_eq!(run(&art, "sum_cube", &n(v)).unwrap(), ref_sum_cube(v), "sum_cube n={v}");
        assert_eq!(run(&art, "sum_binom", &n(v)).unwrap(), ref_sum_binom(v), "sum_binom n={v}");
        assert_eq!(run(&art, "sum_k_binom", &n(v)).unwrap(), ref_sum_k_binom(v), "sum_k_binom n={v}");
    }
}
