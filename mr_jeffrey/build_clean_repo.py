"""
HARAN v25 V1 — build a CLEAN web-app repo from the (large) engine repo.
=======================================================================
Computes the transitive import closure from server.py (the web entry point) and copies ONLY those
modules + the web assets into ../haran-web/. The 200+ unused engine/kernel files are left behind; the
original mr_jeffrey/ is untouched (copy, not move).

Run:  python3 build_clean_repo.py        # → ../haran-web/  (reproducible, deterministic)
"""
from __future__ import annotations

import ast
import os
import shutil

SRC = os.path.dirname(os.path.abspath(__file__))
DST = os.path.abspath(os.path.join(SRC, "..", "haran-web"))
ENTRY = "server"
ASSETS = ["haran.html", "requirements.txt", "Dockerfile", "docker-compose.yml", ".dockerignore"]


def _local_modules() -> set:
    return {f[:-3] for f in os.listdir(SRC) if f.endswith(".py")}


def _imports_of(mod: str, local: set) -> set:
    path = os.path.join(SRC, mod + ".py")
    if not os.path.exists(path):
        return set()
    tree = ast.parse(open(path, encoding="utf-8").read())
    out = set()
    for n in ast.walk(tree):
        if isinstance(n, ast.Import):
            out |= {a.name.split(".")[0] for a in n.names if a.name.split(".")[0] in local}
        elif isinstance(n, ast.ImportFrom) and n.module:
            base = n.module.split(".")[0]
            if base in local:
                out.add(base)
    return out


def closure() -> list:
    """Transitive import closure from ENTRY, restricted to local modules. Also pulls in the lazily
    string-imported language frontends declared in hir._FRONTEND_MODULES (dynamic __import__)."""
    local = _local_modules()
    seen, stack = set(), [ENTRY]
    while stack:
        m = stack.pop()
        if m in seen:
            continue
        seen.add(m)
        stack += [d for d in _imports_of(m, local) if d not in seen]
    # dynamic frontends (hir does __import__(name) from a string table) — include if present
    for fe in ("frontend_c", "frontend_go", "frontend_rust", "frontend_js", "frontend_java",
               "frontend_native"):
        if fe in local:
            seen.add(fe)
    return sorted(seen)


def build():
    mods = closure()
    if os.path.exists(DST):
        shutil.rmtree(DST)
    os.makedirs(DST)
    for m in mods:
        shutil.copy2(os.path.join(SRC, m + ".py"), os.path.join(DST, m + ".py"))
    for a in ASSETS:
        s = os.path.join(SRC, a)
        if os.path.exists(s):
            shutil.copy2(s, os.path.join(DST, a))
    # the clean repo's README is maintained at mr_jeffrey/haran_web_README.md (so re-runs keep it)
    rs = os.path.join(SRC, "haran_web_README.md")
    if os.path.exists(rs):
        shutil.copy2(rs, os.path.join(DST, "README.md"))
    # standalone .gitignore so `git init && git add .` in haran-web is clean
    with open(os.path.join(DST, ".gitignore"), "w", encoding="utf-8") as f:
        f.write("__pycache__/\n*.pyc\n*.pyo\n.env\n.venv/\n.coverage\n")
    print(f"clean repo → {DST}")
    print(f"  modules: {len(mods)}  (from {len(_local_modules())} in the engine)")
    print(f"  assets : {[a for a in ASSETS if os.path.exists(os.path.join(SRC, a))]}")
    return mods


if __name__ == "__main__":
    build()
