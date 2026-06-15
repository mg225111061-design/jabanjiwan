# JEFF v2 — PQC product (군대·은행). §C

v1 (the 41-stage fold engine) is untouched; v2 builds a certifiable PQC product on top.
Discipline: byte-exact or named certificate / measured-or-proven only / no fake pass / premise
corrections noted / ACVP official vectors from NIST, byte-for-byte / STOP+report raw bytes on any
mismatch. Verifier re-proved green (jeff-verify 49/0) at entry/exit.

## §C v2-A — full-parameter ACVP byte-exact — **BUILT (all 5 sets)**

Refactored `mlkem.rs` (`KemParams`) and `mldsa.rs` (`DsaParams`) to thread the parameter set
through the existing 768/44 serialization/sampling path; the 768/44 public APIs stay as wrappers so
their embedded KATs are preserved. Verified **byte-for-byte against official NIST ACVP** (drivers
`examples/mlkem_param`, `examples/mldsa_param`; embedded regression in `mod acvp_kat_v2`):

| scheme | keyGen | encaps/sigGen | decaps/sigVer | certificate |
|---|---|---|---|---|
| ML-KEM-512 | 25/25 | 25/25 | 10/10 | integer-exact byte-for-byte |
| ML-KEM-768 | 25/25 | 25/25 | 10/10 | integer-exact (prior) |
| ML-KEM-1024 | 25/25 | 25/25 | 10/10 | integer-exact byte-for-byte |
| ML-DSA-44 | 25/25 | 90/90 | 45/45 | integer-exact (prior) |
| ML-DSA-65 | 25/25 | 90/90 | 45/45 | integer-exact byte-for-byte |
| ML-DSA-87 | 25/25 | 90/90 | 45/45 | integer-exact byte-for-byte |

(sigGen/sigVer counts are the non-`preHash` groups: external/pure + internal + external-μ,
deterministic + hedged, including the negative sigVer tests. `preHash`/HashML-DSA framing is the
same crypto core with a different `M'` prefix — not wired, stated.)

Parameter-dependent pieces that had to be right per set: ML-KEM `η1` (512 uses 3, not 2),
`du`/`dv` (1024 uses 11/5); ML-DSA `η` (65 uses 4 → 4-bit RejBoundedPoly + 4-bit `s` pack), `γ1`
(65/87 use 2^19 → 20-bit ExpandMask/`z`), `γ2` (65/87 → 4-bit `w1Encode`), `ω`, and the variable
challenge length `c̃` (32/48/64). All verified byte-for-byte. No fake pass; every set's vectors
were run through the real driver and compared to the official bytes.

Workspace: mlkem 10 tests, mldsa 7 tests (incl. embedded 512/1024 + 65/87 KAT). clippy
`--all-targets -D warnings` clean (after `cargo clean -p jeff-backend` to dodge the known AMX
inline-asm incremental-compilation ICE — not a code bug). Verifier 49/0.
