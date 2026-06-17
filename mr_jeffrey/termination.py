"""
HARAN v20 Part P · STAGE P5b — termination via ranking-function synthesis.
=========================================================================
A loop terminates if there is a well-founded (discrete-Lyapunov) ranking function r that is bounded below
while the guard holds and strictly decreases each iteration. We synthesize a LINEAR r with Z3:
  ∃ coefficients.  ∀ loop-state.  guard ⟹ (r ≥ 0  ∧  r(next) ≤ r − 1).
If found → the loop TERMINATES (with the witness r). If not → "termination unknown" (honest — could be
non-terminating, or need a non-linear/lexicographic measure, which is DEFER).
"""
from __future__ import annotations

import ast
from dataclasses import dataclass
from typing import Dict, List, Optional


@dataclass
class RankingResult:
    terminates: Optional[bool]    # True = ranking found; None = unknown (no linear ranking)
    ranking: str
    note: str


def _z3expr(expr: str, env: dict):
    import z3
    tree = ast.parse(expr, mode="eval").body

    def go(n):
        if isinstance(n, ast.Name):
            return env[n.id]
        if isinstance(n, ast.Constant):
            return z3.IntVal(int(n.value))
        if isinstance(n, ast.BinOp):
            l, r = go(n.left), go(n.right)
            return {ast.Add: l + r, ast.Sub: l - r, ast.Mult: l * r}[type(n.op)]
        if isinstance(n, ast.UnaryOp) and isinstance(n.op, ast.USub):
            return -go(n.operand)
        if isinstance(n, ast.Compare) and len(n.ops) == 1:
            l, r = go(n.left), go(n.comparators[0])
            return {ast.Lt: l < r, ast.LtE: l <= r, ast.Gt: l > r, ast.GtE: l >= r,
                    ast.Eq: l == r, ast.NotEq: l != r}[type(n.ops[0])]
        raise ValueError(f"unsupported {ast.dump(n)}")
    return go(tree)


def synthesize_ranking(loop_vars: List[str], guard: str, updates: Dict[str, str]) -> RankingResult:
    """loop_vars = state variables; guard = loop condition (str); updates = {var: next_expr}."""
    try:
        import z3
    except Exception:
        return RankingResult(None, "", "z3 unavailable")
    state = {v: z3.Int(v) for v in loop_vars}
    coef = {v: z3.Int(f"a_{v}") for v in loop_vars}
    const = z3.Int("a0")

    def rank(env):
        e = const
        for v in loop_vars:
            e = e + coef[v] * env[v]
        return e

    g = _z3expr(guard, state)
    nxt = {v: (_z3expr(updates[v], state) if v in updates else state[v]) for v in loop_vars}
    r_now = rank(state)
    r_next = rank(nxt)
    s = z3.Solver()
    s.add(z3.ForAll(list(state.values()),
                    z3.Implies(g, z3.And(r_now >= 0, r_next <= r_now - 1))))
    s.add(z3.Or([coef[v] != 0 for v in loop_vars]))    # nontrivial ranking
    if s.check() == z3.sat:
        m = s.model()
        terms = []
        for v in loop_vars:
            c = m[coef[v]]
            if c is not None and c.as_long() != 0:
                terms.append(f"{c}*{v}")
        c0 = m[const]
        if c0 is not None and c0.as_long() != 0:
            terms.append(str(c0))
        return RankingResult(True, " + ".join(terms) or "0",
                             "decreasing well-founded ranking found ⇒ loop terminates")
    return RankingResult(None, "",
                         "no LINEAR ranking found ⇒ termination unknown (may not terminate, or needs a "
                         "non-linear/lexicographic measure — DEFER)")
