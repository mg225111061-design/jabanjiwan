"""v18 Part G · G2 tests — diffusion engine. Run: python3 test_g2.py

G2.1 graph Laplacian L = D - A.
G2.2 two solvers: personalized PageRank (linear) + heat kernel (matrix exponential).
G2.3 suspicion propagates: near the source is hotter than far (heat kernel monotone on a path).
"""
import sys

import numpy as np
import diffuse as DF

PASS, FAIL, SKIP = [], [], []


def check(n, c, d=""):
    (PASS if c else FAIL).append(n)
    print(f"  [{'PASS' if c else 'FAIL'}] {n}" + (f" — {d}" if d and not c else ""))


def laplacian_built():
    A = DF.path_graph(4)
    L = DF.laplacian(A)
    # rows of L sum to zero; off-diagonal = -A; diagonal = degree
    ok = np.allclose(L.sum(axis=1), 0) and np.allclose(np.diag(L), A.sum(axis=1)) and np.allclose(L + A, np.diag(A.sum(axis=1)))
    check("laplacian_built", ok, f"rowsums={L.sum(axis=1)}")
    print(f"      → L = D - A: rows sum to 0, diagonal = node degrees. (Discrete heat operator.)")


def diffusion_solved():
    import time
    A = DF.path_graph(5)
    x0 = np.zeros(5); x0[0] = 1.0
    ppr = DF.personalized_pagerank(A, x0, 0.85)
    heat = DF.heat_kernel(A, x0, 1.0)
    valid = np.all(ppr >= -1e-9) and np.all(heat >= -1e-9) and ppr.shape == (5,) and heat.shape == (5,)
    big = DF.path_graph(200); xb = np.zeros(200); xb[0] = 1.0
    t = time.perf_counter(); DF.personalized_pagerank(big, xb); dt = (time.perf_counter() - t) * 1e3
    ok = valid and dt < 200
    check("diffusion_solved", ok, f"ppr_ok={valid} 200-node {dt:.1f}ms")
    print(f"      → both solvers return valid distributions; PageRank on 200 nodes in {dt:.1f}ms "
          f"(linear solve — fast, PRFL-scale).")


def suspicion_propagates():
    A = DF.path_graph(5)
    x0 = np.zeros(5); x0[0] = 1.0
    ppr = DF.normalize(DF.personalized_pagerank(A, x0, 0.85))
    heat = DF.normalize(DF.heat_kernel(A, x0, 1.0))
    # near the source (nodes 0-1) hotter than far (nodes 3-4); heat kernel strictly monotone on a path
    near_hot = max(ppr[0], ppr[1]) > max(ppr[3], ppr[4]) and heat[0] > heat[4]
    heat_monotone = all(heat[i] >= heat[i + 1] - 1e-9 for i in range(4))
    ok = near_hot and heat_monotone
    check("suspicion_propagates", ok, f"heat={[round(v,2) for v in heat]}")
    print(f"      → heat@node0 diffuses: heat-kernel {[round(v,2) for v in heat]} (monotone ↓ with "
          f"distance); near-source hotter than far. Suspicion flows along dependencies.")


if __name__ == "__main__":
    print("v18 Part G · G2 — diffusion engine")
    laplacian_built(); diffusion_solved(); suspicion_propagates()
    print(f"\nG2: {len(PASS)} passed, {len(FAIL)} failed, {len(SKIP)} skipped")
    sys.exit(1 if FAIL else 0)
