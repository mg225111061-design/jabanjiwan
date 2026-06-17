# HARAN v21 — verification speed (perceived-zero) + two modes

Make verification *imperceptible* in the common case and offer a speed/quality choice. Built on v16's
caching (×19) and parallelism (×3), extended with tiering, fold-first, incremental, and background — all
measured. The claim is **"same speed + Xms for a PROVEN result"**, not "infinitely fast".

## R1 baseline (8-item corpus, easy folds → hard Coq)
6/8 fast (<50ms, fold/Z3) · 2/8 slow (Coq unbounded ∀, ~360ms, **fundamental** inductive cost). avg 90ms,
dominated by the few hard cases. This is what every optimization is measured against.

## Optimizations (each measured before/after)
- **R2 fast-path tiering** (abstract-interp/fold → Z3 → Coq, escalate only when needed): **75% resolve at
  the fast tier (~14ms) without Coq**; common case ×25 vs forcing it through Coq; overall ×3.6.
- **R3 fold-first** (closed form ⇒ O(1) verify): fold-first stays **flat ~9ms while n grows 100×**; naive
  evaluation is O(n) → **×92 at n=10⁷**, unbounded. (Structured/fold-closeable code only; unstructured = Ω(N).)
- **R4 incremental** (re-verify only what changed): edit one function → **~6ms** (×9 vs full ~53ms); Merkle
  dependency invalidation (edit `a` → {a, its callers} re-verify; rest cached).
- **R5 background** (hard cases don't block): user **blocked ~55ms** (fast tier) while ~680ms of Coq runs
  in the background; honest status ✅proven / ⏳verifying / ❌counterexample / ⏱️timeout.

## R6 — perceived-zero speed table (vs baseline 90ms)
| dimension | result |
|---|---|
| easy verify (fast-path/fold) | **~13ms** (perceived 0) |
| edit loop (incremental) | **~6ms** (perceived 0) |
| hard proposition | user-blocked **~54ms**, ~677ms in **background** (not blocking) |
| **code-gen vs gen+verify** | gen ~9ms → **+13ms for a PROVEN result** |

## R7 — two modes (same engine, different depth)
| mode | solved | time | hard ∀ |
|---|---|---|---|
| **NORMAL** (speed) | 6/8 | **~80ms** | UNRESOLVED-shallow (not attempted deep — *not wrong*) |
| **EXTENDED** (quality) | **8/8** | ~770ms | PROVEN (deep Coq + background) |
NORMAL is **×~10 faster**; EXTENDED solves **+2 more** (the hard unbounded-∀). **Both give ZERO wrong
answers.**

## Honesty (the discipline)
- **"Perceived zero" = not blocked, NOT "instant".** Easy/edit/fold are genuinely ~ms; hard
  (NP-hard/inductive) cases are intrinsically slow and run in the **background** — the user isn't made to
  wait, but the work still takes time. No "always instant" claim.
- **Extended ≠ "solves everything".** It solves *more*; NP-hard/inductive timeouts are reported honestly
  as ⏱️ UNRESOLVED-timeout. No "all" claim.
- **Correctness is invariant.** Every optimization is sound (fold is a proven transform; fast tools are
  sound; no skipping; cache never returns a stale verdict). **Normal mode is shallow but never *wrong*** —
  it may say UNRESOLVED-shallow, never a false PROVEN.
- Measured numbers only (before/after ms, real speedups); the slow Coq cases are recorded as fundamental,
  not "fixed".

## One line
**Easy verification and the edit loop are perceived-zero (~6–14ms); hard proofs never block (background);
"+~13ms for a proven result"; and two modes let you pick NORMAL (×10 faster, shallow-but-never-wrong) or
EXTENDED (solves the hard ∀ too) — all measured, correctness invariant.**
