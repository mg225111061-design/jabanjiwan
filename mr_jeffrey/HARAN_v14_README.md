# HARAN v14 — Vec codegen + own/& RAII memory management

v9 emitted ONE fixed `axpy` kernel. v14 generalizes to real Vec codegen and ties memory management to
the verified `own`/`&` types — so allocation is automatic and provably leak-free.

## N1 — Vec codegen (static / dynamic / SIMD)
HARAN's element-wise primitive is `map(v, λx. e)`. v14 lowers it to a C array loop in three modes:
- **Static** `Vec<T, N>` (N a literal) → a **fixed-size C buffer**, no heap. `map(v, λx. x*x)` on
  `Vec<Int,4>`: `[3,4,5,6] → [9,16,25,36]`, native == interpreter.
- **Dynamic** `Vec<T, n>` (n a size variable) → **pointer + length** (`malloc`/`free`).
  `map(v, λx. 2x+1)`: `[1,2,3] → [3,5,7]`, native == interpreter on every element.
- **SIMD** (`-march=native`): the loop auto-vectorizes; the `restrict` that permits it is **SAFE
  because HARAN's `own`/`&mut` proved the buffers disjoint** — a C compiler cannot prove this and needs
  the programmer's unchecked promise. HONEST measurement: this element-wise kernel is **bandwidth-bound**,
  so restrict-vs-plain is ~1× over 4M elements (both vectorize) — a constant factor, Ω(N) intact, no
  invented speedup.
- **Honest DEFER:** `filter`/`sorted` (length-changing / need indexing the surface lacks) are **not**
  codegen'd — `compile_map` returns "not a map kernel"; the interpreter handles them. Not faked.

## N2 — own/& RAII (no leak, no double-free — checked, not claimed)
Ownership drives `malloc`/`free`:
- an **`own Vec`** is **freed exactly once** at its last use;
- a **`&`/`&mut` borrow** is **never freed** (the caller keeps ownership) — so no double-free.

This is verified for real with **AddressSanitizer + LeakSanitizer**:
- the correct program is **ASan-CLEAN** (`sum=499500`, no leak, no error);
- a variant that **drops the owner's free → LeakSanitizer CATCHES the leak**;
- a variant where **a borrow frees → AddressSanitizer CATCHES the double-free**.

The check has teeth: the bugs are caught, the correct discipline passes. The free-discipline (one
owner free, zero borrow frees) is **driven by the types**, not hand-annotated.

## Scope / next
- Codegen still goes via C (`cc -O2`). Direct **LLVM IR** (full metadata) + **Proc/Cofix finite-prefix**
  codegen + coverage measurement + final integration is **v15** (the last version).
