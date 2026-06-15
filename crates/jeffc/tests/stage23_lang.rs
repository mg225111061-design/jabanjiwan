//! Stage 23 — language completeness (verified subset). Integer `match` (control flow) is
//! implemented end-to-end: parse → lower → eval. This is a genuine new executable language
//! feature beyond the arithmetic/reduction collapse core. The broader features (structs,
//! full ADTs, closures, generics, modules, error model) parse in the grammar but do not yet
//! lower/codegen — reported as the honest boundary (no fabrication).

use jeffc::{compile, run, Options};
use num_bigint::BigInt;

fn n(v: i64) -> Vec<(String, BigInt)> {
    vec![("n".to_string(), BigInt::from(v))]
}

#[test]
fn integer_match_literal_and_wildcard() {
    let src = "fn step(n: int) -> int:\n    match n:\n        0 => 10\n        1 => 11\n        _ => 20\n";
    let art = compile(src, &Options::default()).expect("compiles");
    assert_eq!(run(&art, "step", &n(0)).unwrap(), BigInt::from(10));
    assert_eq!(run(&art, "step", &n(1)).unwrap(), BigInt::from(11));
    assert_eq!(run(&art, "step", &n(7)).unwrap(), BigInt::from(20));
}

#[test]
fn integer_match_binding_pattern() {
    // a bare variable pattern binds the scrutinee value in the arm body.
    let src = "fn inc(n: int) -> int:\n    match n:\n        0 => 0\n        m => m + 1\n";
    let art = compile(src, &Options::default()).expect("compiles");
    assert_eq!(run(&art, "inc", &n(0)).unwrap(), BigInt::from(0));
    assert_eq!(run(&art, "inc", &n(41)).unwrap(), BigInt::from(42));
    assert_eq!(run(&art, "inc", &n(-5)).unwrap(), BigInt::from(-4));
}

#[test]
fn match_arms_tried_in_order() {
    // first matching arm wins; the wildcard only fires when no literal matched.
    let src = "fn classify(n: int) -> int:\n    match n:\n        2 => 100\n        _ => 0\n";
    let art = compile(src, &Options::default()).expect("compiles");
    assert_eq!(run(&art, "classify", &n(2)).unwrap(), BigInt::from(100));
    assert_eq!(run(&art, "classify", &n(3)).unwrap(), BigInt::from(0));
}
