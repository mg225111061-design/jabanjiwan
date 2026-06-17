"""
HARAN v21 Part R · STAGE R5 — background verification (hard cases never block the user).
=======================================================================================
NP-hard / inductive propositions are intrinsically slow — that cannot be removed. The fix for "perceived
zero" is not "instant" but "NOT BLOCKED": fast results return synchronously (ms); hard ones run in a
background thread while the user keeps working. Status is honest: ✅ proven / ⏳ verifying / ❌
counterexample / ⏱️ timeout (never a false "done"). The user's BLOCKED time = the fast (synchronous)
portion only.
"""
from __future__ import annotations

import concurrent.futures as _cf
import time
from dataclasses import dataclass, field
from typing import Callable, Dict, List, Optional

import verify_speed as VS


@dataclass
class AsyncResult:
    name: str
    status: str          # ✅proven | ❌counterexample | ⏳verifying | ⏱️timeout
    ms: float
    background: bool


class AsyncVerifier:
    """Fast items verified synchronously (immediate); hard items dispatched to a background pool."""

    def __init__(self):
        self.pool = _cf.ThreadPoolExecutor(max_workers=4)
        self.futures: Dict[str, _cf.Future] = {}
        self.submit_t: Dict[str, float] = {}
        self.sync: List[AsyncResult] = []
        self.blocked_ms = 0.0

    def start(self, corpus: List[VS.Item]) -> List[AsyncResult]:
        """Returns the IMMEDIATE (synchronous, fast) results; hard items go to the background."""
        t0 = time.perf_counter()
        for it in corpus:
            if it.kind == "haran":                       # fast tier → synchronous (blocks briefly)
                r = VS.tiered_verify(it, max_tier=2)
                self.sync.append(AsyncResult(it.name, "✅proven" if r.resolved else "❌counterexample",
                                             r.ms, False))
            else:                                        # hard (Coq) → background, user not blocked
                self.submit_t[it.name] = time.perf_counter()
                self.futures[it.name] = self.pool.submit(VS.verify_item, it)
        self.blocked_ms = (time.perf_counter() - t0) * 1000
        return self.sync

    def status(self, per_item_timeout_s: float = 5.0) -> List[AsyncResult]:
        out = list(self.sync)
        for name, fut in self.futures.items():
            elapsed = (time.perf_counter() - self.submit_t[name]) * 1000
            if fut.done():
                tool, resolved = fut.result()
                out.append(AsyncResult(name, "✅proven" if resolved else "❌counterexample", elapsed, True))
            elif elapsed > per_item_timeout_s * 1000:
                out.append(AsyncResult(name, "⏱️timeout", elapsed, True))
            else:
                out.append(AsyncResult(name, "⏳verifying", elapsed, True))
        return out

    def wait(self, timeout_s: float = 30.0) -> List[AsyncResult]:
        _cf.wait(self.futures.values(), timeout=timeout_s)
        return self.status()

    def shutdown(self):
        self.pool.shutdown(wait=False)


def measure_no_block(corpus: List[VS.Item] = None) -> dict:
    """Blocked time (sync fast tier) vs total verification work (incl. background hard cases)."""
    corpus = corpus or VS.CORPUS
    av = AsyncVerifier()
    immediate = av.start(corpus)
    blocked = av.blocked_ms
    final = av.wait(timeout_s=30)
    av.shutdown()
    hard_work = sum(r.ms for r in final if r.background)
    return {"blocked_ms": blocked, "immediate": len(immediate), "background": len(av.futures),
            "background_work_ms": hard_work, "final": final}
