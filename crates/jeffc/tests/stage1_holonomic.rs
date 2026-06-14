//! Stage-1 integration: holonomic (Gosper / Zeilberger) collapses through the full
//! pipeline (CLAUDE.md APPENDIX D.1 A09–A11, PART 9 Stage-1 DoD). Non-summability is
//! exercised at the unit level (`gosper_harmonic_is_not_summable`).

use jeffc::{collapse_report, compile, run, CompileStatus, Options};
use num_bigint::BigInt;

fn n(v: i64) -> Vec<(String, BigInt)> {
    vec![("n".to_string(), BigInt::from(v))]
}

/// Exact binomial reference (the differential oracle, AR-4).
fn binom(n: i64, k: i64) -> BigInt {
    if k < 0 || k > n {
        return BigInt::from(0);
    }
    let mut num = BigInt::from(1);
    let mut den = BigInt::from(1);
    for i in 1..=k {
        num *= n - k + i;
        den *= i;
    }
    num / den
}

fn holonomic_src() -> String {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/e2e/holonomic.jeff");
    std::fs::read_to_string(path).expect("fixture present")
}

#[test]
fn golden_holonomic_report() {
    let art = compile(&holonomic_src(), &Options::default()).unwrap();
    let report = collapse_report(&art);
    let golden = "\
fn sum_binom        collapsed  layer=1(arith/holonomic)  o(n)  cert=ok(exact-telescoper)
fn sum_binom_sq     collapsed  layer=1(arith/holonomic)  o(n)  cert=ok(exact-telescoper)
fn sum_k_binom      collapsed  layer=1(arith/holonomic)  o(n)  cert=ok(exact-telescoper)
";
    assert_eq!(report, golden);
}

#[test]
fn sum_binomial_equals_2n() {
    let art = compile(&holonomic_src(), &Options::default()).unwrap();
    assert!(matches!(
        art.func("sum_binom").unwrap().status,
        CompileStatus::Collapsed { .. }
    ));
    for v in 0..=20i64 {
        let got = run(&art, "sum_binom", &n(v)).unwrap();
        assert_eq!(got, BigInt::from(1i64 << v), "Σ C({v},k) must be 2^{v}");
    }
}

#[test]
fn sum_binomial_squared_equals_central_binomial() {
    let art = compile(&holonomic_src(), &Options::default()).unwrap();
    for v in 0..=18i64 {
        let got = run(&art, "sum_binom_sq", &n(v)).unwrap();
        assert_eq!(got, binom(2 * v, v), "Σ C({v},k)^2 must be C(2*{v},{v})");
    }
}

#[test]
fn sum_k_binomial_equals_n_2_pow_n_minus_1() {
    let art = compile(&holonomic_src(), &Options::default()).unwrap();
    for v in 1..=18i64 {
        let got = run(&art, "sum_k_binom", &n(v)).unwrap();
        let expect = BigInt::from(v) * BigInt::from(1i64 << (v - 1));
        assert_eq!(got, expect, "Σ k*C({v},k) must be {v}*2^({v}-1)");
    }
}

#[test]
fn holonomic_fallback_never_miscompiles() {
    // Forcing fallback must still produce the right answer (P0/R1).
    let src = holonomic_src();
    let collapsed = compile(&src, &Options::default()).unwrap();
    let fell_back = compile(
        &src,
        &Options {
            force_fallback: true,
            ..Default::default()
        },
    )
    .unwrap();
    for v in [0i64, 1, 5, 12, 18] {
        let a = run(&collapsed, "sum_binom", &n(v)).unwrap();
        let b = run(&fell_back, "sum_binom", &n(v)).unwrap();
        assert_eq!(a, b);
        assert_eq!(a, BigInt::from(1i64 << v));
    }
}
