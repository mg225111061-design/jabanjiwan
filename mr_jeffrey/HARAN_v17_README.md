# HARAN v17 — multi-language coverage + Type A·B fusion + both upgrades

v16 gave Type B (property-based probabilistic bug detection) on Python and Type A (fold/Z3/Coq
verification). v17 widens and fuses them.

## Part C — frontline multi-language coverage (one engine, many frontends)
The Type B engine is language-agnostic: a frontend supplies HIR (operations + lines) + a
`make_callable` that runs the function in its **native runtime**. Covered: **Python, C, Go, Rust,
JavaScript, TypeScript, Java** — the SAME descending-sort bug is localized in all 7 (**top-1 = compare,
7/7**). Parsers: `ast`/pycparser/javalang (full) + a token scanner for Go/Rust/JS/TS (full AST = DEFER).
Runtimes: in-process, gcc, `go build`, rustc, persistent node/JVM workers.

## Part D — Type A + B fusion (B detects, A proves)
`analyze_fused` routes a region to the strongest tool:
- **D1 fold**: a B-found loop → fold engine → closed form + certificate + O(1), or NO_STRUCTURE. Python &
  C loop→sum extraction; a differential check guards correctness.
- **D2 Z3**: a spec (`# ensures …`) → proven ∀ or refuted with a counterexample; no spec → properties proxy.
- **D3 Coq**: a sort passing bounded checks → sortedness + permutation proven for **all lengths**.
Measured: fold-closed 3/3 of fold-able, Z3 verified=1/refuted=1, Coq unbounded=1, bug-localized=1,
recurrence → NO_STRUCTURE.

## Part E — both upgrades
- **E1 differential** (B↑): catches **property-invisible** bugs (v16's blind spot). git differential (last
  passing commit IS the spec) + N-version voting. The `x-1`-vs-`x*2` bug — invisible to every metamorphic
  property — is caught.
- **E2 Coq automation + fold class** (A↑): pure-automation unbounded ∀ theorems **3 → 6**; the sort proofs
  still need hand-written lemmas (probed, honest). Fold boundary mapped: poly + hypergeometric close;
  harmonic → ABSENT, factorial → NO_STRUCTURE; next classes DEFER (Zeilberger=engineering, higher-order
  holonomic = Gröbner/EXPSPACE FUNDAMENTAL, Kovacic = ODE domain).
- **E3 integration**: `analyze_v17` = detect → B/A fusion → differential (with reference) → category verdict.

## Final measurement (v17)
| dimension | result |
|---|---|
| languages covered | Python, C, Go, Rust, JS, TS, Java (7) |
| multilang top-1 | 7/7 (same bug, every language) |
| fusion fold-close | 100% of fold-able loops (Σi, Σi², Σi³) |
| Z3 injection | 1 verified ∀ + 1 refuted (counterexample) |
| Coq unbounded | sort all-lengths; auto theorems 3 → 6 |
| differential | catches property-invisible bug B cannot |
| determinism | same answer every run |
| mirage-free | no homology/TDA/Ricci/LLL — probability + properties + abstract-interpretation + fold only |

## Honest ceilings / DEFER (자백)
- **Fusion value lands only on fold-able / spec'd / recognized code** — arbitrary logic → NO_STRUCTURE or
  B-level localization; "general language" changes nothing (same engine).
- **Op→line for Go/Rust/JS/TS via heuristic token scanner** (full AST DEFER); **causal mutation + fix loop
  Python-only**; loop→sum extractor for languages beyond Python/C DEFER.
- **Dynamic typing is weak**: Python/JS scalar functions with polymorphic ops (`x*2` on a list) can be
  mis-analyzed without types — reference-mode (differential) is the reliable path there.
- **Z3 only as good as the spec**; **Coq semi-automatic** (manual sort lemmas; arbitrary-algorithm
  translation DEFER; Admitted never counted).
- **Fold ceiling**: higher-order holonomic = Gröbner/Mayr–Meyer EXPSPACE (FUNDAMENTAL); harmonic/factorial
  correctly non-closing.
- **Differential needs a reference**; **digit ceiling ~10⁻⁶**; property + differential still miss bugs no
  oracle distinguishes; Defects4J/BugsInPy full benchmarks DEFER (heavy checkout).

## One line
**Covered languages → property-based localization with a proven digit in seconds; fold-able regions → A
closes them to O(1) with a proof; specs → Z3/Coq prove them (all lengths); regressions → differential —
each only where real structure or a reference exists, honestly nothing otherwise.**
