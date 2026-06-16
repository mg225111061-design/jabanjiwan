# HARAN v16 Part A — Type A upgrade (verification speed + Coq for unbounded ∀)

Goal: make "Mr랑 짝짝" real — verification *fast* in the edit loop, parallel across cores, and able to
prove the unbounded ∀ properties Z3 never could.

## A1 — verification caching (skip re-verify of unchanged functions)
Each function is keyed by a **Merkle hash**: its span-free AST+spec combined with the content hashes of
its transitive callees. Unchanged → cache hit (prover skipped). Change a dependency A → every caller B's
key changes → B is re-verified automatically.
- **Measured edit-verify loop: cold full verify → warm unchanged ≈ ×19–20** (all hits, no prover).
- Editing one function re-verifies only that function (+ its callers).
- HONEST: the **first** full verification still pays the prover in full.

## A2 — parallel verification (function-level, multicore)
`verify_fn` only reads the (read-only) function table, so functions are verification-independent and fan
out across cores with a process pool (fork + shared globals → tasks carry only an index, no O(n²)
pickling).
- **Measured: 16 heavy obligations, seq → 2 cores ×1.65, 4 cores ×3.0**; verdicts identical to sequential.
- HONEST: sub-linear (fork/IPC, result pickling, per-process Z3 startup); a **tiny/fast** suite is
  overhead-bound (×~1) — reported, not hidden.

## A3 — Coq integration (unbounded ∀ — the Z3 ceiling crossed)
Z3 has no induction, so v8 proved sort correct only for ∀-VALUES at **length ≤ 4**. Coq proves by
induction → **all lengths / all n**. `coqc` is gated (BLOCKED honestly if absent; here Coq 8.18 is live).
- HARAN spec → Coq theorem for recognized shapes (sortedness+permutation, length-preserving, Faulhaber);
  arbitrary specs → honest DEFER.
- **5/5 unbounded ∀ proven** (coqc QED, no admits):
  - `map`/`rev` length-preservation, Faulhaber Σi=n(n+1)/2 — **auto** (one tactic);
  - **insertion-sort sortedness + permutation for ALL list lengths** — **manual** (hand-written script).
- Honesty gate: a proof with `Admitted`/`admit`/`Axiom` compiles but is **never** counted as proven.
- HONEST (A3.3): Coq proofs are hand-written where automation doesn't close them — that's the cost of
  crossing the induction ceiling.

## A4 — integration (`FastVerifier`) + speed summary
`FastVerifier` = caching + parallelism: cache hits are instant, the misses (changed functions) re-verify
in parallel. Verdicts equal the plain sequential Mr.Jeffrey.

**Measured verification speed (honest):**
| dimension | result |
|---|---|
| edit-verify loop (caching) | ×~19 on unchanged re-verify |
| parallel (4 cores) | ×~3.0 on independent obligations |
| Coq unbounded ∀ | 5 theorems proven for all inputs (vs Z3 length ≤ 4) |
| caveat | first full verify + non-linear SMT still slow |

The win is the **edit loop** + **multicore fan-out** + **unbounded ∀ for recognized shapes** — not the
intrinsic cost of one hard proof, which is unchanged. Part B (Type B) builds on this.
