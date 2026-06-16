# HARAN v11 — general native codegen (scalars / ops / loops → C → native)

v9 emitted only a fold's CLOSED FORM (O(1)). v11 is the GENERAL lowering: any finite HARAN function
body — scalar arithmetic, operators, `let`/`block`, `match`, and non-closing `fold` loops — lowers to
C (`cc -O2`), runs natively, and is checked against the interpreter.

## Two-layer philosophy (what is HARAN-unique, what is just LLVM)
- **Layer 1 (HARAN-unique):** `fold` shrinks code to a closed form — *orders of magnitude* — and
  attaches verified facts (refinement → range assume, own/& → `noalias`/`restrict`) as metadata.
- **Layer 2 (LLVM/C):** whatever does NOT close lowers to a plain native loop — C-grade, constant
  factor, Ω(N) intact. **The loop lowering never invents orders of magnitude.** Only the closed form
  (v9) does. fold does not "accelerate LLVM"; it works *above* LLVM to reduce LLVM's work.

## K1 — scalar / operator codegen
- `poly(n) = n*n + 2*n + 1` → native; `poly(10) = 121`. `**` with a literal exponent expands to repeated
  multiply; `%`, comparison, boolean ops mapped to C.
- **Native == interpreter** for ops, `match` (→ if/else-if/else chain), and 2-argument functions.

## K2 — loop (non-closing fold) codegen
- `fold k in 1..n { k*k }` lowers to a real C `for`-loop with a fresh accumulator; `s(100) = 338350`
  matches the interpreter.
- **The verified loop range is reflected directly as the C bounds** — invariant `k ∈ [1, n]`.
- **Measured Ω(N), not orders of magnitude:** native loop `t(1e7) ≈ 14 ms → t(1e8) ≈ 128 ms` (~10×,
  linear in n). This is the honest point: the general loop is C-grade; only the v9 closed form is O(1).

## K3 — verification metadata into codegen
- Refinement param `{ x: Int | x < 100 }` → `if (!(x < 100)) __builtin_unreachable();` — a verified
  range assume `-O2` may exploit. Invariant recorded: `m: (m < 100) (refinement assumed)`.
- `noalias` kernel emitted with `restrict` (backed by v8's aliasing check — C cannot prove it itself).
- **HONEST:** we pass *proven* facts, never assume unproven ones; the optimizer uses them
  opportunistically — no speedup is claimed beyond what is measured.

## Scope / next
- Codegen via C (`cc -O2`) by default; direct LLVM IR is v15. Lists/ADT patterns, `FnCall` attributes,
  `Recur`, bignum, `Vec`, and `own/&` RAII are the v12–v15 stages.
