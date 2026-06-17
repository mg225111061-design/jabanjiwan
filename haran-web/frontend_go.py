"""HARAN v17 Part C · C2 — Go frontend: token-scan → HIR + `go build` compile-and-run runtime."""
from __future__ import annotations

import hashlib
import os
import shutil
import subprocess
import tempfile
from typing import Callable

import hir
import frontend_native

_GO = shutil.which("go")


def available() -> bool:
    return _GO is not None


def go_to_hir(source: str) -> hir.HModule:
    fn = frontend_native.scan_function(source, "go")
    return hir.HModule("go", [fn] if fn else [], source)


_BIN: dict = {}


def _harness(hfn: hir.HFunction) -> str:
    name, kind = hfn.name, hfn.signature["kind"]
    head = 'package main\nimport ("fmt";"os";"strconv")\n' + hfn.source + "\n"
    read = ('var a []int; for _,s:=range os.Args[1:]{v,_:=strconv.Atoi(s); a=append(a,v)}; ')
    if kind == "array_return":
        body = read + f"r:={name}(a); for _,v:=range r{{fmt.Printf(\"%d \",v)}}"
    elif kind == "array_inplace":
        body = read + f"{name}(a); for _,v:=range a{{fmt.Printf(\"%d \",v)}}"
    else:
        body = 'x:=0; if len(os.Args)>1 {x,_=strconv.Atoi(os.Args[1])}; ' + f"fmt.Printf(\"%d \",{name}(x))"
    return head + "func main(){" + body + "}\n"


def _compile(hfn: hir.HFunction) -> str:
    key = hashlib.sha256((hfn.source + hfn.name).encode()).hexdigest()[:16]
    if key in _BIN:
        return _BIN[key]
    d = tempfile.mkdtemp()
    src, binp = os.path.join(d, "m.go"), os.path.join(d, "m.bin")
    open(src, "w").write(_harness(hfn))
    gocache = os.path.join(tempfile.gettempdir(), "haran_gocache")   # persistent → fast repeat builds
    r = subprocess.run([_GO, "build", "-o", binp, src], capture_output=True, text=True, timeout=90,
                       env={**os.environ, "GO111MODULE": "off", "GOCACHE": gocache})
    if r.returncode != 0:
        raise RuntimeError(f"go build failed: {r.stderr[:200]}")
    _BIN[key] = binp
    return binp


def go_callable(hfn: hir.HFunction) -> Callable:
    binp = _compile(hfn)
    scalar = hfn.signature["kind"] == "scalar"

    def run(x):
        args = [str(int(v)) for v in (x if isinstance(x, (list, tuple)) else [x])]
        out = subprocess.run([binp, *args], capture_output=True, text=True, timeout=15)
        toks = out.stdout.split()
        vals = [int(t) for t in toks] if toks else []
        return (vals[0] if vals else 0) if scalar else vals
    return run


hir.FRONTENDS["go"] = go_to_hir if available() else None
