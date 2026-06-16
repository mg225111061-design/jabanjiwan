# HARAN v6.5 — Caesar/HeyVL activated (probabilistic approximation PROVEN)

v6 left Caesar BLOCKED (external tool absent). v6.5 activates the real **Caesar 4.0.2** binary
(Z3 static-linked) and upgrades probabilistic sketch error bounds from TESTED-BOUND to
**PROVEN-BOUND (probabilistic, Caesar)**.

## What works (measured)
- **Caesar executes** (`caesar 4.0.2`, features `static_link_z3`): `verify` works without Storm.
- **Genuine**: Caesar VERIFIES a correct HeyVL `proc` (fair coin E[x]=0.5 → "veni, vidi, vici!") and
  REJECTS a wrong one (claims 0.7, "pre-quantity evaluated to: 0.5" → 1 failed). Not always-pass.
- **Bridge end-to-end**: `caesar_bridge.py` maps a sketch's first-moment bound to a HeyVL `proc` and
  runs Caesar:
  - Count-Min: E[per-item overestimate] = 1/w → **PROVEN-BOUND (probabilistic, expectation)**.
  - KMV/HLL: E[1{hash<t}] = t → PROVEN-BOUND (probabilistic, expectation).
- **Upgrade**: probabilistic sketches TESTED→PROVEN = 2/2 (Count-Min, KMV expectation bounds).
- **Conquest ratio**: v6 75% (Caesar blocked) → **v6.5 100%** (4/4 approximated PROVEN-BOUND).
- **Five error types stay distinct**: PROVEN(deterministic Z3) / PROVEN(runtime) /
  PROVEN(probabilistic Caesar) / TESTED / REJECTED — never merged.

## Honest scope / DEFERRED
- Caesar proves the **EXPECTATION (first-moment)** bound. The full high-probability **(ε,δ) TAIL bound**
  (Chernoff concentration) is a harder HeyVL proof → **DEFERRED**. So a probabilistic PROVEN here means
  "expected-error bound proven", distinct from a deterministic worst-case Z3 proof.
- The Caesar binary (40 MB) is an **external, ephemeral** tool — gitignored, not committed. Provide it
  via `tools/caesar/` or `CAESAR_BIN`. If absent, v6.5 tests SKIP and probabilistic stays TESTED-BOUND.
- `mc` (Storm model checking) not exercised — `verify` (Z3) suffices for the expectation bounds.
