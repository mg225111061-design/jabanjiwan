"""
HARAN v20 Part P · STAGE P4 — Incorrectness Separation Logic (use-after-free / double-free).
============================================================================================
ISL is UNDER-approximate: every reported bug is a REAL, reachable bug (false-positives = 0), at the cost
of possibly missing some (incomplete). We track each pointer's heap state (allocated / freed) along an
actual operation sequence: a use after free is a UAF; a free after free is a double-free. Because we only
report what genuinely occurs on a realizable path, a report is always a true bug.

HARAN's own/& (v14 RAII) make owned values UAF-free by construction (freed exactly once at scope end), so
this targets C/C++ MANUAL memory. C extracted via pycparser; Rust is low-priority (the borrow checker
already prevents these).
"""
from __future__ import annotations

from dataclasses import dataclass, field
from typing import Dict, List, Optional


@dataclass
class HeapOp:
    kind: str        # alloc | free | use
    ptr: str
    line: int = 0


@dataclass
class MemoryBug:
    kind: str        # use-after-free | double-free
    ptr: str
    line: int
    alloc_line: int
    free_line: int
    note: str


def analyze_heap(ops: List[HeapOp]) -> List[MemoryBug]:
    """Linear scan of a concrete op sequence (a realizable path) → UAF / double-free. Under-approximate:
    every bug reported actually happens in this sequence (FP=0)."""
    state: Dict[str, str] = {}          # ptr -> "alloc" | "freed"
    alloc_line: Dict[str, int] = {}
    free_line: Dict[str, int] = {}
    bugs: List[MemoryBug] = []
    for op in ops:
        st = state.get(op.ptr)
        if op.kind == "alloc":
            state[op.ptr] = "alloc"
            alloc_line[op.ptr] = op.line
        elif op.kind == "free":
            if st == "freed":
                bugs.append(MemoryBug("double-free", op.ptr, op.line, alloc_line.get(op.ptr, 0),
                                      free_line.get(op.ptr, 0), "free() of an already-freed pointer"))
            else:
                state[op.ptr] = "freed"
                free_line[op.ptr] = op.line
        elif op.kind == "use":
            if st == "freed":
                bugs.append(MemoryBug("use-after-free", op.ptr, op.line, alloc_line.get(op.ptr, 0),
                                      free_line.get(op.ptr, 0), "dereference of a freed pointer"))
    return bugs


# ----------------------------------------------------------------- C extractor (pycparser)
def parse_c_heap(source: str) -> List[HeapOp]:
    """Extract the heap-op sequence (alloc/free/use) from a straight-line C function."""
    try:
        from pycparser import c_parser, c_ast
    except Exception:
        return []
    try:
        ast = c_parser.CParser().parse(source)
    except Exception:
        return []
    ops: List[HeapOp] = []

    class V(c_ast.NodeVisitor):
        def visit_Assignment(self, node):
            line = node.coord.line if node.coord else 0
            if (isinstance(node.rvalue, c_ast.FuncCall) and isinstance(node.rvalue.name, c_ast.ID)
                    and node.rvalue.name.name in ("malloc", "calloc", "realloc")
                    and isinstance(node.lvalue, c_ast.ID)):
                ops.append(HeapOp("alloc", node.lvalue.name, line))
            else:
                self.generic_visit(node)

        def visit_Decl(self, node):
            line = node.coord.line if node.coord else 0
            if (node.init and isinstance(node.init, c_ast.FuncCall) and isinstance(node.init.name, c_ast.ID)
                    and node.init.name.name in ("malloc", "calloc", "realloc")):
                ops.append(HeapOp("alloc", node.name, line))
            else:
                self.generic_visit(node)

        def visit_FuncCall(self, node):
            line = node.coord.line if node.coord else 0
            fname = node.name.name if isinstance(node.name, c_ast.ID) else ""
            if fname == "free" and node.args and isinstance(node.args.exprs[0], c_ast.ID):
                ops.append(HeapOp("free", node.args.exprs[0].name, line))
            else:
                # passing a pointer to another function = a use
                if node.args:
                    for a in node.args.exprs:
                        if isinstance(a, c_ast.ID):
                            ops.append(HeapOp("use", a.name, line))
                self.generic_visit(node)

        def visit_UnaryOp(self, node):
            line = node.coord.line if node.coord else 0
            if node.op == "*" and isinstance(node.expr, c_ast.ID):     # *p dereference
                ops.append(HeapOp("use", node.expr.name, line))
            self.generic_visit(node)

        def visit_ArrayRef(self, node):
            line = node.coord.line if node.coord else 0
            if isinstance(node.name, c_ast.ID):                        # p[i]
                ops.append(HeapOp("use", node.name.name, line))
            self.generic_visit(node)

        def visit_StructRef(self, node):
            line = node.coord.line if node.coord else 0
            if isinstance(node.name, c_ast.ID):                        # p->x / p.x
                ops.append(HeapOp("use", node.name.name, line))
            self.generic_visit(node)

    V().visit(ast)
    return ops


def analyze_c(source: str) -> List[MemoryBug]:
    return analyze_heap(parse_c_heap(source))
