"""
HARAN v18 Part G · STAGE G2 — diffusion engine (graph Laplacian / PageRank / heat kernel).
==========================================================================================
Suspicion spreads over the PDG exactly as heat over a graph — this is the GRAPH LAPLACIAN / random-walk
formulation (mathematically identical to the discrete heat equation), the one PRFL measured on Defects4J.
NOT a fluid/physics metaphor.

Two solvers (both linear, fast):
  • personalized PageRank   x = (1-α)·(I - α·P)⁻¹·x₀        (P = column-stochastic transition)
  • heat kernel             x(t) = exp(-t·L)·x₀             (L = D - A graph Laplacian)
"""
from __future__ import annotations

import numpy as np
from scipy.linalg import expm, solve


def laplacian(A: np.ndarray) -> np.ndarray:
    D = np.diag(A.sum(axis=1))
    return D - A


def transition(A: np.ndarray) -> np.ndarray:
    """Column-stochastic transition matrix (random walk on the graph)."""
    col = A.sum(axis=0).astype(float)
    col[col == 0] = 1.0
    return A / col


def personalized_pagerank(A: np.ndarray, x0: np.ndarray, alpha: float = 0.85) -> np.ndarray:
    n = A.shape[0]
    s = x0.sum()
    v = x0 / s if s > 0 else np.ones(n) / n
    P = transition(A)
    x = (1.0 - alpha) * solve(np.eye(n) - alpha * P, v)
    return x


def heat_kernel(A: np.ndarray, x0: np.ndarray, t: float = 1.0) -> np.ndarray:
    L = laplacian(A)
    return expm(-t * L) @ x0


def normalize(x: np.ndarray) -> np.ndarray:
    m = x.max()
    return x / m if m > 0 else x


# small helper: a path-graph adjacency (for verification)
def path_graph(n: int) -> np.ndarray:
    A = np.zeros((n, n))
    for i in range(n - 1):
        A[i, i + 1] = A[i + 1, i] = 1.0
    return A
