"""v18 Part G · G1 tests — PDG construction. Run: python3 test_g1.py

G1.1/.2 PDG from HIR: nodes = statements, edges = data (def→use) + control (header→body). Python + C.
G1.3 graph representation (adjacency); other languages DEFER honestly.
"""
import sys

import pdg

PASS, FAIL, SKIP = [], [], []


def check(n, c, d=""):
    (PASS if c else FAIL).append(n)
    print(f"  [{'PASS' if c else 'FAIL'}] {n}" + (f" — {d}" if d and not c else ""))


BUBBLE = ("def bubble(xs):\n    a = list(xs)\n    for i in range(len(a)):\n"
          "        for j in range(len(a)-1):\n            if a[j] > a[j+1]:\n"
          "                a[j], a[j+1] = a[j+1], a[j]\n    return a\n")


def pdg_built_from_hir():
    g = pdg.build_pdg(BUBBLE, "b.py")
    kinds = {nd.kind for nd in g.nodes}
    has_data = any(k == "data" for _, _, k in g.edges)
    has_control = any(k == "control" for _, _, k in g.edges)
    ok = g.lang == "python" and g.n() == 6 and {"assign", "for", "if", "return"} <= kinds and has_data and has_control
    check("pdg_built_from_hir", ok, f"nodes={g.n()} edges={len(g.edges)} kinds={sorted(kinds)}")
    print(f"      → bubble → PDG: {g.n()} statement-nodes, {len(g.edges)} edges (data def→use + control "
          f"header→body). Suspicion will diffuse over this graph.")


def pdg_nodes_edges_correct():
    g = pdg.build_pdg(BUBBLE, "b.py")
    edge_lines = {(g.line_of(u), g.line_of(v), k) for u, v, k in g.edges}
    # control: the `if` (L5) controls the swap (L6); data: the swap (L6) feeds `return a` (L7)
    control_ok = (5, 6, "control") in edge_lines
    data_ok = (6, 7, "data") in edge_lines and (2, 3, "data") in edge_lines   # a defined L2 feeds the loop
    import numpy as np
    A = g.adjacency()
    symmetric = bool(np.allclose(A, A.T)) and A.shape == (6, 6)
    ok = control_ok and data_ok and symmetric
    check("pdg_nodes_edges_correct", ok, f"control(5→6)={control_ok} data(6→7)={data_ok} sym={symmetric}")
    print(f"      → control edge if@L5 → swap@L6; data edge swap@L6 → return@L7; adjacency 6×6 symmetric.")


def c_best_effort_others_defer():
    gc = pdg.build_pdg("int f(int n){int s=0;int i;for(i=1;i<=n;i=i+1){s=s+i;}return s;}", "f.c")
    go = pdg.build_pdg("func f(a []int){}", "x.go")
    ok = gc.lang == "c" and gc.n() >= 3 and go.lang == "go" and go.n() == 0   # C best-effort, Go DEFER
    check("c_best_effort_others_defer", ok, f"C nodes={gc.n()} Go nodes={go.n()}")
    print(f"      → C PDG best-effort ({gc.n()} nodes via pycparser); Go/Rust/JS/TS → DEFER (need their "
          f"own def-use extractor; precise PDGs need alias/pointer analysis we don't claim).")


if __name__ == "__main__":
    print("v18 Part G · G1 — PDG construction")
    pdg_built_from_hir(); pdg_nodes_edges_correct(); c_best_effort_others_defer()
    print(f"\nG1: {len(PASS)} passed, {len(FAIL)} failed, {len(SKIP)} skipped")
    sys.exit(1 if FAIL else 0)
