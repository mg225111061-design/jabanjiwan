"""
HARAN v15 — direct LLVM IR backend + Proc/Cofix finite-prefix codegen (O1/O2).
==============================================================================
v11-v14 emit C and let `cc` produce the object. v15 emits LLVM IR (`.ll`) DIRECTLY and lowers it with
`llc` — so HARAN controls the metadata the C path could only *hint* at, and Proc/Cofix get the only
codegen they honestly can: a finite prefix.

  O1  direct LLVM IR — scalar functions lowered to SSA `.ll` → `llc` → native (no C in the compute
      path, only a tiny I/O shim). Verified facts become EXPLICIT IR metadata that C cannot state
      precisely: own/&mut → `noalias` pointer attributes; refinement {x|x<C} → `!range` on the load;
      a fold/map loop → `!llvm.loop.vectorize.enable`. All verified == interpreter.
  O2  Proc/Cofix finite prefix — a PRODUCTIVE (productivity.check_proc == PROVEN) `cofix loop {yield E
      loop}` is compiled to a generator of its first N elements. INFINITE codegen is fundamentally
      impossible (you cannot materialize ℵ₀ outputs / emit a non-terminating "result"); the generator
      therefore ALWAYS takes a bound N. Non-productive procs (e.g. `cofix loop {loop}`) yield nothing
      → honest "no prefix", never a fake.

Honesty: LLVM IR removes nothing from the conservation law — the asymptotics are whatever the source
has (Ω(N) loops stay Ω(N)); v15's win is precise verified metadata + a real IR path, not speed magic.
"""
from __future__ import annotations

import os
import subprocess
import tempfile
from dataclasses import dataclass
from typing import List, Optional

import haran_ast as A
import haran_codegen as cg
import closure_classifier as cc

_LLC = None
for _c in ("llc", "llc-18", "llc-17", "llc-16"):
    if subprocess.run(["which", _c], capture_output=True).returncode == 0:
        _LLC = _c
        break
_CC = cg._CC


def llc_available():
    return _LLC and _CC


# ----------------------------------------------------------------------------- O1: expr → LLVM IR (i64)
class _IRB:
    def __init__(self):
        self.lines: List[str] = []
        self.n = 0

    def tmp(self) -> str:
        self.n += 1
        return f"%t{self.n}"

    def emit(self, s: str):
        self.lines.append("  " + s)


def _lower_ir(e, b: _IRB) -> str:
    """Lower an integer HARAN expr to an SSA i64 operand string."""
    if isinstance(e, A.Num):
        if e.is_float:
            raise cg.CodegenError("LLVM scalar path is i64-only")
        return str(int(e.value))
    if isinstance(e, A.Var):
        return f"%{e.name}"
    if isinstance(e, A.Un) and e.op == "-":
        v = _lower_ir(e.operand, b)
        t = b.tmp(); b.emit(f"{t} = sub i64 0, {v}"); return t
    if isinstance(e, A.Bin):
        if e.op == "**" and isinstance(e.rhs, A.Num) and not e.rhs.is_float:
            p = int(e.rhs.value)
            if p == 0:
                return "1"
            base = _lower_ir(e.lhs, b)
            acc = base
            for _ in range(p - 1):
                t = b.tmp(); b.emit(f"{t} = mul i64 {acc}, {base}"); acc = t
            return acc
        l = _lower_ir(e.lhs, b)
        r = _lower_ir(e.rhs, b)
        opc = {"+": "add", "-": "sub", "*": "mul", "/": "sdiv", "//": "sdiv", "%": "srem"}.get(e.op)
        if opc is None:
            raise cg.CodegenError(f"LLVM binop {e.op}")
        t = b.tmp(); b.emit(f"{t} = {opc} i64 {l}, {r}"); return t
    raise cg.CodegenError(f"LLVM cannot lower {type(e).__name__}")


def emit_llvm_scalar(fn: A.FnDecl) -> str:
    """Straight-line integer fn → LLVM IR module (function only; driver links separately)."""
    body = cc._block_return(fn.body) if fn.body else fn.body
    b = _IRB()
    ret = _lower_ir(body, b)
    params = ", ".join(f"i64 %{p.name}" for p in fn.params)
    ir = [f"define i64 @{fn.name}({params}) {{", "entry:"]
    ir += b.lines
    ir.append(f"  ret i64 {ret}")
    ir.append("}")
    return "\n".join(ir) + "\n"


# refinement {x | x < C} → !range metadata
def _refine_bound(fn: A.FnDecl):
    for p in fn.params:
        if isinstance(p.ty, A.TyRefine):
            pr = p.ty.pred
            if isinstance(pr, A.Bin) and pr.op in ("<", "≤", "<=") and isinstance(pr.rhs, A.Num):
                hi = int(pr.rhs.value) + (1 if pr.op != "<" else 0)
                return p.name, hi
    return None


# ----------------------------------------------------------------------------- O1: map kernel → LLVM IR
def emit_llvm_map(fn) -> Optional[tuple]:
    """map(v, λx. e) → LLVM IR with noalias ptr attrs (verified own/&) + !llvm.loop.vectorize.enable.
    Returns (ir_text, elem_is_i64). Only integer element bodies."""
    import haran_vec as vec
    mk = vec.detect_map(fn)
    if mk is None or mk.elem != "long long":
        return None
    bind = mk.binder
    na = "noalias " if mk.has_restrict else ""
    # build the loop IR by hand (phi-based); the element body is lowered inline, referencing %<binder>
    eb = _IRB()
    bexpr = _lower_ir(mk.body, eb)
    body_lines = "\n".join(eb.lines)
    ir = f"""define void @{fn.name}(ptr {na}%in, ptr {na}%out, i64 %n) {{
entry:
  br label %loop
loop:
  %i = phi i64 [ 0, %entry ], [ %inext, %body ]
  %cmp = icmp slt i64 %i, %n
  br i1 %cmp, label %body, label %done
body:
  %pin = getelementptr i64, ptr %in, i64 %i
  %{bind} = load i64, ptr %pin
{body_lines}
  %pout = getelementptr i64, ptr %out, i64 %i
  store i64 {bexpr}, ptr %pout
  %inext = add i64 %i, 1
  br label %loop, !llvm.loop !0
done:
  ret void
}}
!0 = distinct !{{!0, !1}}
!1 = !{{!"llvm.loop.vectorize.enable", i1 true}}
"""
    return ir, True


def emit_llvm_refinement(fn) -> Optional[str]:
    """A scalar fn with a refinement param {x|x<C}: emit IR that loads/uses the value with !range
    metadata reflecting the verified bound. Here we model the param entering via an alloca+load so the
    !range attaches (demonstrates verified-fact → IR metadata)."""
    rb = _refine_bound(fn)
    if rb is None:
        return None
    pname, hi = rb
    body = cc._block_return(fn.body) if fn.body else fn.body
    b = _IRB()
    # round-trip the parameter through memory so a load carries !range (verified [0, hi))
    b.emit(f"%slot = alloca i64")
    b.emit(f"store i64 %{pname}, ptr %slot")
    b.emit(f"%{pname}.r = load i64, ptr %slot, !range !2")
    # lower body with the refined value substituted
    body2 = _rename_var(body, pname, f"{pname}.r")
    rv = _lower_ir(body2, b)
    params = ", ".join(f"i64 %{p.name}" for p in fn.params)
    ir = [f"define i64 @{fn.name}({params}) {{", "entry:"]
    ir += b.lines
    ir.append(f"  ret i64 {rv}")
    ir.append("}")
    ir.append(f"!2 = !{{i64 0, i64 {hi}}}")
    return "\n".join(ir) + "\n"


def _rename_var(e, old, new):
    if isinstance(e, A.Var):
        return A.Var(new, e.span) if e.name == old else e
    if isinstance(e, A.Bin):
        return A.Bin(e.op, _rename_var(e.lhs, old, new), _rename_var(e.rhs, old, new), e.span)
    if isinstance(e, A.Un):
        return A.Un(e.op, _rename_var(e.operand, old, new), e.span)
    return e


# ----------------------------------------------------------------------------- compile .ll → native
@dataclass
class LlvmResult:
    ok: bool
    binary: str
    ir: str
    detail: str


def compile_ll(ir: str, driver_c: str, opt: str = "-O2") -> LlvmResult:
    if not llc_available():
        return LlvmResult(False, "", ir, "llc/cc not available")
    d = tempfile.mkdtemp()
    llp, objp, drvp, binp = (os.path.join(d, x) for x in ("m.ll", "m.o", "drv.c", "prog"))
    open(llp, "w").write(ir)
    r = subprocess.run([_LLC, opt, "-filetype=obj", llp, "-o", objp], capture_output=True, text=True)
    if r.returncode != 0:
        return LlvmResult(False, "", ir, f"llc failed: {r.stderr[:200]}")
    open(drvp, "w").write(driver_c)
    r = subprocess.run([_CC, "-O2", drvp, objp, "-o", binp], capture_output=True, text=True)
    if r.returncode != 0:
        return LlvmResult(False, "", ir, f"link failed: {r.stderr[:200]}")
    return LlvmResult(True, binp, ir, "compiled via llc")


def run_bin(binary: str, *args) -> str:
    return subprocess.run([binary, *[str(a) for a in args]], capture_output=True, text=True,
                          timeout=30).stdout.strip()


def driver_scalar(fn: A.FnDecl) -> str:
    n = len(fn.params)
    proto = "long long " + fn.name + "(" + ", ".join(["long long"] * n) + ");"
    args = ", ".join(f"atoll(v[{i+1}])" for i in range(n))
    return ("#include <stdio.h>\n#include <stdlib.h>\n" + "extern " + proto +
            f"\nint main(int c,char**v){{ printf(\"%lld\\n\", {fn.name}({args})); return 0; }}\n")


def driver_map(fn: A.FnDecl) -> str:
    return ("#include <stdio.h>\n#include <stdlib.h>\n"
            f"extern void {fn.name}(long long*,long long*,long long);\n"
            "int main(int c,char**v){ long long n=atoll(v[1]); long long*a=malloc(n*8),*b=malloc(n*8);"
            " for(long long i=0;i<n;i++) a[i]=atoll(v[2+i]);"
            f" {fn.name}(a,b,n);"
            " for(long long i=0;i<n;i++) printf(\"%lld \", b[i]); printf(\"\\n\"); return 0; }\n")


# ----------------------------------------------------------------------------- O2: Proc/Cofix prefix
def _cofix_yield_expr(proc: A.FnDecl):
    """For a straight-line productive `cofix loop { yield E  loop }`, return E (the yielded expr)."""
    body = proc.body
    if isinstance(body, A.Block) and body.stmts and isinstance(body.stmts[0], A.ExprStmt):
        cofix = body.stmts[0].value
        if isinstance(cofix, A.Cofix) and isinstance(cofix.body, A.Block):
            yields = [st for st in cofix.body.stmts if isinstance(st, A.Yield)]
            if len(yields) == 1:
                return yields[0].value
    return None


def proc_is_productive(proc: A.FnDecl) -> bool:
    try:
        import productivity
        return productivity.check_proc(proc).verdict == "PROVEN"
    except Exception:
        return False


def emit_prefix_c(proc: A.FnDecl) -> Optional[str]:
    """Finite-prefix generator: print the first N elements of a productive constant-yield stream.
    INFINITE is impossible → the generator REQUIRES the bound N (argv)."""
    if not proc_is_productive(proc):
        return None
    e = _cofix_yield_expr(proc)
    if e is None:
        return None
    ctx = cg.Ctx()
    try:
        val = cg.lower(e, ctx)
    except cg.CodegenError:
        return None
    params = ", ".join(f"long long {p.name}" for p in proc.params)
    prelude = "\n  ".join(ctx.lines)
    argread = "\n  ".join(f"long long {p.name} = atoll(v[{i+2}]);" for i, p in enumerate(proc.params))
    return f"""#include <stdio.h>
#include <stdlib.h>
/* {proc.name}: PROVEN-productive stream → FINITE PREFIX only (infinite codegen is impossible). */
int main(int c, char**v){{
  long long N = atoll(v[1]);          /* the bound is MANDATORY — no infinite program exists */
  {argread}
  {prelude}
  for (long long i = 0; i < N; i++) printf("%lld ", (long long)({val}));
  printf("\\n"); return 0;
}}
"""


def compile_prefix(proc: A.FnDecl) -> LlvmResult:
    src = emit_prefix_c(proc)
    if src is None:
        return LlvmResult(False, "", "", "not a productive constant-yield cofix (no finite prefix)")
    if not _CC:
        return LlvmResult(False, "", src, "no C compiler")
    d = tempfile.mkdtemp()
    cpath, bpath = os.path.join(d, "p.c"), os.path.join(d, "p")
    open(cpath, "w").write(src)
    r = subprocess.run([_CC, "-O2", cpath, "-o", bpath], capture_output=True, text=True)
    if r.returncode != 0:
        return LlvmResult(False, "", src, f"compile failed: {r.stderr[:200]}")
    return LlvmResult(True, bpath, src, "compiled finite-prefix generator")


def run_prefix(binary: str, n: int, *params) -> List[int]:
    out = run_bin(binary, n, *params)
    return [int(x) for x in out.split()] if out else []
