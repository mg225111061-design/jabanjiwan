"""
HARAN v16 Part B · STAGE B7 (performance) — hotspots by MEASUREMENT (~99%), not probability.
============================================================================================
Performance is its own category: we PROFILE (cProfile) and report "function f is X% of runtime". This
is a measurement, not a probabilistic correctness digit — its ~99% confidence comes from the measured
share, and it must never be blended with the correctness top-k.
"""
from __future__ import annotations

import cProfile
import io
import pstats
from dataclasses import dataclass
from typing import Callable, List, Optional


@dataclass
class Hotspot:
    func: str
    selftime: float      # time spent IN this function (excludes children) — the optimization target
    pct: float           # self-time share of total runtime


def profile_hotspots(module_src: str, entry: str, inputs: List, repeat: int = 1) -> List[Hotspot]:
    ns: dict = {}
    exec(module_src, ns)
    fn = ns[entry]

    def driver():
        for _ in range(repeat):
            for x in inputs:
                try:
                    fn(list(x) if isinstance(x, list) else x)
                except Exception:
                    pass

    pr = cProfile.Profile()
    pr.enable()
    driver()
    pr.disable()
    st = pstats.Stats(pr, stream=io.StringIO())
    total = st.total_tt or 1e-12
    rows = []
    for key, val in st.stats.items():  # type: ignore[attr-defined]
        name = key[2]                  # (filename, lineno, funcname)
        tt = val[2]                    # (cc, nc, tt, ct[, callers]) — tt = SELF time (excl. children)
        if name in ns and callable(ns.get(name)):   # only user functions from the analysed module
            rows.append(Hotspot(name, tt, 100.0 * tt / total))
    rows.sort(key=lambda h: h.selftime, reverse=True)
    return rows


def top_hotspot(module_src: str, entry: str, inputs: List, repeat: int = 1) -> Optional[Hotspot]:
    hs = profile_hotspots(module_src, entry, inputs, repeat)
    return hs[0] if hs else None
