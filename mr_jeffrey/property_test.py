"""
HARAN v16 Part B · STAGE B3 — property testing (fast: run the code, don't read it).
==================================================================================
Each extracted relation is tested against thousands of inputs by RUNNING the function and inspecting
outputs only — no static analysis, so it is fast (the "뚝딱"). We record every violation (property +
the input that broke it) for the fault-mapping (B4) and minimal-counterexample (B8) stages.

Deterministic: the input generator is seeded, so the same code yields the same violations every run
(the determinism the whole pipeline promises).
"""
from __future__ import annotations

import random
import time
from dataclasses import dataclass, field
from typing import Callable, Dict, List

import properties as PR


# ----------------------------------------------------------------- input generation (seeded)
def gen_int_lists(n: int, seed: int = 0, max_len: int = 8, val: int = 20) -> List[list]:
    rng = random.Random(seed)
    edges = [[], [0], [1], [1, 1], [2, 1], [1, 2], [1, 2, 3], [3, 2, 1], [5, 5, 5], [-1, 0, 1]]
    out = list(edges)
    for _ in range(n):
        L = rng.randint(0, max_len)
        out.append([rng.randint(-val, val) for _ in range(L)])
    return out


def gen_int_scalars(n: int, seed: int = 0, val: int = 50) -> List[int]:
    rng = random.Random(seed)
    edges = [0, 1, 2, 3, 5, 10, -1, -3]
    return edges + [rng.randint(-val, val) for _ in range(n)]


def infer_domain(hfn) -> str:
    """Probe whether the function takes a LIST or a SCALAR, so we don't feed lists to a scalar function
    (which would crash and be miscounted as a violation — a false positive)."""
    import properties as PR
    try:
        fn = PR.compile_callable(hfn)
    except Exception:
        return "list"
    try:
        fn([1, 2, 3])
        return "list"
    except Exception:
        try:
            fn(3)
            return "scalar"
        except Exception:
            return "list"


def gen_inputs(hfn, n: int = 400, seed: int = 0) -> List:
    """Domain-aware input generation: scalars for scalar functions, int-lists for list functions."""
    return gen_int_scalars(n, seed) if infer_domain(hfn) == "scalar" else gen_int_lists(n, seed)


# ----------------------------------------------------------------- testing
@dataclass
class TestReport:
    violations: Dict[str, List[list]]   # property name -> failing inputs
    tested_inputs: int
    properties: int
    elapsed_s: float
    crashes: int = 0

    def violated_properties(self) -> List[str]:
        return [p for p, v in self.violations.items() if v]

    def held_properties(self) -> List[str]:
        return [p for p, v in self.violations.items() if not v]

    @property
    def checks(self):
        return self.tested_inputs * self.properties

    def checks_per_sec(self):
        return self.checks / self.elapsed_s if self.elapsed_s > 0 else float("inf")


def test_properties(fn: Callable, props: List[PR.Property], inputs: List[list]) -> TestReport:
    violations: Dict[str, List[list]] = {p.name: [] for p in props}
    crashes = 0
    t = time.perf_counter()
    for x in inputs:
        for p in props:
            try:
                ok = p.check(fn, x)
            except Exception:
                ok = False           # a crash on a metamorphic check is itself a violation
                crashes += 1
            if not ok:
                violations[p.name].append(x)
    dt = time.perf_counter() - t
    return TestReport(violations, len(inputs), len(props), dt, crashes)


def run(hfn, n_random: int = 2000, seed: int = 0) -> TestReport:
    """Convenience: compile the HIR function, extract its properties, test on edges + n random inputs."""
    fn = PR.compile_callable(hfn)
    props = PR.extract_properties(hfn)
    inputs = gen_int_lists(n_random, seed=seed)
    return test_properties(fn, props, inputs)
