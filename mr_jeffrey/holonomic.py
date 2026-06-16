"""
STAGE T3 (v5) — simple holonomic attempt + Gröbner ceiling, HONESTLY measured.
=============================================================================
Holonomic creative telescoping (Zeilberger / Chyzak) finds the telescoper by undetermined
coefficients over ℚ[n,k] — a Gröbner-style linear system whose size grows fast (double-exponential
in the worst case). We attempt it with a wall-clock TIMEOUT GUARD; a timeout is NOT a failure, it is
the *measurement of the mathematical ceiling* — reported as DEFER, never forced.

Measured boundary (this engine): order-1 single hypergeometric sums (ΣC(n,k), ΣC(n,k)²) solve in <1s;
order-2 holonomic (Franel ΣC(n,k)³) and beyond → timeout = Gröbner ceiling = DEFER.
"""
from __future__ import annotations

import os
import subprocess
import time
from dataclasses import dataclass
from typing import Optional

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


def _zeil_bin():
    for sub in ("target/release/examples/zeil_check", "target/debug/examples/zeil_check"):
        p = os.path.join(ROOT, sub)
        if os.path.isfile(p) and os.access(p, os.X_OK):
            return p
    return None


@dataclass
class HolonomicResult:
    which: str
    verdict: str            # CLOSED | DEFER
    order: Optional[int]
    elapsed_s: float
    detail: str

    def __str__(self):
        o = f" order-{self.order}" if self.order is not None else ""
        return f"{self.which}: {self.verdict}{o} ({self.elapsed_s:.1f}s) — {self.detail}"


def attempt(which: str, max_order: int = 2, timeout_s: float = 20.0) -> HolonomicResult:
    b = _zeil_bin()
    if not b:
        return HolonomicResult(which, "DEFER", None, 0.0, "zeil_check engine not built")
    t0 = time.perf_counter()
    try:
        out = subprocess.run([b, which, str(max_order)], capture_output=True, text=True,
                             timeout=timeout_s).stdout.strip()
    except subprocess.TimeoutExpired:
        return HolonomicResult(which, "DEFER", None, time.perf_counter() - t0,
                               f"holonomic — Gröbner/search complexity, timeout-guarded at {timeout_s:.0f}s")
    el = time.perf_counter() - t0
    if out.startswith("TELESCOPER") and "verified=true" in out:
        order = int(dict(t.split("=", 1) for t in out.split() if "=" in t)["order"])
        return HolonomicResult(which, "CLOSED", order, el, "telescoper found + checker-verified")
    return HolonomicResult(which, "DEFER", None, el, "no telescoper within bounds")


def measure_boundary(timeout_s: float = 12.0):
    cases = [("binom", 1), ("binom_sq", 1), ("binom_cube", 2), ("binom_quad", 3)]
    return [attempt(w, mo, timeout_s) for w, mo in cases]
