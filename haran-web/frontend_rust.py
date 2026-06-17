"""HARAN v17 Part C · C2 — Rust frontend: token-scan → HIR + `rustc` compile-and-run runtime.

Honest: op extraction is the heuristic token scanner (syn AST = DEFER, needs crates.io). Execution is
the real rustc — so property testing still observes genuine Rust behaviour.
"""
from __future__ import annotations

import hashlib
import os
import shutil
import subprocess
import tempfile
from typing import Callable

import hir
import frontend_native

_RUSTC = shutil.which("rustc")


def available() -> bool:
    return _RUSTC is not None


def rust_to_hir(source: str) -> hir.HModule:
    fn = frontend_native.scan_function(source, "rust")
    return hir.HModule("rust", [fn] if fn else [], source)


_BIN: dict = {}


def _harness(hfn: hir.HFunction) -> str:
    name, kind = hfn.name, hfn.signature["kind"]
    body_read = "let a:Vec<i32>=std::env::args().skip(1).map(|s|s.parse().unwrap()).collect();"
    if kind in ("array_return", "array_inplace"):
        call = f"for v in {name}(a){{print!(\"{{}} \",v);}}"
        main = "fn main(){" + body_read + call + "}\n"
    else:
        main = ("fn main(){let x:i32=std::env::args().nth(1).unwrap_or(\"0\".into()).parse().unwrap_or(0);"
                f"print!(\"{{}} \",{name}(x));}}\n")
    return hfn.source + "\n" + main


def _compile(hfn: hir.HFunction) -> str:
    key = hashlib.sha256((hfn.source + hfn.name).encode()).hexdigest()[:16]
    if key in _BIN:
        return _BIN[key]
    d = tempfile.mkdtemp()
    src, binp = os.path.join(d, "m.rs"), os.path.join(d, "m.bin")
    open(src, "w").write(_harness(hfn))
    r = subprocess.run([_RUSTC, "-O", "-A", "warnings", src, "-o", binp],
                       capture_output=True, text=True, timeout=60)
    if r.returncode != 0:
        raise RuntimeError(f"rustc failed: {r.stderr[:200]}")
    _BIN[key] = binp
    return binp


def rust_callable(hfn: hir.HFunction) -> Callable:
    binp = _compile(hfn)
    scalar = hfn.signature["kind"] == "scalar"

    def run(x):
        args = [str(int(v)) for v in (x if isinstance(x, (list, tuple)) else [x])]
        out = subprocess.run([binp, *args], capture_output=True, text=True, timeout=15)
        toks = out.stdout.split()
        vals = [int(t) for t in toks] if toks else []
        return (vals[0] if vals else 0) if scalar else vals
    return run


hir.FRONTENDS["rust"] = rust_to_hir if available() else None
