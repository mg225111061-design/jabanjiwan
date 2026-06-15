//! Stage 11.1 tripwires (master directive PART D — surface-wiring).
//!
//! Two named gates make the surface real:
//!  - `surface_call_roundtrips`: a `.jeff` program calls a kernel and GETS THE VERIFIED
//!    RESULT — the recovered structure is read out of the certificate's `evidence` (exactly
//!    the value the checker proved, P2) and the certificate is independently re-verified
//!    after a serde round-trip (R25 cert-replay: a result you can serialize, ship, and
//!    re-check).
//!  - `collapse_auto_triggers`: a structured loop in source is recognized and folded to a
//!    verified closed form, while an unstructured input HONESTLY DEFERS with a named
//!    barrier (P1/P3/R4 — never a fabricated fold).

use jeff_cert::{CertClass, Evidence};
use jeffc::kernels::KernelStatus;
use jeffc::{compile, run, CompileStatus, Options};
use num_bigint::BigInt;

fn n(name: &str, v: i64) -> Vec<(String, BigInt)> {
    vec![(name.to_string(), BigInt::from(v))]
}

// ---- tripwire 1 ----

#[test]
fn surface_call_roundtrips() {
    // A `.jeff` program calls two exact-certificate kernels with literal data.
    let src = r#"
module test.surface

# Reed-Solomon list decoding over GF(97): received word of p(x) = 3 + 2x + x^2, 2 errors.
fn decode():
    list_decode([1,2,3,4,5,6,7], [6,16,18,27,47,51,66], 3, 97)

# A planted 4-clique on vertices {0,1,2,3} inside a 6-vertex graph.
fn clique():
    planted_clique([[0,1,1,1,1,0],[1,0,1,1,0,1],[1,1,0,1,0,0],[1,1,1,0,0,0],[1,0,0,0,0,0],[0,1,0,0,0,0]], 4)
"#;
    let art = compile(src, &Options::default()).expect("program compiles");

    // (a) the calls were recognized and dispatched to kernels (the surface is wired).
    let decode = art.kernel("decode").expect("decode is surface-wired");
    let clique = art.kernel("clique").expect("clique is surface-wired");

    // (b) each collapsed with an EXACT certificate — the class is read from the real cert,
    // never guessed (DR2), so it can never overclaim.
    assert!(matches!(
        decode.status,
        KernelStatus::Collapsed { cert_class: CertClass::Exact, .. }
    ));
    assert!(matches!(
        clique.status,
        KernelStatus::Collapsed { cert_class: CertClass::Exact, .. }
    ));

    // (c) GET THE VERIFIED RESULT: read the recovered structure out of the certificate's
    // evidence and check it against an independent oracle.
    let vc = decode.cert.as_ref().expect("collapsed ⇒ certificate present");
    match &vc.certificate().evidence {
        Evidence::ListDecode { coeffs, q, k, .. } => {
            assert_eq!(*q, 97);
            assert_eq!(*k, 3);
            // p(x) = 3 + 2x + x^2  — the planted codeword polynomial, recovered exactly.
            assert_eq!(coeffs.as_slice(), &[3u64, 2, 1]);
        }
        other => panic!("expected ListDecode evidence, got {other:?}"),
    }
    let vc = clique.cert.as_ref().expect("collapsed ⇒ certificate present");
    match &vc.certificate().evidence {
        Evidence::PlantedClique { clique: verts, k, .. } => {
            assert_eq!(*k, 4);
            let mut v = verts.clone();
            v.sort_unstable();
            assert_eq!(v, vec![0usize, 1, 2, 3]); // the planted clique, recovered exactly
        }
        other => panic!("expected PlantedClique evidence, got {other:?}"),
    }

    // (d) the verified result ROUND-TRIPS: serialize the certificate, deserialize it, and
    // re-verify it from scratch — it must still be Valid (R25). This is independent of the
    // collapser that produced it: the gate, not the producer, is the arbiter.
    for kf in [decode, clique] {
        let cert = kf.cert.as_ref().unwrap().certificate().clone();
        let json = serde_json::to_string(&cert).expect("certificate serializes");
        let back: jeff_cert::Certificate =
            serde_json::from_str(&json).expect("certificate deserializes");
        assert!(
            jeff_verify::verify(back).is_some(),
            "{}: certificate must re-verify after a round-trip",
            kf.name
        );
    }
}

// ---- tripwire 2 ----

#[test]
fn collapse_auto_triggers() {
    // (a) STRUCTURED loop: a power sum is auto-recognized and folded to a closed form.
    let structured = compile(
        "total fn s2(n: nat) -> nat: sum i in 0..=n: i*i\n",
        &Options::default(),
    )
    .expect("compiles");
    let s2 = structured.func("s2").expect("s2 present");
    assert!(
        matches!(s2.status, CompileStatus::Collapsed { .. }),
        "a power sum must auto-collapse, got {:?}",
        s2.status
    );
    // and the folded answer is correct (P0): Σ_{i=0}^{10} i² = 385.
    assert_eq!(run(&structured, "s2", &n("n", 10)).unwrap(), BigInt::from(385));

    // (a') STRUCTURED hypergeometric sum: recognized holonomic, folded by Zeilberger (P0
    // creative telescoping wired into the surface recognizer).
    let holonomic = compile(
        "total fn sb(n: nat) -> nat: sum k in 0..=n: C(n, k)\n",
        &Options::default(),
    )
    .expect("compiles");
    let sb = holonomic.func("sb").expect("sb present");
    assert!(
        matches!(sb.status, CompileStatus::Collapsed { .. }),
        "a binomial sum must auto-collapse, got {:?}",
        sb.status
    );
    // Σ_k C(12,k) = 2^12 = 4096.
    assert_eq!(run(&holonomic, "sb", &n("n", 12)).unwrap(), BigInt::from(4096));

    // (b) UNSTRUCTURED input: Prony on white noise has no low-order recurrence, so the
    // collapse HONESTLY DEFERS with a named barrier — never a fabricated fold (P1/P3/R4).
    let unstructured = compile(
        "fn noise():\n    prony([0.4, -1.2, 0.7, 2.1, -0.3, 1.5, -2.0, 0.1], 2)\n",
        &Options::default(),
    )
    .expect("compiles");
    let noise = unstructured.kernel("noise").expect("noise is surface-wired");
    match &noise.status {
        KernelStatus::Deferred { tag } => {
            // a NAMED barrier (not a silent skip), and nothing verified was kept.
            assert!(!tag.as_str().is_empty(), "defer must name a barrier");
            assert!(noise.cert.is_none(), "a defer keeps no certificate");
        }
        other => panic!("noise must defer with a named barrier, got {other:?}"),
    }
}
