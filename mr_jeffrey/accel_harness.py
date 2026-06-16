"""
STAGE W2-W6 harness — drives the RELEASE accel_bench and parses its measured lines.
Honest framing: constant-factor only; compute-bound kernels gain more, memory-bound less; techniques
share the memory bottleneck (overlap, not product); Ω(N) is never broken.
"""
from __future__ import annotations

import os
import subprocess

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


def accel_bin():
    for sub in ("target/release/examples/accel_bench", "target/debug/examples/accel_bench"):
        p = os.path.join(ROOT, sub)
        if os.path.isfile(p) and os.access(p, os.X_OK):
            return p
    return None


def run(mode: str, n: int, *extra) -> dict:
    b = accel_bin()
    if not b:
        return {}
    cmd = [b, mode, str(n)] + [str(e) for e in extra]
    out = subprocess.run(cmd, capture_output=True, text=True, timeout=180).stdout.strip()
    d = {}
    for tok in out.split():
        if "=" in tok:
            k, v = tok.split("=", 1)
            if v in ("true", "false"):
                d[k] = (v == "true")
            else:
                try:
                    d[k] = float(v) if ("." in v) else int(v)
                except ValueError:
                    d[k] = v
    return d
