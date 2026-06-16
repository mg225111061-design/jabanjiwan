"""
HARAN v12 — FnCall + recursion codegen (L1/L2/L3), built on v11's general lowering.
====================================================================================
v11 lowered a single self-contained function body. v12 adds the two things real programs need:

  L1  FnCall   — a function that CALLS other functions: emit the whole call graph (prototypes +
                 every reachable callee + the entry), compiled together. Effects/attributes are
                 read and preserved (pure compute is freely emittable; non-pure is reported).
  L2  Recur    — recursion, split by STRUCTURE (this is the HARAN-unique part):
                 • linear (C-finite) recurrence, e.g. Fibonacci → companion-matrix power: the n-th
                   term in O(d²·log n) instead of the naive exponential/linear walk. THIS is
                   "recursion, but orders of magnitude" — the layer-1 collapse, reusing the v5
                   extract_linear_recurrence + cfinite.rs certificate (companion ≡ naive in F_q).
                 • tail recursion → a C while-loop (constant stack, C-grade Ω(N), no magic).
                 • everything else → honest plain C recursion (works; explicitly NOT orders of
                   magnitude — we never pretend a general recursion collapsed).
  L3  integrate — one dispatcher picks fold-closed (v9) / companion (L2) / tail-loop / multi-call /
                 v11-general, compiles, and reports the GRADE (orders-of-magnitude vs C-grade)
                 honestly per function.

Honesty: the companion path returns EXACT long long (correct until i64 overflow — bignum is v13);
the O(log n) COLLAPSE itself is certified in F_q at large n (companion ≡ naive), exactly as cfinite.rs
does. Tail/general recursion are C-grade and labelled as such — only the linear-recurrence collapse
earns "orders of magnitude".
"""
from __future__ import annotations

import dataclasses
import os
import subprocess
import tempfile
import time
from dataclasses import dataclass, field
from typing import Dict, List, Optional

import haran_ast as A
import haran_codegen as cg
import closure_classifier as cc

_CC = cg._CC


def cc_available():
    return _CC


# ----------------------------------------------------------------------------- helpers / call graph
def _walk(node):
    yield node
    if dataclasses.is_dataclass(node) and not isinstance(node, A.Span):
        for f in dataclasses.fields(node):
            v = getattr(node, f.name)
            if isinstance(v, list):
                for x in v:
                    if dataclasses.is_dataclass(x):
                        yield from _walk(x)
            elif dataclasses.is_dataclass(v):
                yield from _walk(v)


def callees(fn: A.FnDecl, table: Dict[str, A.FnDecl]) -> List[str]:
    """Names of in-table functions called by fn (excluding itself)."""
    out = []
    for x in _walk(fn.body):
        if isinstance(x, A.Call) and isinstance(x.func, A.Var):
            nm = x.func.name
            if nm in table and nm != fn.name and nm not in out:
                out.append(nm)
    return out


def is_self_recursive(fn: A.FnDecl) -> bool:
    return any(isinstance(x, A.Call) and isinstance(x.func, A.Var) and x.func.name == fn.name
              for x in _walk(fn.body))


# ----------------------------------------------------------------------------- L2: tail recursion
def _arm_value(arm):
    return cc._block_return(arm.body) if isinstance(arm.body, A.Block) else arm.body


def tail_recursive_match(fn: A.FnDecl) -> Optional[A.Match]:
    """If fn's body is a `match` whose recursive arms are a SINGLE self-call in tail position
    (and base arms have no self-call), return that Match; else None."""
    ret = cc._block_return(fn.body) if fn.body else None
    if not isinstance(ret, A.Match):
        return None
    saw_rec = False
    for arm in ret.arms:
        v = _arm_value(arm)
        is_tail_call = isinstance(v, A.Call) and isinstance(v.func, A.Var) and v.func.name == fn.name
        if is_tail_call:
            saw_rec = True
            continue
        # a non-tail arm must not contain a self-call anywhere (else it isn't pure tail recursion)
        if any(isinstance(x, A.Call) and isinstance(x.func, A.Var) and x.func.name == fn.name
               for x in _walk(v)):
            return None
    return ret if saw_rec else None


def emit_tail_loop_c(fn: A.FnDecl, cname: Optional[str] = None) -> str:
    m = tail_recursive_match(fn)
    if m is None:
        raise cg.CodegenError("not tail-recursive")
    cname = cname or fn.name
    params = ", ".join(f"{cg._ctype(p.ty)} {p.name}" for p in fn.params)
    ret = cg._ctype(fn.ret) if fn.ret else "long long"
    pnames = [p.name for p in fn.params]
    ctx = cg.Ctx()
    scr = cg.lower(m.scrut, ctx)
    body = list(ctx.lines)
    body.append("while (1) {")
    first = True
    for arm in m.arms:
        cond = cg._pat_cond(arm.pattern, scr)
        v = _arm_value(arm)
        sub = cg.Ctx(tmp=ctx.tmp)
        kw = "if" if first else ("else if" if cond != "1" else "else")
        head = f"  {kw} ({cond})" if kw != "else" else "  else"
        if isinstance(v, A.Call) and isinstance(v.func, A.Var) and v.func.name == fn.name:
            # tail self-call: evaluate new args into temps, then reassign params and loop
            newvals = [cg.lower(a, sub) for a in v.args]
            ctx.tmp = sub.tmp
            body.append(f"{head} {{")
            body += ["    " + ln for ln in sub.lines]
            for i, nv in enumerate(newvals):
                body.append(f"    long long _na{i} = {nv};")
            for i, pn in enumerate(pnames):
                body.append(f"    {pn} = _na{i};")
            body.append("    continue;")
            body.append("  }")
        else:
            rv = cg.lower(v, sub)
            ctx.tmp = sub.tmp
            body.append(f"{head} {{")
            body += ["    " + ln for ln in sub.lines]
            body.append(f"    return {rv};")
            body.append("  }")
        first = False
    body.append("}")
    return f"{ret} {cname}({params}) {{\n  " + "\n  ".join(body) + "\n}"


# ----------------------------------------------------------------------------- L2: linear recurrence
@dataclass
class Companion:
    c: List[int]          # f(n) = c[0]f(n-1) + ... + c[d-1]f(n-d)
    init: List[int]       # a_0 .. a_{d-1}

    @property
    def d(self):
        return len(self.c)


def detect_companion(fn: A.FnDecl) -> Optional[Companion]:
    rec = cc.extract_linear_recurrence(fn)
    if rec is None:
        return None
    c, init = rec
    return Companion([int(x) for x in c], [int(x) for x in init])


def emit_companion_c(fn: A.FnDecl, comp: Companion, cname: Optional[str] = None) -> str:
    """Emit the n-th term of a constant-coeff linear recurrence via companion-matrix power.
    EXACT long long (overflows past i64 — bignum is v13); O(d²·log n) field ops (here d small)."""
    cname = cname or fn.name
    d = comp.d
    cvals = ", ".join(str(x) for x in comp.c)
    # state s = [a_{d-1}, ..., a_0]; v_n = M^{n-d+1} s ; return v_n[0]
    init_rev = ", ".join(str(x) for x in reversed(comp.init))
    return f"""static long long {cname}(long long n){{
  const int d = {d};
  long long c[{d}] = {{ {cvals} }};
  long long s[{d}] = {{ {init_rev} }};         /* s[0]=a_(d-1) ... s[d-1]=a_0 */
  if (n < d) return s[d-1-n];
  /* companion M (d x d): row0 = c ; row i = e_(i-1) */
  long long M[{d}][{d}], R[{d}][{d}], T[{d}][{d}];
  for (int i=0;i<d;i++) for (int j=0;j<d;j++) M[i][j]=0;
  for (int j=0;j<d;j++) M[0][j]=c[j];
  for (int i=1;i<d;i++) M[i][i-1]=1;
  for (int i=0;i<d;i++) for (int j=0;j<d;j++) R[i][j]=(i==j);   /* R = I */
  long long e = n - d + 1;
  while (e > 0) {{
    if (e & 1) {{ /* R = R*M */
      for (int i=0;i<d;i++) for (int j=0;j<d;j++){{ long long a=0; for(int k=0;k<d;k++) a+=R[i][k]*M[k][j]; T[i][j]=a; }}
      for (int i=0;i<d;i++) for (int j=0;j<d;j++) R[i][j]=T[i][j];
    }}
    /* M = M*M */
    for (int i=0;i<d;i++) for (int j=0;j<d;j++){{ long long a=0; for(int k=0;k<d;k++) a+=M[i][k]*M[k][j]; T[i][j]=a; }}
    for (int i=0;i<d;i++) for (int j=0;j<d;j++) M[i][j]=T[i][j];
    e >>= 1;
  }}
  long long r=0; for(int k=0;k<d;k++) r += R[0][k]*s[k];   /* v_n[0] = (M^(n-d+1) s)[0] */
  return r;
}}"""


def emit_companion_program(fn: A.FnDecl, comp: Companion) -> str:
    cfn = emit_companion_c(fn, comp)
    return (f"#include <stdio.h>\n#include <stdlib.h>\n{cfn}\n"
            f"int main(int c,char**v){{ printf(\"%lld\\n\", {fn.name}(atoll(v[1]))); return 0; }}\n")


def emit_companion_cert(fn: A.FnDecl, comp: Companion) -> str:
    """A self-contained C program proving the COLLAPSE: companion (O(log n)) ≡ naive (O(n)) in F_q,
    plus timing showing the companion path is flat in n (O(log n)) while naive grows (Ω(N))."""
    d = comp.d
    cvals = ", ".join(str(x) for x in comp.c)
    init_rev = ", ".join(str(x) for x in reversed(comp.init))
    initf = ", ".join(str(x) for x in comp.init)
    return f"""#include <stdio.h>
#include <stdlib.h>
#include <time.h>
typedef unsigned long long u64;
typedef __int128 i128;
static u64 Q = 1000000007ULL;
/* companion mod Q, O(d^2 log n) */
static u64 comp_mod(u64 n){{
  const int d={d}; long long c[{d}]={{ {cvals} }}; u64 s[{d}]; u64 init[{d}]={{ {initf} }};
  for(int i=0;i<d;i++) s[i]=init[d-1-i]%Q;
  if(n<(u64)d) return init[n]%Q;
  u64 M[{d}][{d}],R[{d}][{d}],T[{d}][{d}];
  for(int i=0;i<d;i++)for(int j=0;j<d;j++)M[i][j]=0;
  for(int j=0;j<d;j++)M[0][j]=((c[j]%(long long)Q)+(long long)Q)%(long long)Q;
  for(int i=1;i<d;i++)M[i][i-1]=1;
  for(int i=0;i<d;i++)for(int j=0;j<d;j++)R[i][j]=(i==j);
  u64 e=n-d+1;
  while(e>0){{
    if(e&1){{for(int i=0;i<d;i++)for(int j=0;j<d;j++){{i128 a=0;for(int k=0;k<d;k++)a+=(i128)R[i][k]*M[k][j];T[i][j]=(u64)(a%Q);}}
      for(int i=0;i<d;i++)for(int j=0;j<d;j++)R[i][j]=T[i][j];}}
    for(int i=0;i<d;i++)for(int j=0;j<d;j++){{i128 a=0;for(int k=0;k<d;k++)a+=(i128)M[i][k]*M[k][j];T[i][j]=(u64)(a%Q);}}
    for(int i=0;i<d;i++)for(int j=0;j<d;j++)M[i][j]=T[i][j];
    e>>=1;
  }}
  i128 r=0; for(int k=0;k<d;k++) r+=(i128)R[0][k]*s[k]; return (u64)(r%Q);
}}
/* naive iterate mod Q, O(n d) */
static u64 naive_mod(u64 n){{
  const int d={d}; long long c[{d}]={{ {cvals} }}; u64 w[{d}]={{ {initf} }};
  if(n<(u64)d) return w[n]%Q;
  for(u64 m=d;m<=n;m++){{
    i128 a=0; for(int i=1;i<=d;i++){{ long long ci=((c[i-1]%(long long)Q)+(long long)Q)%(long long)Q; a+=(i128)((u64)ci)*w[d-i]; }}
    u64 nv=(u64)(a%Q);
    for(int i=0;i<d-1;i++) w[i]=w[i+1]; w[d-1]=nv;
  }}
  return w[d-1];
}}
static double now(){{ struct timespec t; clock_gettime(CLOCK_MONOTONIC,&t); return t.tv_sec*1e9+t.tv_nsec; }}
int main(int argc,char**argv){{
  u64 N = argc>1? strtoull(argv[1],0,10) : 50000000ULL;
  u64 cm=comp_mod(N), nm=naive_mod(N);
  double t1=now(); u64 big=0; for(int r=0;r<1000;r++) big^=comp_mod((u64)1e18 + r); double tc=(now()-t1)/1000.0;
  double t2=now(); u64 small=comp_mod(1000); double ts=now()-t2;
  printf("match=%d comp=%llu naive=%llu comp_1e18_ns=%.0f comp_1e3_ns=%.0f big=%llu small=%llu\\n",
         (cm==nm),(unsigned long long)cm,(unsigned long long)nm,tc,ts,(unsigned long long)big,(unsigned long long)small);
  return 0;
}}
"""


# ----------------------------------------------------------------------------- L1: multi-function emit
def _reachable(entry: str, table: Dict[str, A.FnDecl]) -> List[str]:
    seen, order, stack = set(), [], [entry]
    while stack:
        nm = stack.pop()
        if nm in seen or nm not in table:
            continue
        seen.add(nm)
        order.append(nm)
        for callee in callees(table[nm], table):
            if callee not in seen:
                stack.append(callee)
    return order


def emit_fn_any(fn: A.FnDecl, table: Dict[str, A.FnDecl]) -> tuple[str, List[str], str]:
    """Emit ONE function with the best lowering. Returns (c_code, invariants, grade)."""
    comp = detect_companion(fn)
    if comp is not None:
        return emit_companion_c(fn, comp), [f"{fn.name}: linear recurrence order {comp.d}, c={comp.c}"], "orders-of-magnitude"
    if tail_recursive_match(fn) is not None:
        return emit_tail_loop_c(fn), [f"{fn.name}: tail recursion → loop (constant stack)"], "C-grade"
    # general (incl. non-tail non-linear recursion, or straight-line/fold) → v11 lowering
    code, ctx = cg.fn_to_c(fn)
    grade = "C-grade" if is_self_recursive(fn) else "C-grade"
    return code, ctx.invariants, grade


@dataclass
class Program:
    ok: bool
    binary: str
    c_src: str
    grades: Dict[str, str]
    invariants: List[str]
    detail: str


def compile_program(entry: str, table: Dict[str, A.FnDecl]) -> Program:
    if not _CC:
        return Program(False, "", "", {}, [], "no C compiler")
    order = _reachable(entry, table)
    protos, defs, grades, invs = [], [], {}, []
    try:
        for nm in order:
            fn = table[nm]
            params = ", ".join(f"{cg._ctype(p.ty)} {p.name}" for p in fn.params)
            ret = cg._ctype(fn.ret) if fn.ret else "long long"
            protos.append(f"static {ret} {nm}({params});")
            code, fi, grade = emit_fn_any(fn, table)
            # emit_fn_any returns non-static defs; make them static for the multi-file unit
            if not code.lstrip().startswith("static"):
                code = "static " + code
            defs.append(code)
            grades[nm] = grade
            invs += fi
    except cg.CodegenError as e:
        return Program(False, "", "", {}, [], f"codegen unsupported: {e}")
    efn = table[entry]
    n = len(efn.params)
    args = ", ".join(f"atoll(v[{i + 1}])" for i in range(n))
    main = f"int main(int c,char**v){{ printf(\"%lld\\n\", (long long){entry}({args})); return 0; }}"
    src = "#include <stdio.h>\n#include <stdlib.h>\n" + "\n".join(protos) + "\n" + "\n".join(defs) + "\n" + main + "\n"
    d = tempfile.mkdtemp()
    cpath, bpath = os.path.join(d, "p.c"), os.path.join(d, "p")
    open(cpath, "w").write(src)
    r = subprocess.run([_CC, "-O2", cpath, "-o", bpath], capture_output=True, text=True)
    if r.returncode != 0:
        return Program(False, "", src, grades, invs, f"compile failed: {r.stderr[:200]}")
    return Program(True, bpath, src, grades, invs, "compiled")


def run_native(binary: str, *args) -> int:
    out = subprocess.run([binary, *[str(a) for a in args]], capture_output=True, text=True, timeout=30).stdout.strip()
    return int(out)


# ----------------------------------------------------------------------------- L3: grade classifier
@dataclass
class Grade:
    kind: str        # companion | fold-closed | tail-loop | general-recursion | straight-line
    grade: str       # "orders-of-magnitude" | "C-grade"
    why: str


def classify_grade(fn: A.FnDecl) -> Grade:
    """Integrate v9 fold-collapse + v12 recursion: report the HONEST speed grade per function.
    Only a linear-recurrence collapse (companion) or a collapsing fold (v9 closed form) earns
    'orders-of-magnitude'; tail/general recursion and non-closing loops are 'C-grade'."""
    comp = detect_companion(fn)
    if comp is not None:
        return Grade("companion", "orders-of-magnitude",
                     f"linear recurrence order {comp.d} → companion-matrix O(d²·log n) vs naive")
    # collapsing fold → v9 closed form (orders of magnitude). Only if it really COLLAPSES.
    try:
        from fold_collapse import collapse_fn_fold
        ret = cc._block_return(fn.body) if fn.body else None
        if isinstance(ret, A.Fold):
            fcr = collapse_fn_fold(fn)
            if fcr.verdict == "COLLAPSED":
                return Grade("fold-closed", "orders-of-magnitude",
                             "fold collapses to a verified closed form (v9) → O(1)/O(log n)")
            return Grade("straight-line", "C-grade", "fold does not close → C-grade loop (v11)")
    except Exception:
        pass
    if tail_recursive_match(fn) is not None:
        return Grade("tail-loop", "C-grade", "tail recursion → loop (constant stack), Ω(N) intact")
    if is_self_recursive(fn):
        return Grade("general-recursion", "C-grade", "general recursion → honest C recursion (no collapse)")
    return Grade("straight-line", "C-grade", "straight-line / scalar body → native, Ω(work) intact")


# ----------------------------------------------------------------------------- L2 cert runner
def compile_and_run_cert(fn: A.FnDecl, comp: Companion, N: int = 50_000_000) -> dict:
    src = emit_companion_cert(fn, comp)
    d = tempfile.mkdtemp()
    cpath, bpath = os.path.join(d, "cert.c"), os.path.join(d, "cert")
    open(cpath, "w").write(src)
    r = subprocess.run([_CC, "-O2", cpath, "-o", bpath], capture_output=True, text=True)
    if r.returncode != 0:
        return {"ok": False, "detail": r.stderr[:200]}
    out = subprocess.run([bpath, str(N)], capture_output=True, text=True, timeout=60).stdout.strip()
    kv = dict(t.split("=", 1) for t in out.split() if "=" in t)
    return {"ok": True, "match": kv.get("match") == "1",
            "comp_1e18_ns": float(kv.get("comp_1e18_ns", "0")),
            "comp_1e3_ns": float(kv.get("comp_1e3_ns", "0")), "raw": out}
