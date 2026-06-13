//! Stage-0 end-to-end integration test (CLAUDE.md PART 9 Stage-0 DoD, PART 19,
//! APPENDIX K.1 templates (1)(3)(4)(5)). Exercises the full pipeline as a library
//! consumer and pins a golden collapse-report (R25).

use jeffc::{collapse_report, compile, run, run_llvm, CompileStatus, Options};
use num_bigint::BigInt;

fn n(v: i64) -> Vec<(String, BigInt)> {
    vec![("n".to_string(), BigInt::from(v))]
}

/// (1) unit + (5) golden: a known program produces a fixed, honest collapse report.
#[test]
fn golden_collapse_report() {
    let src = "total fn triangular(n: nat) -> nat: sum i in 0..=n: i\n\
               total fn s2(n: nat) -> nat: sum i in 0..=n: i*i\n";
    let art = compile(src, &Options::default()).unwrap();
    let report = collapse_report(&art);
    let golden = "\
fn triangular       collapsed  layer=1(arith/faulhaber)  O(1)  cert=ok(exact-coeff-zero)
fn s2               collapsed  layer=1(arith/faulhaber)  O(1)  cert=ok(exact-coeff-zero)
";
    assert_eq!(report, golden);
}

/// Property-style equivalence (DR7): collapsed residual == naive sum for many n.
/// Deterministic (R11) rather than randomised; proptest is adopted in Stage 1.
#[test]
fn collapse_equals_naive_over_range() {
    let art = compile(
        "total fn s2(n: nat) -> nat: sum i in 0..=n: i*i\n",
        &Options::default(),
    )
    .unwrap();
    for v in 0..=200i64 {
        let got = run(&art, "s2", &n(v)).unwrap();
        let naive: i128 = (0..=v as i128).map(|i| i * i).sum();
        assert_eq!(got, BigInt::from(naive), "mismatch at n={v}");
    }
}

/// (4) fallback (R1/P0): forcing fallback still computes the right answer and equals
/// the collapse path — the never-miscompile guarantee.
#[test]
fn fallback_never_miscompiles() {
    let src = "total fn triangular(n: nat) -> nat: sum i in 0..=n: i\n";
    let collapsed = compile(src, &Options::default()).unwrap();
    let fell_back = compile(
        src,
        &Options {
            force_fallback: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(matches!(
        fell_back.func("triangular").unwrap().status,
        CompileStatus::Deferred { .. }
    ));
    for v in [0i64, 1, 9, 50, 777, 10_000] {
        let a = run(&collapsed, "triangular", &n(v)).unwrap();
        let b = run(&fell_back, "triangular", &n(v)).unwrap();
        assert_eq!(a, b);
        assert_eq!(a, BigInt::from((0..=v).sum::<i64>()));
    }
}

/// Real LLVM codegen → clang → run, for both collapsed and fallback regions.
#[test]
fn llvm_codegen_runs_both_paths() {
    let src = "total fn triangular(n: nat) -> nat: sum i in 0..=n: i\n";
    let collapsed = compile(src, &Options::default()).unwrap();
    let fell_back = compile(
        src,
        &Options {
            force_fallback: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(run_llvm(&collapsed, "triangular", &[100]).unwrap(), BigInt::from(5050));
    assert_eq!(run_llvm(&fell_back, "triangular", &[100]).unwrap(), BigInt::from(5050));
}

/// The repository's e2e fixture file parses and collapses (PART 19 witness).
#[test]
fn repo_fixture_compiles() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/e2e/triangular.jeff");
    let src = std::fs::read_to_string(path).expect("fixture present");
    let art = compile(&src, &Options::default()).unwrap();
    assert!(art.func("triangular").unwrap().region.is_collapsed());
    assert_eq!(run(&art, "triangular", &n(100000)).unwrap(), BigInt::from(5_000_050_000i64));
}
