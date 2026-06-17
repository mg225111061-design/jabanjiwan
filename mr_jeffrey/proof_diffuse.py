"""
HARAN v18 Part G · STAGE G3 — proof boundary conditions (the v18 novelty).
=========================================================================
We inject HARAN's verification assets into the diffusion as boundary conditions:
  • initial heat x₀      = Bayesian posterior over operations (B5 narrow) mapped to PDG lines;
  • heat SOURCE          = metamorphic property-violation lines (B4 fault_map), weighted by likelihood;
  • heat SINK  (NOVEL)   = PROVEN-SAFE lines (Z3-verified D2 / Coq-proven D3 / abstract-interp-safe B7),
                           clamped to 0 (Dirichlet) and made absorbing — heat drains into them;
  • conductance          = Hoeffding confidence (B6): a high-confidence safe node insulates strongly.

PRFL cannot use proofs; using proofs as sinks is the entire v18 bet. Whether it actually helps is decided
by measurement in G5 — no favoritism. Honest: abstract-interp gives only CRASH-safety (a crash-safe line
can still be the correctness bug), so it is a WEAK sink; Z3/Coq correctness proofs are STRONG sinks.
"""
from __future__ import annotations

from dataclasses import dataclass, field
from typing import Dict, List, Optional, Set

import numpy as np

import hir
import pdg as PDG
import properties as PR
import property_test as PT
import narrow as NA
import fault_map as FM
import crash_safety as CS


@dataclass
class Boundary:
    graph: PDG.PDG
    x0: np.ndarray            # initial heat (Bayesian) per node
    source: np.ndarray       # violation heat per node
    sinks: Set[int]          # proven-safe node ids (Dirichlet 0, absorbing)
    sink_strength: Dict[int, float]   # 1.0 Z3/Coq, ~0.7 abstract-interp
    conductance: np.ndarray  # per-node confidence (insulation)


def _lines_heat(op_scores, n_nodes, graph) -> np.ndarray:
    """Spread each operation's score across the PDG nodes on its line(s)."""
    h = np.zeros(n_nodes)
    for s in op_scores:
        for ln in (s.lines or []):
            nid = graph.node_at_line(ln)
            if nid is not None:
                h[nid] += s.posterior / max(1, len(s.lines))
    return h


def build_boundary(source_code: str, filename: Optional[str] = None,
                   n_random: int = 300) -> Optional[Boundary]:
    g = PDG.build_pdg(source_code, filename)
    if g.n() == 0:
        return None
    fr = hir.to_hir(source_code, filename)
    if not fr.supported or not fr.module.functions:
        return None
    hfn = fr.module.functions[0]
    fn = PR.compile_callable(hfn)
    props = PR.extract_properties(hfn)
    rep = PT.test_properties(fn, props, PT.gen_inputs(hfn, n_random))
    violated = [p for p in props if p.name in rep.violated_properties()]

    # G3.1 initial heat = Bayesian posterior (B5)
    res = NA.bayesian_narrow(hfn, violated)
    x0 = _lines_heat(res.ranked, g.n(), g)
    if x0.sum() == 0:
        x0 = np.ones(g.n())

    # G3.2 source = violation-implicated lines (B4), weighted by likelihood ratio
    fm = FM.map_violations(hfn, violated)
    source = np.zeros(g.n())
    for s in fm.suspects:
        if s.lr > 1.0:
            for ln in s.lines:
                nid = g.node_at_line(ln)
                if nid is not None:
                    source[nid] += s.lr

    # G3.3 sinks = proven-safe lines (abstract-interp-safe ∩ not-implicated)  [STRONG sinks via Z3/Coq added by caller]
    warned = {w.line for w in CS.static_safety(hfn)}
    implicated = {ln for s in fm.suspects if s.lr > 1.0 for ln in s.lines}
    sinks: Set[int] = set()
    strength: Dict[int, float] = {}
    for nd in g.nodes:
        # a line abstract-interp can't fault AND that no violated property implicates → safe candidate
        if nd.line not in warned and nd.line not in implicated and nd.kind in ("assign", "for", "return"):
            sinks.add(nd.id)
            strength[nd.id] = 0.7          # abstract-interp/safe-zone = WEAK (crash-safety, not correctness)

    # G3.4 conductance = confidence proxy (more violated-property evidence → more confident insulation)
    conductance = np.ones(g.n())
    for nid in sinks:
        conductance[nid] = strength[nid]
    return Boundary(g, x0, source, sinks, strength, conductance)


def add_proof_sinks(b: Boundary, proven_safe_lines: List[int], strength: float = 1.0) -> Boundary:
    """Upgrade lines proven by Z3 (D2) / Coq (D3) to STRONG sinks (correctness proofs, not just crash)."""
    for ln in proven_safe_lines:
        nid = b.graph.node_at_line(ln)
        if nid is not None:
            b.sinks.add(nid)
            b.sink_strength[nid] = strength
            b.conductance[nid] = 1.0 - strength + 0.01   # strong proof → strong insulation (low conductance)
    return b


# ===================================================================================================
# G4 — solve the diffusion with the boundary conditions, then rank.
# ===================================================================================================
import diffuse as _DF  # noqa: E402


def solve_diffusion(b: Boundary, alpha: float = 0.85, use_sinks: bool = True) -> np.ndarray:
    """Absorbing random walk: heat is injected (x₀ + source), flows along conductance-weighted edges,
    and is ABSORBED at proven-safe sinks (which stay at 0). use_sinks=False → pure PRFL (no sinks)."""
    g = b.graph
    n = g.n()
    A = g.adjacency().astype(float)
    inj = (b.x0 / (b.x0.sum() or 1.0)) + (b.source / (b.source.sum() or 1.0))
    if use_sinks:
        A = A * np.outer(b.conductance, b.conductance)        # insulation weights the edges
        transient = [i for i in range(n) if i not in b.sinks]
    else:
        transient = list(range(n))
    if not transient:
        return np.zeros(n)
    P = _DF.transition(A)
    Q = P[np.ix_(transient, transient)]
    bt = inj[transient]
    xt = np.linalg.solve(np.eye(len(transient)) - alpha * Q, bt)
    x = np.zeros(n)
    for k, i in enumerate(transient):
        x[i] = max(0.0, xt[k])
    return x


def rank_lines(b: Boundary, use_sinks: bool = True, alpha: float = 0.85):
    """Return [(line, heat)] sorted hottest-first (proven-safe sinks fall to 0)."""
    x = solve_diffusion(b, alpha=alpha, use_sinks=use_sinks)
    pairs = [(b.graph.line_of(i), x[i]) for i in range(b.graph.n())]
    pairs.sort(key=lambda p: p[1], reverse=True)
    return pairs
