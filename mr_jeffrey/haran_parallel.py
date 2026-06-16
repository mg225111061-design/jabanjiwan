"""
HARAN v16 Part A · STAGE A2 — parallel verification (function-level, multicore).
================================================================================
Each function's verification (mr_haran.verify_fn) only READS the shared function table — it never needs
another function's *verdict* — so functions are verification-independent and fan out across cores with a
process pool (true parallelism past the GIL; Z3 also releases the GIL during solving). We measure the
speedup vs worker count.

Honest limits: process pools pay pickling + startup, so tiny/fast suites can be SLOWER in parallel than
sequential (overhead > work) — reported, not hidden. A pipeline whose proof of B reused A's proof would
NOT parallelize those two; here there is no such proof-result dependency.
"""
from __future__ import annotations

import concurrent.futures as cf
import multiprocessing as mp
import time
from dataclasses import dataclass
from typing import List, Tuple

import haran_ast as A
from haran_parser import parse
import mr_haran

# shared, read-only verification context — set BEFORE the pool is created so forked workers inherit it
# for free (no per-task pickling of the function table; tasks carry only an integer index).
_SHARED: dict = {}


def _verify_idx(i: int):
    return mr_haran.verify_fn(_SHARED["fns"][i], _SHARED["ftab"], _SHARED["proc_names"])


def _verify_one(args):
    fn, ftab, proc_names = args
    return mr_haran.verify_fn(fn, ftab, proc_names)


def _prep(src: str):
    prog = parse(src)
    if prog.errors:
        raise ValueError("; ".join(str(e) for e in prog.errors))
    fns = [it for it in prog.items if isinstance(it, A.FnDecl)]
    ftab = {f.name: f for f in fns}
    proc_names = {f.name for f in fns if f.kind == "proc"}
    return fns, ftab, proc_names


def verify_program_sequential(src: str) -> List[mr_haran.FnReport]:
    fns, ftab, proc_names = _prep(src)
    return [mr_haran.verify_fn(f, ftab, proc_names) for f in fns]


def verify_program_parallel(src: str, workers: int = 4) -> List[mr_haran.FnReport]:
    fns, ftab, proc_names = _prep(src)
    if workers <= 1 or len(fns) <= 1:
        return [mr_haran.verify_fn(f, ftab, proc_names) for f in fns]
    # set shared context, then fork: workers inherit fns/ftab via COW memory (no pickling per task)
    _SHARED["fns"], _SHARED["ftab"], _SHARED["proc_names"] = fns, ftab, proc_names
    try:
        ctx = mp.get_context("fork")
    except ValueError:
        ctx = None
    with cf.ProcessPoolExecutor(max_workers=workers, mp_context=ctx) as ex:
        return list(ex.map(_verify_idx, range(len(fns)), chunksize=max(1, len(fns) // (workers * 4))))


def verify_subset_parallel(fns: list, ftab: dict, proc_names: set, workers: int = 4):
    """Verify a given subset of functions in parallel (used by the cache+parallel FastVerifier:
    only the cache MISSES are re-verified, and those fan out across cores)."""
    if not fns:
        return []
    if workers <= 1 or len(fns) == 1:
        return [mr_haran.verify_fn(f, ftab, proc_names) for f in fns]
    _SHARED["fns"], _SHARED["ftab"], _SHARED["proc_names"] = fns, ftab, proc_names
    try:
        ctx = mp.get_context("fork")
    except ValueError:
        ctx = None
    with cf.ProcessPoolExecutor(max_workers=workers, mp_context=ctx) as ex:
        return list(ex.map(_verify_idx, range(len(fns))))


@dataclass
class ParallelMeasurement:
    n_functions: int
    seq_s: float
    by_workers: dict          # workers -> (seconds, speedup)
    verdicts_match: bool      # parallel verdicts == sequential verdicts (correctness preserved)


def measure_parallel(src: str, worker_counts=(1, 2, 4)) -> ParallelMeasurement:
    fns, _, _ = _prep(src)
    t = time.perf_counter()
    seq_reports = verify_program_sequential(src)
    seq = time.perf_counter() - t
    seq_verdicts = [(r.name, r.verdict) for r in seq_reports]
    by = {}
    match = True
    for w in worker_counts:
        t = time.perf_counter()
        rep = verify_program_parallel(src, workers=w)
        dt = time.perf_counter() - t
        by[w] = (dt, seq / dt if dt > 0 else float("inf"))
        if [(r.name, r.verdict) for r in rep] != seq_verdicts:
            match = False
    return ParallelMeasurement(len(fns), seq, by, match)


# ----------------------------------------------------------------- workload generator (chunky, honest)
def faulhaber_corpus(count: int) -> str:
    """`count` independent Faulhaber-style functions of varying degree — each a real collapse+Z3
    obligation, so the per-function work is non-trivial (amortizes pool overhead)."""
    closed = {
        2: "n*(n+1)/2", 3: "n*(n+1)*(2*n+1)/6", 4: "(n*(n+1)/2)*(n*(n+1)/2)",
    }
    summand = {2: "k", 3: "k*k", 4: "k*k*k"}
    parts = []
    for i in range(count):
        d = 2 + (i % 3)
        parts.append(f"fn f{i}(n: Nat) -> Nat\n  ensures result = {closed[d]}\n"
                     f"{{ fold k in 1..n {{ {summand[d]} }} }}\n")
    return "\n".join(parts)


def heavy_corpus(count: int) -> str:
    """`count` higher-degree power sums (deg 5-7): each collapse+Z3 obligation is ~15-30ms, so the
    per-function work dominates pool overhead and the multicore speedup is visible honestly."""
    parts = []
    for i in range(count):
        d = 5 + (i % 3)                       # degree 5, 6, 7
        body = "*".join(["k"] * d)
        parts.append(f"fn h{i}(n: Nat) -> Nat\n  ensures result >= 0\n"
                     f"{{ fold k in 1..n {{ {body} }} }}\n")
    return "\n".join(parts)
