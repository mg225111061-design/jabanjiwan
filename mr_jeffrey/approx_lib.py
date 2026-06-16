"""
STAGE X2 — unstructured conquest: approximation + PROVEN/TESTED error (the v3 lifeline).
=======================================================================================
v2 isolated NO_STRUCTURE/ABSENT work (count, quantile over arbitrary data). Here we *conquer* some of
it by approximation — but "conquest" counts ONLY when the error is PROVEN (PROVEN-BOUND). If the
error is merely measured on random inputs it is TESTED-BOUND — structured, but NOT conquered. The two
are never mixed (the v3 honesty line; same discipline that kept count_primes honest in v2).

  PROVEN-BOUND   deterministic approximation whose |approx − exact| ≤ ε is PROVEN ∀ (Z3, X1).
                 e.g. bucketed quantile: any x in a width-w bucket is within w/2 of the midpoint.
  TESTED-BOUND   probabilistic sketch; error measured empirically / statistical only.
                 e.g. KMV distinct-count: ~1.04/√k expected — adversarial input can break it.
  REJECTED-EXACT exact required (payment/crypto/exact-search) → approximation REFUSED, keep Ω(N).
  NO_STRUCTURE   approximation not authorized / not cleanly approximable → unstructured kept.

Speed honesty: SKETCHES legitimately change complexity (sublinear SPACE), stated with the error
caveat. The EXACT unstructured fallback claims only Ω(N) / constant-factor — never orders of magnitude.
"""
from __future__ import annotations

import hashlib
import random
from dataclasses import dataclass
from typing import Callable, List, Optional

import z3_adapter


@dataclass
class ConquestResult:
    op: str
    kind: str            # PROVEN-BOUND | TESTED-BOUND | REJECTED-EXACT | NO_STRUCTURE
    epsilon: str
    proof: str
    complexity: str
    evidence: Optional[float] = None

    def conquered(self) -> bool:
        return self.kind == "PROVEN-BOUND"

    def __str__(self):
        return f"{self.op}: {self.kind}  ε={self.epsilon}  [{self.complexity}]  ({self.proof})"


# --------------------------------------------------------- deterministic: bucketed quantile (PROVEN-BOUND)
def bucketed_quantile(values: List[float], q: float, lo: float, hi: float, m: int) -> float:
    """Approximate the q-quantile by m fixed buckets; report the containing bucket's midpoint.
    O(n) time, O(m) space (vs O(n log n) exact sort)."""
    w = (hi - lo) / m
    counts = [0] * m
    for v in values:
        i = 0 if v < lo else (m - 1 if v >= hi else int((v - lo) / w))
        counts[i] += 1
    target = q * len(values)
    cum = 0
    for i in range(m):
        cum += counts[i]
        if cum >= target:
            return lo + (i + 0.5) * w
    return hi - w / 2


def approx_quantile_conquest() -> ConquestResult:
    # the worst-case error is the bucket-midpoint lemma — PROVEN ∀ by Z3 (X1).
    r = z3_adapter.prove_predicate(
        "abs(m - x) <= w / 2",
        {"m": "Float", "x": "Float", "w": "Float", "blo": "Float"},
        assumptions=["blo <= x", "x <= blo + w", "m = blo + w / 2"])
    if r.verdict == "PROVEN":
        return ConquestResult("approx_quantile (bucketing)", "PROVEN-BOUND", "w/2 = range/(2m)",
                              "Z3 ∀-proof: x in a width-w bucket ⇒ |midpoint − x| ≤ w/2",
                              "O(n) time, O(m) space vs O(n log n) sort — PROVEN error")
    return ConquestResult("approx_quantile (bucketing)", "TESTED-BOUND", "w/2 (unproven)",
                          f"Z3 did not prove the bound ({r.verdict})", "O(n)/O(m)")


# --------------------------------------------------------- probabilistic: KMV distinct count (TESTED-BOUND)
def kmv_distinct(items, k: int = 1024) -> float:
    """Bottom-k (KMV) distinct-count sketch. O(n) time, O(k) space (vs O(distinct) exact set)."""
    hs = sorted({int(hashlib.blake2b(str(x).encode(), digest_size=8).hexdigest(), 16) / 2 ** 64
                 for x in items})
    if len(hs) <= k:
        return float(len(hs))            # exact when fewer than k distinct
    return (k - 1) / hs[k - 1]


def approx_distinct_conquest(k: int = 1024, trials: int = 40, seed: int = 20260616) -> ConquestResult:
    rng = random.Random(seed)
    max_rel = 0.0
    for _ in range(trials):
        d = rng.randint(2000, 8000)
        items = list(range(d)) + [rng.randrange(d) for _ in range(d * 2)]
        est = kmv_distinct(items, k)
        max_rel = max(max_rel, abs(est - d) / d)
    return ConquestResult("approx_distinct (KMV sketch)", "TESTED-BOUND", "~1.04/√k (statistical)",
                          f"TESTED: max relative error {max_rel:.3f} over {trials} random sets (k={k}); "
                          f"probabilistic — NOT a worst-case proof",
                          "O(n) time, O(k) space vs O(distinct) — TESTED error", evidence=max_rel)


# --------------------------------------------------------- routing (X2.3)
def route(op: str, approx_ok: bool, exact_required: bool, conquest_fn: Callable[[], ConquestResult]) -> ConquestResult:
    if exact_required:
        return ConquestResult(op, "REJECTED-EXACT", "0 (exact)",
                              "exact required (payment/crypto/exact-search) → approximation REFUSED",
                              "Ω(N) exact, constant-factor only — NOT conquered (honest)")
    if approx_ok:
        return conquest_fn()
    return ConquestResult(op, "NO_STRUCTURE", "—", "approximation not authorized → unstructured kept", "Ω(N)")


# --------------------------------------------------------- conquest ratio (X2.4)
@dataclass
class ConquestRatio:
    rows: List[ConquestResult]

    def pct(self, *kinds):
        return round(100 * sum(1 for r in self.rows if r.kind in kinds) / len(self.rows)) if self.rows else 0

    def conquest_pct(self):
        # of the items where approximation was attempted (not exact-rejected), how many are PROVEN-BOUND
        attempted = [r for r in self.rows if r.kind in ("PROVEN-BOUND", "TESTED-BOUND")]
        if not attempted:
            return 0
        return round(100 * sum(1 for r in attempted if r.kind == "PROVEN-BOUND") / len(attempted))


def conquest_ratio() -> ConquestRatio:
    rows = [
        route("quantile",  approx_ok=True,  exact_required=False, conquest_fn=approx_quantile_conquest),
        route("distinct",  approx_ok=True,  exact_required=False, conquest_fn=approx_distinct_conquest),
        route("payment",   approx_ok=False, exact_required=True,  conquest_fn=approx_quantile_conquest),
        route("count_primes", approx_ok=False, exact_required=False, conquest_fn=approx_quantile_conquest),
    ]
    return ConquestRatio(rows)


def render_ratio(r: ConquestRatio) -> str:
    lines = ["unstructured conquest:"]
    for row in r.rows:
        lines.append(f"   · {row}")
    lines.append(f"   ─ {len(r.rows)} ops: {r.pct('PROVEN-BOUND')}% PROVEN-BOUND(conquered), "
                 f"{r.pct('TESTED-BOUND')}% TESTED-BOUND(structured≠conquered), "
                 f"{r.pct('REJECTED-EXACT')}% REJECTED-EXACT, {r.pct('NO_STRUCTURE')}% NO_STRUCTURE")
    lines.append(f"   ─ ★ of approximated ops, {r.conquest_pct()}% are PROVEN-BOUND (truly conquered)")
    return "\n".join(lines)
