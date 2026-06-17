# HARAN v18 — proof-boundary diffusion FL (a bet that was MEASURED, and did not pay off)

The idea: diffuse bug suspicion over the program dependency graph as heat (graph Laplacian / random walk —
the PRFL formulation, mathematically the discrete heat equation, **not** a fluid metaphor), with HARAN's
verification assets as boundary conditions — Bayesian posterior as initial heat, property violations as
sources, and the novelty **proven-safe statements as heat sinks** (which PRFL cannot do). Then **measure**
whether it beats the existing v16 property method. Addition + comparison, never replacement.

## What was built
- **G1 PDG**: statements → nodes, data (def→use) + control edges. Python (ast) + C (pycparser); others DEFER.
- **G2 diffusion**: graph Laplacian L=D-A, personalized PageRank and heat-kernel solvers (scipy, ~ms).
- **G3 proof boundary**: Bayesian initial heat (B5) + violation sources (B4) + **proven-safe sinks**
  (Z3/D2, Coq/D3 = STRONG; abstract-interp/B7 = WEAK crash-safety) + Hoeffding conductance (B6).
- **G4 solve+rank**: absorbing random walk — heat drains into sinks; rank surviving lines.
- **G5 three-way comparison**: property vs pure diffusion vs proof-boundary diffusion.

## Measured result (curated 6-bug corpus, top-1 / top-5)
| method | top-1 | top-5 |
|---|---|---|
| **v16 property** (B5 narrow) | **67%** (4/6) | **100%** (6/6) |
| pure diffusion (PRFL-style, no sinks) | 67% (4/6) | 100% (6/6) |
| proof-boundary diffusion (weak sinks) | 50% (3/6) | 67% (4/6) |

## Honest conclusion: **KEEP property. The bet did not pay off.**
- **Pure diffusion only TIES property** (67%/100%) — it does not beat it, so the simpler property method
  stays (per the v18 rule: tie → keep simple).
- **Proof-boundary diffusion with WEAK (abstract-interp) sinks is WORSE** — it *drained real correctness
  bugs*: `sort_dup`'s bug at L2 (`sorted(set(xs))`) and `partial`'s bug at L4 (inner-loop range) were
  clamped as "safe" because they are crash-safe and not directly property-implicated, yet they ARE the
  bug. This is exactly the risk flagged in G3: **crash-safety ≠ correctness**, so a weak sink can absorb
  the very line you are hunting.
- **Strong sinks (Z3/Coq correctness proofs)** would be safe (a proven-correct line cannot be the
  correctness bug), but arbitrary buggy code rarely carries such proofs, so they reduce to pure diffusion
  here (no effect). The boundary-condition novelty is sound in principle but has **no measured benefit on
  this corpus**.

The diffusion machinery (G1–G4) is committed and works; it is **not adopted** into the default pipeline.
v17's property/fusion path remains the answer.

## Discipline check
- **No mirage**: diffusion is graph-Laplacian / random-walk only (the PRFL-measured formulation); zero
  fluid-dynamics / homology / TDA / Ricci / LLL.
- **No favoritism**: the bet lost on measurement and we say so plainly — property is kept.
- Existing suite stays green; confidences un-mixed.

## DEFERRED / honest
- Non-Python/C PDG (need per-language def-use); precise aliasing/pointers; Defects4J/BugsInPy full
  benchmarks (heavy checkout). A larger corpus *might* shift the numbers — but on what was measured,
  diffusion did not win, and weak proof-sinks actively hurt.

## One line
**We tried diffusing suspicion with proof boundaries; we measured it honestly; it tied at best and the
weak-sink variant hurt — so we keep the v16 property method and report the loss without spin.**
