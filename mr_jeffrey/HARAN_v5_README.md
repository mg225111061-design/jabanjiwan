# HARAN v5 — raising the fold ceiling (hypergeometric + Kovacic, with certificates)

v2 folded polynomial/C-finite. v5 extends closed-form closure to **hypergeometric** sums and
**Kovacic** (2nd-order ODE Liouvillian), each with a machine-verified certificate — and stops
HONESTLY at the **Gröbner ceiling** for higher holonomic.

## What v5 adds (measured)

| math class | engine | certificate | example |
|---|---|---|---|
| polynomial / rational | Faulhaber (jeff_foldsum) | FULL (coeff-zero) | Σk² |
| C-finite | cfinite (companion≡naive) | FULL | fib |
| **hypergeometric** | **Gosper/Zeilberger** (Rust `zeilberger`+`telescoper_holds`) | **PARTIAL (provisos)** | ΣC(n,k)=2ⁿ, ΣC(n,k)²=C(2n,n) |
| holonomic order-2+ | creative telescoping | — DEFER (Gröbner ceiling) | Franel ΣC(n,k)³ |
| **Kovacic (ODE)** | y''=r·y Liouvillian | CLOSED / ABSENT | y''=(x²+1)y ⇒ e^{x²/2} ; Airy ABSENT |
| nonholonomic / data | — | — | Σis_prime(k) → NO_STRUCTURE |

## Core measurements (raw)

- **Fold ratio: v2 38% → v5 62% (+24 pp)** on an 8-item corpus. The gain is binomial/factorial sums
  (`ΣC(n,k)`, `ΣC(n,k)²`) which v2 couldn't even sympify (→ NO_STRUCTURE) now folding as hypergeometric.
- **Certificates: of CLOSED, 3 full (poly/C-finite) + 2 partial (hypergeometric provisos).**
  The telescoping IDENTITY is machine-verified (exact rational identity, coefficient-zero); the boundary
  provisos are ASSERTED, not machine-proven → honestly PARTIAL. **No FULL claim with unproven provisos.**
- **Holonomic ceiling (measured):** order-1 single sums solve checker-verified in <1s; order-2 (Franel)
  and beyond → timeout-guarded DEFER. *Timeout = measurement of the math ceiling, not a failure.*
- **Kovacic:** y''=y, y''=(x²+1)y → CLOSED; Airy y''=xy → ABSENT (SL₂ non-solvable); y''=x²y → UNKNOWN
  (Case-1 miss, Cases 2/3 out of scope — NOT relabeled ABSENT). Consistent with v2 galois.rs erf-absence.

## The four buckets stay distinct (the v2 honesty line)

`CLOSED` (real closed form + certificate) · `ABSENT` (impossibility PROVEN: Gosper-nonsummable / Galois /
Kovacic odd-degree) · `NO_STRUCTURE` (data-dependent, Ω(N)) · `UNKNOWN` (undecided). Recognition is never
relabeled as proof; "probably not closed" is UNKNOWN, "proven absent" is ABSENT.

## Deferred / honest limits

- **Holonomic order-2+ / multi-sums**: Gröbner double-exponential → DEFER (the math ceiling, measured).
- **Provisos**: hypergeometric certificates are PARTIAL (boundary/singularity provisos asserted not proven).
- **Kovacic**: focused (Case 1 + odd-degree-absence); Cases 2/3 with poles → UNKNOWN.
- "초기하까지"가 목표지 "모든 holonomic"은 아니다 — the ceiling is reported, not crossed.
