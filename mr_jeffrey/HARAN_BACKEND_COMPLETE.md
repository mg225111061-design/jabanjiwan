# HARAN — complete native/LLVM backend (v11 → v15)

The two-layer compiler is complete: **Layer 1 (HARAN-unique)** shrinks code by structure (fold→closed
form, recursion→companion) and attaches verified facts as metadata; **Layer 2 (LLVM/C)** natively
compiles the reduced code. fold does not accelerate LLVM — it works *above* LLVM to reduce its work.

## What each version added (all committed, all tests green)
| ver | stage | added | grade it earns |
|---|---|---|---|
| v11 | K1–K3 | general native codegen: scalars/ops/loops/match/let → C; refinement→assume, noalias→restrict | loops = **C-grade Ω(N)** |
| v12 | L1–L3 | FnCall (whole call graph); recursion split by structure: linear→companion O(log n), tail→loop, general→honest recursion | linear rec = **orders of magnitude** |
| v13 | M1–M2 | arbitrary-precision via GMP `mpz`: fold closed form O(1) exact, companion O(log n) exact (fib(100000)=20899 digits) | **exact, no i64 ceiling** |
| v14 | N1–N2 | Vec codegen (static/dynamic/SIMD) via `map`; own/& RAII malloc/free, ASan-verified no-leak/no-double-free | SIMD = **C-grade**, memory = **safe** |
| v15 | O1–O3 | direct LLVM IR (`.ll`→llc) with explicit metadata (noalias/!range/!llvm.loop); Proc/Cofix finite prefix | metadata = **precise**; infinite = **impossible** |

## The honest grade ladder (never crossed)
- **Orders of magnitude** — ONLY from structure: a collapsing fold (Faulhaber, O(1)) or a linear
  recurrence (companion matrix, O(log n)). With GMP (v13) these stay exact at any size.
- **C-grade, Ω(N)** — general loops, tail recursion, SIMD element-wise kernels. A constant factor over a
  naive loop; the information floor is never broken.
- **Honest DEFER / ceiling** — general recursion (no fake collapse), filter/sorted (length-changing),
  stateful streams (surface can't thread state), and **infinite Proc/Cofix (fundamentally impossible —
  finite prefix only)**.

## Verification at every layer
- Every native/IR result is checked **== the interpreter** (or, for bignum, == Python's exact integer).
- Companion collapse carries the `cfinite` certificate: O(log n) companion ≡ O(n) naive in F_q.
- own/& memory is checked by **AddressSanitizer + LeakSanitizer** (leaks and double-frees are *caught*).
- Backends **agree** with each other (v11 C ≡ v15 LLVM; v12 i64 ≡ v13 bignum below overflow).
- **Coverage: 87%** line coverage across the six codegen modules.

## Toolchain (this environment; ephemeral)
- C: `cc`/`gcc 13`/`clang 18`; LLVM 18 (`llc`, `opt`, `lli`); GMP (`libgmp` + dev header); ASan/LSan;
  `coverage.py`. Tests degrade gracefully (SKIP) when a tool is absent; GMP-absent → honest BLOCKED.

## License note (disclosed)
v13 links **GMP (LGPL)** at the user's explicit direction for the HARAN/Mr.Jeffrey layer. It is
*dynamically* linked (LGPL §6), **not vendored**, and **no `Cargo.toml` was touched** — so CLAUDE.md
R5's `cargo tree` license gate (which governs the JEFF Rust crates) is not triggered; the link lives
only in C emitted by `mr_jeffrey/` tooling. A clean-room/MIT bignum could replace GMP behind the same
`emit_*` interface if a fully license-clean exact path is later required.

## One line
**Structure collapses (with a certificate); everything else runs at the hardware floor; the impossible
(infinite streams, i64-free exactness without bignum) is named, not faked — and now it all lowers to
real native/LLVM code.**
