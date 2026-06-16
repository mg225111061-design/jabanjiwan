"""
STAGE W1 — verified aliasing → noalias (HARAN's unique edge over C).
==================================================================
HARAN's own/& types make buffer disjointness a CHECKED fact: a `&mut`/`own` buffer is an exclusive
borrow, so it provably does not alias any other parameter. A C compiler cannot prove this — it needs
the programmer's UNCHECKED `restrict` promise (UB if wrong). So HARAN can emit LLVM `noalias` SAFELY.

This module reports which buffers are noalias-eligible. The *measured* payoff (accel_bench, W1) is
workload-dependent — and on memory-bound element-wise kernels it is ≈1× (bandwidth-bound), reported
honestly. The win is SAFETY (verified) + whatever the workload yields; never an exaggerated number.
"""
from __future__ import annotations

from dataclasses import dataclass
from typing import List

import haran_ast as A


@dataclass
class NoaliasInfo:
    name: str
    reason: str
    c_can_prove: bool = False   # can a C compiler prove this without `restrict`? No.


def noalias_eligible(fn: A.FnDecl) -> List[NoaliasInfo]:
    """Buffers HARAN can mark `noalias` from verified own/& semantics.
    A `&mut`/`own` parameter is an exclusive borrow ⇒ disjoint from every other parameter."""
    out = []
    for p in fn.params:
        if isinstance(p.ty, A.TyOwn):
            out.append(NoaliasInfo(p.name, "own (moved, unique owner) ⇒ exclusive, disjoint"))
        elif isinstance(p.ty, A.TyRef) and p.ty.mutable:
            out.append(NoaliasInfo(p.name, "&mut (exclusive borrow) ⇒ disjoint from all other params"))
    return out
