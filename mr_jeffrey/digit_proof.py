"""
HARAN v16 Part B · STAGE B6 — Caesar digit interrogation (heart 3: the overconfidence moat).
============================================================================================
B5 produced TENTATIVE digits (a raw posterior that may multiply correlated properties). B6 refuses to
let any digit be claimed unless it is PROVABLE:

  B6.1 Hoeffding / rule-of-three — with N samples you can only justify a rate down to ≈ -ln(δ)/N. To
       claim 10⁻ᵏ you need N ≥ -ln(δ)·10ᵏ samples. If N is short, the digit is DISCOUNTED to what N
       supports (e.g. "≤10⁻³, sample-limited"), never the optimistic claim.
  B6.2 independence — measure how correlated the multiplied properties actually are (overlap of the
       inputs they fail on). Correlated ⇒ the product overcounts ⇒ collapse to the effective independent
       factor count (e.g. raw 16⁴ but 2 independent groups ⇒ 16²).
  B6.3 Caesar/HeyVL — a real PROVEN-BOUND stamp on the probabilistic model (expectation/first moment;
       the (ε,δ) tail stays DEFERRED, as the v6.5 bridge already documents). Absent ⇒ Hoeffding only.
  B6.4 certificate — "op X: P(bug outside top-k) ≤ <proven bound> (sample N, k independent signals,
       Hoeffding[/Caesar])". The PROVEN bound, with the honest discount and the ~10⁻⁶ practical ceiling.

★ digits are claimed ONLY when proven. No infinite digits — that is the sample-ceiling mirage. ★
"""
from __future__ import annotations

import math
from dataclasses import dataclass, field
from typing import Dict, List, Optional

import caesar_bridge

PRACTICAL_FLOOR = 1e-6      # below this, sampling needs astronomical N — honestly out of reach


# ----------------------------------------------------------------- B6.1 Hoeffding / rule-of-three
def hoeffding_eps(n: int, delta: float = 0.05) -> float:
    """Two-sided Hoeffding error on a [0,1] mean from n samples at confidence 1-δ."""
    return math.sqrt(math.log(2.0 / delta) / (2.0 * n)) if n > 0 else 1.0


def rule_of_three_upper(n: int, delta: float = 0.05) -> float:
    """One-sided (1-δ) upper bound on a rate observed ZERO times in n trials: -ln(δ)/n."""
    return min(1.0, -math.log(delta) / n) if n > 0 else 1.0


def samples_needed_for(target: float, delta: float = 0.05) -> int:
    return math.ceil(-math.log(delta) / target) if target > 0 else 10 ** 18


@dataclass
class HoeffdingCheck:
    n: int
    claimed: float          # the tentative digit B5 wants to claim
    provable: float         # the smallest rate the sample actually supports (rule of three)
    sufficient: bool        # is the claim within what N supports?
    needed_n: int


def hoeffding_sample_check(claimed: float, n: int, delta: float = 0.05) -> HoeffdingCheck:
    provable = rule_of_three_upper(n, delta)
    sufficient = claimed >= provable            # claim is honest only if it's ≥ what N can prove
    return HoeffdingCheck(n, claimed, provable, sufficient, samples_needed_for(claimed, delta))


# ----------------------------------------------------------------- B6.2 independence discount
def _jaccard(a: set, b: set) -> float:
    if not a and not b:
        return 1.0
    u = len(a | b)
    return len(a & b) / u if u else 0.0


@dataclass
class IndependenceResult:
    raw_factors: int
    effective_independent: int
    correlated_pairs: List[tuple]
    note: str


def independence_discount(violation_inputs: Dict[str, List[list]], thresh: float = 0.5) -> IndependenceResult:
    """Group violated properties by how much they fail on the SAME inputs; effective independent count
    = number of groups. Highly-overlapping properties (Jaccard ≥ thresh) are ONE factor, not many."""
    names = [n for n, v in violation_inputs.items() if v]
    sets = {n: {tuple(x) for x in violation_inputs[n]} for n in names}
    parent = {n: n for n in names}

    def find(x):
        while parent[x] != x:
            parent[x] = parent[parent[x]]
            x = parent[x]
        return x

    pairs = []
    for i in range(len(names)):
        for j in range(i + 1, len(names)):
            a, b = names[i], names[j]
            if _jaccard(sets[a], sets[b]) >= thresh:
                parent[find(a)] = find(b)
                pairs.append((a, b))
    groups = len({find(n) for n in names})
    note = ("correlated properties (same failing inputs) collapsed to one factor — the product is not "
            "multiplied independently across them (prevents fake extra digits).")
    return IndependenceResult(len(names), max(1, groups), pairs, note if pairs else "no strong correlation found")


# ----------------------------------------------------------------- B6.3 Caesar bound
@dataclass
class CaesarBound:
    available: bool
    verdict: str
    detail: str


def caesar_digit_bound() -> CaesarBound:
    if not caesar_bridge.caesar_available():
        return CaesarBound(False, "BLOCKED", "Caesar not installed → Hoeffding-only (honest).")
    r = caesar_bridge.verify_probabilistic("kmv")
    return CaesarBound(True, r.verdict, r.detail)


# ----------------------------------------------------------------- B6.4 certificate
@dataclass
class DigitCertificate:
    op: str
    tentative: float
    proven_upper: float
    n_samples: int
    effective_independent: int
    method: str
    caesar_verdict: str
    discounted: bool
    note: str

    def render(self) -> str:
        return (f"CERT op '{self.op}' innocent: P(bug here / outside top-k) ≤ {self.proven_upper:.2e}  "
                f"[sample N={self.n_samples}, {self.effective_independent} independent signal(s), "
                f"{self.method}, Caesar={self.caesar_verdict}]"
                + (f"  ⟵ DISCOUNTED from tentative {self.tentative:.2e} (sample-limited)" if self.discounted else ""))


def digit_certificate(op: str, tentative: float, n_samples: int,
                      violation_inputs: Dict[str, List[list]], delta: float = 0.05) -> DigitCertificate:
    hc = hoeffding_sample_check(tentative, n_samples, delta)
    ind = independence_discount(violation_inputs)
    cb = caesar_digit_bound()
    # the PROVEN bound is what the sample supports (rule of three), never smaller than the floor we can
    # actually reach; if the tentative claim was more optimistic than N allows, we DISCOUNT up to provable.
    proven = max(hc.provable, PRACTICAL_FLOOR if tentative < PRACTICAL_FLOOR else tentative)
    proven = max(proven, hc.provable)
    discounted = proven > tentative + 1e-18
    method = "Hoeffding+Caesar" if (cb.available and cb.verdict.startswith("PROVEN")) else "Hoeffding"
    note = (f"to claim {tentative:.0e} needs N≥{hc.needed_n}; have N={n_samples} ⇒ provable ≤ "
            f"{hc.provable:.2e}. Practical floor {PRACTICAL_FLOOR:.0e} (below = astronomical samples).")
    return DigitCertificate(op, tentative, proven, n_samples, ind.effective_independent,
                            method, cb.verdict, discounted, note)
