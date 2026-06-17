"""
HARAN v20 Part P · STAGE P5a — MaxSAT fault localization (multiple bugs, minimal set).
======================================================================================
A failing execution becomes a formula: each statement `v == rhs` is a SOFT constraint; the inputs and the
correct expected output are HARD constraints. Z3 MaxSAT (Optimize.add_soft) finds the MINIMUM set of
statements to relax so the formula is satisfiable — that minimal set is the bug(s). Unlike SBFL /
diffusion (which assume a single fault), MaxSAT pinpoints SEVERAL bugs at once, minimally.

Reuses HARAN's Z3 directly. Honest: works on straight-line integer computations here; loops/heap need
unrolling/SMT theories (DEFER for the richer cases).
"""
from __future__ import annotations

import ast
from dataclasses import dataclass, field
from typing import Dict, List, Tuple


@dataclass
class Diagnosis:
    min_bugs: int                # MINIMAL number of statements that must be wrong (bug count)
    suspects: List[str]          # statements each of which, relaxed alone, fixes it (real bug is here)
    sample_set: List[str]        # one concrete minimal correction set (size == min_bugs)
    note: str


def _to_z3(expr: str, env: dict):
    import z3
    tree = ast.parse(expr, mode="eval").body

    def go(n):
        if isinstance(n, ast.Name):
            return env.setdefault(n.id, z3.Int(n.id))
        if isinstance(n, ast.Constant):
            return z3.IntVal(int(n.value))
        if isinstance(n, ast.BinOp):
            l, r = go(n.left), go(n.right)
            return {ast.Add: l + r, ast.Sub: l - r, ast.Mult: l * r}[type(n.op)] \
                if not isinstance(n.op, (ast.Div, ast.FloorDiv, ast.Mod)) else l / r
        if isinstance(n, ast.UnaryOp) and isinstance(n.op, ast.USub):
            return -go(n.operand)
        raise ValueError(f"unsupported expr {ast.dump(n)}")
    return go(tree)


def diagnose(statements: List[Tuple[str, str]], inputs: Dict[str, int],
             expected: Dict[str, int]) -> Diagnosis:
    """statements = [(var, rhs_expr)] in order; inputs/expected = {var: value}. Returns the minimal set
    of statements whose relaxation restores consistency (the bug set)."""
    import z3

    def build(env):
        cons = []
        for v, val in inputs.items():
            cons.append(("HARD", env.setdefault(v, z3.Int(v)) == val))
        labels = []
        for i, (v, rhs) in enumerate(statements):
            lhs = env.setdefault(v, z3.Int(v))
            c = (lhs == _to_z3(rhs, env))
            lbl = f"S{i}:{v}={rhs}"
            labels.append((lbl, c))
        out = []
        for v, val in expected.items():
            out.append(env.setdefault(v, z3.Int(v)) == val)
        return cons, labels, out

    # 1. MIN correction size via MaxSAT (Optimize.add_soft)
    env: dict = {}
    hard, labels, outs = build(env)
    opt = z3.Optimize()
    for _, c in hard:
        opt.add(c)
    for o in outs:
        opt.add(o)
    for lbl, c in labels:
        opt.add_soft(c, 1, lbl)
    if opt.check() != z3.sat:
        return Diagnosis(0, [], [], "unsatisfiable even relaxing all — model error")
    m = opt.model()
    relaxed = [lbl for lbl, c in labels if not z3.is_true(m.eval(c, model_completion=True))]
    min_bugs = len(relaxed)

    # 2. suspects = statements whose INDIVIDUAL relaxation restores the spec (size-1 fixes; real bug ∈ here)
    suspects = []
    for skip in range(len(statements)):
        env2: dict = {}
        hard2, labels2, outs2 = build(env2)
        s = z3.Solver()
        for _, c in hard2:
            s.add(c)
        for o in outs2:
            s.add(o)
        for i, (lbl, c) in enumerate(labels2):
            if i != skip:
                s.add(c)
        if s.check() == z3.sat:
            suspects.append(labels[skip][0])
    return Diagnosis(min_bugs, suspects, relaxed,
                     "MaxSAT: minimal #faulty statements + the suspect set (each a single fix; the real "
                     "bug is among them) — finds MULTIPLE bugs minimally, unlike single-fault SBFL")
