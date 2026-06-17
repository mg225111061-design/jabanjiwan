"""
HARAN v14 — Vec codegen + own/& RAII memory management (N1/N2).
==============================================================
v9 emitted ONE fixed axpy kernel with `restrict`. v14 generalizes to real Vec codegen and ties memory
management to the verified own/& types.

  N1  Vec codegen — `map(v, λx. e)` (the language's element-wise primitive) → a C array loop, in three
      modes: STATIC `Vec<T,N>` (N literal → fixed-size buffer), DYNAMIC `Vec<T,n>` (runtime length →
      pointer+len), and SIMD (the same loop auto-vectorizes under -O2 -march=native; the `restrict`
      that enables it is SAFE because HARAN's own/&mut proved the buffers disjoint — C can't). Verified
      element-by-element against the interpreter.
  N2  own/& RAII — ownership drives malloc/free: an `own Vec` is freed exactly once at its last use; a
      `&`/`&mut` borrow is NEVER freed (no double-free). Checked for real with AddressSanitizer +
      LeakSanitizer: the correct program is ASan-clean, and a deliberately leaky / double-freeing
      variant is CAUGHT (the check has teeth).

Honesty: only `map` (length-preserving, element-wise) codegens here; filter/sorted/reduce (length-
changing or needing indexing the surface lacks) stay interpreter-handled → honest DEFER. SIMD is a
measured constant factor, Ω(N) intact — never an invented order of magnitude.
"""
from __future__ import annotations

import os
import subprocess
import tempfile
from dataclasses import dataclass, field
from typing import List, Optional

import haran_ast as A
import haran_codegen as cg
import closure_classifier as cc

_CC = cg._CC


def cc_available():
    return _CC


def _asan_available() -> bool:
    if not _CC:
        return False
    d = tempfile.mkdtemp()
    cpath, bpath = os.path.join(d, "a.c"), os.path.join(d, "a")
    open(cpath, "w").write("int main(){return 0;}")
    r = subprocess.run([_CC, "-fsanitize=address,leak", "-O1", cpath, "-o", bpath],
                       capture_output=True, text=True)
    return r.returncode == 0


def asan_available():
    return _asan_available()


# ----------------------------------------------------------------------------- N1: map → Vec codegen
def _unwrap(ty):
    while isinstance(ty, (A.TyOwn, A.TyRef)):
        ty = ty.inner
    return ty


def _elem_ctype(vty: A.TyName) -> str:
    if vty.args and vty.args[0].kind == "type" and isinstance(vty.args[0].value, A.TyName):
        return cg._ctype(vty.args[0].value)
    return "long long"


def _vec_size(vty: A.TyName):
    """Return ('static', N:int) or ('dynamic', sizevar:str)."""
    if len(vty.args) >= 2:
        a = vty.args[1]
        if a.kind == "term" and isinstance(a.value, A.Num) and not a.value.is_float:
            return ("static", int(a.value.value))
        if a.kind == "type" and isinstance(a.value, A.TyName):
            return ("dynamic", a.value.name)
    return ("dynamic", "n")


@dataclass
class MapKernel:
    fn: A.FnDecl
    vecparam: A.Param
    vty: A.TyName
    binder: str
    body: object
    mode: str       # "static" | "dynamic"
    size: object    # int (static) | str (dynamic sizevar)
    elem: str       # C element type
    has_restrict: bool


def detect_map(fn: A.FnDecl) -> Optional[MapKernel]:
    body = cc._block_return(fn.body) if fn.body else None
    if not (isinstance(body, A.Call) and isinstance(body.func, A.Var)
            and body.func.name == "map" and len(body.args) == 2):
        return None
    vecarg, lam = body.args
    if not (isinstance(vecarg, A.Var) and isinstance(lam, A.Lambda) and len(lam.params) == 1):
        return None
    vp = None
    for p in fn.params:
        if p.name == vecarg.name:
            vp = p
    if vp is None:
        return None
    vty = _unwrap(vp.ty)
    if not (isinstance(vty, A.TyName) and vty.name == "Vec"):
        return None
    mode, size = _vec_size(vty)
    # verified noalias: own / &mut input ⇒ exclusive ⇒ disjoint from the fresh output ⇒ `restrict` SAFE
    has_restrict = isinstance(vp.ty, A.TyOwn) or (isinstance(vp.ty, A.TyRef) and vp.ty.mutable) \
        or isinstance(vp.ty, A.TyRef)
    return MapKernel(fn, vp, vty, lam.params[0], lam.body, mode, size, _elem_ctype(vty), has_restrict)


def _fmt(elem: str):
    return "%g" if elem == "double" else "%lld"


def _atoX(elem: str):
    return "atof" if elem == "double" else "atoll"


def emit_map_c(mk: MapKernel) -> str:
    elem = mk.elem
    R = "restrict " if mk.has_restrict else ""
    ctx = cg.Ctx()
    bexpr = cg.lower(mk.body, ctx)            # lambda body, with the binder in scope as a C local
    prelude = "\n    ".join(ctx.lines)
    name = mk.fn.name
    if mk.mode == "static":
        N = mk.size
        kernel = f"""static void {name}(const {elem}* {R}in, {elem}* {R}out) {{
  for (long long i = 0; i < {N}; i++) {{
    {elem} {mk.binder} = in[i];
    {prelude}
    out[i] = {bexpr};
  }}
}}"""
        main = f"""int main(int argc, char**argv) {{
  const long long n = {N};
  {elem} in[{N}], out[{N}];
  for (long long i=0;i<n;i++) in[i] = {_atoX(elem)}(argv[1+i]);
  {name}(in, out);
  for (long long i=0;i<n;i++) printf("{_fmt(elem)} ", out[i]);
  printf("\\n"); return 0;
}}"""
    else:
        sv = mk.size
        kernel = f"""static void {name}(const {elem}* {R}in, {elem}* {R}out, long long {sv}) {{
  for (long long i = 0; i < {sv}; i++) {{
    {elem} {mk.binder} = in[i];
    {prelude}
    out[i] = {bexpr};
  }}
}}"""
        main = f"""int main(int argc, char**argv) {{
  long long {sv} = atoll(argv[1]);
  {elem}* in = malloc({sv}*sizeof({elem}));
  {elem}* out = malloc({sv}*sizeof({elem}));
  for (long long i=0;i<{sv};i++) in[i] = {_atoX(elem)}(argv[2+i]);
  {name}(in, out, {sv});
  for (long long i=0;i<{sv};i++) printf("{_fmt(elem)} ", out[i]);
  printf("\\n"); free(in); free(out); return 0;
}}"""
    return f"#include <stdio.h>\n#include <stdlib.h>\n{kernel}\n{main}\n"


@dataclass
class Compiled:
    ok: bool
    binary: str
    c_src: str
    mode: str
    has_restrict: bool
    detail: str


def compile_map(fn: A.FnDecl, flags: Optional[List[str]] = None) -> Compiled:
    if not _CC:
        return Compiled(False, "", "", "", False, "no C compiler")
    mk = detect_map(fn)
    if mk is None:
        return Compiled(False, "", "", "", False, "not a map(v, λx. e) kernel")
    src = emit_map_c(mk)
    d = tempfile.mkdtemp()
    cpath, bpath = os.path.join(d, "v.c"), os.path.join(d, "v")
    open(cpath, "w").write(src)
    r = subprocess.run([_CC, "-O2", *(flags or []), cpath, "-o", bpath], capture_output=True, text=True)
    if r.returncode != 0:
        return Compiled(False, "", src, mk.mode, mk.has_restrict, f"compile failed: {r.stderr[:200]}")
    return Compiled(True, bpath, src, mk.mode, mk.has_restrict, "compiled")


def run_map(binary: str, vec: List[int], dynamic: bool = True) -> List[int]:
    args = ([str(len(vec))] if dynamic else []) + [str(x) for x in vec]
    out = subprocess.run([binary, *args], capture_output=True, text=True, timeout=30).stdout.strip()
    return [int(x) for x in out.split()] if out else []


# ----------------------------------------------------------------------------- N1: SIMD throughput
def map_simd_throughput(fn: A.FnDecl, n: int = 1 << 22) -> Optional[dict]:
    """Emit a self-timed bench of the dynamic map over a large vector with verified-noalias restrict,
    compiled -O2 -march=native (auto-vectorizes). Honest: measured throughput, constant-factor, Ω(N)."""
    mk = detect_map(fn)
    if mk is None or mk.mode != "dynamic" or not _CC:
        return None
    elem = mk.elem
    ctx = cg.Ctx()
    bexpr = cg.lower(mk.body, ctx)
    prelude = "\n    ".join(ctx.lines)
    sv = mk.size
    src = f"""#include <stdio.h>
#include <stdlib.h>
#include <time.h>
static void kern(const {elem}* restrict in, {elem}* restrict out, long long {sv}) {{
  for (long long i=0;i<{sv};i++) {{ {elem} {mk.binder}=in[i]; {prelude} out[i]={bexpr}; }}
}}
static void kern_noalias_off(const {elem}* in, {elem}* out, long long {sv}) {{
  for (long long i=0;i<{sv};i++) {{ {elem} {mk.binder}=in[i]; {prelude} out[i]={bexpr}; }}
}}
static double now(){{ struct timespec t; clock_gettime(CLOCK_MONOTONIC,&t); return t.tv_sec*1e9+t.tv_nsec; }}
int main(){{
  long long n={n};
  {elem}*a=malloc(n*sizeof({elem})),*b=malloc(n*sizeof({elem})),*c=malloc(n*sizeof({elem}));
  for(long long i=0;i<n;i++) a[i]=(i%7);
  double t;
  t=now(); for(int r=0;r<5;r++) kern(a,b,n); double tr=(now()-t)/5.0;
  t=now(); for(int r=0;r<5;r++) kern_noalias_off(a,c,n); double tn=(now()-t)/5.0;
  long long ok=1; for(long long i=0;i<n;i++) if(b[i]!=c[i]) ok=0;
  printf("restrict_ns=%.0f plain_ns=%.0f correct=%lld\\n", tr, tn, ok);
  free(a);free(b);free(c); return 0;
}}
"""
    d = tempfile.mkdtemp()
    cpath, bpath = os.path.join(d, "s.c"), os.path.join(d, "s")
    open(cpath, "w").write(src)
    r = subprocess.run([_CC, "-O2", "-march=native", cpath, "-o", bpath], capture_output=True, text=True)
    if r.returncode != 0:
        return {"ok": False, "detail": r.stderr[:200]}
    out = subprocess.run([bpath], capture_output=True, text=True, timeout=30).stdout.strip()
    kv = dict(t.split("=", 1) for t in out.split() if "=" in t)
    return {"ok": True, "restrict_ns": float(kv["restrict_ns"]), "plain_ns": float(kv["plain_ns"]),
            "correct": kv["correct"] == "1", "raw": out}


# ----------------------------------------------------------------------------- N2: own/& RAII
def emit_raii_program(leaky: bool = False, double_free: bool = False) -> str:
    """A driver whose malloc/free is DRIVEN by own/& semantics:
      • `own Vec` (consume) frees exactly once at last use;
      • `&`/`&mut` borrow (sumread) NEVER frees.
    leaky=True drops the owner's free (leak); double_free=True frees a borrow too (double free)."""
    consume_free = "" if leaky else "  free(v);            /* own: freed once at last use */\n"
    borrow_free = "  free(in);            /* BUG: borrow must not free */\n" if double_free else \
                  "  /* & borrow: no free (caller owns) */\n"
    return f"""#include <stdio.h>
#include <stdlib.h>
/* &Vec borrow — reads only, NEVER frees (caller keeps ownership). */
static long long sumread(const long long* in, long long n) {{
  long long s=0; for(long long i=0;i<n;i++) s+=in[i];
{borrow_free}  return s;
}}
/* own Vec — receives ownership, frees exactly once at last use. */
static long long consume(long long* v, long long n) {{
  long long s = sumread(v, n);     /* lend a & borrow */
{consume_free}  return s;
}}
int main() {{
  long long n=1000;
  long long* v = malloc(n*sizeof(long long));   /* allocate the owned buffer */
  for(long long i=0;i<n;i++) v[i]=i;
  long long s = consume(v, n);                   /* move ownership in; consume frees it */
  printf("sum=%lld\\n", s);
  return 0;
}}
"""


@dataclass
class AsanResult:
    ok: bool          # compiled+ran clean (no sanitizer error)
    clean: bool       # no leak / no error reported
    detail: str
    output: str = ""


def asan_check(src: str) -> AsanResult:
    if not _CC or not _asan_available():
        return AsanResult(False, False, "ASan not available")
    d = tempfile.mkdtemp()
    cpath, bpath = os.path.join(d, "r.c"), os.path.join(d, "r")
    open(cpath, "w").write(src)
    r = subprocess.run([_CC, "-fsanitize=address,leak", "-g", "-O1", cpath, "-o", bpath],
                       capture_output=True, text=True)
    if r.returncode != 0:
        return AsanResult(False, False, f"compile failed: {r.stderr[:200]}")
    env = dict(os.environ, ASAN_OPTIONS="detect_leaks=1")
    run = subprocess.run([bpath], capture_output=True, text=True, timeout=30, env=env)
    err = run.stderr or ""
    bad = any(s in err for s in ("ERROR: AddressSanitizer", "LeakSanitizer", "detected memory leaks",
                                 "double-free", "heap-use-after-free"))
    return AsanResult(True, not bad, "ran", run.stdout.strip() + (" | " + err[:160] if bad else ""))


# ----------------------------------------------------------------------------- v19 W1: Vec REDUCE kernel
@dataclass
class ReduceKernel:
    fn: A.FnDecl
    vecparam: A.Param
    vty: A.TyName
    binder: str
    body: object
    mode: str
    size: object
    elem: str


def detect_reduce(fn: A.FnDecl) -> Optional[ReduceKernel]:
    """`fold x in xs { e }` over a Vec parameter → a reduction kernel (the missing companion of map)."""
    body = cc._block_return(fn.body) if fn.body else None
    if not (isinstance(body, A.Fold) and isinstance(body.domain, A.Var)):
        return None
    vp = next((p for p in fn.params if p.name == body.domain.name), None)
    if vp is None:
        return None
    vty = _unwrap(vp.ty)
    if not (isinstance(vty, A.TyName) and vty.name == "Vec"):
        return None
    mode, size = _vec_size(vty)
    inner = cc._block_return(body.body) if isinstance(body.body, A.Block) else body.body
    return ReduceKernel(fn, vp, vty, body.binder, inner, mode, size, _elem_ctype(vty))


def emit_reduce_c(rk: ReduceKernel) -> str:
    elem, name = rk.elem, rk.fn.name
    ctx = cg.Ctx()
    bexpr = cg.lower(rk.body, ctx)
    prelude = "\n    ".join(ctx.lines)
    if rk.mode == "static":
        N = rk.size
        kernel = (f"static {elem} {name}(const {elem}* in) {{\n  {elem} acc = 0;\n"
                  f"  for (long long i = 0; i < {N}; i++) {{\n    {elem} {rk.binder} = in[i];\n"
                  f"    {prelude}\n    acc += {bexpr};\n  }}\n  return acc;\n}}")
        main = (f"int main(int argc, char**argv) {{\n  {elem} in[{N}];\n"
                f"  for (long long i=0;i<{N};i++) in[i] = {_atoX(elem)}(argv[1+i]);\n"
                f"  printf(\"{_fmt(elem)}\\n\", {name}(in)); return 0;\n}}")
    else:
        sv = rk.size
        kernel = (f"static {elem} {name}(const {elem}* in, long long {sv}) {{\n  {elem} acc = 0;\n"
                  f"  for (long long i = 0; i < {sv}; i++) {{\n    {elem} {rk.binder} = in[i];\n"
                  f"    {prelude}\n    acc += {bexpr};\n  }}\n  return acc;\n}}")
        main = (f"int main(int argc, char**argv) {{\n  long long {sv} = atoll(argv[1]);\n"
                f"  {elem}* in = malloc({sv}*sizeof({elem}));\n"
                f"  for (long long i=0;i<{sv};i++) in[i] = {_atoX(elem)}(argv[2+i]);\n"
                f"  printf(\"{_fmt(elem)}\\n\", {name}(in, {sv})); free(in); return 0;\n}}")
    return f"#include <stdio.h>\n#include <stdlib.h>\n{kernel}\n{main}\n"


def compile_reduce(fn: A.FnDecl, flags: Optional[List[str]] = None) -> Compiled:
    if not _CC:
        return Compiled(False, "", "", "", False, "no C compiler")
    rk = detect_reduce(fn)
    if rk is None:
        return Compiled(False, "", "", "", False, "not a fold-over-Vec reduction kernel")
    src = emit_reduce_c(rk)
    d = tempfile.mkdtemp()
    cpath, bpath = os.path.join(d, "r.c"), os.path.join(d, "r")
    open(cpath, "w").write(src)
    r = subprocess.run([_CC, "-O2", *(flags or []), cpath, "-o", bpath], capture_output=True, text=True)
    if r.returncode != 0:
        return Compiled(False, "", src, rk.mode, False, f"compile failed: {r.stderr[:200]}")
    return Compiled(True, bpath, src, rk.mode, False, "compiled")


def run_reduce(binary: str, vec: List[int], dynamic: bool = True) -> int:
    args = ([str(len(vec))] if dynamic else []) + [str(x) for x in vec]
    out = subprocess.run([binary, *args], capture_output=True, text=True, timeout=30).stdout.strip()
    return int(out) if out else 0
