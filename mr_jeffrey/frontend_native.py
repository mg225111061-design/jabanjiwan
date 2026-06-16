"""
HARAN v17 Part C · STAGE C2/C3 — shared language-aware token scanner → HIR operations.
=====================================================================================
Full AST parsers (go/parser, syn, tree-sitter) are a refinement; here we extract the HIR operation
list (compare/arith/index_load/index_store/call/return + line numbers) with a language-aware TOKEN
scanner, which is enough for op-kind localization. Execution (the part that actually finds bugs) uses
the REAL native compiler per language (frontend_go/rust/js). Honest: the scanner is heuristic — it may
miss/merge operations on pathological lines; a true AST frontend is DEFER (syn/tree-sitter).
"""
from __future__ import annotations

import re
from typing import List, Optional, Tuple

import hir

_HEADER = {
    "go": re.compile(r"func\s+(\w+)\s*\(([^)]*)\)\s*([^\{]*)\{"),
    "rust": re.compile(r"fn\s+(\w+)\s*\(([^)]*)\)\s*(->\s*[^\{]*)?\{"),
    "javascript": re.compile(r"function\s+(\w+)\s*\(([^)]*)\)\s*\{"),
    "typescript": re.compile(r"function\s+(\w+)\s*\(([^)]*)\)\s*(:[^\{]*)?\{"),
}
_ARR = re.compile(r"\[\s*\]|Vec\s*<|\[\s*i\d+|\&\s*\[|Array|number\[\]|\:\s*\w+\[\]|list")


def _params(ptext: str) -> List[str]:
    out = []
    for part in ptext.split(","):
        part = part.strip()
        if not part:
            continue
        m = re.match(r"(?:mut\s+)?(\w+)", part)
        if m:
            out.append(m.group(1))
    return out


def _brace_body(source: str, open_idx: int) -> Tuple[str, int]:
    depth, i = 0, open_idx
    for i in range(open_idx, len(source)):
        if source[i] == "{":
            depth += 1
        elif source[i] == "}":
            depth -= 1
            if depth == 0:
                return source[open_idx:i + 1], i
    return source[open_idx:], len(source)


_CMP = re.compile(r"(?<![-=<>!])(<=|>=|==|!=|<|>)(?![=>])")
_AUG = re.compile(r"[-+*/%]=")
_ARITH = re.compile(r"(?<![-+*/%])[-+*/%](?![-+*/%=])")
_CALL = re.compile(r"\b(\w+)\s*\(")
_IDX = re.compile(r"\w+\s*\[")
_KW = {"if", "for", "while", "func", "fn", "function", "return", "let", "match", "switch", "range"}


def _scan_ops(body: str, base_line: int) -> List[hir.HOp]:
    ops: List[hir.HOp] = []
    for k, line in enumerate(body.splitlines()):
        ln = base_line + k
        if re.search(r"\.swap\s*\(", line):
            ops.append(hir.HOp("index_store", ln, "swap"))
        if re.search(r"\.sort\b", line) or re.search(r"sorted\s*\(", line):
            ops.append(hir.HOp("sort", ln))
        for _ in _CMP.findall(line):
            ops.append(hir.HOp("compare", ln))
        # index store vs load: an indexed target left of a single '=' is a store
        if _IDX.search(line) and re.search(r"\w+\s*\[[^\]]*\]\s*=(?!=)", line):
            ops.append(hir.HOp("index_store", ln))
        for _ in _IDX.findall(line):
            ops.append(hir.HOp("index_load", ln))
        for _ in _AUG.findall(line):
            ops.append(hir.HOp("aug", ln))
        for _ in _ARITH.findall(line):
            ops.append(hir.HOp("arith", ln))
        for name in _CALL.findall(line):
            if name not in _KW:
                ops.append(hir.HOp("call", ln, name))
        if re.search(r"\breturn\b", line):
            ops.append(hir.HOp("return", ln))
    return ops


def scan_function(source: str, lang: str) -> Optional[hir.HFunction]:
    hdr = _HEADER.get(lang)
    if not hdr:
        return None
    m = hdr.search(source)
    if not m:
        return None
    name, ptext = m.group(1), m.group(2)
    ret = (m.group(3) or "") if m.lastindex and m.lastindex >= 3 else ""
    open_idx = source.index("{", m.end() - 1)
    body, _ = _brace_body(source, open_idx)
    base_line = source[:open_idx].count("\n") + 1
    ops = _scan_ops(body, base_line)
    params = _params(ptext)
    is_arr = bool(_ARR.search(ptext))
    ret_arr = bool(_ARR.search(ret))
    kind = ("array_return" if (is_arr and (ret_arr or lang in ("go", "rust")))
            else ("array_inplace" if is_arr else "scalar"))
    sig = {"kind": kind, "arr": params[0] if params else None,
           "n": params[1] if len(params) > 1 else None, "ret_arr": ret_arr}
    start = source[:m.start()].count("\n") + 1
    end = source[:open_idx + len(body)].count("\n") + 1
    return hir.HFunction(name, params, ops, source, start, end, lang=lang, signature=sig)
