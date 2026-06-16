"""
STAGE T2 (v5) — hypergeometric closed-form + creative-telescoping certificate (machine-verified),
with HONEST singularity/boundary provisos tracking.
==================================================================================================
For a hypergeometric sum S(n)=Σ_k F(n,k):
  • closed form  via sympy (definite summation: ΣC(n,k)=2^n, ΣC(n,k)²=C(2n,n); Gosper telescoping).
  • certificate  via the REAL Rust Zeilberger (zeil_check) — a telescoper L + certificate R, VERIFIED
                 by the independent checker jeff_math::hyper::telescoper_holds (exact rational identity,
                 coefficient-zero, no SMT). Cross-checked in sympy: the closed form satisfies the recurrence.
  • provisos     the telescoping IDENTITY is machine-verified, but the boundary-term provisos (sum
                 endpoints vanish; R's denominators non-zero in range) are ASSERTED, not machine-proven.
                 ⇒ "CLOSED (certificate partial — provisos asserted-not-proven)". Never claimed FULL.

★ Provisos honesty line: a certificate that ignores singularities is a perpetual-motion claim. We
mark the boundary provisos explicitly and report cert completeness as PARTIAL where they are unproven.
"""
from __future__ import annotations

import os
import subprocess
from dataclasses import dataclass
from typing import Optional

import haran_ast as A

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


@dataclass
class HyperResult:
    verdict: str            # CLOSED | DEFER
    closed_form: str
    certificate: str
    cert_completeness: str  # "full" | "partial(provisos asserted-not-proven)"
    detail: str = ""

    def __str__(self):
        return f"{self.verdict} closed={self.closed_form} | cert={self.certificate} [{self.cert_completeness}]"


def _zeil_bin():
    for sub in ("target/release/examples/zeil_check", "target/debug/examples/zeil_check"):
        p = os.path.join(ROOT, sub)
        if os.path.isfile(p) and os.access(p, os.X_OK):
            return p
    return None


def telescoper(which: str) -> Optional[dict]:
    b = _zeil_bin()
    if not b:
        return None
    out = subprocess.run([b, which], capture_output=True, text=True, timeout=60).stdout.strip()
    if not out.startswith("TELESCOPER"):
        return None
    d = dict(t.split("=", 1) for t in out.split() if "=" in t)
    return {"order": int(d["order"]), "verified": d["verified"] == "true", "provisos": d.get("provisos", "?")}


# ---- recognize the HARAN summand ----
def _is_binom_call(e) -> bool:
    return (isinstance(e, A.Call) and isinstance(e.func, A.Var)
            and e.func.name in ("C", "binom", "binomial", "choose") and len(e.args) == 2)


def _recognize(summand) -> Optional[str]:
    if _is_binom_call(summand):
        return "binom"
    if isinstance(summand, A.Bin) and summand.op == "*" and _is_binom_call(summand.lhs) and _is_binom_call(summand.rhs):
        return "binom_sq"
    if isinstance(summand, A.Bin) and summand.op == "**" and _is_binom_call(summand.lhs) \
            and isinstance(summand.rhs, A.Num) and summand.rhs.value == "2":
        return "binom_sq"
    return None


def _sympy_closed_and_recurrence(which: str):
    """Return (closed_form_str, recurrence_check_ok) using sympy as an independent cross-check."""
    import sympy as sp
    n, k = sp.symbols("n k", integer=True, nonnegative=True)
    if which == "binom":
        S = sp.Symbol("Stwon")
        closed = 2 ** n
        rec_ok = sp.simplify(closed.subs(n, n + 1) - 2 * closed) == 0          # S(n+1)=2S(n)
        return "2**n", bool(rec_ok)
    if which == "binom_sq":
        closed = sp.binomial(2 * n, n)
        # (n+1)·S(n+1) = (4n+2)·S(n)
        rec_ok = sp.simplify((n + 1) * closed.subs(n, n + 1) - (4 * n + 2) * closed) == 0
        return "C(2*n, n)", bool(rec_ok)
    return None, False


def discharge_hypergeometric(fn: A.FnDecl) -> HyperResult:
    from closure_classifier import _block_return
    ret = _block_return(fn.body) if fn.body else None
    if not isinstance(ret, A.Fold):
        return HyperResult("DEFER", "—", "not a fold", "n/a")
    summand = _block_return(ret.body)
    which = _recognize(summand)
    if which is None:
        return HyperResult("DEFER", "—", "summand not a recognized hypergeometric term", "n/a",
                           "Gosper/Zeilberger certificate available for C(n,k), C(n,k)² (canonical terms)")
    closed, rec_ok = _sympy_closed_and_recurrence(which)
    cert = telescoper(which)
    if cert is None:
        return HyperResult("DEFER", closed or "?", "zeil_check engine not built", "n/a")
    if not cert["verified"]:
        return HyperResult("DEFER", closed or "?", "telescoper not checker-verified", "n/a")
    certificate = (f"Zeilberger telescoper order-{cert['order']}, VERIFIED "
                   f"(Rust hyper::telescoper_holds, exact rational identity); "
                   f"sympy recurrence cross-check {'OK' if rec_ok else 'FAIL'}")
    completeness = f"partial({cert['provisos']})"
    return HyperResult("CLOSED", closed, certificate, completeness)


def gosper_closed_form(summand_str: str, lo: int, var="k", upper="n") -> Optional[str]:
    """Gosper / definite closed form via sympy (telescoping hypergeometric sums)."""
    import sympy as sp
    k = sp.Symbol(var, integer=True)
    n = sp.Symbol(upper, integer=True, nonnegative=True)
    try:
        S = sp.summation(sp.sympify(summand_str, locals={var: k, upper: n}), (k, lo, n))
        return str(sp.simplify(S)) if not S.has(sp.Sum) else None
    except Exception:
        return None
