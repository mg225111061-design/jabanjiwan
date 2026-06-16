# HARAN v17 Part D — Type A + B fusion (B detects, A proves)

Type B (v16) localizes suspect/slow regions in ordinary code. Part D injects Type A's machinery —
the fold engine (closed forms + certificate), Z3/JEFF (exact ∀ correctness), and Coq (unbounded ∀) —
into those regions. One `analyze_fused()` routes each function to the strongest applicable tool.

## The four injections
- **D1 fold** (`fold_inject`): a B-found loop → HARAN fold engine. Σ-shaped loop → closed form + proof +
  O(1); else NO_STRUCTURE. Loop→sum extraction for Python (`ast`) and C (pycparser); a **differential
  check** evaluates the closed form against the REAL loop, so a wrong range can never be reported CLOSED.
- **D2 Z3** (`z3_inject`): a spec (`# ensures result == …`) → synthesize HARAN fold+ensures → Z3/JEFF
  proves it ∀ or refutes it with a counterexample. No spec → properties as the (weaker) proxy.
- **D3 Coq** (`coq_inject`): a sort that passes bounded checks but needs all-lengths confirmation → route
  to Coq (v16 A3) → sortedness + permutation proven for ALL lengths (Z3 could only do length ≤ 4).
- **D4 pipeline** (`analyze_fused`): spec → verified/refuted · fold-able → closed-form · sort → unbounded
  or bug-localized · else → B localization with a proven digit.

## Measured (fusion corpus)
| outcome | count | example |
|---|---|---|
| fold closed-form | 3/3 of fold-able | Σi³ → ¼n²+½n³+¼n⁴ (=(n(n+1)/2)²) |
| Z3 verified ∀ | 1 | Σi + spec n(n+1)/2 → PROVEN |
| Z3 refuted | 1 | Σi + spec n² → FAILED, cx n=2 |
| Coq unbounded | 1 | correct sort → sorted ∀ lengths |
| bug localized | 1 | descending sort → compare@5 |
| NO_STRUCTURE | 1 | s=s*31+i recurrence |
Avg ~100 ms/function.

## Honest limits (자백)
- **Fusion value lands only on fold-able / spec'd / recognized code.** Arbitrary logic → NO_STRUCTURE or
  B-level localization, never a closed-form digit. Being in a "general" language does **not** make fold
  work more — it is the same engine (v17 rule 6).
- **Loop→sum extraction**: Python + C done; other languages share the engine once their extractor is
  added → DEFER.
- **Z3 is only as good as the spec**; no spec → properties proxy (weak without a spec).
- **Coq is semi-automatic**: it proves the canonical sort theorems unbounded (manual scripts); translating
  an ARBITRARY user algorithm to Coq is DEFER. Admitted proofs never counted.

## One line
**B finds the slow/suspect region; A collapses it to a closed form, proves it against a spec, or proves it
for all lengths — but only where real structure exists, honestly NO_STRUCTURE otherwise.**
