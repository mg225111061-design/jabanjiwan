"""
HARAN v17 Part C · STAGE C1 — C frontend (pycparser → HIR) + gcc compile-and-run runtime.
=========================================================================================
Parses C into the common HIR (operations + line numbers, same vocabulary as Python) so B2–B6 apply
unchanged, and provides c_callable() which compiles the function with gcc and runs it on real inputs —
so property testing observes the REAL C behaviour, not a reimplementation.

Honest scope (C1 discipline): we cover the common numeric / int-array shapes (in-place or returned
`int*`, and scalar `int→int`). Pointer aliasing beyond a single array, structs, and full preprocessor
includes are DEFER. Undefined behaviour (UB) is C's own hazard — flagged separately, not silently fixed.
"""
from __future__ import annotations

import hashlib
import os
import subprocess
import tempfile
from typing import Callable, List, Optional

import hir

try:
    from pycparser import c_parser, c_ast
    _OK = True
except Exception:  # pragma: no cover
    _OK = False


def available() -> bool:
    return _OK


# ----------------------------------------------------------------- C AST → HIR operations
if _OK:
    class _Ops(c_ast.NodeVisitor):
        def __init__(self):
            self.ops: List[hir.HOp] = []

        def _ln(self, node):
            return node.coord.line if node.coord else 0

        def visit_BinaryOp(self, node):
            if node.op in ("<", ">", "<=", ">=", "==", "!="):
                self.ops.append(hir.HOp("compare", self._ln(node), node.op))
            else:
                self.ops.append(hir.HOp("arith", self._ln(node), node.op))
            self.generic_visit(node)

        def visit_Assignment(self, node):
            if isinstance(node.lvalue, c_ast.ArrayRef):
                self.ops.append(hir.HOp("index_store", self._ln(node)))
            elif node.op != "=":
                self.ops.append(hir.HOp("aug", self._ln(node), node.op))
            self.generic_visit(node)

        def visit_ArrayRef(self, node):
            self.ops.append(hir.HOp("index_load", self._ln(node)))
            self.generic_visit(node)

        def visit_FuncCall(self, node):
            name = node.name.name if isinstance(node.name, c_ast.ID) else "call"
            self.ops.append(hir.HOp("call", self._ln(node), name))
            self.generic_visit(node)

        def visit_Return(self, node):
            self.ops.append(hir.HOp("return", self._ln(node)))
            self.generic_visit(node)


def _signature(fdef) -> dict:
    """Classify the calling shape from the FuncDef declaration."""
    decl = fdef.decl.type
    ret = decl.type
    # return type spelling
    ret_ptr = isinstance(ret, c_ast.PtrDecl)
    params = []
    if decl.args:
        for p in decl.args.params:
            is_ptr = isinstance(p.type, c_ast.PtrDecl) or isinstance(p.type, c_ast.ArrayDecl)
            params.append((p.name, "ptr" if is_ptr else "scalar"))
    arr = next((nm for nm, k in params if k == "ptr"), None)
    nparam = next((nm for nm, k in params if k == "scalar"), None)
    if arr is not None:
        kind = "array_return" if ret_ptr else "array_inplace"
        return {"kind": kind, "arr": arr, "n": nparam}
    return {"kind": "scalar", "arg": params[0][0] if params else None}


def c_to_hir(source: str) -> hir.HModule:
    if not _OK:
        raise RuntimeError("pycparser not available")
    # Full C preprocessor is out of scope (see module docstring). pycparser cannot parse `#...`
    # directives and throws an opaque ParseError; detect them up front and raise a *catchable*
    # SyntaxError so the caller (hir.to_hir) reports an honest DEFER instead of crashing.
    if any(ln.lstrip().startswith("#") for ln in source.splitlines()):
        raise SyntaxError("C preprocessor directives are out of scope (DEFER): "
                          "run the preprocessor first, or pass directive-free C")
    ast = c_parser.CParser().parse(source)
    src_lines = source.splitlines()
    fns: List[hir.HFunction] = []
    for ext in ast.ext:
        if isinstance(ext, c_ast.FuncDef):
            v = _Ops()
            v.visit(ext.body)
            params = []
            if ext.decl.type.args:
                params = [p.name for p in ext.decl.type.args.params]
            start = ext.decl.coord.line if ext.decl.coord else 1
            end = max([o.line for o in v.ops] + [start])
            fns.append(hir.HFunction(ext.decl.name, params, v.ops,
                                     source, start, end, lang="c", signature=_signature(ext)))
    return hir.HModule("c", fns, source)


# ----------------------------------------------------------------- gcc compile + run runtime
_BINCACHE: dict = {}


def _harness(hfn: hir.HFunction) -> str:
    sig = hfn.signature
    name = hfn.name
    pre = "#include <stdio.h>\n#include <stdlib.h>\n"
    if sig["kind"] in ("array_inplace", "array_return"):
        call = (f"{name}(a, n);" if sig["kind"] == "array_inplace"
                else f"int* r = {name}(a, n); for(int i=0;i<n;i++) a[i]=r[i];")
        return (pre + hfn.source + "\n"
                "int main(int argc,char**argv){int n=argc-1;int* a=malloc(sizeof(int)*(n>0?n:1));"
                "for(int i=0;i<n;i++)a[i]=atoi(argv[i+1]);" + call +
                "for(int i=0;i<n;i++)printf(\"%d \",a[i]);return 0;}\n")
    return (pre + hfn.source + "\n"
            "int main(int argc,char**argv){int x=argc>1?atoi(argv[1]):0;"
            f"printf(\"%d \",{name}(x));return 0;}}\n")


def _compile(hfn: hir.HFunction) -> str:
    key = hashlib.sha256((hfn.source + hfn.name).encode()).hexdigest()[:16]
    if key in _BINCACHE:
        return _BINCACHE[key]
    d = tempfile.mkdtemp()
    cpath, bpath = os.path.join(d, "p.c"), os.path.join(d, "p.bin")
    open(cpath, "w").write(_harness(hfn))
    r = subprocess.run(["gcc", "-O0", "-w", cpath, "-o", bpath], capture_output=True, text=True, timeout=30)
    if r.returncode != 0:
        raise RuntimeError(f"gcc failed: {r.stderr[:200]}")
    _BINCACHE[key] = bpath
    return bpath


def c_callable(hfn: hir.HFunction) -> Callable:
    binp = _compile(hfn)
    scalar = hfn.signature["kind"] == "scalar"

    def run(x):
        args = [str(int(v)) for v in (x if isinstance(x, (list, tuple)) else [x])]
        out = subprocess.run([binp, *args], capture_output=True, text=True, timeout=15)
        toks = out.stdout.split()
        vals = [int(t) for t in toks] if toks else []
        return (vals[0] if vals else 0) if scalar else vals
    return run


hir.FRONTENDS["c"] = c_to_hir if _OK else None
