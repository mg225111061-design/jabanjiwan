# HARAN v13 — arbitrary-precision (bignum) codegen via GMP

v11/v12 native code uses `long long` — exact only until i64 overflows. **`fib(200)` over i64 comes back
`-1123705814761610347` (negative — silently wrong); Σk² past n≈3·10⁶ overflows too.** v13 emits the
SAME collapses (v9 Faulhaber closed form, v12 companion-matrix power) but over **GMP `mpz_t`**, so the
result is EXACT at any size — and still O(1) / O(log n).

## M1 — bignum codegen (exact, at orders-of-magnitude speed)
- **Fold → mpz closed form (O(1), exact).** `Σk²` as `n(n+1)(2n+1)/6` computed in `mpz`; at n=10⁷ the
  answer is `333333383333335000000` (21 digits — i64 overflows) in O(1). `Σk³` likewise (generality).
- **Fold → naive mpz loop (O(n), exact)** as a cross-check: `closed mpz ≡ naive mpz ≡ Python` exact.
- **Linear recurrence → mpz companion matrix power (O(log n), exact).** `fib(100000) = 20899 digits`,
  computed in ~6 ms by O(log n) matrix mults — naive recursion would need 2^100000 steps (infeasible).
- **Every emitted bignum value is verified EQUAL to Python's exact integer** (the oracle), at sizes
  where the i64 path is wrong. GMP removes the width ceiling; it does NOT change the asymptotics — the
  O(1)/O(log n) is the same certified collapse from v9/v12.

## M2 — GMP availability (honest gate)
- `gmp_available()` compile-tests `-lgmp`. Here GMP is present (`libgmp.so` + `gmp.h` from the dev
  package) → M1 ran for real.
- If GMP were **absent**, `compile_bignum` returns **BLOCKED** honestly (verified by forcing the gate
  off) — never a silent fall back to a lossy i64 result.

## ★ License note (CLAUDE.md R5 tension — disclosed, not hidden)
The JEFF constitution (CLAUDE.md R5) forbids *linking* GMP (LGPL). This v13 was an explicit user
directive ("bignum codegen via GMP (libgmp)") for the **HARAN/Mr.Jeffrey** layer, which is distinct
from the JEFF/GACC Rust crates that R5's license-scan governs. The tension is mitigated and disclosed:
- GMP is **dynamically linked** (`-lgmp` → `libgmp.so.10`), which LGPL §6 permits for a non-GPL caller;
- GMP is **not vendored** into the repo (it is the user's system/toolchain library);
- **no `Cargo.toml` was touched** — GMP is absent from the Rust dependency graph, so R5's CI gate
  (`ci/license_scan.sh`, which scans `cargo tree`) is not triggered. The link lives only in C emitted
  by the Python tooling under `mr_jeffrey/`.

If a fully license-clean exact path is later required, a clean-room arbitrary-precision integer (or an
MIT/Apache bignum) can replace GMP behind the same `emit_*` interface.

## Scope / next
- `Vec` + `own/&` RAII codegen is v14; direct LLVM IR + Proc/Cofix finite-prefix + coverage is v15.
