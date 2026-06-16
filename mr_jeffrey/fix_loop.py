"""
HARAN v16 Part B · STAGE B8 (fix loop) — AI write→verify→fix with context separation.
=====================================================================================
The fixer proposes a repair; an INDEPENDENT verifier (the property tests — the Mr side) re-checks. The
two are different modules, so the fixer cannot make the verifier pass by fiat — the "수학적 분리" that
prevents self-deception (B8.4). The deterministic fixer is mutation-based program repair guided by the
B8 report's suspect line; if an ANTHROPIC_API_KEY is present, Claude can be swapped in as the writer
(the loop and re-verification are what matter — the model is swappable, per v7).

Honest: mutation-repair fixes operator bugs (a wrong comparison/operator); structural bugs (a dropped
element, a missing branch) are NOT repaired by operator mutation → the loop honestly escalates after the
round limit instead of pretending success.
"""
from __future__ import annotations

import ast
import os
from dataclasses import dataclass, field
from typing import List, Optional, Tuple

import hir
import properties as PR
import property_test as PT
import narrow as NA
import report as RPT


# ----------------------------------------------------------------- INDEPENDENT verifier (Mr side)
def reverify(source: str, name: str, n: int = 400) -> Tuple[bool, List[str]]:
    """Independent property re-test: compile the candidate, extract properties, run them. Returns
    (all_hold, violated). This is a SEPARATE module from the fixer — no shared state to fake."""
    h = hir.python_to_hir(source).fn(name)
    fn = PR.compile_callable(h)
    rep = PT.test_properties(fn, PR.extract_properties(h), PT.gen_int_lists(n))
    violated = rep.violated_properties()
    return (not violated), violated


# ----------------------------------------------------------------- mutation-repair candidate generator
def _candidate_repairs(source: str, focus_lines: List[int]) -> List[Tuple[str, str]]:
    """Single-operator mutants (comparisons + binops), focus lines first. (desc, mutated_source)."""
    out: List[Tuple[str, str]] = []

    def mutate(swap_kind, only_line):
        tree = ast.parse(source)

        class M(ast.NodeTransformer):
            def __init__(self):
                self.done = None

            def visit_Compare(self, node):
                self.generic_visit(node)
                if swap_kind == "cmp" and len(node.ops) == 1 and (only_line is None or node.lineno == only_line):
                    sw = {ast.Lt: ast.Gt, ast.Gt: ast.Lt, ast.LtE: ast.GtE, ast.GtE: ast.LtE,
                          ast.Eq: ast.NotEq, ast.NotEq: ast.Eq}
                    t = type(node.ops[0])
                    if t in sw and self.done is None:
                        node.ops[0] = sw[t]()
                        self.done = f"swap comparison @ line {node.lineno}"
                return node

            def visit_BinOp(self, node):
                self.generic_visit(node)
                if swap_kind == "bin" and (only_line is None or node.lineno == only_line):
                    sw = {ast.Add: ast.Sub, ast.Sub: ast.Add, ast.Mult: ast.FloorDiv, ast.FloorDiv: ast.Mult}
                    t = type(node.op)
                    if t in sw and self.done is None:
                        node.op = sw[t]()
                        self.done = f"swap operator @ line {node.lineno}"
                return node

        m = M()
        tree = m.visit(tree)
        if m.done:
            ast.fix_missing_locations(tree)
            return m.done, ast.unparse(tree)
        return None

    # focus lines first (from the report), then global
    for line in list(focus_lines) + [None]:
        for kind in ("cmp", "bin"):
            r = mutate(kind, line)
            if r and r not in out:
                out.append(r)
    return out


# ----------------------------------------------------------------- Claude writer (optional, swappable)
def _claude_available() -> bool:
    return bool(os.environ.get("ANTHROPIC_API_KEY"))


def _claude_fix(source: str, report_text: str) -> Optional[str]:  # pragma: no cover - network/dev only
    if not _claude_available():
        return None
    try:
        import ai_loop
        return ai_loop.propose_fix(source, report_text)
    except Exception:
        return None


# ----------------------------------------------------------------- the loop
@dataclass
class FixStep:
    round: int
    candidate_desc: str
    accepted: bool
    remaining: List[str]


@dataclass
class FixResult:
    fixed: bool
    rounds: int
    steps: List[FixStep]
    fixed_source: Optional[str]
    fixer: str               # "mutation-repair" | "claude"
    detail: str


def ai_fix_loop(hfn: hir.HFunction, max_rounds: int = 3) -> FixResult:
    rep = RPT.build_report(hfn)
    if rep is None:
        return FixResult(True, 0, [], hfn.source, "none", "no violation — nothing to fix")
    focus = list(rep.where_lines)
    steps: List[FixStep] = []
    fixer = "claude" if _claude_available() else "mutation-repair"
    source = hfn.source
    for rnd in range(1, max_rounds + 1):
        candidate = None
        desc = ""
        if fixer == "claude":
            c = _claude_fix(source, rep.render())
            if c:
                candidate, desc = c, "claude proposed fix"
        if candidate is None:           # deterministic mutation-repair (also the no-key fallback)
            cands = _candidate_repairs(source, focus)
            if rnd - 1 < len(cands):
                desc, candidate = cands[rnd - 1]
                fixer = "mutation-repair"
        if candidate is None:
            steps.append(FixStep(rnd, "no further candidate", False, []))
            break
        all_hold, remaining = reverify(candidate, hfn.name)   # INDEPENDENT verification
        steps.append(FixStep(rnd, desc, all_hold, remaining))
        if all_hold:
            return FixResult(True, rnd, steps, candidate, fixer,
                             f"repaired in round {rnd}: {desc} (independently re-verified — all properties hold)")
        focus = []   # broaden the search after a failed focused attempt
    return FixResult(False, len(steps), steps, None, fixer,
                     "no operator-mutation repair recovered all properties → escalate (structural bug / needs Claude)")
