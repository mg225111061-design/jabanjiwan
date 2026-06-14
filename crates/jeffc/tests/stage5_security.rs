//! Stage-5 integration: security is part of correctness (CLAUDE.md PART 7.4, R6, D.5
//! P02/P03). The driver must reject a data-dependent branch/index on `secret[T]` as a
//! *compile error*, and pass a clean constant-time function. PQC math correctness
//! (NTT/Montgomery/CRT) is exercised at the unit level in jeff-math / jeff-verify.

use jeff_span::DiagCode;
use jeffc::{compile, const_time_audit, Options};

fn errors(src: &str) -> Vec<DiagCode> {
    match compile(src, &Options::default()) {
        Ok(_) => vec![],
        Err(ds) => ds.iter().map(|d| d.code).collect(),
    }
}

#[test]
fn secret_dependent_branch_is_a_compile_error() {
    // P02: branch on a secret value. JEFF has no `if`; the branch is `match`.
    let src = "\
@constant_time
fn decap(sk: secret[Vec[u8, 4]]) -> u8:
    match sk[0]:
        0 => 1
        _ => 0
";
    assert!(
        errors(src).contains(&DiagCode::SecretBranch),
        "secret-dependent branch must be E0301 at compile time (R6)"
    );
}

#[test]
fn secret_dependent_index_is_a_compile_error() {
    // P03: index a public table by a secret value (cache-timing leak).
    let src = "\
@constant_time
fn lookup(sk: secret[Vec[u8, 4]], table: Vec[u8, 256]) -> u8:
    table[sk[0]]
";
    assert!(
        errors(src).contains(&DiagCode::SecretIndex),
        "secret-dependent index must be E0302 at compile time (R6)"
    );
}

#[test]
fn constant_time_arithmetic_passes_the_audit() {
    // Pure data-oblivious arithmetic on secrets is allowed (P01-style core).
    let src = "\
@constant_time
fn mix(a: secret[u32], b: secret[u32]) -> secret[u32]:
    a + b
";
    // compiles (no secret-taint error)
    assert!(compile(src, &Options::default()).is_ok());
    // and the audit reports OK for the secret-input function
    let (report, all_ok) = const_time_audit(src).unwrap();
    assert!(all_ok, "clean const-time fn must pass, report:\n{report}");
    assert!(report.contains("mix"));
    assert!(report.contains("OK"));
}

#[test]
fn public_function_is_unaffected_by_the_audit() {
    // No secret inputs ⇒ no secret-taint constraints; ordinary code compiles.
    let src = "total fn s2(n: nat) -> nat: sum i in 0..=n: i*i\n";
    assert!(compile(src, &Options::default()).is_ok());
    let (report, all_ok) = const_time_audit(src).unwrap();
    assert!(all_ok);
    assert!(report.is_empty(), "no secret[T] fns ⇒ empty audit report");
}
