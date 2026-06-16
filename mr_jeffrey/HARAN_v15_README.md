# HARAN v15 — direct LLVM IR backend + Proc/Cofix finite-prefix (the last version)

v11–v14 emit C and let `cc` build the object. v15 emits **LLVM IR (`.ll`) directly** and lowers it with
`llc` — so HARAN states the verified metadata explicitly, and the coinductive forms get the only
codegen they honestly can.

## O1 — direct LLVM IR (full metadata, not C hints)
The compute path is `.ll → llc → native` (only a tiny I/O shim stays in C):
- **Scalar** `poly(n)=n*n+2*n+1` → hand-built SSA IR (`mul`/`add`) → `llc` → native; `poly(10)=121`,
  == interpreter at n up to 1000.
- **Map kernel** → IR with **`noalias` pointer attributes** (SAFE because verified `own`/`&mut` — C
  needs an unchecked `restrict`) and **`!llvm.loop.vectorize.enable`**. `[1,2,3,4,5]→[3,5,7,9,11]`.
  These are *explicit* metadata the C path can only hint at.
- **Refinement** `{x | x < 100}` → **`!range !{0,100}`** on the load — a verified fact becomes IR
  metadata the optimizer can rely on. `sq(9)=81`.

## O2 — Proc/Cofix finite prefix (infinite is fundamentally impossible)
- A **PROVEN-productive** `cofix loop { yield E  loop }` (gated by `productivity.check_proc`) compiles
  to a generator of its **first N** elements: `rep(7)|N=5 → [7,7,7,7,7]`, `sqs(6)|4 → [37,37,37,37]`.
- **The bound N is mandatory.** There is NO infinite codegen — you cannot materialize ℵ₀ outputs, and a
  non-terminating program is not a "compiled result." We emit a finite prefix, never an infinite loop.
  **This is the honest ceiling, stated plainly.**
- A **non-productive** `cofix loop { loop }` (spin) yields nothing → **no prefix emitted** (honest, not
  faked).
- Scope: the surface `cofix` doesn't thread state through its recursion, so the prefix is a guarded
  constant-yield stream (E may depend on params). Richer stateful streams need state-threading the
  surface doesn't expose → honest DEFER.

## O3 — integration + coverage
- **Backends agree:** the same HARAN function through different backends gives identical results —
  v11 C ≡ v15 LLVM (scalar & map); v12 i64 ≡ v13 bignum for n ≤ 90 (below i64 overflow), after which
  v13 continues exact.
- **Coverage (measured, `coverage.py`):** **87%** line coverage across the six codegen modules
  (`codegen` 95%, `haran_recur` 95%, `haran_vec` 92%, `haran_llvm` 83%, `haran_codegen` 82%,
  `haran_bignum` 74%).

## Honest meaning of the v11→v15 backend
LLVM removes nothing from the conservation law: Ω(N) loops stay Ω(N); only structure (fold→closed form,
linear recurrence→companion) gives orders of magnitude, and only GMP gives unbounded exactness. v15's
contribution is a **real IR path with precise verified metadata** + the **honest finite-prefix ceiling**
for coinduction — not speed magic.
