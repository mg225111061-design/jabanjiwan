# HARAN v17 Part C — frontline multi-language coverage (one engine, many frontends)

v16 built Type B with B1 designed as "engine common, frontend per language." Part C fills that extension
point with real frontends so the SAME engine (properties → testing → fault-map → narrowing → digit
proof) runs on seven languages. A frontend supplies two things: HIR (operations + lines) and a
`make_callable` that runs the function in its native runtime — the engine never knows the language.

## Covered languages
| language | parser (→HIR ops) | runtime (property testing) | typing |
|---|---|---|---|
| Python | `ast` (full) | in-process exec | dynamic |
| C | pycparser (full AST) | gcc compile+run | static |
| Go | token scanner | `go build` compile+run | static |
| Rust | token scanner | `rustc` compile+run | static |
| JavaScript | token scanner | persistent `node` worker | dynamic |
| TypeScript | token scanner | `tsc` → `node` worker | static |
| Java | javalang (full AST) | `javac` + persistent JVM | static |

## Measured (the SAME descending-sort bug in every language)
All 7 covered → **top-1 = `compare` (7/7)**; the engine finds `ordered_output` violated from the real
native output and accuses the comparison. Speeds (first run, incl. compile): Python 2ms, JS 66ms,
C ~0.7s, Rust ~0.9s, Java ~0.7s, TS ~1.3s, Go ~5s (cold stdlib build; cached after). Native binaries are
cached; JVM and node use long-lived workers so per-input checks stay fast.

## Honest limits (자백)
- **Op→line extraction**: Python/C/Java use full ASTs; Go/Rust/JS/TS use a heuristic **token scanner**
  (full AST via syn/go-parser/tree-sitter is DEFER; esprima's native build failed). Op KINDS are
  reliable; line precision on dense lines is approximate.
- **Static-typed languages are stronger** (explicit array/scalar shape); dynamic JS/Python infer shape
  from usage (weaker), exactly as v16 noted for Python.
- **Causal mutation (B5 L4) and the fix loop (B8) are Python-only** — other languages get static LR
  narrowing + the digit certificate, but not (yet) operator-mutation repair or op→line refinement.
- **Per-language scope**: numeric / int-array / scalar shapes. C pointer aliasing, structs, Rust
  ownership, Go goroutines, JS async, Java OOP (inheritance/generics) are DEFER.
- **Defects4J** (Java) and **BugsInPy** (Python) real benchmarks need heavy per-project checkouts → DEFER;
  single bugs are demo'd end-to-end per language.

## One line
**One probabilistic property engine, seven language frontends: detect → parse → run natively → localize
the suspect operation with a proven digit — strongest on statically-typed, numeric/data-structure code.**
