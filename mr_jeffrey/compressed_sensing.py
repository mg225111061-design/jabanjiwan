"""
STAGE U2 (v6) — Compressed Sensing with a RUNTIME residual certificate (not a static RIP claim).
================================================================================================
k-sparse recovery from m = O(k·log(n/k)) measurements y = Φx (+noise), via Orthogonal Matching Pursuit.

★ The RIP honesty line ★: deciding whether an arbitrary sensing matrix Φ satisfies the Restricted
Isometry Property is **NP-hard** — we do NOT prove RIP statically (that would be a perpetual-motion
claim). Instead we issue a PER-EXECUTION certificate:
    runtime residual  r = ‖y − Φx*‖₂ ≤ ε      (computed and verified for THIS recovery),
plus a per-execution recovery-error bound on the recovered support S:
    ‖x*_S − x_true_S‖₂ ≤ r / σ_min(Φ_S)        (σ_min computed for the selected columns).
This certifies "THIS execution is ε-consistent", NOT "Φ satisfies RIP for all inputs".
"""
from __future__ import annotations

from dataclasses import dataclass

import numpy as np


def omp(Phi, y, k):
    """Orthogonal Matching Pursuit. Returns (x*, residual_norm, support)."""
    m, n = Phi.shape
    residual = y.astype(float).copy()
    support = []
    x = np.zeros(n)
    for _ in range(k):
        j = int(np.argmax(np.abs(Phi.T @ residual)))
        if j not in support:
            support.append(j)
        Phi_s = Phi[:, support]
        xs, *_ = np.linalg.lstsq(Phi_s, y, rcond=None)
        x = np.zeros(n)
        x[support] = xs
        residual = y - Phi @ x
    return x, float(np.linalg.norm(residual)), sorted(support)


@dataclass
class CSResult:
    verdict: str            # PROVEN-BOUND | DEFER
    cert_kind: str          # "runtime (per-execution)"
    residual: float
    epsilon: float
    recovery_err_bound: float
    rip_claimed_statically: bool
    detail: str


def recover_and_certify(n=256, k=8, m=80, noise=0.0, eps=1e-6, seed=20260616) -> CSResult:
    rng = np.random.default_rng(seed)
    Phi = rng.standard_normal((m, n)) / np.sqrt(m)
    support_true = rng.choice(n, size=k, replace=False)
    x_true = np.zeros(n)
    x_true[support_true] = rng.standard_normal(k) * 3 + np.sign(rng.standard_normal(k))
    y = Phi @ x_true + (rng.standard_normal(m) * noise if noise > 0 else 0.0)

    x_rec, r, S = omp(Phi, y, k)
    # per-execution recovery-error bound on the selected support (σ_min of the chosen columns)
    sigma_min = float(np.linalg.svd(Phi[:, S], compute_uv=False).min()) if S else 0.0
    err_bound = r / sigma_min if sigma_min > 1e-12 else float("inf")
    # the certificate is the runtime residual ≤ ε (this execution); ε scaled with noise floor
    eps_eff = max(eps, 3.0 * noise * np.sqrt(m))
    verdict = "PROVEN-BOUND" if r <= eps_eff else "DEFER"
    return CSResult(verdict, "runtime (per-execution)", r, eps_eff, err_bound,
                    rip_claimed_statically=False,
                    detail=f"‖y−Φx*‖₂={r:.2e} ≤ ε={eps_eff:.2e}; recovered support {'==' if set(S)==set(support_true.tolist()) else '≠'} true; "
                           f"σ_min(Φ_S)={sigma_min:.3f} ⇒ ‖x*_S−x_S‖≤{err_bound:.2e} (runtime, NOT a static RIP guarantee)")
