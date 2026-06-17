"""
HARAN v11 — general native codegen: scalars / ops / loops (and match/let) → C → native.
========================================================================================
v9 emitted only a fold's CLOSED FORM (O(1)). This is the general lowering: any finite HARAN
function body → C (cc -O2) → native, verified against the interpreter.

Two-layer philosophy: fold (layer 1) shrinks code to a closed form (orders of magnitude); whatever
does NOT close lowers to a plain C loop (layer 2) — C-grade, constant-factor, Ω(N) intact. The loop
lowering never invents orders of magnitude; only the closed form (v9) does.
"""
from __future__ import annotations

import os
import subprocess
import tempfile
from dataclasses import dataclass, field
from typing import List, Optional

import haran_ast as A

_CC = None
for _c in ("cc", "gcc", "clang"):
    if subprocess.run(["which", _c], capture_output=True).returncode == 0:
        _CC = _c
        break


def cc_available():
    return _CC


_BINOP = {"+": "+", "-": "-", "*": "*", "/": "/", "%": "%",
          "==": "==", "=": "==", "≠": "!=", "!=": "!=", "<": "<", "≤": "<=", "<=": "<=",
          ">": ">", "≥": ">=", ">=": ">=", "∧": "&&", "&&": "&&", "∨": "||", "||": "||"}


def _ctype(ty) -> str:
    if isinstance(ty, A.TyName):
        if ty.name in ("Nat", "Int"):
            return "long long"
        if ty.name in ("Float", "Real"):
            return "double"
        if ty.name == "Bool":
            return "int"
    return "long long"


class CodegenError(Exception):
    pass


@dataclass
class Ctx:
    lines: List[str] = field(default_factory=list)
    tmp: int = 0
    invariants: List[str] = field(default_factory=list)   # K2/K3: verified facts reflected in C

    def fresh(self, prefix="_t"):
        self.tmp += 1
        return f"{prefix}{self.tmp}"


def _block_return(b):
    return b.stmts[-1].value if isinstance(b, A.Block) and b.stmts and isinstance(b.stmts[-1], A.ExprStmt) else b


def lower(e, ctx: Ctx) -> str:
    """Lower a HARAN expr to a C expression string, emitting any prelude statements to ctx.lines."""
    if isinstance(e, A.Num):
        return e.value if e.is_float else str(int(e.value))
    if isinstance(e, A.BoolLit):
        return "1" if e.value else "0"
    if isinstance(e, A.Var):
        return e.name
    if isinstance(e, A.Un):
        if e.op == "-":
            return f"(-{lower(e.operand, ctx)})"
        if e.op in ("¬", "!"):
            return f"(!{lower(e.operand, ctx)})"
        raise CodegenError(f"unary {e.op}")
    if isinstance(e, A.Bin):
        if e.op == "**" and isinstance(e.rhs, A.Num) and not e.rhs.is_float:
            base = lower(e.lhs, ctx)
            return "(" + "*".join([base] * int(e.rhs.value)) + ")" if int(e.rhs.value) > 0 else "1"
        op = _BINOP.get(e.op)
        if op is None:
            raise CodegenError(f"binop {e.op}")
        return f"({lower(e.lhs, ctx)} {op} {lower(e.rhs, ctx)})"
    if isinstance(e, A.Call) and isinstance(e.func, A.Var):
        return f"{e.func.name}(" + ", ".join(lower(a, ctx) for a in e.args) + ")"
    if isinstance(e, A.Block):
        for st in e.stmts[:-1]:
            if isinstance(st, A.Let):
                ctx.lines.append(f"long long {st.name} = {lower(st.value, ctx)};")
            elif isinstance(st, A.ExprStmt):
                lower(st.value, ctx)
        last = e.stmts[-1]
        return lower(last.value if isinstance(last, A.ExprStmt) else last, ctx)
    if isinstance(e, A.Fold):
        if not isinstance(e.domain, A.Range):
            raise CodegenError("fold domain not a range")
        acc = ctx.fresh("_acc")
        lo = lower(e.domain.lo, ctx)
        hi = lower(e.domain.hi, ctx)
        ctx.lines.append(f"long long {acc} = 0;")
        # K2.2: the verified loop range is the structural invariant reflected directly in the C bounds
        ctx.invariants.append(f"{e.binder} ∈ [{lo}, {hi}]")
        sub = Ctx(tmp=ctx.tmp)
        bval = lower(_block_return(e.body), sub)
        ctx.tmp = sub.tmp
        ctx.lines.append(f"for (long long {e.binder} = {lo}; {e.binder} <= {hi}; {e.binder}++) {{")
        ctx.lines += ["  " + ln for ln in sub.lines]
        ctx.lines.append(f"  {acc} += {bval};")
        ctx.lines.append("}")
        return acc
    if isinstance(e, A.Match):
        res = ctx.fresh("_m")
        ctx.lines.append(f"long long {res};")
        scr = lower(e.scrut, ctx)
        first = True
        for arm in e.arms:
            cond = _pat_cond(arm.pattern, scr)
            sub = Ctx(tmp=ctx.tmp)
            bval = lower(_block_return(arm.body) if isinstance(arm.body, A.Block) else arm.body, sub)
            ctx.tmp = sub.tmp
            kw = "if" if first else ("else if" if cond != "1" else "else")
            head = f"{kw} ({cond})" if kw != "else" else "else"
            ctx.lines.append(f"{head} {{")
            ctx.lines += ["  " + ln for ln in sub.lines]
            ctx.lines.append(f"  {res} = {bval};")
            ctx.lines.append("}")
            first = False
        return res
    raise CodegenError(f"cannot lower {type(e).__name__}")


def _pat_cond(pat, scr) -> str:
    if isinstance(pat, (A.PWild, A.PVar)):
        return "1"
    if isinstance(pat, A.PNum):
        return f"({scr} == {int(pat.value)})"
    if isinstance(pat, A.PBool):
        return f"({scr} == {1 if pat.value else 0})"
    raise CodegenError(f"pattern {type(pat).__name__} not codegen'able (lists/ADT → future)")


def _subst_var(e, old, new):
    if isinstance(e, A.Var):
        return A.Var(new, e.span) if e.name == old else e
    if isinstance(e, A.Bin):
        return A.Bin(e.op, _subst_var(e.lhs, old, new), _subst_var(e.rhs, old, new), e.span)
    if isinstance(e, A.Un):
        return A.Un(e.op, _subst_var(e.operand, old, new), e.span)
    if isinstance(e, A.Call):
        return A.Call(e.func, [_subst_var(a, old, new) for a in e.args], e.span)
    return e


def fn_to_c(fn: A.FnDecl, cname: Optional[str] = None) -> tuple[str, Ctx]:
    cname = cname or fn.name
    params = ", ".join(f"{_ctype(p.ty)} {p.name}" for p in fn.params)
    ret = _ctype(fn.ret) if fn.ret else "long long"
    ctx = Ctx()
    # K3: refinement type {x:T | pred} on a param → emit an ASSUME so -O2 can use the verified range.
    for p in fn.params:
        if isinstance(p.ty, A.TyRefine):
            try:
                pred = _subst_var(p.ty.pred, p.ty.var, p.name)
                cpred = lower(pred, ctx)
                ctx.lines.append(f"if (!({cpred})) __builtin_unreachable();  /* verified refinement */")
                ctx.invariants.append(f"{p.name}: {cpred} (refinement assumed)")
            except CodegenError:
                pass
    val = lower(fn.body, ctx)
    body = "\n  ".join(ctx.lines + [f"return {val};"])
    return f"{ret} {cname}({params}) {{\n  {body}\n}}", ctx


def emit_program(fn: A.FnDecl) -> tuple[str, Ctx]:
    cfn, ctx = fn_to_c(fn)
    n = len(fn.params)
    args = ", ".join(f"atoll(v[{i + 1}])" for i in range(n))
    main = (f"int main(int c, char**v){{ printf(\"%lld\\n\", (long long){fn.name}({args})); return 0; }}")
    return f"#include <stdio.h>\n#include <stdlib.h>\n{cfn}\n{main}\n", ctx


@dataclass
class Compiled:
    ok: bool
    binary: str
    c_src: str
    invariants: List[str]
    detail: str


def compile_fn(fn: A.FnDecl) -> Compiled:
    if not _CC:
        return Compiled(False, "", "", [], "no C compiler")
    try:
        src, ctx = emit_program(fn)
    except CodegenError as e:
        return Compiled(False, "", "", [], f"codegen unsupported: {e}")
    d = tempfile.mkdtemp()
    cpath, bpath = os.path.join(d, "m.c"), os.path.join(d, "m")
    open(cpath, "w").write(src)
    r = subprocess.run([_CC, "-O2", cpath, "-o", bpath], capture_output=True, text=True)
    if r.returncode != 0:
        return Compiled(False, "", src, [], f"compile failed: {r.stderr[:160]}")
    return Compiled(True, bpath, src, ctx.invariants, "compiled")


def run_native(binary: str, *args) -> int:
    r = subprocess.run([binary, *[str(a) for a in args]], capture_output=True, text=True, timeout=30)
    if r.returncode != 0:
        # W3: a native trap (e.g. division by zero → SIGFPE) is a CLEAR error, never a silent wrong answer
        raise RuntimeError(f"native runtime trap (exit {r.returncode}: e.g. division-by-zero / overflow trap)")
    return int(r.stdout.strip())
