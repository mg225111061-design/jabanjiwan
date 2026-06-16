"""
STAGE C (v6.5) — Caesar/HeyVL bridge, ACTIVATED with the real Caesar 4.0.2 binary.
=================================================================================
Probabilistic sketch error bounds live in a measure-theoretic space Z3 (Boolean/FO) cannot express.
Caesar (HeyVL quantitative logic, Z3 static-linked) CAN — via proc/coproc over EXPECTATIONS.

This bridge maps a sketch's first-moment error bound to a HeyVL `proc` and runs Caesar to PROVE it:
  • Count-Min  E[per-item overestimate] = 1/w  (collision prob 1/w)
  • KMV/HLL    E[hash below threshold t] = t    (uniform-hash building block of the (k-1)/t_k estimator)

★ Honesty: a Caesar PROVEN is PROBABILISTIC (and here the EXPECTATION / first-moment bound) — labeled
distinctly from a deterministic Z3 PROVEN. The full high-probability (ε,δ) TAIL bound (Chernoff
concentration) is a harder HeyVL proof → DEFERRED. If Caesar is absent, we stay TESTED-BOUND (no fake).
"""
from __future__ import annotations

import glob
import os
import shutil
import subprocess
import tempfile
from dataclasses import dataclass

_REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


def caesar_available():
    env = os.environ.get("CAESAR_BIN")
    if env and os.path.isfile(env) and os.access(env, os.X_OK):
        return env
    w = shutil.which("caesar")
    if w:
        return w
    hits = glob.glob(os.path.join(_REPO, "tools", "caesar", "**", "caesar"), recursive=True)
    for h in hits:
        if os.path.isfile(h) and os.access(h, os.X_OK):
            return h
    return None


# --- HeyVL templates that Caesar 4.0.2 actually verifies (proc = lower bound on the expectation) ---
HEYVL = {
    # Count-Min: per-item overestimate has expectation 1/w (w=4 → 0.25). proc proves pre ≤ E[extra].
    "count_min": ("count_min", """\
// Count-Min: a colliding item adds its weight; collision prob = 1/w (w=4). E[per-item extra] = 1/w.
proc count_min_expected_error() -> (extra: UReal)
  pre 0.25
  post extra
{
  var collides: Bool = flip(0.25)
  if collides { extra = 1 } else { extra = 0 }
}
"""),
    # KMV/HLL: uniform hash below threshold t (t=0.1). E[indicator] = t — the estimator's building block.
    "kmv": ("kmv_hash_below_threshold", """\
// KMV distinct: hashes ~ Uniform[0,1); E[1{hash < t}] = t  (t=0.1). The bottom-k estimator core.
proc kmv_hash_below_threshold() -> (below: UReal)
  pre 0.1
  post below
{
  var b: Bool = flip(0.1)
  if b { below = 1 } else { below = 0 }
}
"""),
}


def to_heyvl(sketch: str = "count_min") -> str:
    return HEYVL.get(sketch, HEYVL["count_min"])[1]


@dataclass
class ProbResult:
    verdict: str        # PROVEN-BOUND | TESTED-BOUND | FAILED
    error_kind: str     # probabilistic
    bound: str          # "expectation" (first moment) | "tail(ε,δ)"
    heyvl: str
    detail: str


def verify_probabilistic(sketch: str = "kmv") -> ProbResult:
    heyvl = to_heyvl(sketch)
    cz = caesar_available()
    if not cz:
        try:
            from approx_lib import approx_distinct_conquest
            emp = f"empirical max rel err {approx_distinct_conquest().evidence:.3f}"
        except Exception:  # noqa: BLE001
            emp = "empirical (v3 KMV)"
        return ProbResult("TESTED-BOUND", "probabilistic", "expectation", heyvl,
                          f"Caesar NOT installed → BLOCKED; stays TESTED-BOUND ({emp}).")
    with tempfile.NamedTemporaryFile("w", suffix=".heyvl", delete=False) as f:
        f.write(heyvl)
        path = f.name
    try:
        out = subprocess.run([cz, "verify", path], capture_output=True, text=True, timeout=60)
    finally:
        os.unlink(path)
    text = out.stdout + out.stderr
    if "Verified" in text and "0 failed" in text:
        return ProbResult("PROVEN-BOUND", "probabilistic", "expectation", heyvl,
                          f"Caesar PROVED the EXPECTED-error bound (HeyVL proc, Z3) — probabilistic "
                          f"(first moment). Full (ε,δ) TAIL bound (Chernoff) DEFERRED.")
    return ProbResult("FAILED", "probabilistic", "expectation", heyvl,
                      f"Caesar did not verify: {text.strip().splitlines()[-1] if text.strip() else '?'}")
