"""
HARAN v16 Part B · STAGE B1 — language detection + frontend → HARAN-IR (HIR).
=============================================================================
Type B accepts arbitrary-language code, lowers it to a common IR (HIR), and runs the property-based /
probabilistic engine on the IR (language-agnostic). Python is the first frontend (ast module is exact
and free); C/Rust/Go are registered EXTENSION POINTS only (DEFER — added later via pycparser etc.).

HIR is deliberately small: per function we keep the parameter list, the source text (so property testing
can `exec` the real code — we test behaviour, not a reimplementation), and a flat list of OPERATIONS
with line numbers (kind ∈ append/pop/slice/compare/arith/index_store/call/...). That operation list is
what STAGE B4 maps violated properties onto, and what B8 slices.

Honest scope: detection is extension+syntax heuristics (optional AI confirm only if an API key exists);
only Python lowers to HIR now — every other language is an explicit registered stub, not a fake.
"""
from __future__ import annotations

import ast
from dataclasses import dataclass, field
from typing import Callable, Dict, List, Optional


# ----------------------------------------------------------------- HIR data model
@dataclass
class HOp:
    kind: str        # append|pop|insert|slice|sort|reverse|compare|arith|aug|index_load|index_store|call|return|swap
    line: int
    detail: str = ""


@dataclass
class HFunction:
    name: str
    params: List[str]
    ops: List[HOp]
    source: str          # exact source of this function (for behavioural property testing & slicing)
    start_line: int
    end_line: int
    lang: str = "python"        # language of `source` (drives runtime.make_callable)
    signature: dict = field(default_factory=dict)   # frontend-supplied calling info (param/return shapes)

    def op_kinds(self) -> set:
        return {o.kind for o in self.ops}


@dataclass
class HModule:
    lang: str
    functions: List[HFunction]
    source: str

    def fn(self, name: str) -> Optional[HFunction]:
        return next((f for f in self.functions if f.name == name), None)


# ----------------------------------------------------------------- B1.1 language detection
_EXT = {".py": "python", ".c": "c", ".h": "c", ".rs": "rust", ".go": "go",
        ".js": "javascript", ".ts": "typescript", ".java": "java", ".cpp": "cpp"}


def detect_language(filename: Optional[str] = None, source: str = "") -> str:
    if filename:
        for ext, lang in _EXT.items():
            if filename.endswith(ext):
                return lang
    s = source
    if "#include" in s or ("int main(" in s and "{" in s):
        return "c"
    if "fn " in s and ("let " in s or "->" in s) and "{" in s:
        return "rust"
    if "package " in s and "func " in s:
        return "go"
    if "function " in s or "const " in s and "=>" in s:
        return "javascript"
    if "def " in s or "import " in s or "lambda" in s:
        # confirm it actually parses as Python
        try:
            ast.parse(s)
            return "python"
        except SyntaxError:
            pass
    try:
        ast.parse(s)
        return "python"
    except SyntaxError:
        return "unknown"


def ai_confirm_language(source: str) -> Optional[str]:
    """Optional AI confirmation. DISABLED under the level-1 key policy (no env keys ever) — returns
    None so the keyword heuristic stands. Any AI augmentation would take an explicit per-call key
    (level-1), never an environment variable."""
    return None


# ----------------------------------------------------------------- B1.2 Python → HIR
_CALL_KIND = {"append": "append", "pop": "pop", "insert": "insert", "extend": "extend",
              "remove": "remove", "sort": "sort", "reverse": "reverse", "sorted": "sort",
              "reversed": "reverse", "min": "compare", "max": "compare", "len": "len"}


class _PyOps(ast.NodeVisitor):
    def __init__(self):
        self.ops: List[HOp] = []

    def _add(self, kind, node, detail=""):
        self.ops.append(HOp(kind, getattr(node, "lineno", 0), detail))

    def visit_Call(self, node):
        kind = "call"
        detail = ""
        f = node.func
        if isinstance(f, ast.Attribute):
            detail = f.attr
            kind = _CALL_KIND.get(f.attr, "call")
        elif isinstance(f, ast.Name):
            detail = f.id
            kind = _CALL_KIND.get(f.id, "call")
        self._add(kind, node, detail)
        self.generic_visit(node)

    def visit_Subscript(self, node):
        ctx = "index_store" if isinstance(node.ctx, ast.Store) else "index_load"
        if isinstance(node.slice, ast.Slice):
            self._add("slice", node)
        else:
            self._add(ctx, node)
        self.generic_visit(node)

    def visit_Compare(self, node):
        self._add("compare", node, detail="".join(type(o).__name__ for o in node.ops))
        self.generic_visit(node)

    def visit_BinOp(self, node):
        self._add("arith", node, detail=type(node.op).__name__)
        self.generic_visit(node)

    def visit_AugAssign(self, node):
        self._add("aug", node, detail=type(node.op).__name__)
        self.generic_visit(node)

    def visit_Return(self, node):
        self._add("return", node)
        self.generic_visit(node)


def python_to_hir(source: str) -> HModule:
    tree = ast.parse(source)
    src_lines = source.splitlines()
    fns: List[HFunction] = []
    for node in ast.walk(tree):
        if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)):
            v = _PyOps()
            for stmt in node.body:
                v.visit(stmt)
            params = [a.arg for a in node.args.args]
            start = node.lineno
            end = getattr(node, "end_lineno", start)
            src = "\n".join(src_lines[start - 1:end])
            fns.append(HFunction(node.name, params, v.ops, src, start, end))
    return HModule("python", fns, source)


# ----------------------------------------------------------------- B1.3 extension registry
FRONTENDS: Dict[str, Optional[Callable[[str], HModule]]] = {
    "python": python_to_hir,
    "c": None,           # DEFER — pycparser frontend (extension point exists, engine is shared)
    "rust": None,        # DEFER
    "go": None,          # DEFER
    "javascript": None,  # DEFER
}


@dataclass
class FrontendResult:
    lang: str
    module: Optional[HModule]
    supported: bool
    detail: str


_FRONTEND_MODULES = {"c": "frontend_c", "go": "frontend_go", "rust": "frontend_rust",
                     "javascript": "frontend_js", "typescript": "frontend_js", "java": "frontend_java"}


def to_hir(source: str, filename: Optional[str] = None) -> FrontendResult:
    lang = detect_language(filename, source)
    fe = FRONTENDS.get(lang)
    if fe is None and lang in _FRONTEND_MODULES:        # lazy-load the frontend, which self-registers
        try:
            __import__(_FRONTEND_MODULES[lang])
            fe = FRONTENDS.get(lang)
        except Exception as e:
            return FrontendResult(lang, None, False, f"{lang}: frontend unavailable ({e}) — BLOCKED")
    if fe is None:
        return FrontendResult(lang, None, False,
                              f"{lang}: frontend not implemented (registered extension point; DEFER)")
    try:
        return FrontendResult(lang, fe(source), True, "lowered to HIR")
    except SyntaxError as e:
        return FrontendResult(lang, None, False, f"parse error: {e}")
