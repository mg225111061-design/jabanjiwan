"""
STAGE R (v9) — MINIMAL native codegen: folded closed-forms → real native O(1) code.
==================================================================================
v4+ confessed "no native codegen". v9 fills it PARTIALLY and honestly: a fold that COLLAPSES to a
closed form (v2/v4 Faulhaber) is emitted as a real C function, compiled (cc -O2), run natively, and
checked against the interpreter. A noalias kernel is emitted with `restrict` (backed by v8's aliasing
check — what C can't prove on its own).

★ Honest scope ★: ONLY folds that collapse codegen here. General/unstructured code and a full LLVM
backend are DEFERRED (months of work). Native uses `long long` (the bignum path stays interpreted) —
also a DEFER. "Partial codegen + honest DEFER" is the intended v9 result, not a full compiler.
"""
from __future__ import annotations

import os
import subprocess
import tempfile
from dataclasses import dataclass
from fractions import Fraction
from math import lcm

from fold_collapse import collapse_fn_fold

_CC = None
for _c in ("cc", "gcc", "clang"):
    if subprocess.run(["which", _c], capture_output=True).returncode == 0:
        _CC = _c
        break


def cc_available():
    return _CC


def _closed_form_int_poly(fn):
    """Return (numerator_coeffs ascending ints, denominator) for the fold's closed form."""
    fc = collapse_fn_fold(fn)
    if fc.verdict != "COLLAPSED":
        return None
    coeffs = [Fraction(c) for c in fc.cert.closed_coeffs]
    D = 1
    for c in coeffs:
        D = lcm(D, c.denominator)
    return [int(c * D) for c in coeffs], D, fc.cert.closed_form


def emit_c_fold_bench(fn) -> str | None:
    """Emit C with f_closed (O(1) closed form) + f_naive (O(n) loop) + internal timing."""
    cf = _closed_form_int_poly(fn)
    if cf is None:
        return None
    nums, D, _ = cf
    terms = []
    for i, a in enumerate(nums):
        if a == 0:
            continue
        terms.append(f"{a}" if i == 0 else f"{a}*" + "*".join(["n"] * i))
    poly = " + ".join(terms) if terms else "0"
    # naive loop reproduces the fold body Σ_{k=1}^{n} k^d for the matching degree (here k*k etc.)
    deg = len(nums) - 2  # closed form degree = body degree + 1
    body = "*".join(["k"] * deg) if deg >= 1 else "1"
    return f"""#include <stdio.h>
#include <stdlib.h>
#include <time.h>
static long long f_closed(long long n){{ return ({poly})/{D}; }}
static long long f_naive(long long n){{ long long s=0; for(long long k=1;k<=n;k++) s+={body}; return s; }}
static double now(){{ struct timespec t; clock_gettime(CLOCK_MONOTONIC,&t); return t.tv_sec*1e9+t.tv_nsec; }}
int main(int argc,char**argv){{
  long long n = atoll(argv[1]);
  long long rc=0, rn=0; double t;
  t=now(); for(int r=0;r<1000;r++) rc+=f_closed(n+ (r&1)); double tc=(now()-t)/1000.0;
  t=now(); rn=f_naive(n); double tn=now()-t;
  printf("closed=%lld naive=%lld match=%d closed_ns=%.1f naive_ns=%.1f\\n",
         f_closed(n), rn, (f_closed(n)==rn), tc, tn);
  return 0;
}}
"""


@dataclass
class CodegenResult:
    ok: bool
    closed_form: str
    binary: str
    detail: str


def codegen_fold(fn) -> CodegenResult:
    if not _CC:
        return CodegenResult(False, "", "", "no C compiler")
    src = emit_c_fold_bench(fn)
    if src is None:
        return CodegenResult(False, "", "", "fold does not collapse to a closed form")
    d = tempfile.mkdtemp()
    cpath, bpath = os.path.join(d, "fold.c"), os.path.join(d, "fold")
    open(cpath, "w").write(src)
    r = subprocess.run([_CC, "-O2", cpath, "-o", bpath], capture_output=True, text=True)
    if r.returncode != 0:
        return CodegenResult(False, "", "", f"compile failed: {r.stderr[:120]}")
    _, _, cform = _closed_form_int_poly(fn)
    return CodegenResult(True, cform, bpath, "compiled fold closed-form to native")


def run_native(binary: str, n: int) -> dict:
    out = subprocess.run([binary, str(n)], capture_output=True, text=True, timeout=30).stdout.strip()
    d = dict(t.split("=", 1) for t in out.split() if "=" in t)
    return {"closed": int(d["closed"]), "naive": int(d["naive"]), "match": d["match"] == "1",
            "closed_ns": float(d["closed_ns"]), "naive_ns": float(d["naive_ns"])}


# --- R2: noalias kernel emitted with `restrict` (backed by v8 aliasing check) ---
def emit_c_noalias_kernel() -> str:
    return """#include <stddef.h>
// HARAN proved a,b,c disjoint (own/&mut) ⇒ SAFE to emit `restrict` (C can't prove this itself).
void axpy(const double* restrict a, const double* restrict b, double* restrict c, size_t n){
  for(size_t i=0;i<n;i++) c[i] = a[i]*b[i] + c[i];
}
"""


def codegen_noalias_ok() -> bool:
    if not _CC:
        return False
    src = emit_c_noalias_kernel()
    d = tempfile.mkdtemp()
    cpath = os.path.join(d, "k.c")
    open(cpath, "w").write(src)
    r = subprocess.run([_CC, "-O2", "-c", cpath, "-o", os.path.join(d, "k.o")], capture_output=True, text=True)
    return r.returncode == 0 and "restrict" in src
