"""
HARAN v17 Part C · STAGE C4 — Java frontend (javalang AST → HIR) + javac/persistent-JVM runtime.
================================================================================================
javalang gives an accurate AST (op kinds) but spotty positions on expressions, so we propagate the
enclosing statement's line to its operations. Execution wraps the method in a class with a long-lived
line-oriented main (the JVM stays warm — per-call `java` startup would be ~300ms).

Honest scope: static int / int[] methods (the numeric / array shape). OOP (inheritance, polymorphism,
generics, objects) is DEFER. Defects4J (the real Java benchmark) needs a heavy project checkout → DEFER;
we demo a single Java bug end-to-end here.
"""
from __future__ import annotations

import hashlib
import os
import shutil
import subprocess
import tempfile
import atexit
from typing import Callable, List, Optional

import hir

try:
    import javalang
    import javalang.tree as JT
    _OK = True
except Exception:  # pragma: no cover
    _OK = False

_JAVAC = shutil.which("javac")
_JAVA = shutil.which("java")


def available() -> bool:
    return _OK and _JAVAC is not None and _JAVA is not None


_CMP = {"<", ">", "<=", ">=", "==", "!="}


def _walk(node, line, ops: List[hir.HOp]):
    pos = getattr(node, "position", None)
    if pos is not None:
        line = pos.line
    if isinstance(node, JT.BinaryOperation):
        ops.append(hir.HOp("compare" if node.operator in _CMP else "arith", line, node.operator))
    elif isinstance(node, JT.MethodInvocation):
        ops.append(hir.HOp("call", line, node.member))
    elif isinstance(node, JT.Assignment):
        tgt = node.expressionl
        if isinstance(tgt, JT.MemberReference) and getattr(tgt, "selectors", None):
            ops.append(hir.HOp("index_store", line))
    elif isinstance(node, JT.ReturnStatement):
        ops.append(hir.HOp("return", line))
    elif isinstance(node, JT.MemberReference) and getattr(node, "selectors", None):
        ops.append(hir.HOp("index_load", line))
    # recurse over children
    for child in getattr(node, "children", []):
        for c in (child if isinstance(child, list) else [child]):
            if isinstance(c, JT.Node):
                _walk(c, line, ops)


def java_to_hir(source: str) -> hir.HModule:
    if not _OK:
        raise RuntimeError("javalang not available")
    wrapped = source if "class" in source.split("(")[0] else "class T {\n" + source + "\n}\n"
    tree = javalang.parse.parse(wrapped)
    fns: List[hir.HFunction] = []
    for _, m in tree.filter(JT.MethodDeclaration):
        ops: List[hir.HOp] = []
        for stmt in (m.body or []):
            _walk(stmt, m.position.line if m.position else 1, ops)
        params = [p.name for p in m.parameters]
        ret = m.return_type
        ret_arr = bool(ret and getattr(ret, "dimensions", None))
        parr = any(getattr(p.type, "dimensions", None) for p in m.parameters)
        kind = "array_return" if (parr and ret_arr) else ("array_inplace" if parr else "scalar")
        start = m.position.line if m.position else 1
        end = max([o.line for o in ops] + [start])
        fns.append(hir.HFunction(m.name, params, ops, source, start, end, lang="java",
                                 signature={"kind": kind, "arr": params[0] if params else None}))
    return hir.HModule("java", fns, source)


# ----------------------------------------------------------------- javac + persistent JVM worker
def _main_java(hfn: hir.HFunction) -> str:
    name, kind = hfn.name, hfn.signature["kind"]
    method = hfn.source
    if not method.strip().startswith(("static", "public", "private")):
        method = "static " + method
    if kind in ("array_return", "array_inplace"):
        call = (f"int[] r = {name}(a);" if kind == "array_return" else f"{name}(a); int[] r = a;")
        emit = ("StringBuilder sb=new StringBuilder(); for(int v:r) sb.append(v).append(\" \"); "
                "System.out.println(sb.toString().trim());")
        parse = ("int[] a = line.isEmpty()? new int[0] : "
                 "java.util.Arrays.stream(line.split(\"\\\\s+\")).mapToInt(Integer::parseInt).toArray();")
    else:
        call = f"int r = {name}(Integer.parseInt(line.isEmpty()?\"0\":line));"
        emit = "System.out.println(r);"
        parse = ""
    return ("public class Main {\n" + method + "\n"
            "public static void main(String[] args) throws Exception {\n"
            "java.io.BufferedReader br=new java.io.BufferedReader(new java.io.InputStreamReader(System.in));\n"
            "String line; while((line=br.readLine())!=null){ line=line.trim(); try {\n"
            + parse + call + emit + "\n} catch(Exception e){ System.out.println(\"ERR\"); } } }\n}\n")


_DIRS: dict = {}


def _compile(hfn: hir.HFunction) -> str:
    key = hashlib.sha256((hfn.source + hfn.name).encode()).hexdigest()[:16]
    if key in _DIRS:
        return _DIRS[key]
    d = tempfile.mkdtemp()
    open(os.path.join(d, "Main.java"), "w").write(_main_java(hfn))
    r = subprocess.run([_JAVAC, "Main.java"], capture_output=True, text=True, cwd=d, timeout=60)
    if r.returncode != 0:
        raise RuntimeError(f"javac failed: {r.stderr[:200]}")
    _DIRS[key] = d
    return d


class _JavaWorker:
    def __init__(self, d: str):
        self.p = subprocess.Popen([_JAVA, "-cp", d, "Main"], stdin=subprocess.PIPE,
                                  stdout=subprocess.PIPE, text=True, bufsize=1)
        atexit.register(self.close)

    def call(self, line: str) -> str:
        self.p.stdin.write(line + "\n"); self.p.stdin.flush()
        return self.p.stdout.readline().strip()

    def close(self):
        try:
            self.p.stdin.close(); self.p.terminate()
        except Exception:
            pass


def java_callable(hfn: hir.HFunction) -> Callable:
    worker = _JavaWorker(_compile(hfn))
    scalar = hfn.signature["kind"] == "scalar"

    def run(x):
        line = " ".join(str(int(v)) for v in (x if isinstance(x, (list, tuple)) else [x]))
        out = worker.call(line)
        if out == "ERR":
            raise RuntimeError("java method threw")
        toks = out.split()
        vals = [int(t) for t in toks] if toks else []
        return (vals[0] if vals else 0) if scalar else vals
    return run


hir.FRONTENDS["java"] = java_to_hir if available() else None
