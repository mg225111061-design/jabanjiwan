# HARAN v9 — minimal native codegen (fold → real native O(1))

v4+ confessed "no native codegen". v9 fills it PARTIALLY and honestly: a fold that COLLAPSES to a
closed form is emitted as real C, compiled (cc -O2), and run natively.

## R1 — fold → native
- `Σk²` → closed form `1/6·n + 1/2·n² + 1/3·n³` → emitted as a native C function (common-denominator
  integer polynomial), compiled with `cc -O2`, run natively.
- **Native matches the interpreter** (n=10/100/1000) and the C-internal closed==naive check holds at
  n=1e6 (the interpreter is O(n) and step-limited there).

## R2 — noalias + measurement
- Emitted `axpy(const double* restrict a, ..., double* restrict c, ...)` — `restrict` is SAFE because
  v8 *checked* the disjointness (own/&mut); C alone can't prove it. Compiles clean.
- **Measured native fold (O(1)) vs native naive loop (O(n))**:
  n=1e3 → 764× · n=1e5 → 83,742× · n=1e7 → **7,248,656×**. Native closed form is ~1 ns flat (O(1)
  confirmed in compiled code); naive grows linearly — orders of magnitude, in REAL native code.

## ★ Honest scope / DEFERRED
- **Only collapsing folds codegen** to native (non-folding code is correctly NOT emitted).
- **Native uses `long long`** — the arbitrary-precision (bignum) path stays interpreted → DEFER.
- **A full LLVM backend** (general code, all types, optimization passes) is months of work → DEFER.
- "Partial codegen + honest DEFER" is the v9 result — not a complete compiler.
