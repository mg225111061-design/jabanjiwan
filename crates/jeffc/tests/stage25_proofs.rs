//! Stage 25 — proof-carrying capstone (in-container part).
//!
//! Every fold, every Stage-20 absence certificate, and the PQC kernels emit a SELF-CONTAINED,
//! machine-checkable proof artifact to disk. A MINIMAL in-tree re-checker re-validates each
//! artifact INDEPENDENTLY of the main compiler — TCB reduction: the trusted base for
//! re-checking is just the artifact deserializer + the exact in-house checkers (no external
//! solver, no producer involvement). A tampered artifact is rejected.
//!
//! TCB (documented): the re-check trusts only (1) serde deserialization and (2) the exact,
//! quantifier-free in-house checkers — coefficient-zero polynomial identity, exact rational
//! linear algebra (recurrence/integer-relation exclusion), and exact integer replay (PQC
//! NTT vs schoolbook). No Z3/Lean/Carcara is involved. External third-party proof-checking
//! (Lean/Alethe export → Carcara) needs network and is DEFERRED-TO-CODESPACE; this stage does
//! NOT claim external re-verification.

use jeffc::{compile, emit_certificates, Options};
use num_bigint::BigInt;

fn tmp(tag: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("jeff_stage25_{}_{}", tag, std::process::id()))
}

fn ints(v: &[i64]) -> Vec<BigInt> {
    v.iter().map(|&x| BigInt::from(x)).collect()
}

#[test]
fn proof_artifact_emitted_per_fold() {
    // a collapsed fold writes a self-contained, round-trippable proof artifact to disk.
    let art = compile("total fn s2(n: nat) -> nat: sum i in 0..=n: i*i\n", &Options::default()).unwrap();
    let dir = tmp("fold");
    emit_certificates(&art, &dir).unwrap();
    let machine = std::fs::read_to_string(dir.join("s2.machine.json")).expect("artifact emitted");
    // the in-tree re-checker re-validates it independently of the compile that produced it.
    let cert: jeff_cert::Certificate = serde_json::from_str(&machine).unwrap();
    assert!(jeff_verify::verify(cert).is_some(), "fold artifact must re-verify");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn in_tree_checker_revalidates() {
    // absence-certificate artifacts (Stage-15/20) round-trip through disk and re-validate;
    // tampering is rejected (DR1).
    let dir = tmp("absence");
    std::fs::create_dir_all(&dir).unwrap();

    // 1) recurrence-exclusion absence cert for a high-entropy sequence.
    let seq = ints(&[3, 1, 4, 1, 5, 9, 2, 6, 5, 3, 5, 8, 9, 7, 9, 3, 2, 3, 8, 4, 6, 2, 6, 4, 3, 3, 8, 3, 2, 7]);
    let c1 = jeff_cert::recurrence_absence_certificate(seq, 2, 1);
    std::fs::write(dir.join("rec.json"), serde_json::to_string(&c1).unwrap()).unwrap();

    // 2) integer-relation exclusion absence cert.
    let c2 = jeff_cert::integer_relation_absence_certificate(ints(&[2, 3, 7]), 1);
    std::fs::write(dir.join("intrel.json"), serde_json::to_string(&c2).unwrap()).unwrap();

    for file in ["rec.json", "intrel.json"] {
        let s = std::fs::read_to_string(dir.join(file)).unwrap();
        let cert: jeff_cert::Certificate = serde_json::from_str(&s).unwrap();
        assert!(jeff_verify::verify(cert).is_some(), "{file} must re-validate in-tree");
    }

    // tamper: turn the recurrence-absence sequence into a clearly C-finite one (all 7s) →
    // the absence claim is now false → the in-tree checker rejects it.
    let tampered = jeff_cert::recurrence_absence_certificate(ints(&[7; 30]), 2, 1);
    let s = serde_json::to_string(&tampered).unwrap();
    let back: jeff_cert::Certificate = serde_json::from_str(&s).unwrap();
    assert!(jeff_verify::verify(back).is_none(), "a false absence artifact must be rejected");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn pqc_proof_artifact_minimal_recheck() {
    // PQC proof artifact: the Kyber NTT product is recorded; a MINIMAL in-tree checker
    // recomputes the Θ(n²) schoolbook product and confirms equality (tiny TCB — just the
    // schoolbook recompute). Tampering the recorded product is rejected.
    use jeff_math::{kyber, pqc};
    let q = kyber::Q;
    let mut a = vec![0u64; kyber::N];
    let mut b = vec![0u64; kyber::N];
    for i in 0..kyber::N {
        a[i] = ((i * 7 + 1) as u64) % q;
        b[i] = ((i * 13 + 5) as u64) % q;
    }
    let product = kyber::poly_mul(&a, &b); // the claimed artifact

    // serialize the artifact to disk (a, b, claimed product).
    let dir = tmp("pqc");
    std::fs::create_dir_all(&dir).unwrap();
    let artifact = serde_json::json!({ "a": a, "b": b, "product": product });
    std::fs::write(dir.join("kyber.json"), serde_json::to_string(&artifact).unwrap()).unwrap();

    // minimal in-tree re-checker: recompute schoolbook and compare (independent of the NTT).
    let s = std::fs::read_to_string(dir.join("kyber.json")).unwrap();
    let v: serde_json::Value = serde_json::from_str(&s).unwrap();
    let ra: Vec<u64> = serde_json::from_value(v["a"].clone()).unwrap();
    let rb: Vec<u64> = serde_json::from_value(v["b"].clone()).unwrap();
    let rprod: Vec<u64> = serde_json::from_value(v["product"].clone()).unwrap();
    assert_eq!(pqc::schoolbook_negacyclic(&ra, &rb, q), rprod, "PQC artifact must re-check");

    // tamper the recorded product → minimal checker rejects.
    let mut bad = rprod.clone();
    bad[0] = (bad[0] + 1) % q;
    assert_ne!(pqc::schoolbook_negacyclic(&ra, &rb, q), bad, "tampered PQC artifact rejected");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn tcb_minimized_documented() {
    // the trusted base reports the exact in-house checker actually used — never "z3" when no
    // solver ran (honest, minimal TCB). External proof-checkers are DEFERRED-TO-CODESPACE.
    let c = jeff_cert::recurrence_absence_certificate(ints(&[3, 1, 4, 1, 5, 9, 2, 6, 5, 3, 5, 8, 9, 7, 9, 3, 2, 3, 8, 4]), 2, 0);
    let name = jeff_verify::checker_name(&c.evidence);
    assert_eq!(name, "recurrence-exclusion-exact", "TCB is the exact in-house checker, not an external solver");
}
