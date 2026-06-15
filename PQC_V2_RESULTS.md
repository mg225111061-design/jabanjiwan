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

## §C v2-B (start) — constant-time audit + TFF applicability — **AUDIT DONE + 1 fix + TFF applicable**

Raw grep+analysis of the PQC code (`mlkem`/`mldsa`/`kyber`/`dilithium`/`keccak`/`modular`) for
secret-dependent branches and memory indices:

| site | finding | status |
|---|---|---|
| ML-KEM `decaps` FO check (`if c == c2.as_slice()`) | slice `==` short-circuits + `if/else` branch on the secret-derived re-encryption ⇒ **not constant-time** | **FIXED** |
| ML-DSA `sign` rejection (`continue` on ‖z‖/‖r0‖/hint bounds) | variable iteration count on secret-derived candidates | **standard caveat** (ML-DSA refs are variable-time here; constant-time signing is open research) — stated, not hidden |
| ML-DSA `sample_eta` / ML-KEM `sample_ntt` rejection | RejBoundedPoly rejects on σ (secret-derived for `sample_eta`); ExpandA rejects on ρ (public) | `sample_eta` variable-time (standard); ExpandA OK (ρ public) |
| ML-KEM `sample_cbd` | reads a **fixed** byte count, no rejection | **constant-time OK** |
| ML-DSA `sample_in_ball` index `c[jj]` | `jj` derived from `c̃` (the challenge, a **public** signature element) | **OK** (not secret) |
| modular inverse (variable-time ext-Euclid) | **no `.inv()`** on the PQC hot paths (grep empty) | **OK** |

**Fix applied (the one real finding):** `decaps` now uses `ct_eq` (compares **all** bytes, no early
exit) + `ct_select` (mask-based, no branch) — data-oblivious FO implicit rejection. Output is
**byte-identical** to the branch version, so the official ACVP decaps KAT (10/10 each set) and the
implicit-rejection test still pass. (Source-level constant-time; hardware micro-timing/cache is out
of scope and stated — same honesty boundary as v1 G.4.)

**TFF applicability (Stage 38 → constant-time):** built `jeff_math::cttaint` — a secret-taint
abstract interpretation that **reuses the TFF fixpoint framework** with a different lattice:
- TFF: interval lattice (infinite height ⇒ widening/narrowing).
- taint: `Public ⊑ Secret` (height 2 ⇒ plain Kleene, no widening) — same complete-lattice laws
  (`leq`/`join`/`⊥`), verified against TFF's interval lattice in `tff_framework_reused`.

`analyze` runs the monotone taint fixpoint and flags **branch-on-Secret** / **index-by-Secret**
(`secret_branch_flagged`, `secret_index_flagged`); data-oblivious arithmetic passes
(`oblivious_arithmetic_passes`), and the v2-B `ct_select` mask pattern passes
(`ct_select_pattern_passes`) while the old `if c==c2` pattern is flagged. Certificate kind:
**taint-lattice / sound over-approximation** (never misses a real leak; may be conservative).

**Honest scope:** v2-B is *audit + one fix + applicability* (per the directive). A full PQC-wide
constant-time certificate (taint-analyze every `mlkem`/`mldsa` path, prove no secret branch/index
end-to-end) is the next build — the engine (`cttaint` over the TFF framework) is now in place.
6 cttaint tests. Workspace 564/0, clippy `--all-targets -D warnings` clean, verifier 49/0.

## v2 status table
| item | status |
|---|---|
| ML-KEM-512/768/1024 ACVP byte-exact | **BUILT** (keyGen/encaps/decaps, all sets) |
| ML-DSA-44/65/87 ACVP byte-exact | **BUILT** (keyGen/sigGen/sigVer, all sets) |
| ML-KEM decaps constant-time FO check | **FIXED** (ct_eq + ct_select, ACVP-preserving) |
| constant-time audit (secret branch/index) | **DONE** (1 real finding fixed; ML-DSA rejection = standard caveat) |
| TFF → constant-time taint analysis | **APPLICABLE + demonstrated** (`cttaint`); full PQC sweep = next build |
