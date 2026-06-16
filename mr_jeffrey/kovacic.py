"""
STAGE T4 (v5) — Kovacic algorithm (focused): Liouvillian solutions of y'' = r(x)·y.
==================================================================================
Kovacic decides whether a 2nd-order homogeneous linear ODE y''=r·y has a Liouvillian (closed-form)
solution — the differential-Galois solvability question. This is the *extension* of v2's Liouville
integral-absence (galois.rs erf_elementary_absence) from ∫ to 2nd-order ODEs.

Scope (honest): a FOCUSED Kovacic —
  • constant r                       → CLOSED (y = exp(±√r·x), Liouvillian).
  • polynomial r of ODD degree       → ABSENT (no Liouvillian; Galois group SL₂ non-solvable; Airy y''=xy is d=1).
  • polynomial r of EVEN degree      → Case 1: search ω∈ℚ[x] with ω'+ω²=r; found → CLOSED (y=exp∫ω); else UNKNOWN.
  • rational r with poles            → UNKNOWN (full Cases 1–3 with poles are out of scope).

★ Honesty (rule 2): proven absence → ABSENT; merely "Case-1 not found" → UNKNOWN (never relabeled ABSENT).
"""
from __future__ import annotations

from dataclasses import dataclass


@dataclass
class KovacicResult:
    verdict: str        # CLOSED | ABSENT | UNKNOWN
    detail: str
    def __str__(self):
        return f"{self.verdict} — {self.detail}"


def kovacic(r_str: str, x_name: str = "x") -> KovacicResult:
    import sympy as sp
    x = sp.Symbol(x_name)
    r = sp.expand(sp.sympify(r_str, locals={x_name: x}))

    if not r.has(x):                                   # constant coefficient
        return KovacicResult("CLOSED", f"constant r={r}: y=exp(±√({r})·x) — Liouvillian")

    if r.is_polynomial(x):
        p = sp.Poly(r, x)
        d = p.degree()
        if d % 2 == 1:                                  # odd-degree polynomial ⇒ no Liouvillian
            return KovacicResult("ABSENT",
                                 f"y''=({r})·y: polynomial r of ODD degree {d} ⇒ NO Liouvillian solution "
                                 f"(differential Galois group SL₂(ℂ) non-solvable; Airy y''=x·y is the d=1 case)")
        # Case 1: ω = polynomial of degree d/2 with ω' + ω² = r
        m = d // 2
        coeffs = sp.symbols(f"a0:{m + 1}")
        omega = sum(c * x ** i for i, c in enumerate(coeffs))
        eqs = sp.Poly(sp.expand(sp.diff(omega, x) + omega ** 2 - r), x).all_coeffs()
        sols = sp.solve(eqs, coeffs, dict=True)
        for s in sols:
            om = sp.simplify(omega.subs(s))
            if om.free_symbols <= {x}:                  # fully determined
                return KovacicResult("CLOSED",
                                     f"Case 1: ω={om}, y=exp(∫ω dx) — Liouvillian")
        return KovacicResult("UNKNOWN",
                             f"y''=({r})·y: Case 1 (rational-exponential) has no solution; Kovacic Cases 2/3 "
                             f"out of scope ⇒ UNKNOWN (NOT claimed ABSENT)")
    return KovacicResult("UNKNOWN",
                         f"y''=({r})·y: rational r with poles — full Kovacic (Cases 1–3 with poles) out of scope ⇒ UNKNOWN")
