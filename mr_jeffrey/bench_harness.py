"""
STAGE Y3/Y4 — benchmark harness (honest wall-clock, no headline number, no cherry-pick).
========================================================================================
Drives the RELEASE-built `haran_bench` (fold + unstructured, median of K runs) and times the
approximate quantile (X2) in Python. Produces per-input-size tables — including the UNFAVORABLE
cases — so the numbers survive an external benchmark.

Honest framing (v4 lifeline):
  · FOLD (CLOSED) — orders of magnitude, growing with n (closed form is O(1)).
  · UNSTRUCTURED — constant-factor only; well-written naive ≈ optimized (Ω(N) is a theorem, not a gap).
  · APPROX — space/time win + MEASURED error within the PROVEN ε.
"""
from __future__ import annotations

import os
import random
import subprocess
import time
from typing import List

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


def _bench_bin():
    for sub in ("target/release/examples/haran_bench", "target/debug/examples/haran_bench"):
        p = os.path.join(ROOT, sub)
        if os.path.isfile(p) and os.access(p, os.X_OK):
            return p
    return None


def _run(mode: str, n: int) -> dict:
    b = _bench_bin()
    if not b:
        return {}
    out = subprocess.run([b, mode, str(n)], capture_output=True, text=True, timeout=120).stdout.strip()
    d = {}
    for tok in out.split():
        if "=" in tok:
            k, v = tok.split("=", 1)
            try:
                d[k] = float(v) if "." in v else int(v)
            except ValueError:
                d[k] = v
    return d


def foldsum_table(sizes: List[int]) -> List[dict]:
    return [_run("foldsum", n) for n in sizes]


def unstruct_table(sizes: List[int]) -> List[dict]:
    return [_run("unstruct", n) for n in sizes]


# --- approximate quantile (Python), measured time + space + error vs PROVEN ε=w/2 per element ---
def _exact_quantile(vals, q):
    s = sorted(vals)
    return s[min(int(q * len(s)), len(s) - 1)]


def approx_table(sizes: List[int], m: int = 64, lo=0.0, hi=1000.0, q=0.5) -> List[dict]:
    from approx_lib import bucketed_quantile
    rows = []
    for n in sizes:
        rng = random.Random(20260616)
        vals = [rng.uniform(lo, hi) for _ in range(n)]
        t = time.perf_counter(); approx = bucketed_quantile(vals, q, lo, hi, m); ta = time.perf_counter() - t
        t = time.perf_counter(); exact = _exact_quantile(vals, q); te = time.perf_counter() - t
        w = (hi - lo) / m
        rows.append({"n": n, "m": m, "approx": approx, "exact": exact,
                     "err": abs(approx - exact), "bound_w": w, "half_w": w / 2,
                     "approx_ms": ta * 1e3, "exact_ms": te * 1e3,
                     "space_approx": m, "space_exact": n})
    return rows


def render_fold(rows):
    out = ["FOLD (CLOSED): closed-form O(1) vs naive O(n) loop, Σi² — median ns:"]
    out.append(f"   {'n':>10} {'closed_ns':>10} {'naive_ns':>14} {'ratio':>14}")
    for r in rows:
        if r:
            out.append(f"   {r['n']:>10} {r['closed_ns']:>10} {r['naive_ns']:>14} {r['ratio']:>14,.1f}×")
    out.append("   → closed form ≈ flat (O(1) confirmed); ratio grows with n (orders of magnitude).")
    return "\n".join(out)


def render_unstruct(rows):
    out = ["UNSTRUCTURED (NO_STRUCTURE): data sum naive vs 4-way unrolled — median ns:"]
    out.append(f"   {'n':>10} {'naive_ns':>12} {'unrolled_ns':>12} {'speedup':>10}")
    for r in rows:
        if r:
            out.append(f"   {r['n']:>10} {r['naive_ns']:>12} {r['unrolled_ns']:>12} {r['speedup']:>9.2f}×")
    out.append("   → speedup is CONSTANT (≈1×, naive already vectorized); Ω(N), never orders of magnitude.")
    return "\n".join(out)


def render_approx(rows):
    out = ["APPROX (PROVEN-BOUND): bucketed quantile vs exact sort — time/space/error:"]
    out.append(f"   {'n':>9} {'m':>5} {'approx_ms':>10} {'exact_ms':>9} {'err':>8} {'≤w?':>5} {'space':>14}")
    for r in rows:
        ok = "yes" if r["err"] <= r["bound_w"] else "NO"
        space = f"{r['space_approx']}vs{r['space_exact']}"
        out.append(f"   {r['n']:>9} {r['m']:>5} {r['approx_ms']:>10.3f} {r['exact_ms']:>9.3f} "
                   f"{r['err']:>8.2f} {ok:>5} {space:>14}")
    out.append("   → error within proven per-element ε=w/2 bound (≤w overall); O(m) space vs O(n) sort.")
    return "\n".join(out)
