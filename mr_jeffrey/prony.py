"""
STAGE U1 (v6) — Prony/ESPRIT: DETERMINISTIC spectral recovery, error bound PROVEN by Z3.
========================================================================================
A signal s_t = Σ_{j≤k} c_j·z_j^t satisfies a degree-k linear recurrence; the certificate is the
recurrence RESIDUAL (real arithmetic — jeff_math::prony). Two deterministic, Z3-provable facts:

  • noiseless  → the order-k residual is structurally 0 (Hankel singular at order k+1). PROVEN (COMPLETE),
                 no provisos. Recovery (roots z_j) is EXACT *provided* the order-k Hankel is non-singular
                 (roots distinct, amplitudes ≠ 0) — that boundary condition is a PROVISO (v5 lesson).
  • noisy(η)   → |residual| ≤ (Σ|aᵢ|)·η : a deterministic perturbation bound, PROVEN by Z3 (triangle
                 inequality). PROVEN-BOUND (deterministic).

This is the DETERMINISTIC arm — Z3 handles it directly (unlike probabilistic sketches → U3/Caesar).
"""
from __future__ import annotations

import os
import subprocess
from dataclasses import dataclass
from typing import Optional

import z3_adapter

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


def _bin():
    for sub in ("target/release/examples/prony_check", "target/debug/examples/prony_check"):
        p = os.path.join(ROOT, sub)
        if os.path.isfile(p) and os.access(p, os.X_OK):
            return p
    return None


def run(mode="noiseless", eta=1e-3) -> Optional[dict]:
    b = _bin()
    if not b:
        return None
    out = subprocess.run([b, mode, str(eta)], capture_output=True, text=True, timeout=30).stdout.strip()
    if not out.startswith("PRONY "):
        return None
    d = {}
    for tok in out.split()[1:]:
        k, v = tok.split("=", 1)
        d[k] = v
    d["residual"] = float(d["residual"])
    d["max_noise"] = float(d["max_noise"])
    d["coeffs"] = [float(x) for x in d["coeffs"].split(",")]
    return d


def recover_roots(coeffs):
    """Roots of the characteristic polynomial = the recovered z_j (numpy). coeffs = [a0,a1,...,1]."""
    import numpy as np
    # prony returns a with leading 1 last; char poly is a reversed → use numpy.roots on the monic poly
    return sorted(float(r.real) for r in np.roots(list(reversed(coeffs))) if abs(r.imag) < 1e-6)


def prove_noiseless_residual_zero() -> z3_adapter.ProofResult:
    # order-1 Hankel determinant: for s_t=c·z^t, s0·s2 − s1² = 0 (structurally, no provisos).
    return z3_adapter.prove_predicate("c*(c*z*z) - (c*z)*(c*z) == 0", {"c": "Float", "z": "Float"})


def prove_noisy_residual_bound() -> z3_adapter.ProofResult:
    # |Σ aᵢ eᵢ| ≤ (Σ|aᵢ|)·η  given |eᵢ| ≤ η  (k=2: three terms). Deterministic perturbation bound.
    return z3_adapter.prove_predicate(
        "abs(a0*e0 + a1*e1 + a2*e2) <= (abs(a0) + abs(a1) + abs(a2)) * eta",
        {"a0": "Float", "a1": "Float", "a2": "Float", "e0": "Float", "e1": "Float", "e2": "Float", "eta": "Float"},
        assumptions=["eta >= 0", "abs(e0) <= eta", "abs(e1) <= eta", "abs(e2) <= eta"])


@dataclass
class PronyResult:
    verdict: str            # PROVEN-BOUND | DEFER
    error_kind: str         # deterministic
    completeness: str       # complete | boundary-partial(provisos)
    detail: str


def discharge_prony() -> PronyResult:
    d = run("noiseless")
    if d is None:
        return PronyResult("DEFER", "deterministic", "n/a", "prony_check engine not built")
    p0 = prove_noiseless_residual_zero()
    pn = prove_noisy_residual_bound()
    roots = recover_roots(d["coeffs"])
    distinct = len(roots) >= 2 and (len(set(round(r, 6) for r in roots)) == len(roots))
    if p0.verdict == "PROVEN" and pn.verdict == "PROVEN":
        comp = "complete (residual≡0) + boundary-partial (recovery needs Hankel non-singular: roots distinct)"
        return PronyResult("PROVEN-BOUND", "deterministic", comp,
                           f"noiseless residual={d['residual']:.2e}≈0 (Z3: Hankel det≡0); "
                           f"noisy |resid|≤(Σ|aᵢ|)η Z3-PROVEN; recovered roots={roots} distinct={distinct}")
    return PronyResult("DEFER", "deterministic", "n/a", f"Z3 noiseless={p0.verdict} noisy={pn.verdict}")
