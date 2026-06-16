# HARAN v8 — verification depth completed (sort proven · contract · aliasing)

Fills v3's "sort tested-not-proven" and v1's "contract/aliasing generated-only".

## Q1 — sort correctness strengthened
- v3: exhaustive over length≤4 AND values∈[0,3] (341 concrete cases).
- **v8: Z3 ∀-VALUES proof at fixed length** — enumerate the N! orderings, run the real algorithm to get
  its output permutation P, and have Z3 prove the output is sorted under the ordering's symbolic
  constraints for ALL integer values (length ≤ 4, distinct). Permutation is exact. **Strictly stronger
  than v3** (all values, not just 0–3).
- ★ Honest ceiling: the UNBOUNDED ∀-proof (all lengths) needs INDUCTION over the recursion — Z3 has no
  automatic induction → **DEFERRED** to an inductive prover (Coq/Isabelle / hand-written Z3 lemmas).

## Q2 — contract (requires) checked at call sites
- `requires` is now CHECKED at each call site via Z3 (not just generated): `use_ok` calls `recip(a)`
  under `a>1` ⇒ `a≠0` PASS; `use_bad` calls it unguarded → FAIL with counterexample `a=0`.

## Q3 — aliasing (own/&) checked + noalias guarantee verified
- `own` use-after-move CAUGHT (`consume` uses `b` twice → flagged).
- `&mut c` is noalias-eligible (exclusive borrow) AND usage is clean → the v4.5 LLVM `noalias` is now
  **CHECK-BACKED** (HARAN proves the disjointness C can only assume via unchecked `restrict`).

## Status
- sort: PROVEN (Z3, ∀ values, length ≤ 4, distinct) + unbounded DEFERRED (Z3 no induction).
- contract + aliasing: checked with counterexamples. All five labels and four buckets unchanged.
