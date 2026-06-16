"""
HARAN v16 Part B · STAGE B4 — property-violation → operation mapping (heart 1).
==============================================================================
A violated property accuses the operations that could CAUSE it: length-preservation → append/pop/slice;
ordered-output → compare/swap; element-preservation (permutation) → value-changing ops. We quantify with
a likelihood ratio LR = P(property violated | this op is the bug) / P(property violated | it is not).

Model (carried into B5's Bayesian update): for a VIOLATED property V and an operation kind k,
  related (k constrains V):   P(V violated | k buggy) = P_REL   (default 0.8)
  unrelated:                  P(V violated | k buggy) = P_UNREL (default 0.05)
  LR(k | V) = P_REL/P_UNREL  if related else 1.
Only violated properties contribute (a property that HELD did not catch the bug — it does not exonerate;
this is the standard spectrum-based-FL stance and avoids false cross-penalties).

Honest: the op↔property table is a heuristic. If it is wrong, the narrowing is wrong — so B6
(Hoeffding/Caesar) checks the digits before any confidence is claimed.
"""
from __future__ import annotations

from dataclasses import dataclass, field
from typing import Dict, List

import hir
import properties as PR

P_REL = 0.8
P_UNREL = 0.05
LR_HIT = P_REL / P_UNREL          # 16× odds bump per violated property that implicates an op kind


def likelihood_ratio(op_kind: str, prop: PR.Property) -> float:
    """LR contributed by a VIOLATED property to an operation kind (1.0 if unrelated)."""
    return LR_HIT if op_kind in prop.operations else 1.0


@dataclass
class Suspect:
    op_kind: str
    lines: List[int]
    lr: float                       # product of LRs from all violated properties
    implicated_by: List[str]        # which violated properties accuse this op kind


@dataclass
class FaultMap:
    suspects: List[Suspect]         # ranked, highest LR first
    violated: List[str]

    def top(self, k: int = 1) -> List[Suspect]:
        return self.suspects[:k]


def map_violations(hfn: hir.HFunction, violated_props: List[PR.Property]) -> FaultMap:
    # all operation kinds actually present in the function (with their line occurrences)
    kinds: Dict[str, List[int]] = {}
    for o in hfn.ops:
        kinds.setdefault(o.kind, []).append(o.line)
    suspects: List[Suspect] = []
    for k, lines in kinds.items():
        lr = 1.0
        impl = []
        for v in violated_props:
            r = likelihood_ratio(k, v)
            if r > 1.0:
                lr *= r
                impl.append(v.name)
        suspects.append(Suspect(k, sorted(set(lines)), lr, impl))
    suspects.sort(key=lambda s: s.lr, reverse=True)
    return FaultMap(suspects, [v.name for v in violated_props])
