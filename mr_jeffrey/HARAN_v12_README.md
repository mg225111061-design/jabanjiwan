# HARAN v12 — FnCall + recursion codegen (the call graph, and recursion split by structure)

v11 lowered a single self-contained body. v12 adds what real programs need: functions that **call
other functions**, and **recursion** — lowered by STRUCTURE, not by hope.

## L1 — FnCall (the whole call graph)
- A function that calls others (`f(n) = sq(n) + sq(n+1)`, or a chain `g → {dbl, inc}`) emits the whole
  reachable call graph: prototypes + every callee + the entry, compiled together (`cc -O2`).
- Effects/attributes are read and preserved; pure compute is freely emittable.
- **Native == interpreter** for multi-function programs and call chains.

## L2 — Recur (split by structure — this is the HARAN-unique part)
- **Linear (C-finite) recurrence → companion-matrix power.** Fibonacci `fib(n)=fib(n-1)+fib(n-2)` is a
  constant-coefficient linear recurrence (detected by the v5 `extract_linear_recurrence`). Instead of
  the naive exponential/linear walk, the n-th term is computed by **companion-matrix exponentiation in
  O(d²·log n)**. Exact native `fib(90) = 2880067194370816120` (correct, fits i64).
  - **The collapse is certified** (same certificate as `cfinite.rs`): companion (O(log n)) ≡ naive
    (O(n)) in F_q at N = 5·10⁷. And it is genuinely O(log n): the **10¹⁸-th Fibonacci term computes in
    ~1.3 µs** — flat in n; the naive O(N) walk is simply impossible there.
  - **THIS is "recursion, but orders of magnitude"** — the layer-1 collapse for recursion, the analogue
    of fold → closed form.
- **Tail recursion → C while-loop.** `sumto` lowers to a constant-stack loop; `sumto(10⁶, 0)` runs
  natively with no stack growth. **C-grade, Ω(N) intact — not orders of magnitude.**
- **General recursion → honest C recursion.** Factorial `n*fact(n-1)` has a polynomial coefficient, so
  it is NOT C-finite and NOT tail — it lowers to genuine C recursion. `fact(20)` matches the
  interpreter. **No fake collapse**: we never pretend a general recursion became sublinear.

## L3 — integration (one dispatcher, honest grades)
The dispatcher integrates v9 fold-collapse + v12 recursion and reports the **grade** per function:

| function | kind | grade |
|---|---|---|
| `fib` (linear rec) | companion | **orders-of-magnitude** |
| `Σk²` (collapsing fold) | fold-closed (v9) | **orders-of-magnitude** |
| `k%7` (non-closing fold) | straight-line (v11) | C-grade |
| `sumto` (tail rec) | tail-loop | C-grade |
| `fact` (general rec) | general-recursion | C-grade |

Only a linear-recurrence collapse or a collapsing fold earns *orders-of-magnitude*; everything else is
honestly C-grade. Every dispatched path compiles and **matches the interpreter**.

## Scope / next
- Companion returns **exact long long** (correct until i64 overflow); arbitrary-precision exact terms
  are **v13 (bignum / GMP)**. `Vec` + `own/&` RAII is v14; direct LLVM IR + Proc/Cofix finite-prefix is
  v15. Codegen remains via C (`cc -O2`) until v15.
