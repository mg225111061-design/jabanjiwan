"""
HARAN v17 — runtime.make_callable: turn an HFunction (any language) into a Python-callable.
===========================================================================================
This is the seam that lets the ONE Type B engine (properties → testing → narrowing) run on every
language: each frontend lowers to HIR + a language tag; here we produce a callable that, given a Python
input, returns the function's real output by EXECUTING it in its native runtime (exec for Python; compile
+ run for C/Go/Rust/Java; node for JS/TS). The engine never knows the language — it just calls.

Honest: native runtimes are cached-compiled per source; a compile error or missing toolchain surfaces as
an exception the caller treats as a property crash (never a silent pass).
"""
from __future__ import annotations

from typing import Callable, Optional

import hir


def make_callable(hfn: hir.HFunction, glb: Optional[dict] = None) -> Callable:
    lang = getattr(hfn, "lang", "python")
    if lang == "python":
        ns: dict = {} if glb is None else dict(glb)
        exec(hfn.source, ns)
        return ns[hfn.name]
    if lang == "c":
        import frontend_c
        return frontend_c.c_callable(hfn)
    if lang in ("go",):
        import frontend_go
        return frontend_go.go_callable(hfn)
    if lang in ("rust",):
        import frontend_rust
        return frontend_rust.rust_callable(hfn)
    if lang in ("javascript", "typescript"):
        import frontend_js
        return frontend_js.js_callable(hfn)
    if lang == "java":
        import frontend_java
        return frontend_java.java_callable(hfn)
    raise NotImplementedError(f"no runtime for language '{lang}'")
