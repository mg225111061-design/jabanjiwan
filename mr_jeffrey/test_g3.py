"""v18 Part G · G3 tests — proof boundary conditions (the novelty). Run: python3 test_g3.py

G3.1 Bayesian posterior as initial heat.  G3.2 property violations as heat source.
G3.3 PROVEN-SAFE lines as heat sinks (Dirichlet 0, absorbing) — the v18 novelty.
G3.4 Hoeffding confidence as conductance (strong proof → strong insulation).
"""
import sys

import proof_diffuse as PD

PASS, FAIL, SKIP = [], [], []


def check(n, c, d=""):
    (PASS if c else FAIL).append(n)
    print(f"  [{'PASS' if c else 'FAIL'}] {n}" + (f" — {d}" if d and not c else ""))


BUG = ("def bubble(xs):\n    a = list(xs)\n    for i in range(len(a)):\n"
       "        for j in range(len(a)-1):\n            if a[j] < a[j+1]:\n"
       "                a[j], a[j+1] = a[j+1], a[j]\n    return a\n")
_B = PD.build_boundary(BUG, "b.py")


def _line(nid):
    return _B.graph.line_of(nid)


def bayesian_as_initial_heat():
    # hottest initial heat at the suspect lines (the if @ L5 / swap @ L6), cool at structural lines
    hot = {_line(i) for i in range(_B.graph.n()) if _B.x0[i] >= 0.4}
    cool = {_line(i) for i in range(_B.graph.n()) if _B.x0[i] < 0.1}
    ok = 5 in hot and 6 in hot and 2 in cool
    check("bayesian_as_initial_heat", ok, f"hot={hot} cool={cool}")
    print(f"      → initial heat = Bayesian posterior (B5): hottest at L5/L6 (the suspects), cool at the "
          f"list-copy/loops. The diffusion starts from B's verdict.")


def violation_as_source():
    src_lines = {_line(i) for i in range(_B.graph.n()) if _B.source[i] > 0}
    ok = src_lines == {5, 6} and _B.source[_B.graph.node_at_line(5)] >= 16
    check("violation_as_source", ok, f"source_lines={src_lines}")
    print(f"      → heat source = property-violation lines (B4): L5/L6 injected with likelihood-ratio "
          f"weight (16×); everywhere else 0.")


def proof_as_sink():
    sink_lines = {_line(i) for i in _B.sinks}
    bug_not_sink = _B.graph.node_at_line(5) not in _B.sinks and _B.graph.node_at_line(6) not in _B.sinks
    # the safe structural lines (list copy, loop headers, return) are sinks; the bug lines are NOT
    ok = {2, 3, 4, 7} <= sink_lines and bug_not_sink
    check("proof_as_sink", ok, f"sinks={sink_lines} bug_excluded={bug_not_sink}")
    print(f"      → SINKS (proven-safe, NOVEL) = {sorted(sink_lines)} (abstract-interp-safe + not "
          f"implicated); the bug lines L5/L6 are NOT sinks. Heat drains into the safe lines. "
          f"HONEST: abstract-interp = crash-safety only ⇒ WEAK sink (strength 0.7); Z3/Coq = STRONG.")


def confidence_conductance():
    # sink nodes have reduced conductance (insulation); a Z3/Coq proof upgrades to strong insulation
    weak_insulated = all(_B.conductance[i] <= 0.71 for i in _B.sinks)
    b2 = PD.build_boundary(BUG, "b.py")
    PD.add_proof_sinks(b2, [2], strength=1.0)            # pretend L2 is Z3-proven safe → strong sink
    n2 = b2.graph.node_at_line(2)
    strong = b2.sink_strength[n2] == 1.0 and b2.conductance[n2] < 0.1
    ok = weak_insulated and strong
    check("confidence_conductance", ok, f"weak<=0.71={weak_insulated} strong_proof_insulates={strong}")
    print(f"      → conductance = confidence: weak (abstract-interp) sinks insulate at 0.7; a Z3/Coq "
          f"proof (add_proof_sinks) upgrades to strength 1.0 + strong insulation (<0.1). Hoeffding (B6) "
          f"governs how much a safe node blocks heat.")


if __name__ == "__main__":
    print("v18 Part G · G3 — proof boundary conditions")
    bayesian_as_initial_heat(); violation_as_source(); proof_as_sink(); confidence_conductance()
    print(f"\nG3: {len(PASS)} passed, {len(FAIL)} failed, {len(SKIP)} skipped")
    sys.exit(1 if FAIL else 0)
