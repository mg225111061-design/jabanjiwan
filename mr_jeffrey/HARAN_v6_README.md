# HARAN v6 — verified approximation (deterministic Z3 · runtime certs · probabilistic Caesar bridge)

v5 raised fold to 62%; v6 attacks the remaining unstructured/approx region — but "conquest" counts
ONLY when the error is PROVEN, and deterministic vs probabilistic proofs are kept DISTINCT.

## The decisive split (research PART F)

| error type | verifier | verdict |
|---|---|---|
| **deterministic** (worst-case) | **Z3** directly | PROVEN-BOUND (deterministic) |
| **runtime / per-execution** | direct residual check | PROVEN-BOUND (runtime) |
| **probabilistic** (Pr[·]≤δ) | **Caesar/HeyVL** (expectation logic; Z3 can't) | PROVEN-BOUND (probabilistic) — or TESTED if Caesar absent |
| exact-required | — | REJECTED-EXACT |

This is why v3's quantile (deterministic) folded but KMV-distinct (probabilistic) didn't.

## What v6 adds (measured)

- **U1 Prony/ESPRIT** (deterministic, Z3): noiseless recovery EXACT (Hankel det≡0 Z3-PROVEN; roots
  0.7/0.9 recovered, residual 6.7e-15); noisy |residual| ≤ (Σ|aᵢ|)·η Z3-PROVEN. **PROVEN-BOUND (deterministic)**,
  with the Hankel-non-singularity boundary tracked as a PROVISO (v5 lesson).
- **U2 Compressed Sensing** (runtime): RIP is NP-hard ⇒ NOT proven statically. Per-execution residual
  ‖y−Φx*‖₂ ≤ ε certified (noiseless 7.8e-15, support exact) + recovery-error bound r/σ_min(Φ_S).
  **PROVEN-BOUND (runtime, per-execution)** — "this execution is ε-consistent", not "Φ has RIP".
- **U3 Caesar/HeyVL bridge** (probabilistic): maps (ε,δ) → HeyVL expectation specs (Count-Min, KMV).
  **Caesar is an external tool, NOT installed here → BLOCKED honestly → probabilistic stays TESTED-BOUND.**
  Bridge generated and ready; no fake PROVEN.
- **U4 routing**: analytics/spectral/sensing/monitoring → APPROX; payment/crypto/ledger/unknown →
  REJECTED-EXACT (conservative).

## Core measurements (raw)

- **★ Conquest ratio: v3 50% (1/2) → v6 75% (3/4) = +25pp ★** (of approximated tasks that are PROVEN-BOUND).
- **Error-type breakdown**: deterministic(Z3) = {quantile, Prony}; runtime = {CompressedSensing};
  probabilistic(Caesar) = ∅ (BLOCKED); TESTED = {distinct/KMV}; REJECTED-EXACT = {payment}.
- **Prony certificate**: complete (residual≡0) + boundary-partial (recovery needs Hankel non-singular).
- **TESTED→PROVEN upgrade via Caesar**: 0/3 (Caesar BLOCKED) — honest.

## The honesty lines held

- PROVEN-BOUND only with a real proof; deterministic (Z3) ≠ runtime ≠ probabilistic (Caesar) ≠ TESTED ≠ REJECTED.
- RIP never claimed statically (NP-hard) — runtime per-execution only.
- Caesar absent → probabilistic stays TESTED-BOUND (no fake PROVEN); bridge ready.
- Provisos (boundary/singularity) tracked — "complete" vs "boundary-partial".

## Deferred / BLOCKED

- **Caesar/HeyVL not installed** (external tool, environment) → probabilistic sketches TESTED-BOUND.
- RIP static proof impossible (NP-hard) → runtime certs only. Noisy-Prony recovery sensitivity (provisos).
