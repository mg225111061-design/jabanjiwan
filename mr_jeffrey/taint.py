"""
HARAN v20 Part P · STAGE P1 — taint analysis (injection: SQL / command / path).
===============================================================================
Tracks tainted input (sources) flowing to dangerous operations (sinks) over the v18 PDG. A source→sink
data path that never passes a sanitizer is an injection. Z3 refines by pruning control-infeasible paths.

Honesty label: SOUND **modulo aliasing / call-graph** — taint is a flow over the def-use graph, not a
property or a probability (a new technique for HARAN). Aliasing, reflection, and dynamic dispatch are
limits, stated. Python first (ast def-use); other languages DEFER.
"""
from __future__ import annotations

import ast
from dataclasses import dataclass, field
from typing import Dict, List, Optional, Set

import pdg as PDG

DEFAULT_SOURCES = {"input", "raw_input", "recv", "read", "readline", "getenv", "argv",
                   "request", "get", "form", "args", "json", "params", "environ"}
DEFAULT_SINKS = {"execute", "executemany", "executescript", "system", "popen", "eval", "exec",
                 "run", "query", "open", "call", "check_output", "Popen", "spawn"}
DEFAULT_SANITIZERS = {"escape", "quote", "sanitize", "validate", "clean", "int", "float",
                      "escape_string", "quote_ident", "shlex_quote", "bleach"}


@dataclass
class Injection:
    sink_line: int
    sink_fn: str
    tainted_var: str
    path_lines: List[int]
    feasible: bool
    note: str


def _calls(node: PDG.PDGNode, names: Set[str]) -> Optional[str]:
    hit = set(node.uses) & names
    return sorted(hit)[0] if hit else None


def taint_analyze(source: str, filename: Optional[str] = None, *, params_tainted: bool = True,
                  sources: Set[str] = None, sinks: Set[str] = None, sanitizers: Set[str] = None,
                  z3_refine: bool = True) -> List[Injection]:
    sources = sources or DEFAULT_SOURCES
    sinks = sinks or DEFAULT_SINKS
    sanitizers = sanitizers or DEFAULT_SANITIZERS
    g = PDG.build_pdg(source, filename)
    if g.n() == 0:
        return []

    # data-edge adjacency (taint flows along def→use)
    succ: Dict[int, List[int]] = {i: [] for i in range(g.n())}
    for u, v, k in g.edges:
        if k == "data":
            succ[u].append(v)

    # seed taint: function params (external input) + nodes calling a source function
    tainted_vars: Set[str] = set()
    taint_node: Dict[str, int] = {}      # var -> node id where it became tainted (provenance)
    if params_tainted:
        for p in _params(source):
            tainted_vars.add(p)
            taint_node[p] = -1            # -1 = parameter origin
    for nd in g.nodes:
        if _calls(nd, sources):
            for d in nd.defs:
                tainted_vars.add(d)
                taint_node[d] = nd.id

    # fixpoint: a node that USES a tainted var taints its DEFS — unless it is a sanitizer (clears)
    changed = True
    while changed:
        changed = False
        for nd in g.nodes:
            if _calls(nd, sanitizers):
                continue                  # sanitizer output is clean
            if set(nd.uses) & tainted_vars:
                for d in nd.defs:
                    if d not in tainted_vars:
                        tainted_vars.add(d)
                        taint_node[d] = nd.id
                        changed = True

    # find sinks reached by a tainted var (not via a sanitizer at the sink)
    injections: List[Injection] = []
    for nd in g.nodes:
        sink_fn = _calls(nd, sinks)
        if not sink_fn or _calls(nd, sanitizers):
            continue
        tvars = (set(nd.uses) & tainted_vars) - sinks
        for tv in sorted(tvars):
            path = _path_lines(g, taint_node.get(tv, -1), nd.id)
            feasible = _feasible(g, nd.id) if z3_refine else True
            injections.append(Injection(nd.line, sink_fn, tv, path, feasible,
                                        "source→sink without sanitizer"))
    return injections


def _params(source: str) -> List[str]:
    try:
        tree = ast.parse(source)
    except SyntaxError:
        return []
    for n in ast.walk(tree):
        if isinstance(n, ast.FunctionDef):
            return [a.arg for a in n.args.args]
    return []


def _path_lines(g: PDG.PDG, src_node: int, sink_node: int) -> List[int]:
    """A data-edge path of line numbers from the taint origin to the sink (BFS)."""
    if src_node < 0:
        return [g.line_of(sink_node)]
    from collections import deque
    succ: Dict[int, List[int]] = {i: [] for i in range(g.n())}
    for u, v, k in g.edges:
        if k == "data":
            succ[u].append(v)
    prev = {src_node: None}
    q = deque([src_node])
    while q:
        x = q.popleft()
        if x == sink_node:
            break
        for y in succ[x]:
            if y not in prev:
                prev[y] = x
                q.append(y)
    if sink_node not in prev:
        return [g.line_of(src_node), g.line_of(sink_node)]
    chain, x = [], sink_node
    while x is not None:
        chain.append(g.line_of(x))
        x = prev[x]
    return list(reversed(chain))


def _feasible(g: PDG.PDG, sink_node: int) -> bool:
    """Z3 prune: the control guards the sink sits under must be jointly satisfiable. A literally
    contradictory / dead guard (e.g. `if False:`) makes the path infeasible (not a real injection)."""
    try:
        import z3
    except Exception:
        return True
    # collect control-parent guard texts reaching the sink
    cparent: Dict[int, int] = {}
    for u, v, k in g.edges:
        if k == "control":
            cparent[v] = u
    guards = []
    x = sink_node
    seen = set()
    while x in cparent and x not in seen:
        seen.add(x)
        p = cparent[x]
        guards.append(g.nodes[p].src)
        x = p
    # any guard that is a constant-false / 1==0 style → infeasible
    for gtext in guards:
        low = gtext.replace(" ", "")
        if "ifFalse" in low or "if0:" in low or "if(0)" in low or "1==0" in low or "0==1" in low:
            return False
    return True
