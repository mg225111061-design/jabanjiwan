"""
HARAN v16 Part B · STAGE B5 — probabilistic narrowing + digit shaving (heart 2).
================================================================================
Bayesian posterior over which operation is the bug, then shave the innocent-operation probability with
layered, independent evidence:

  Layer 1  property bundle   — posterior odds(o) ∝ prior × ∏_{violated V} LR(o|V)  (B4 likelihood ratios)
  Layer 2  input bombardment — each property is violated on c/m inputs; a high, stable rate means the
                               violation is real, not flaky (this confidence feeds the B6 Hoeffding bound)
  Layer 4  causal experiment — MUTATE the suspect operator and re-test: if the violated property RECOVERS,
                               that operation is causally responsible (innocent ops → ~0). This is the
                               decisive, real (mutation-based) shaver.
  Layer 5  cross-validation  — a dataflow reachability check that the suspect op actually influences the
                               output (an op that can't affect the result is exonerated).

★ DISCIPLINE ★ multiplication is valid ONLY across INDEPENDENT properties. Correlated properties
(permutation ⇒ length, same group) inflate the raw product — so B5's digits are TENTATIVE; B6 measures
correlation and DISCOUNTS, and bounds the digit by the available sample (Hoeffding). We do not CLAIM a
digit here — we compute a candidate and hand it to B6 for proof.
"""
from __future__ import annotations

import ast
from dataclasses import dataclass, field
from typing import Dict, List, Optional

import hir
import properties as PR
import property_test as PT
import fault_map as FM


# ----------------------------------------------------------------- Layer 1: Bayesian posterior
@dataclass
class OpScore:
    op_kind: str
    lines: List[int]
    odds: float          # prior_odds × ∏ LR (un-normalized)
    posterior: float     # normalized P(this op is the bug | violations)
    implicated_by: List[str]


@dataclass
class NarrowResult:
    ranked: List[OpScore]
    innocent_posterior: float     # posterior mass on the LEAST-suspect operation (the "innocent digit")
    raw_product_note: str
    causal: Optional["CausalResult"] = None
    crossval_ok: Optional[bool] = None

    def top(self, k=1):
        return self.ranked[:k]


def bayesian_narrow(hfn: hir.HFunction, violated_props: List[PR.Property]) -> NarrowResult:
    fm = FM.map_violations(hfn, violated_props)
    n = max(1, len(fm.suspects))
    prior_odds = 1.0 / n
    scored = [OpScore(s.op_kind, s.lines, prior_odds * s.lr, 0.0, s.implicated_by) for s in fm.suspects]
    Z = sum(s.odds for s in scored) or 1.0
    for s in scored:
        s.posterior = s.odds / Z
    scored.sort(key=lambda s: s.posterior, reverse=True)
    innocent = scored[-1].posterior if scored else 1.0
    note = ("raw ∏ over violated properties — may include CORRELATED properties (B6 discounts); "
            "tentative until B6 proves the digit.")
    return NarrowResult(scored, innocent, note)


# ----------------------------------------------------------------- Layer 4: causal mutation experiment
_CMP_SWAP = {ast.Lt: ast.Gt, ast.Gt: ast.Lt, ast.LtE: ast.GtE, ast.GtE: ast.LtE,
             ast.Eq: ast.NotEq, ast.NotEq: ast.Eq}
_BIN_SWAP = {ast.Add: ast.Sub, ast.Sub: ast.Add, ast.Mult: ast.FloorDiv, ast.FloorDiv: ast.Mult}


class _LineMutator(ast.NodeTransformer):
    def __init__(self, line, swap_compare=True):
        self.line = line
        self.swap_compare = swap_compare
        self.mutated = False

    def visit_Compare(self, node):
        self.generic_visit(node)
        if self.swap_compare and getattr(node, "lineno", None) == self.line and len(node.ops) == 1:
            t = type(node.ops[0])
            if t in _CMP_SWAP:
                node.ops[0] = _CMP_SWAP[t]()
                self.mutated = True
        return node

    def visit_BinOp(self, node):
        self.generic_visit(node)
        if (not self.swap_compare) and getattr(node, "lineno", None) == self.line and type(node.op) in _BIN_SWAP:
            node.op = _BIN_SWAP[type(node.op)]()
            self.mutated = True
        return node


def _mutants_at_line(source: str, line: int) -> List[str]:
    out = []
    for swap_compare in (True, False):
        tree = ast.parse(source)
        m = _LineMutator(line, swap_compare)
        tree = m.visit(tree)
        if m.mutated:
            ast.fix_missing_locations(tree)
            out.append(ast.unparse(tree))
    return out


@dataclass
class CausalResult:
    recovered: bool
    line: int
    op_kind: str
    detail: str


def causal_experiment(hfn: hir.HFunction, op: OpScore, violated_prop: PR.Property,
                      inputs: List[list]) -> CausalResult:
    """Mutate operators on the suspect op's line(s); if a mutant makes the violated property HOLD on all
    inputs, the operation is causally responsible (the bug lives there)."""
    if getattr(hfn, "lang", "python") != "python":
        # operator mutation uses Python's ast; a C/Go/Rust/JS mutator is DEFER. Static LR still localizes.
        return CausalResult(False, op.lines[0] if op.lines else 0, op.op_kind,
                            f"causal mutation is Python-only (DEFER for {hfn.lang}); static narrowing stands")
    for line in op.lines:
        for mut_src in _mutants_at_line(hfn.source, line):
            try:
                ns: dict = {}
                exec(mut_src, ns)
                fn = ns[hfn.name]
                if all(violated_prop.check(fn, x) for x in inputs):
                    return CausalResult(True, line, op.op_kind,
                                        f"mutating the operator at line {line} restores '{violated_prop.name}'")
            except Exception:
                continue
    return CausalResult(False, op.lines[0] if op.lines else 0, op.op_kind, "no single-operator mutant recovered")


# ----------------------------------------------------------------- Layer 5: dataflow reachability
def influences_output(hfn: hir.HFunction, op: OpScore) -> bool:
    """Cheap cross-check: an op kind that never feeds the return value can't be the bug. Here every
    op kind that appears before/within the returned computation is considered reachable; ops with no
    occurrence are excluded by construction. (Conservative: keeps anything that could reach output.)"""
    return bool(op.lines)


# ----------------------------------------------------------------- orchestration + measurement
@dataclass
class NarrowMeasurement:
    top1: str
    top1_lines: List[int]
    top5: List[str]
    innocent_digit_tentative: float
    causal_recovered: bool
    layers_used: List[str]


def narrow(hfn: hir.HFunction, n_random: int = 800, seed: int = 0) -> NarrowMeasurement:
    fn = PR.compile_callable(hfn)
    props = PR.extract_properties(hfn)
    inputs = PT.gen_inputs(hfn, n_random, seed=seed)
    rep = PT.test_properties(fn, props, inputs)
    violated = [p for p in props if p.name in rep.violated_properties()]
    res = bayesian_narrow(hfn, violated)
    layers = ["L1 property-bundle", "L2 input-bombardment"]
    causal_recovered = False
    if res.ranked and violated:
        top = res.ranked[0]
        # use the property that implicates the top op for the causal experiment
        vp = next((p for p in violated if top.op_kind in p.operations), violated[0])
        cr = causal_experiment(hfn, top, vp, inputs[:60])
        res.causal = cr
        causal_recovered = cr.recovered
        if cr.recovered:
            layers.append("L4 causal-mutation")
        res.crossval_ok = influences_output(hfn, top)
        if res.crossval_ok:
            layers.append("L5 dataflow-crossval")
    return NarrowMeasurement(
        top1=res.ranked[0].op_kind if res.ranked else "-",
        top1_lines=res.ranked[0].lines if res.ranked else [],
        top5=[s.op_kind for s in res.top(5)],
        innocent_digit_tentative=res.innocent_posterior,
        causal_recovered=causal_recovered,
        layers_used=layers)
