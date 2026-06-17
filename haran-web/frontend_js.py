"""
HARAN v17 Part C · C3 — JavaScript / TypeScript frontend.
=========================================================
Token-scan → HIR (esprima's native build failed → AST DEFER) + a PERSISTENT node worker for execution
(node startup is ~80ms; a long-lived line-oriented process amortizes it so thousands of property checks
stay fast). TypeScript is compiled with `tsc` first, then run by the same worker.

Honest: JS is dynamically typed — without type info the array/scalar shape is inferred from usage (weak,
like Python). TypeScript's annotations make the shape explicit → stronger. Async / closures DEFER.
"""
from __future__ import annotations

import atexit
import os
import shutil
import subprocess
import tempfile
from typing import Callable, Optional

import hir
import frontend_native

_NODE = shutil.which("node")
_TSC = shutil.which("tsc")


def available() -> bool:
    return _NODE is not None


def _infer_array(source: str, fn: hir.HFunction) -> bool:
    return ("[" in source and "]" in source) or ".length" in source or ".push" in source \
        or "number[]" in source or bool({"index_load", "index_store", "sort"} & fn.op_kinds())


def js_to_hir(source: str, lang: str = "javascript") -> hir.HModule:
    fn = frontend_native.scan_function(source, lang)
    if fn and _infer_array(source, fn):
        fn.signature["kind"] = "array_return"
    elif fn:
        fn.signature["kind"] = "scalar"
    return hir.HModule(lang, [fn] if fn else [], source)


def ts_to_hir(source: str) -> hir.HModule:
    return js_to_hir(source, "typescript")


def _worker_js(hfn: hir.HFunction, body_js: str) -> str:
    name, scalar = hfn.name, hfn.signature["kind"] == "scalar"
    parse = "Number(line.trim())" if scalar else \
            "line.trim()? line.trim().split(/\\s+/).map(Number): []"
    emit = "String(r)" if scalar else "(Array.isArray(r)? r.join(' '): String(r))"
    return (body_js + "\n"
            "const rl=require('readline').createInterface({input:process.stdin});\n"
            "rl.on('line',(line)=>{try{const x=" + parse + ";const r=" + name + "(x);"
            "process.stdout.write(" + emit + "+'\\n');}catch(e){process.stdout.write('ERR\\n');}});\n")


def _prepare_js(hfn: hir.HFunction) -> str:
    """Return a path to a runnable .js worker (compiling TS with tsc if needed)."""
    d = tempfile.mkdtemp()
    if hfn.lang == "typescript" and _TSC is not None:
        tspath = os.path.join(d, "f.ts")
        open(tspath, "w").write("// @ts-nocheck\n" + hfn.source)
        subprocess.run([_TSC, "--target", "es2019", "--outDir", d, "--noEmitOnError", "false", tspath],
                       capture_output=True, text=True, timeout=60)
        jspath = os.path.join(d, "f.js")
        body = open(jspath).read() if os.path.exists(jspath) else hfn.source
    else:
        body = hfn.source
    worker = os.path.join(d, "w.js")
    open(worker, "w").write(_worker_js(hfn, body))
    return worker


class _JsWorker:
    """Long-lived node process: write one input line, read one output line."""
    def __init__(self, worker_path: str):
        self.p = subprocess.Popen([_NODE, worker_path], stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                  text=True, bufsize=1)
        atexit.register(self.close)

    def call(self, line: str) -> str:
        self.p.stdin.write(line + "\n")
        self.p.stdin.flush()
        return self.p.stdout.readline().strip()

    def close(self):
        try:
            self.p.stdin.close(); self.p.terminate()
        except Exception:
            pass


def js_callable(hfn: hir.HFunction) -> Callable:
    worker = _JsWorker(_prepare_js(hfn))
    scalar = hfn.signature["kind"] == "scalar"

    def run(x):
        line = " ".join(str(int(v)) for v in (x if isinstance(x, (list, tuple)) else [x]))
        out = worker.call(line)
        if out == "ERR":
            raise RuntimeError("js function threw")
        toks = out.split()
        vals = [int(float(t)) for t in toks] if toks else []
        return (vals[0] if vals else 0) if scalar else vals
    return run


hir.FRONTENDS["javascript"] = js_to_hir if available() else None
hir.FRONTENDS["typescript"] = ts_to_hir if available() else None
