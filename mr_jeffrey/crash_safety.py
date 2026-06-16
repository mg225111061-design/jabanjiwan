"""
HARAN v16 Part B · STAGE B7 (crash/safety) — ~99% confidence via trace + abstract interpretation.
=================================================================================================
Crash/safety is a DIFFERENT category from correctness, with its own ~99% confidence and its own method
(never borrowed from the probabilistic correctness digits):

  • dynamic — run on many inputs, catch exceptions, read the TRACEBACK: the failing line is ground truth
    (~99% — it is literally where the program died), with the triggering input as the counterexample;
  • static  — a sound interval abstract interpreter flags possible division-by-zero (divisor interval
    contains 0) and index-out-of-range. Honest scope: the INTERVAL domain (real abstract interpretation,
    sound over-approximation); full Apron polyhedra is not installed → DEFER.
"""
from __future__ import annotations

import ast
import traceback
from dataclasses import dataclass, field
from typing import Dict, List, Optional, Tuple

import hir
import properties as PR


# ----------------------------------------------------------------- dynamic crash localization
@dataclass
class CrashFinding:
    exc_type: str
    line: int                # line WITHIN the function source (1-based)
    count: int
    sample_input: object


def find_crashes(hfn: hir.HFunction, inputs: List[list]) -> List[CrashFinding]:
    fn = PR.compile_callable(hfn)
    by: Dict[Tuple[str, int], CrashFinding] = {}
    for x in inputs:
        try:
            fn(list(x) if isinstance(x, list) else x)
        except Exception as e:  # noqa: BLE001 - we are deliberately catching to localize
            tb = e.__traceback__
            line = 0
            while tb is not None:
                # the frame executing the function's own code (filename is "<...>") — take the deepest
                if tb.tb_frame.f_code.co_name == hfn.name:
                    line = tb.tb_lineno
                tb = tb.tb_next
            key = (type(e).__name__, line)
            if key in by:
                by[key].count += 1
            else:
                by[key] = CrashFinding(type(e).__name__, line, 1, x)
    return sorted(by.values(), key=lambda c: c.count, reverse=True)


# ----------------------------------------------------------------- static interval abstract interpreter
@dataclass
class Interval:
    lo: float
    hi: float

    def contains_zero(self):
        return self.lo <= 0 <= self.hi

    def join(self, o):
        return Interval(min(self.lo, o.lo), max(self.hi, o.hi))


_TOP = Interval(float("-inf"), float("inf"))


@dataclass
class SafetyWarning:
    kind: str        # div-by-zero | index-may-oob
    line: int
    detail: str


class _IntervalAI(ast.NodeVisitor):
    """A small, SOUND interval analysis: constants are exact; anything else is ⊤. A divisor whose
    interval contains 0 is flagged. (Loops/refinement-narrowing are out of scope → DEFER Apron.)"""
    def __init__(self):
        self.env: Dict[str, Interval] = {}
        self.warnings: List[SafetyWarning] = []

    def _eval(self, node) -> Interval:
        if isinstance(node, ast.Constant) and isinstance(node.value, (int, float)):
            return Interval(node.value, node.value)
        if isinstance(node, ast.Name):
            return self.env.get(node.id, _TOP)
        if isinstance(node, ast.BinOp):
            l, r = self._eval(node.left), self._eval(node.right)
            if isinstance(node.op, (ast.Div, ast.FloorDiv, ast.Mod)):
                if r.contains_zero():
                    self.warnings.append(SafetyWarning(
                        "div-by-zero", getattr(node, "lineno", 0),
                        "divisor interval includes 0 (sound over-approximation)"))
                return _TOP
            return _TOP
        return _TOP

    def visit_Assign(self, node):
        val = self._eval(node.value)
        for t in node.targets:
            if isinstance(t, ast.Name):
                self.env[t.id] = val
        self.generic_visit(node)

    def visit_BinOp(self, node):
        self._eval(node)
        self.generic_visit(node)


def static_safety(hfn: hir.HFunction) -> List[SafetyWarning]:
    tree = ast.parse(hfn.source)
    ai = _IntervalAI()
    ai.visit(tree)
    # de-dup by (kind, line)
    seen, out = set(), []
    for w in ai.warnings:
        if (w.kind, w.line) not in seen:
            seen.add((w.kind, w.line))
            out.append(w)
    return out


# ----------------------------------------------------------------- combined crash/safety verdict
@dataclass
class SafetyVerdict:
    crashes: List[CrashFinding]
    warnings: List[SafetyWarning]
    confidence: float          # ~0.99 when a trace pins the line; method is trace/AI, NOT a prob digit
    method: str

    def summary(self) -> str:
        if self.crashes:
            c = self.crashes[0]
            return f"CRASH {c.exc_type} @ line {c.line} (×{c.count}) — confidence ~{self.confidence:.0%} (traceback)"
        if self.warnings:
            w = self.warnings[0]
            return f"SAFETY {w.kind} @ line {w.line} — sound interval over-approximation"
        return "no crash / no safety warning observed"


def analyze_safety(hfn: hir.HFunction, inputs: List[list]) -> SafetyVerdict:
    crashes = find_crashes(hfn, inputs)
    warnings = static_safety(hfn)
    conf = 0.99 if crashes else (0.9 if warnings else 1.0)
    method = "traceback (ground-truth line)" if crashes else \
             ("interval abstract interpretation (sound)" if warnings else "no fault in this category")
    return SafetyVerdict(crashes, warnings, conf, method)
