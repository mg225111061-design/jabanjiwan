# HARAN v16 Part B — Type B (multi-language property-based probabilistic bug detection + AI fix)

Take arbitrary code → test metamorphic properties fast → narrow the suspect operations probabilistically
(shave the innocent-operation digits) → PROVE the digits (Hoeffding/Caesar, no overconfidence) → judge
by category → hand the fixer a minimal report → fix and re-verify. Deterministic, near-free, and honest
about what it cannot see.

## Pipeline (B1→B9)
- **B1 detect + HIR** — language by extension/syntax; Python → HIR (operations + line numbers). C/Rust/Go
  are registered DEFER extension points (engine shared) → v17.
- **B2 properties** — metamorphic/preservation/algebraic relations from shape+ops (+ optional AI);
  independence assessed (correlated properties grouped). Math/data-structure code = strong; arbitrary
  logic = property-poor (honest).
- **B3 property test** — run the real code on thousands of seeded inputs (~400k checks/s); record
  violations + counterexamples. Different bugs ⇒ different violated property sets.
- **B4 fault map** — a violated property accuses the operations that cause it; likelihood ratio (16×) per
  implicating property. Only violated properties accuse (held ones don't falsely exonerate).
- **B5 narrowing** — Bayesian posterior over operations; layers: property-bundle, input-bombardment,
  **causal mutation** (mutate the operator → does the property recover?), dataflow cross-val. Digits are
  TENTATIVE here.
- **B6 digit proof (the moat)** — Hoeffding/rule-of-three (to claim 10⁻ᵏ you need N≥-ln(δ)·10ᵏ;
  1e-6 ⇒ ~3M samples) + independence discount (correlated properties collapse to one factor) + Caesar
  PROVEN-BOUND. Digits are claimed **only when proven**; practical floor ~10⁻⁶.
- **B7 category verdict** — crash/safety ~99% (traceback line / sound interval AI; Apron polyhedra
  DEFER), performance ~99% (measured cProfile hotspot), correctness (top-k + proven prob). **Never
  mixed.**
- **B8 report + fix** — minimal report (where/why/minimal-cx/limit/grade/safe-zone/fix); mutation-repair
  guided by the report (Claude swappable via key), gated by an **independent** property re-test (context
  separation — the fixer can't self-certify).
- **B9 integration + measurement** — one `analyze()` call wires it all.

## Measured (curated corpus of representative Python bugs)
| metric | result |
|---|---|
| speed | ~50 ms / bug (vs Codex minutes–hours) |
| top-1 | 2/5 (40%) on property-violating bugs |
| top-5 | 4/5 (80%) on property-violating bugs |
| auto-fix | sort_cmp repaired + independently re-verified (operator bug) |
| determinism | same answer every run (seeded) |
| cost | ≈ $0 (no LLM in the deterministic path) |
| proven digit | innocent-op bound from N samples (e.g. ≤1.5e-3 @ N=2000), Caesar-stamped |

Per-bug (no cherry-pick — includes what we miss):
`sort_cmp` HIT@1 +fixed · `sort_drop` HIT@1 · `max_wrong` HIT@5 · `reverse_keep` HIT@5 ·
`scale_bug` **INVISIBLE** (no property catches it) · `biz_poor` MISS (property-poor).

## Honest limits (자백)
- **Property-invisible bugs are missed entirely** — Type B sees a bug only if a property breaks
  (scale_bug). "Perfect" means perfect *in the caught domain*; outside it we say so.
- **Real BugsInPy would score lower** — arbitrary business logic is property-poor; our corpus is
  math/data-structure (Type B's strength). The full BugsInPy checkout (network) is DEFER.
- **Digit ceiling ~10⁻⁶** — below that needs astronomical samples (no "infinite digits" mirage).
- **Mutation-repair fixes operator bugs only** — structural bugs (drop, missing branch) escalate to
  Claude/human, not faked.
- **Caesar proves the expectation bound**; the full (ε,δ) tail (Chernoff) stays DEFERRED.
- **No mirage**: zero persistent-homology / TDA / Ricci / LLL — only probability (SBFL/SmartFL/Caesar),
  metamorphic property testing, and abstract interpretation.
- "Codex보다 빠르다" is apples-to-oranges and holds **only when tests/properties exist** — stated plainly.

## One line
**If properties exist, Type B localizes the suspect operations in seconds with a PROVEN innocent-digit,
deterministically and near-free — and it is honest about the bugs no property can see.**
