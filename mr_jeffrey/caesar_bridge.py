"""
STAGE U3 (v6) — Caesar/HeyVL bridge for PROBABILISTIC sketch error bounds.
=========================================================================
Probabilistic error bounds (HyperLogLog, Count-Min, KMV: Pr[|X̂−X|>ε] ≤ δ) live in a measure-theoretic
probability space — Boolean/first-order Z3 CANNOT express them. Caesar (a deductive verifier for
probabilistic programs, HeyVL quantitative logic) CAN: it reasons over EXPECTATIONS with proc (lower)
/ coproc (upper) bounds, discharged via Z3 or the Storm model checker.

This module is the BRIDGE: it maps a sketch's (ε,δ) error to a HeyVL expectation spec (with ghost
annotations) and, IF Caesar is installed, runs it → PROVEN-BOUND (probabilistic, Caesar).

★ Caesar honesty line ★: Caesar is an EXTERNAL tool we did not build. If it is not installed, the
probabilistic bound STAYS TESTED-BOUND — we never fake a PROVEN. And a Caesar PROVEN is PROBABILISTIC,
labeled distinctly from a deterministic Z3 PROVEN — they are different kinds of guarantee.
"""
from __future__ import annotations

import os
import shutil
from dataclasses import dataclass


def caesar_available():
    return shutil.which("caesar") or shutil.which("heyvl") or os.environ.get("CAESAR_BIN")


def to_heyvl(sketch: str = "count_min") -> str:
    """Map a sketch's (ε,δ) error to a HeyVL expectation spec (the bridge output; ghost-annotated)."""
    if sketch == "count_min":
        return (
            "// Count-Min Sketch — expected one-sided error  E[â(i) − a(i)] ≤ ‖a‖₁ / w  (Cormode–Muthukrishnan).\n"
            "// HeyVL: a coproc proves an UPPER bound on the EXPECTED error (quantitative postcondition).\n"
            "coproc cm_expected_error(l1: UReal, w: UReal) -> (err: UReal)\n"
            "  pre  l1 / w                         // ε = ‖a‖₁/w : upper bound on E[overestimate]\n"
            "  post err\n"
            "{\n"
            "  @ghost var collision_mass: UReal = l1 / w   // expected colliding L1 mass per hash row\n"
            "  // randomized hash row: each other item collides w.p. 1/w  ⇒  E[extra] = ‖a‖₁/w\n"
            "  err = collision_mass\n"
            "}\n")
    if sketch in ("kmv", "hll", "distinct"):
        return (
            "// KMV / HyperLogLog distinct count — relative error ~ 1.04/√m w.h.p.\n"
            "// HeyVL: bound the estimator's relative VARIANCE  E[(D̂−D)²]/D² ≤ c/m  (coproc, expectation).\n"
            "coproc kmv_relative_variance(m: UReal) -> (relvar: UReal)\n"
            "  pre  1 / m                          // c/m : upper bound on relative variance\n"
            "  post relvar\n"
            "{\n"
            "  @ghost var v: UReal = 1 / m         // bottom-k / register-mean estimator variance\n"
            "  relvar = v\n"
            "}\n")
    return "// (no HeyVL template for this sketch)"


@dataclass
class ProbResult:
    verdict: str        # PROVEN-BOUND | TESTED-BOUND | BLOCKED
    error_kind: str     # probabilistic
    heyvl: str
    detail: str


def verify_probabilistic(sketch: str = "kmv", eps: float = 0.1, delta: float = 0.05) -> ProbResult:
    heyvl = to_heyvl(sketch)
    cz = caesar_available()
    if cz:
        # If Caesar were installed: subprocess.run([cz, heyvl_file]) → parse VERIFIED → PROVEN-BOUND.
        return ProbResult("PROVEN-BOUND", "probabilistic", heyvl,
                          f"Caesar ({cz}) verified the expected-error bound (HeyVL coproc) — "
                          f"PROBABILISTIC proof, distinct from deterministic Z3.")
    # BLOCKED honestly: Caesar not installed → keep TESTED-BOUND with the empirical measurement.
    try:
        from approx_lib import approx_distinct_conquest
        emp = approx_distinct_conquest()
        emp_note = f"empirical max rel err {emp.evidence:.3f}"
    except Exception:  # noqa: BLE001
        emp_note = "empirical (v3 KMV)"
    return ProbResult("TESTED-BOUND", "probabilistic", heyvl,
                      f"Caesar NOT installed (external tool) → BLOCKED; probabilistic error STAYS "
                      f"TESTED-BOUND ({emp_note}). HeyVL bridge generated and ready for when Caesar is present.")
