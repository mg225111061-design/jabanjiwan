"""
HARAN v16 Part B · STAGE B8 (report) — structured minimal report for the fixer.
==============================================================================
Research: a MINIMAL counterexample raises fix accuracy markedly. We hand the fixer the smallest possible
report: where (suspect line + minimal slice), why (violated property + posterior), the minimal failing
input (delta-debugged), the limit (B6 proven digit), a grade, the safe zone (operations that held every
property — "don't touch"), and a fix direction. Minimal + precise ⇒ the fixer is fast and accurate;
inflating the grade (reporting a C as an A) causes wasted fixes, so grades are conservative.
"""
from __future__ import annotations

from dataclasses import dataclass, field
from typing import List, Optional

import hir
import properties as PR
import property_test as PT
import narrow as NA
import fault_map as FM


# ----------------------------------------------------------------- minimal counterexample (ddmin-ish)
def minimal_counterexample(fn, prop: PR.Property, failing: list) -> list:
    """Shrink a failing input to a locally-minimal one that still violates the property."""
    cur = list(failing)

    def fails(x):
        try:
            return not prop.check(fn, x)
        except Exception:
            return True

    changed = True
    while changed and len(cur) > 0:
        changed = False
        for i in range(len(cur)):
            cand = cur[:i] + cur[i + 1:]
            if fails(cand):
                cur = cand
                changed = True
                break
    return cur


# ----------------------------------------------------------------- minimal slice
def minimal_slice(hfn: hir.HFunction, lines: List[int]) -> str:
    src_lines = hfn.source.splitlines()
    base = hfn.start_line
    want = set(lines)
    out = []
    for i, ln in enumerate(src_lines):
        absln = base + i
        rel = i + 1
        if rel in want or absln in want or i == 0:   # keep the def header + suspect lines
            out.append(f"{rel:>3}: {ln}")
    return "\n".join(out)


# ----------------------------------------------------------------- structured report
@dataclass
class BugReport:
    function: str
    where_lines: List[int]
    slice_text: str
    why: str                      # violated property + posterior
    counterexample: list
    limit: str                    # B6 proven digit (top-k ε)
    grade: str                    # A | B | C | D
    safe_zone: List[str]          # op kinds that held every property (ABSENCE — don't touch)
    fix_direction: str

    def render(self) -> str:
        return (f"BUG REPORT — {self.function}  [grade {self.grade}]\n"
                f"  where : lines {self.where_lines}\n{self.slice_text}\n"
                f"  why   : {self.why}\n"
                f"  c-ex  : {self.counterexample}  (minimal)\n"
                f"  limit : {self.limit}\n"
                f"  safe  : {self.safe_zone}  (held every property — ABSENCE, do not edit)\n"
                f"  fix   : {self.fix_direction}")


def _grade(causal_recovered: bool, top_posterior: float, n_props: int) -> str:
    if causal_recovered and top_posterior >= 0.3:
        return "A"          # causally confirmed + concentrated posterior
    if top_posterior >= 0.4:
        return "B"          # strong top-1, no causal confirmation
    if n_props >= 2:
        return "C"          # top-k only
    return "D"              # property-poor


def build_report(hfn: hir.HFunction, n_random: int = 400, proven_digit: str = "see B6") -> Optional[BugReport]:
    fn = PR.compile_callable(hfn)
    props = PR.extract_properties(hfn)
    inputs = PT.gen_inputs(hfn, n_random)
    rep = PT.test_properties(fn, props, inputs)
    violated = [p for p in props if p.name in rep.violated_properties()]
    if not violated:
        return None
    res = NA.bayesian_narrow(hfn, violated)
    top = res.ranked[0]
    # causal experiment for grade + fix direction
    vp = next((p for p in violated if top.op_kind in p.operations), violated[0])
    cr = NA.causal_experiment(hfn, top, vp, inputs[:60])
    # minimal counterexample on the most-violated property
    worst = max(violated, key=lambda p: len(rep.violations[p.name]))
    cx = minimal_counterexample(fn, worst, rep.violations[worst.name][0])
    safe = [s.op_kind for s in res.ranked if not s.implicated_by]
    grade = _grade(cr.recovered, top.posterior, len(violated))
    fixdir = (f"the operator at line {cr.line} is causally responsible — {cr.detail}"
              if cr.recovered else
              f"focus on the '{top.op_kind}' operation at lines {top.lines}; it is the top suspect "
              f"for the violated {[p.name for p in violated]}")
    return BugReport(
        function=hfn.name, where_lines=top.lines, slice_text=minimal_slice(hfn, top.lines),
        why=f"violates {[p.name for p in violated]} (posterior P(bug at {top.op_kind})={top.posterior:.2f})",
        counterexample=cx, limit=f"top-k suspects; innocent digit {proven_digit}", grade=grade,
        safe_zone=safe, fix_direction=fixdir)
