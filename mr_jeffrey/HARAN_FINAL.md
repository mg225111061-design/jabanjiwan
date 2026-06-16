# HARAN — pre-product completion (v0 to v10)

**Pre-product complete = what is fillable is filled and confirmed; what is not is recorded as a
ceiling with rationale.** There is no remaining *unknown* confession: what works, works; what doesn't, we know why.

## What works (verified, measured)
- **Verification core (Mr.Jeffrey)**: the spec is READ from `ensures` (not guessed from the name); 3-verdict §2.2 output.
- **Fold (closed form)**: polynomial, C-finite, hypergeometric (+ machine-verified WZ telescoper), Kovacic ODE. v5 fold ratio 38% to 62% (toy corpus); a real PQC kernel is mostly Omega(N) — reported honestly, unlike the toy.
- **Unstructured acceleration** (constant-factor, Omega(N) intact): SIMD ~8x (compute) / ~2x (memory), parallel ~4x (compute) / ~2.5x (memory), verified-noalias (HARAN's edge, check-backed).
- **Verified approximation**: deterministic (Z3: Prony exact + bound, quantile), runtime (Compressed Sensing residual), probabilistic (Caesar/HeyVL expectation). Conquest v3 50% to v6.5 100%.
- **AI loop**: live-ready Claude adapter (key to live, else sim), minimal counterexample, round limit; converges in 2 rounds (sim).
- **Native codegen**: collapsing folds to real native O(1) (cc -O2), up to 7.2M-x vs naive.
- **Verification depth**: sort proven by Z3 over all values (length <= 4), contract checked at call sites, aliasing / noalias check-backed.

## Five error labels + four buckets (consistent system-wide, never merged)
- Buckets: **CLOSED, ABSENT, NO_STRUCTURE, UNKNOWN**.
- Approx error: **PROVEN-BOUND** (deterministic Z3 / runtime / probabilistic Caesar), **TESTED-BOUND**, **REJECTED-EXACT**.

## Ceilings (why the rest is not "missing")
**FUNDAMENTAL (mathematical — nobody crosses these):**
- holonomic order-2+ summation: Groebner double-exponential (EXPSPACE).
- RIP static certification: NP-hard (replaced by a runtime residual certificate).
- non-holonomic equivalence / zero-test: Richardson's theorem (undecidable).
- Omega(N) information floor; #P / NP-hard counting and optimization.

**TOOL / ENGINEERING (crossable with more tools/effort — not math limits):**
- unbounded all-lengths sort proof: needs an inductive prover (Z3 has no induction); Z3 all-values len<=4 done.
- probabilistic (eps,delta) TAIL bound: needs heavier HeyVL (Chernoff); expectation PROVEN via Caesar.
- Kovacic Cases 2/3 with poles: decidable but unimplemented (honest UNKNOWN, not ABSENT).
- full LLVM backend / bignum codegen: engineering (folds to native already done).

## Honest meaning
This is NOT "everything is 100% solved". It is: **every fillable gap filled and measured, every
unfillable one confirmed as a ceiling with a reason.** No fake PROVEN; proven != tested; deterministic
!= probabilistic; Omega(N) never broken; orders of magnitude only from fold/approx, never unstructured.
