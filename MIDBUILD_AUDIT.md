# MIDBUILD_AUDIT.md — JEFF / GACC

> Produced in response to **01 — MID-BUILD CHECKPOINT & ADDENDUM**, before any further
> feature code (§D.1). This is an honest self-audit of work-in-progress (constitution
> §2.4 / addendum §C). Priorities re-pinned (§A.6): **P0 never-miscompile > P1 honesty >
> P2 proof-carrying > P3 conservation > P4 performance > P5 DX.**

**Date:** 2026-06-13 · **Branch:** `claude/funny-maxwell-im9x07`

**Note on inputs (honesty, DR6):** I was given the constitution (`CLAUDE.md`, 2739 lines)
but **not** the `00-START-HERE` bootstrap referenced by the addendum. The three named
tripwires it requires map exactly onto constitution guarantees (P0/P2 type-gate, R31
fallback, DR1 no-fake-cert), so I implemented them by those canonical names and verify
them below. If the bootstrap differs in any detail, flag it and I will reconcile.

---

## §A.1 — VERIFIER-INTEGRITY PROOF (existential check)

**Verdict: the verifier is REAL, not a stub. Proven by tests + code enumeration.**

### Environment reality (ENV, honest — DR2/DR3)
`z3` CLI is **absent** and `lean` is **absent** in this container (`z3 --version` →
not found; `lean --version` → not found). Rather than fake a solver call, every
evidence kind is discharged by an **exact, in-house, deterministic, terminating**
checker — the quantifier-free variants the constitution itself endorses:
- `PolynomialIdentity` → coefficient-zero normalisation over ℚ (**APPENDIX F.1**, called
  "more robust" there).
- `Gf2LinearIdentity` → basis `{0,e_i}` evaluation (**F.3**); sound because the circuit
  is structurally linear (E.3).
- `EigenCharpoly` → Cayley–Hamilton `p(A)=0` over ℚ (**F.5**).
- `NumericResidual` / `PfaffianHolant` → exact modular/integer replay (**F.6/E.5**).

These are sound (return `Valid` only when the identity provably holds) and cannot hang
(no external process; replay is capped, R23). `jeff_verify::checker_name` reports the
*actual* checker ("exact-coeff-zero", "cayley-hamilton", ...), never "z3" when no solver
ran. **This is a documented design choice, not a stub** (DR2). Wiring real Z3/Lean is
deferred to a ticket; the gate (`verify_with`) is solver-agnostic, so adding them is
additive.

### Tripwire 1 — `false_certificate_is_rejected` (DR1/DR7, P0/P1)
A deliberately wrong cert must not verify (→ `None` → fallback). Covers a false
polynomial identity, a lied linear-recurrence term, and a tampered Pfaffian.
```
test tests::false_certificate_is_rejected ... ok
test tests::wrong_polynomial_identity_is_rejected ... ok
test tests::gf2_linear_identity_valid_and_tamper_rejected ... ok
```

### Tripwire 2 — `sorry_yields_fallback` (R31/DR8/R23)
The analogue of Lean `sorry` / Z3 `unknown` / timeout: when the checker cannot discharge
the obligation it returns `Unknown`, and `verify` yields `None` (never `Valid`). Here an
exact replay beyond its safety cap declines instead of hanging.
```
test tests::sorry_yields_fallback ... ok
test tests::unknown_yields_none_then_fallback ... ok
```

### Tripwire 3 — `unverified_collapse_is_unconstructible` (P0/P2, type-level)
`VerifiedCertificate` has a private field; the only constructor is `verify_with` (gated
on `Valid`). `Collapsed` requires a `VerifiedCertificate`. A `compile_fail` doctest
proves forging one does not compile:
```
test crates/jeff-cert/src/lib.rs - VerifiedCertificate (line 299) - compile fail ... ok
```

### grep audit of `-> Valid` and `VerifiedCertificate(...)`
- `VerifiedCertificate(...)` is constructed at **exactly one** site: `jeff-cert/src/lib.rs:307`
  inside `verify_with`, in the `VerifyResult::Valid =>` arm. (The struct definition is line 291.)
- Every `VerifyResult::Valid` in `jeff-verify` is the **last statement of a real check**:
  - `lib.rs:46` after `poly.is_zero()`; `:56` after `identity.is_zero()`; `:64` after
    `matrix.satisfies_charpoly()`; `:110` after the GF(2) basis loop; `:136/:163/:180/:206/:224`
    after exact recompute/compare in `ReplayChecker`.
  - `jeff-cert/src/lib.rs:367` is the `AlwaysValid` **test-only** checker (inside
    `#[cfg(test)]`), used to test the gate itself — not production.
- No path returns a verified result without an actual computation. **No `-> Valid` is unguarded.**

**A.1 result: CLEAN.**

---

## §A.2 — STUB-FOREST AUDIT

- `unimplemented!()` / `todo!()` / `unreachable!()`: **0** across the workspace.
- One-line doc-only stub crates (from ticket **T0.1**, "workspace scaffold — all crates stub"):
  **14** — `jeff-absint, jeff-backend, jeff-barvinok, jeff-codegen, jeff-collapse-arith,
  jeff-collapse-gf2, jeff-collapse-holographic, jeff-collapse-tensor, jeff-core-ir,
  jeff-jlir, jeff-recognizer, jeff-stdlib, jeff-test-oracles, jeff-types`. These are
  required scaffold (T0.1), contain zero logic, and are not claimed as implemented.
- Implemented & tested so far: `jeff-span, jeff-math, jeff-cert, jeff-verify, jeff-syntax`.

### Thin vertical slice — **does NOT execute yet** (honest)
The slice `sum i in 0..=n: i → … → run` is **not yet runnable**: the pipeline tail
(`jeff-core-ir → jeff-recognizer (stub) → jeff-collapse-arith (Faulhaber) → jeff-jlir
fallback harness → jeff-codegen → jeffc → execute`) is **not built yet**. I am mid-Stage-0:
**T0.2 (cert gate) and T0.3 (checkers) are done; T0.4/T0.6/T0.7 remain.** This is
*incomplete progress*, not fakery and not a stubbed verifier. Completing it is the
current milestone (see §A.3 / NEXT).

### Built slightly ahead of the bare slice — reported & justified (not removed)
The full `Evidence` taxonomy and **all six** checker kinds (not just T0.3's two), plus
math primitives (NTT, Bostan–Mori, charpoly, det, GF(2)). Justification: they are the
verification backbone the certificate schema (PART 6.1 / F.6) requires; all are real and
unit-tested; they are needed at Stages 1/3/5/8. Per **AR-5** I will **not** widen further
— next step finishes the slice, not new collapsers. I did not delete them: removing
tested-correct code adds regression risk for no benefit, and the addendum forbids
re-scaffolding.

**A.2 result: no fakery, no stub-forest. Thin slice incomplete (will finish — current milestone).**

---

## §A.3 — SCOPE & DRIFT CHECK

- **Current milestone:** Stage 0 (verification spine + minimal front end), in progress.
- **Drift into Stage 1+?** No. There is **no** Gosper/Zeilberger (Stage 1), **no** egglog
  recognizer (Stage 2), **no** kernel pack/absint (Stage 3), **no** Barvinok (4), GF(2)
  folder (5), backend Layer B (6), tensor (7), or holographic (8). `jeff-math` contains
  primitives some later stages will use, but only as a tested library backing the Stage-0
  certificate checkers.
- **Front end note:** `jeff-syntax` (full lexer+parser) is broader than `triangular`
  strictly needs, but `T0.6` requires `parse → …`, so a parser is in-scope for Stage 0;
  I built the whole grammar (APPENDIX A) once rather than re-touch it per fixture.
- **PRIORITY/ tripwire violations:** none found.

**A.3 result: CLEAN (no unauthorized stage jump).**

---

## §A.4 — BENCHMARK HONESTY CHECK

- No `benches/` content exists; no `criterion` harness; no "Nx faster"/"speedup" strings
  anywhere (grep clean). No performance is claimed because none is measured (R8). When
  benchmarks are added they will be measurements with kernel+N and non-uniform labelling.

**A.4 result: CLEAN.**

---

## §B — STANDING RULES acknowledged
AR-1 (green+honest commits) · AR-2 (totality ≠ collapse-decidability; collapse is always
recognize-or-defer) · AR-3 (secret leakage model will be stated when GF(2)/const-time is
built, Stage 5) · AR-4 (checker-first → naive-correct → fast; defer if unsure) · AR-5 (no
scope expansion) · AR-6 (re-pin context each session). All accepted; nothing currently
violates them.

## Test evidence (full suite, this audit)
`jeff-span 5 · jeff-math 11 · jeff-cert 5 (+1 compile_fail doc) · jeff-verify 8 ·
jeff-syntax 10` → **39 green, 0 failed**; `clippy -D warnings` clean on all implemented
crates.

## Conclusion (§D)
A.1 and A.2 reveal **no fakery, no stubbed verifier, no scope creep requiring removal** —
only honest incompleteness of the Stage-0 thin slice. Per **§D.3** (clean ⇒ continue the
current milestone only), the next step is to **finish the Stage-0 thin slice so it
executes** and returns the correct value, both collapse and fallback paths tested
(T0.4/T0.6/T0.7) — and then **STOP at the Stage 0 boundary and await `go`** before Stage 1.

---

## RESOLUTION (post-audit, same session) — Stage 0 thin slice now EXECUTES

The current milestone is complete. The slice `sum i in 0..=n: i` runs end to end:

```
$ jeffc build tests/e2e/triangular.jeff --collapse-report
fn triangular   collapsed  layer=1(arith/faulhaber)  O(1)  cert=ok(exact-coeff-zero)
$ jeffc run tests/e2e/triangular.jeff triangular 100000
5000050000
```

- Pipeline built: core-ir (lower + exact evaluator) → recognizer → Faulhaber collapse
  (closed form by interpolation, **verifier-checked** PolynomialIdentity) → JLIR →
  LLVM codegen (clang) / evaluator. Fallback at every boundary (R1).
- Collapse path == fallback path == naive sum (P0); forced-fallback still correct (R1).
- On-disk cert-replay (R25) re-verifies emitted certs and **rejects a tampered one**.
- `ci/run.sh`: build, clippy -D warnings, test, license-scan, determinism, cert-replay,
  bench-honesty — **ALL STAGE-0 GATES GREEN**.

Per **§A.3** (no Stage-1+ without an explicit human `go`), the build now **STOPS at the
Stage 0 boundary**. NEXT = Stage 1 (Holonomic: Gosper/Zeilberger) — awaiting `go`.
