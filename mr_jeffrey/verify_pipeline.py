"""
HARAN v21 Part R · STAGE R6 — integrated speed pipeline + final measurement.
============================================================================
Combines the optimizations: fold-first (O(1)) → fast-path tiering (Z3 only if needed) → incremental
re-verify (cache, edit loop) → background (hard cases don't block). Produces the honest speed table vs the
R1 baseline. Correctness is invariant throughout (every optimization is sound; nothing skipped).
"""
from __future__ import annotations

from dataclasses import dataclass
from typing import List

import verify_speed as VS
import haran_cache as HC
import async_verify as AV


def verify_optimized(item: VS.Item) -> VS.TierResult:
    """fold-first (O(1)) → fast-path tier. (Incremental caching + background handled at the suite level.)"""
    if item.kind == "haran":
        ms, ok = VS.fold_first_verify(item)
        if ok:
            return VS.TierResult(item.name, 1, "fold-first(O(1))", ms, True)
    return VS.tiered_verify(item, max_tier=3)


@dataclass
class SpeedTable:
    easy_ms: float            # easy verification (fast-path/fold), perceived-zero
    edit_loop_ms: float       # incremental re-verify after one edit
    hard_blocked_ms: float    # user-blocked time for hard cases (background ⇒ ~0)
    hard_background_ms: float  # actual hard work (runs in background)
    gen_only_ms: float        # simulated code-generation time
    gen_plus_verify_ms: float  # generation + easy verification
    overhead_ms: float         # the "+Xms for a proven result"


def final_measurement() -> SpeedTable:
    # easy: fast-path tier average (the common case)
    tiered = VS.measure_tiered()
    easy = [r for r in tiered.rows if r.tier in (1, 2) and r.resolved]
    easy_ms = sum(r.ms for r in easy) / len(easy)

    # edit loop: incremental re-verify of one changed function in a codebase
    src = "\n\n".join(f"fn f{i}(n: Nat)->Nat\n  ensures result = n*(n+1)/2\n{{ fold k in 1..n {{ k }} }}"
                      for i in range(12))
    edited = src.replace("fold k in 1..n { k }", "fold k in 1..n { k + 0 }", 1)
    el = HC.measure_edit_loop(src, edited)
    edit_loop_ms = el.warm_one_edit_s * 1000

    # hard: background → user blocked ~0; the work happens off the critical path
    nb = AV.measure_no_block()
    hard_blocked = nb["blocked_ms"]
    hard_bg = nb["background_work_ms"]

    # "code gen + verify" vs "gen only": verification adds only the easy-case overhead
    import time
    t = time.perf_counter()
    _ = sum(i * i for i in range(200000))      # stand-in for code generation work
    gen_only = (time.perf_counter() - t) * 1000
    overhead = easy_ms
    return SpeedTable(easy_ms, edit_loop_ms, hard_blocked, hard_bg, gen_only, gen_only + overhead, overhead)
