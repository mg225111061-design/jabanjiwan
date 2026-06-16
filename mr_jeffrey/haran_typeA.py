"""
HARAN v16 Part A · STAGE A4 — Type A integration ("Mr랑 짝짝" = verification is fast).
=====================================================================================
Combines the three A-stage upgrades into one verifier and one honest speed summary:
  • A1 caching   — unchanged functions (Merkle key) are served from cache, never re-proved;
  • A2 parallel  — the cache MISSES (changed functions) fan out across cores;
  • A3 Coq       — recognized specs escalate to Coq for unbounded ∀ (beyond Z3's bounded reach).

FastVerifier.verify = cache hits (instant) + parallel re-verify of misses. In the edit-verify loop
this is the "순식간" path. Honest: the FIRST full verification (all misses) still pays the prover, and
non-linear SMT is still slow — caching/parallelism speed the loop and the multicore fan-out, not the
intrinsic cost of one hard proof.
"""
from __future__ import annotations

from dataclasses import dataclass, field
from typing import Dict, List, Tuple

import haran_ast as A
from haran_parser import parse
import mr_haran
import haran_cache as cache
import haran_parallel as parallel
import haran_coq as coq


@dataclass
class FastStats:
    hits: int = 0
    misses: int = 0
    reverified: List[str] = field(default_factory=list)


class FastVerifier:
    """A1 caching + A2 parallelism combined."""

    def __init__(self):
        self.cache: Dict[str, mr_haran.FnReport] = {}

    def verify(self, src: str, workers: int = 4) -> Tuple[List[mr_haran.FnReport], FastStats]:
        prog = parse(src)
        if prog.errors:
            raise ValueError("; ".join(str(e) for e in prog.errors))
        fns = [it for it in prog.items if isinstance(it, A.FnDecl)]
        ftab = {f.name: f for f in fns}
        proc_names = {f.name for f in fns if f.kind == "proc"}
        keys = [cache.merkle_key(f, ftab) for f in fns]
        miss_idx = [i for i, k in enumerate(keys) if k not in self.cache]
        # re-verify ONLY the misses (changed functions), in parallel across cores
        miss_fns = [fns[i] for i in miss_idx]
        miss_reports = parallel.verify_subset_parallel(miss_fns, ftab, proc_names, workers)
        for i, rep in zip(miss_idx, miss_reports):
            self.cache[keys[i]] = rep
        reports = [self.cache[k] for k in keys]
        stats = FastStats(hits=len(fns) - len(miss_idx), misses=len(miss_idx),
                          reverified=[fns[i].name for i in miss_idx])
        return reports, stats


# ----------------------------------------------------------------- A4.2 combined speed summary
@dataclass
class SpeedSummary:
    cache_edit_loop_speedup: float      # cold full verify / warm unchanged (A1)
    parallel_speedup_4c: float          # heavy corpus, 4 cores (A2)
    coq_unbounded_proven: int           # unbounded ∀ theorems (A3)
    coq_available: bool
    honest_caveat: str


def verification_speed_summary() -> SpeedSummary:
    prog = parallel.faulhaber_corpus(12)
    edited = prog.replace("fold k in 1..n { k }", "fold k in 1..n { k + 0 }", 1)
    m = cache.measure_edit_loop(prog, edited)
    pm = parallel.measure_parallel(parallel.heavy_corpus(16), (1, 4))
    cnt = coq.prove_all().count if coq.coq_available() else 0
    return SpeedSummary(
        cache_edit_loop_speedup=m.speedup_unchanged,
        parallel_speedup_4c=pm.by_workers[4][1],
        coq_unbounded_proven=cnt,
        coq_available=coq.coq_available(),
        honest_caveat="first full verification + non-linear SMT remain slow; caching speeds the edit "
                      "loop, parallelism the fan-out, Coq only recognized unbounded shapes.")


def render(s: SpeedSummary) -> str:
    return ("HARAN Type A — verification speed (measured, honest):\n"
            f"   · edit-verify loop (caching)  : ×{s.cache_edit_loop_speedup:.0f} on unchanged re-verify\n"
            f"   · parallel (4 cores)          : ×{s.parallel_speedup_4c:.2f} on independent obligations\n"
            f"   · Coq unbounded ∀ proven      : {s.coq_unbounded_proven} theorems "
            f"({'coqc live' if s.coq_available else 'BLOCKED'})\n"
            f"   · caveat                      : {s.honest_caveat}")
