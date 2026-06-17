"""v18 Part G · G4 tests — solve diffusion + rank. Run: python3 test_g4.py

G4.1 solve with the G3 boundary conditions → steady-state heat.
G4.2 hot statements = suspects → top-k.
G4.3 demo: sinks exclude proven-safe lines from the ranking (the v18 hypothesis, measured for real in G5).
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


def diffusion_ranking():
    ranked = PD.rank_lines(_B, use_sinks=True)
    top2 = {ln for ln, _ in ranked[:2]}
    ok = ranked[0][1] > 0 and top2 <= {5, 6} and 5 in top2   # the bug region (compare/swap) ranks top
    check("diffusion_ranking", ok, f"top2={[(l,round(h,2)) for l,h in ranked[:2]]}")
    print(f"      → steady-state heat ranks the bug region top: {[(l,round(h,2)) for l,h in ranked[:3]]}.")


def sink_excludes_proven_safe():
    with_sinks = dict(PD.rank_lines(_B, use_sinks=True))
    no_sinks = dict(PD.rank_lines(_B, use_sinks=False))
    # proven-safe lines (2,3,4,7) are 0 WITH sinks but nonzero in pure diffusion (not excluded)
    excluded = all(with_sinks[ln] == 0.0 for ln in (2, 3, 4, 7))
    pure_keeps = any(no_sinks[ln] > 0 for ln in (2, 4))
    ok = excluded and pure_keeps
    check("sink_excludes_proven_safe", ok, f"with_sinks(L4)={with_sinks[4]:.2f} pure(L4)={no_sinks[4]:.2f}")
    print(f"      → WITH proof sinks, the safe lines L2/L3/L4/L7 → 0 (drained/absorbed); pure PRFL keeps "
          f"them hot (L4={no_sinks[4]:.2f}). This is the v18 mechanism — does it help? G5 measures.")


def topk_demo():
    ranked = PD.rank_lines(_B, use_sinks=True)
    top5_lines = [ln for ln, _ in ranked[:5]]
    bug_in_top5 = 5 in top5_lines           # the comparison bug (L5)
    nonzero = [(ln, round(h, 2)) for ln, h in ranked if h > 0]
    ok = bug_in_top5
    check("topk_demo", ok, f"nonzero={nonzero}")
    print(f"      → top-5 lines: {top5_lines}; bug line L5 in top-5 = {bug_in_top5}. "
          f"Only {len(nonzero)} lines survive the sinks: {nonzero}.")
    print("        NOTE: diffusion ranks L6 (swap, data-coupled) at/above L5 (the comparison) — a real "
          "nuance; whether this beats property-only top-1 is decided by G5, not assumed.")


if __name__ == "__main__":
    print("v18 Part G · G4 — solve diffusion + rank")
    diffusion_ranking(); sink_excludes_proven_safe(); topk_demo()
    print(f"\nG4: {len(PASS)} passed, {len(FAIL)} failed, {len(SKIP)} skipped")
    sys.exit(1 if FAIL else 0)
