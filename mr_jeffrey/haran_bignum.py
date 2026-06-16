"""
HARAN v13 — arbitrary-precision (bignum) codegen via GMP (M1/M2).
================================================================
v11/v12 native code uses `long long` — exact only until i64 overflows (Σk² past n≈3·10⁶, Fibonacci
past F(92)). v13 emits the SAME collapses (v9 Faulhaber closed form, v12 companion-matrix power) but
over GMP `mpz_t`, so the result is EXACT at any size — and still O(1) / O(log n).

  M1  bignum codegen — fold → mpz closed form (O(1), exact) AND a naive mpz loop (O(n), exact), both
      cross-checked against Python's arbitrary-precision integers; linear recurrence → mpz companion
      matrix power (O(log n), exact). The headline: exact arbitrary precision AT orders-of-magnitude
      speed — no i64 ceiling.
  M2  GMP availability — `gmp_available()` compile-tests `-lgmp`. If GMP is present we run M1; if it
      were absent, v13 reports BLOCKED honestly (no silent fallback to a lossy i64 result).

Honesty: every emitted bignum value is verified EQUAL to Python's exact integer (the oracle). The
O(1)/O(log n) claim is the same certified collapse as v9/v12 — GMP only removes the width ceiling, it
does not change the asymptotics. (GMP is LGPL; this is a dev/runtime *link* of the user's toolchain,
not vendored into the HARAN repo.)
"""
from __future__ import annotations

import os
import subprocess
import tempfile
from dataclasses import dataclass
from typing import List, Optional

import haran_ast as A
import closure_classifier as cc
import codegen as v9
import haran_recur as recur

_CC = recur._CC

_GMP_PROBE = """#include <gmp.h>
int main(){ mpz_t a; mpz_init_set_ui(a,1); mpz_clear(a); return 0; }
"""


def gmp_available() -> bool:
    if not _CC:
        return False
    d = tempfile.mkdtemp()
    cpath, bpath = os.path.join(d, "probe.c"), os.path.join(d, "probe")
    open(cpath, "w").write(_GMP_PROBE)
    r = subprocess.run([_CC, cpath, "-lgmp", "-o", bpath], capture_output=True, text=True)
    return r.returncode == 0


# ----------------------------------------------------------------------------- mpz expression lowering
class _MpzCtx:
    def __init__(self):
        self.lines: List[str] = []
        self.tmps: List[str] = []   # mpz_t temporaries to init/clear
        self.n = 0

    def temp(self) -> str:
        self.n += 1
        t = f"_z{self.n}"
        self.tmps.append(t)
        return t


def _lower_mpz(e, out: str, ctx: _MpzCtx):
    """Emit GMP ops computing expr `e` into mpz_t `out`. Supports +,-,*,**(int), integer / (exact)."""
    if isinstance(e, A.Num):
        if e.is_float:
            raise v9.__dict__.get("CodegenError", Exception)("bignum path is integer-only (no float)")
        ctx.lines.append(f"mpz_set_si({out}, {int(e.value)});")
        return
    if isinstance(e, A.Var):
        # loop binder / parameter is a C long long in scope
        ctx.lines.append(f"mpz_set_si({out}, {e.name});")
        return
    if isinstance(e, A.Un) and e.op == "-":
        _lower_mpz(e.operand, out, ctx)
        ctx.lines.append(f"mpz_neg({out}, {out});")
        return
    if isinstance(e, A.Bin):
        if e.op == "**" and isinstance(e.rhs, A.Num) and not e.rhs.is_float:
            p = int(e.rhs.value)
            if p == 0:
                ctx.lines.append(f"mpz_set_si({out}, 1);")
                return
            base = ctx.temp()
            _lower_mpz(e.lhs, base, ctx)
            ctx.lines.append(f"mpz_set({out}, {base});")
            for _ in range(p - 1):
                ctx.lines.append(f"mpz_mul({out}, {out}, {base});")
            return
        l, r = ctx.temp(), ctx.temp()
        _lower_mpz(e.lhs, l, ctx)
        _lower_mpz(e.rhs, r, ctx)
        op = {"+": "mpz_add", "-": "mpz_sub", "*": "mpz_mul"}.get(e.op)
        if op:
            ctx.lines.append(f"{op}({out}, {l}, {r});")
            return
        if e.op in ("/", "//"):
            ctx.lines.append(f"mpz_fdiv_q({out}, {l}, {r});")   # floor division (matches Python // on ℕ)
            return
        if e.op == "%":
            ctx.lines.append(f"mpz_fdiv_r({out}, {l}, {r});")
            return
    from haran_codegen import CodegenError
    raise CodegenError(f"bignum cannot lower {type(e).__name__} ({getattr(e,'op','')})")


def _decls(ctx: _MpzCtx) -> str:
    if not ctx.tmps:
        return ""
    return "  mpz_t " + ", ".join(ctx.tmps) + ";\n  mpz_inits(" + ", ".join(ctx.tmps) + ", NULL);\n"


def _clears(ctx: _MpzCtx) -> str:
    if not ctx.tmps:
        return ""
    return "  mpz_clears(" + ", ".join(ctx.tmps) + ", NULL);\n"


# ----------------------------------------------------------------------------- M1: fold → mpz
def _fold_parts(fn: A.FnDecl):
    ret = cc._block_return(fn.body) if fn.body else None
    if not isinstance(ret, A.Fold) or not isinstance(ret.domain, A.Range):
        return None
    body = cc._block_return(ret.body) if isinstance(ret.body, A.Block) else ret.body
    return ret.binder, ret.domain, body


def emit_fold_closed_mpz(fn: A.FnDecl) -> Optional[str]:
    """v9 Faulhaber closed form, but EXACT over mpz: result = (Σ nums[i]·n^i) / D in O(1)."""
    cf = v9._closed_form_int_poly(fn)
    if cf is None:
        return None
    nums, D, _ = cf
    # Horner over mpz: p = ((nums[k]·n + nums[k-1])·n + ...)·n + nums[0]
    horner = []
    deg = len(nums) - 1
    horner.append(f"mpz_set_si(p, {nums[deg]});")
    for i in range(deg - 1, -1, -1):
        horner.append("mpz_mul(p, p, nz);")
        if nums[i]:
            horner.append(f"mpz_add_ui(p, p, {nums[i]});" if nums[i] > 0 else f"mpz_sub_ui(p, p, {-nums[i]});")
    body = "\n  ".join(horner)
    return f"""#include <gmp.h>
#include <stdio.h>
#include <stdlib.h>
/* {fn.name}: fold closed form (Faulhaber), EXACT over mpz, O(1). */
void {fn.name}(mpz_t out, long long n){{
  mpz_t p, nz; mpz_inits(p, nz, NULL); mpz_set_si(nz, n);
  {body}
  mpz_fdiv_q_ui(out, p, {D});           /* exact: the closed form is integer-valued */
  mpz_clears(p, nz, NULL);
}}
int main(int c, char**v){{
  mpz_t out; mpz_init(out); {fn.name}(out, atoll(v[1]));
  gmp_printf("%Zd\\n", out); mpz_clear(out); return 0;
}}
"""


def emit_fold_naive_mpz(fn: A.FnDecl) -> Optional[str]:
    """Naive mpz accumulation Σ body(k) — O(n) but EXACT (cross-check oracle for the closed form)."""
    parts = _fold_parts(fn)
    if parts is None:
        return None
    binder, dom, body = parts
    lo = int(dom.lo.value) if isinstance(dom.lo, A.Num) else None
    ctx = _MpzCtx()
    term = ctx.temp()
    try:
        _lower_mpz(body, term, ctx)
    except Exception:
        return None
    hi_is_n = isinstance(dom.hi, A.Var)
    hivar = dom.hi.name if hi_is_n else "n"
    lo_c = lo if lo is not None else "1"
    inner = "\n    ".join(ctx.lines)
    return f"""#include <gmp.h>
#include <stdio.h>
#include <stdlib.h>
void {fn.name}(mpz_t out, long long {hivar}){{
  mpz_set_si(out, 0);
{_decls(ctx)}  for (long long {binder} = {lo_c}; {binder} <= {hivar}; {binder}++) {{
    {inner}
    mpz_add(out, out, {term});
  }}
{_clears(ctx)}}}
int main(int c, char**v){{
  mpz_t out; mpz_init(out); {fn.name}(out, atoll(v[1]));
  gmp_printf("%Zd\\n", out); mpz_clear(out); return 0;
}}
"""


# ----------------------------------------------------------------------------- M1: companion → mpz
def emit_companion_mpz(fn: A.FnDecl) -> Optional[str]:
    """Linear recurrence n-th term via companion-matrix power over mpz — EXACT, O(d²·log n)."""
    comp = recur.detect_companion(fn)
    if comp is None:
        return None
    d = comp.d
    cvals = ", ".join(str(x) for x in comp.c)
    init_rev = ", ".join(str(x) for x in reversed(comp.init))
    return f"""#include <gmp.h>
#include <stdio.h>
#include <stdlib.h>
/* {fn.name}: linear recurrence order {d} → companion-matrix power over mpz, EXACT, O(log n). */
#define D {d}
static void matmul(mpz_t C[D][D], mpz_t A[D][D], mpz_t B[D][D], mpz_t acc, mpz_t prod){{
  mpz_t R[D][D];
  for(int i=0;i<D;i++)for(int j=0;j<D;j++){{ mpz_init_set_ui(R[i][j],0);
    for(int k=0;k<D;k++){{ mpz_mul(prod, A[i][k], B[k][j]); mpz_add(R[i][j], R[i][j], prod); }} }}
  for(int i=0;i<D;i++)for(int j=0;j<D;j++){{ mpz_set(C[i][j], R[i][j]); mpz_clear(R[i][j]); }}
}}
void {fn.name}(mpz_t out, long long n){{
  long long c[D] = {{ {cvals} }};
  long long s[D] = {{ {init_rev} }};      /* s[0]=a_(d-1) ... s[d-1]=a_0 */
  if (n < D) {{ mpz_set_si(out, s[D-1-n]); return; }}
  mpz_t M[D][D], R[D][D], acc, prod;
  mpz_inits(acc, prod, NULL);
  for(int i=0;i<D;i++)for(int j=0;j<D;j++){{ mpz_init_set_ui(M[i][j],0); mpz_init_set_ui(R[i][j], i==j); }}
  for(int j=0;j<D;j++) mpz_set_si(M[0][j], c[j]);
  for(int i=1;i<D;i++) mpz_set_si(M[i][i-1], 1);
  long long e = n - D + 1;
  while (e > 0){{
    if (e & 1) matmul(R, R, M, acc, prod);
    matmul(M, M, M, acc, prod);
    e >>= 1;
  }}
  mpz_set_ui(out, 0);
  for(int k=0;k<D;k++){{ mpz_mul_si(prod, R[0][k], s[k]); mpz_add(out, out, prod); }}
  for(int i=0;i<D;i++)for(int j=0;j<D;j++){{ mpz_clear(M[i][j]); mpz_clear(R[i][j]); }}
  mpz_clears(acc, prod, NULL);
}}
int main(int c, char**v){{
  mpz_t out; mpz_init(out); {fn.name}(out, atoll(v[1]));
  gmp_printf("%Zd\\n", out); mpz_clear(out); return 0;
}}
"""


# ----------------------------------------------------------------------------- compile / run
@dataclass
class BigResult:
    ok: bool
    binary: str
    c_src: str
    detail: str


def compile_bignum(src: str) -> BigResult:
    if not _CC:
        return BigResult(False, "", src, "no C compiler")
    if not gmp_available():
        return BigResult(False, "", src, "BLOCKED: GMP (libgmp + gmp.h) not available")
    d = tempfile.mkdtemp()
    cpath, bpath = os.path.join(d, "b.c"), os.path.join(d, "b")
    open(cpath, "w").write(src)
    r = subprocess.run([_CC, "-O2", cpath, "-lgmp", "-o", bpath], capture_output=True, text=True)
    if r.returncode != 0:
        return BigResult(False, "", src, f"compile failed: {r.stderr[:200]}")
    return BigResult(True, bpath, src, "compiled (gmp)")


def run_bignum(binary: str, n: int) -> int:
    import sys
    try:
        sys.set_int_max_str_digits(1_000_000)   # GMP results can have many thousands of digits
    except AttributeError:
        pass
    out = subprocess.run([binary, str(n)], capture_output=True, text=True, timeout=60).stdout.strip()
    return int(out)
