"""
HARAN v18 Part G · STAGE G1 — program dependency graph (PDG) from HIR.
=====================================================================
The graph bug-suspicion will diffuse over: nodes = statements (by line), edges = data dependencies
(def → use) and control dependencies (a compound header → its body statements). Python uses `ast`
def-use; C uses pycparser where feasible; other languages are DEFER (precise PDGs need alias/pointer
analysis we do not claim).

Honest scope: intra-procedural, name-level def-use. Subscript stores are treated as a read-modify of the
base name (approximate). Aliasing / pointers / inter-procedural flow are DEFER.
"""
from __future__ import annotations

import ast
from dataclasses import dataclass, field
from typing import Dict, List, Optional, Tuple

import hir


@dataclass
class PDGNode:
    id: int
    line: int
    kind: str
    defs: List[str]
    uses: List[str]
    src: str


@dataclass
class PDG:
    nodes: List[PDGNode]
    edges: List[Tuple[int, int, str]]      # (from_id, to_id, "data"|"control")
    lang: str

    def n(self):
        return len(self.nodes)

    def line_of(self, node_id: int) -> int:
        return self.nodes[node_id].line

    def node_at_line(self, line: int) -> Optional[int]:
        return next((nd.id for nd in self.nodes if nd.line == line), None)

    def adjacency(self):
        import numpy as np
        m = self.n()
        A = np.zeros((m, m))
        for u, v, _ in self.edges:
            A[u, v] = 1.0
            A[v, u] = 1.0          # suspicion flows both ways along a dependency
        return A


# ----------------------------------------------------------------- Python def/use
def _names(node, ctx) -> List[str]:
    out = []
    for n in ast.walk(node):
        if isinstance(n, ast.Name) and isinstance(n.ctx, ctx):
            out.append(n.id)
    return out


def _stmt_def_use(st: ast.stmt) -> Tuple[List[str], List[str]]:
    """Defs and uses of a statement's OWN expressions (not nested compound bodies)."""
    defs, uses = [], []
    if isinstance(st, ast.Assign):
        uses += _names(st.value, ast.Load)
        for t in st.targets:
            defs += _names(t, ast.Store)
            uses += [n.value.id for n in ast.walk(t)
                     if isinstance(n, ast.Subscript) and isinstance(n.value, ast.Name)]  # a[i]=.. mutates a
            defs += [n.value.id for n in ast.walk(t)
                     if isinstance(n, ast.Subscript) and isinstance(n.value, ast.Name)]
    elif isinstance(st, ast.AugAssign):
        defs += _names(st.target, ast.Store)
        uses += _names(st.target, ast.Store) + _names(st.value, ast.Load)
    elif isinstance(st, ast.For):
        defs += _names(st.target, ast.Store)
        uses += _names(st.iter, ast.Load)
    elif isinstance(st, (ast.While, ast.If)):
        uses += _names(st.test, ast.Load)
    elif isinstance(st, ast.Return) and st.value is not None:
        uses += _names(st.value, ast.Load)
    elif isinstance(st, ast.Expr):
        uses += _names(st.value, ast.Load)
    return list(dict.fromkeys(defs)), list(dict.fromkeys(uses))


_KIND = {ast.Assign: "assign", ast.AugAssign: "augassign", ast.For: "for", ast.While: "while",
         ast.If: "if", ast.Return: "return", ast.Expr: "expr"}


def build_pdg_python(source: str) -> PDG:
    tree = ast.parse(source)
    fn = next((n for n in ast.walk(tree) if isinstance(n, ast.FunctionDef)), None)
    if fn is None:
        return PDG([], [], "python")
    src_lines = source.splitlines()
    nodes: List[PDGNode] = []
    edges: List[Tuple[int, int, str]] = []
    last_def: Dict[str, int] = {}

    def visit(stmts: List[ast.stmt], control_parent: Optional[int]):
        for st in stmts:
            defs, uses = _stmt_def_use(st)
            nid = len(nodes)
            line = getattr(st, "lineno", 0)
            text = src_lines[line - 1].strip() if 0 < line <= len(src_lines) else ""
            nodes.append(PDGNode(nid, line, _KIND.get(type(st), "stmt"), defs, uses, text))
            for v in uses:                              # data dependency
                if v in last_def and last_def[v] != nid:
                    edges.append((last_def[v], nid, "data"))
            if control_parent is not None:              # control dependency
                edges.append((control_parent, nid, "control"))
            for v in defs:
                last_def[v] = nid
            # recurse into bodies with this node as the control parent
            for body_attr in ("body", "orelse"):
                body = getattr(st, body_attr, None)
                if body:
                    visit(body, nid)

    visit(fn.body, None)
    return PDG(nodes, edges, "python")


# ----------------------------------------------------------------- dispatch
def build_pdg(source: str, filename: Optional[str] = None) -> PDG:
    lang = hir.detect_language(filename, source)
    if lang == "python":
        return build_pdg_python(source)
    if lang == "c":
        try:
            return build_pdg_c(source)
        except Exception:
            return PDG([], [], "c")
    return PDG([], [], lang)        # DEFER: other languages need their own def-use extractor


def build_pdg_c(source: str) -> PDG:
    """C PDG via pycparser (best-effort): statements → nodes, BinaryOp/assignment def-use. DEFER on
    pointer aliasing."""
    from pycparser import c_parser, c_ast
    ast_c = c_parser.CParser().parse(source)
    nodes: List[PDGNode] = []
    edges: List[Tuple[int, int, str]] = []
    last_def: Dict[str, int] = {}

    class V(c_ast.NodeVisitor):
        def _names(self, node):
            return [n.name for n in self._iter(node) if isinstance(n, c_ast.ID)]

        def _iter(self, node):
            stack = [node]
            while stack:
                x = stack.pop(); yield x
                stack.extend(c for _, c in x.children())

        def _add(self, line, kind, defs, uses, src=""):
            nid = len(nodes)
            nodes.append(PDGNode(nid, line, kind, defs, uses, src))
            for v in uses:
                if v in last_def and last_def[v] != nid:
                    edges.append((last_def[v], nid, "data"))
            for v in defs:
                last_def[v] = nid
            return nid

        def visit_Assignment(self, node):
            line = node.coord.line if node.coord else 0
            defs = self._names(node.lvalue)[:1]
            uses = self._names(node.rvalue) + (self._names(node.lvalue) if "[" in str(node.op) else [])
            self._add(line, "assign", defs, uses)
            self.generic_visit(node)

        def visit_Return(self, node):
            line = node.coord.line if node.coord else 0
            self._add(line, "return", [], self._names(node) if node.expr else [])

    V().visit(ast_c)
    return PDG(nodes, edges, "c")
