"""
STAGE U4 (v6) — approximation routing + exact-required rejection (the honesty line, generalized).
=================================================================================================
Not all unstructured work may be approximated. We route by domain:
  • analytics / monitoring / telemetry / statistics  → APPROX OK → U1 (Prony) / U2 (CS) / U3 (sketch)
  • payment / crypto / billing / ledger / exact-search → REJECTED-EXACT (must be exact; even 1¢ wrong = false)
  • unknown domain                                    → REJECTED-EXACT (conservative: approximate only when authorized)

Generalizes v3's `payment → REJECTED-EXACT`. The reason is always stated.
"""
from __future__ import annotations

from dataclasses import dataclass

EXACT_DOMAINS = {"payment", "crypto", "billing", "ledger", "balance", "auth", "exact_search", "accounting"}
APPROX_OK_DOMAINS = {"analytics", "monitoring", "telemetry", "statistics", "dashboard",
                     "estimate", "stream", "signal", "spectral", "sensing"}

# which engine handles an approx-OK task (by nature of the work)
ENGINE = {
    "spectral": "U1 Prony (deterministic, Z3)",
    "signal": "U1 Prony (deterministic, Z3)",
    "sensing": "U2 Compressed Sensing (runtime residual)",
    "statistics": "U3 sketch (probabilistic, Caesar/TESTED)",
    "monitoring": "U3 sketch (probabilistic, Caesar/TESTED)",
    "telemetry": "U3 sketch (probabilistic, Caesar/TESTED)",
}


@dataclass
class RouteResult:
    task: str
    domain: str
    decision: str           # APPROX | REJECTED-EXACT
    engine: str
    reason: str

    def __str__(self):
        return f"{self.task} [{self.domain}] → {self.decision}" + (f" via {self.engine}" if self.engine else "") + f"  ({self.reason})"


def route(task: str, domain: str, approx_authorized: bool = False) -> RouteResult:
    if domain in EXACT_DOMAINS:
        return RouteResult(task, domain, "REJECTED-EXACT", "",
                           "exact required (payment/crypto/ledger/exact-search) — approximation REFUSED")
    if domain in APPROX_OK_DOMAINS or approx_authorized:
        return RouteResult(task, domain, "APPROX",
                           ENGINE.get(domain, "U3 sketch (probabilistic)"),
                           "approximation authorized for this domain")
    return RouteResult(task, domain, "REJECTED-EXACT", "",
                       "domain not authorized for approximation — conservative default is EXACT")


@dataclass
class RoutingRatio:
    rows: list

    def approx_pct(self):
        return round(100 * sum(1 for r in self.rows if r.decision == "APPROX") / len(self.rows)) if self.rows else 0

    def exact_pct(self):
        return round(100 * sum(1 for r in self.rows if r.decision == "REJECTED-EXACT") / len(self.rows)) if self.rows else 0


def routing_ratio(corpus) -> RoutingRatio:
    return RoutingRatio([route(t, d) for t, d in corpus])
