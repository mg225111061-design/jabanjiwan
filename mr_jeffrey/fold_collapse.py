"""
STAGE H4 — fold collapse certificate (performance AND correctness together).
============================================================================
A HARAN `fold k in lo..n { body(k) }` over a POLYNOMIAL summand collapses to a closed form, and we
issue a machine-checked certificate of the collapse — proven entirely by the JEFF polynomial engine
(poly.rs via the `jeff_foldsum` CLI), no floats / no fuzzing:

  DERIVE  the closed form S(n) by interpolation (jeff_foldsum: UniPoly::interpolate);
  PROVE   it by telescoping coefficient-zero  S(n)−S(n−1)−body(n) ≡ 0  and base  S(lo−1)=0;
  MATCH   (if an `ensures result = RHS` is present) the closed form against the spec via jeff_identity.

Certificate = (original fold, closed form, JEFF proof, equivalence ∀n, O(n)→O(1)).  Folds whose
summand is not a polynomial in k (geometric 2^k → C-finite; an unknown call g(k) → data-dependent)
do NOT collapse here — honest DEFER with the reason, never a fake closed form.

At H2 the summation step used sympy; H4 replaces it with this JEFF-native derive+prove certificate.
"""
from __future__ import annotations

import os
import re
import subprocess
from dataclasses import dataclass
from fractions import Fraction
from typing import List, Optional

import haran_ast as A
import jeff_adapter   # reuse find_jeff_binary() (jeff_identity) for the ensures-match check


class NonPoly(Exception):
    pass


@dataclass
class FoldCertificate:
    fold: str            # "Σ_{k=1}^{n} (k*k)"
    closed_form: str     # "1/6*n + 1/2*n^2 + 1/3*n^3"
    jeff_proof: str      # "telescope=ZERO, base=ZERO (jeff-math poly coeff-zero)"
    equivalence: str     # "∀n: fold = closed_form (induction: step+base)"
    speedup: str         # "O(n) → O(1)"
    verified_by: str     # "jeff-math"
    closed_coeffs: List[str] = None

    def __str__(self):
        return (f"CERT  {self.fold} = {self.closed_form}\n"
                f"      proof: {self.jeff_proof}\n"
                f"      equiv: {self.equivalence}   [{self.speedup}, verified_by={self.verified_by}]")


@dataclass
class FoldCollapseResult:
    verdict: str                       # COLLAPSED | DEFER
    cert: Optional[FoldCertificate]
    detail: str
    matches_ensures: Optional[bool] = None

    def __str__(self):
        if self.verdict == "COLLAPSED":
            m = "" if self.matches_ensures is None else f"  matches_ensures={self.matches_ensures}"
            return f"COLLAPSED{m}\n{self.cert}"
        return f"DEFER — {self.detail}"


# --------------------------------------------------------- tiny univariate poly over Fraction
def _padd(a, b):
    n = max(len(a), len(b))
    return [(a[i] if i < len(a) else Fraction(0)) + (b[i] if i < len(b) else Fraction(0)) for i in range(n)]
def _psub(a, b):
    n = max(len(a), len(b))
    return [(a[i] if i < len(a) else Fraction(0)) - (b[i] if i < len(b) else Fraction(0)) for i in range(n)]
def _pmul(a, b):
    r = [Fraction(0)] * (len(a) + len(b) - 1)
    for i, ai in enumerate(a):
        for j, bj in enumerate(b):
            r[i + j] += ai * bj
    return r
def _pscale(a, s):
    return [x * s for x in a]
def _ptrim(a):
    a = list(a)
    while len(a) > 1 and a[-1] == 0:
        a.pop()
    return a


def body_to_coeffs(e, binder: str) -> List[Fraction]:
    """HARAN arithmetic expr in `binder` → ascending Fraction coefficients. Raises NonPoly otherwise."""
    if isinstance(e, A.Num):
        return [Fraction(e.value)]
    if isinstance(e, A.Var):
        if e.name == binder:
            return [Fraction(0), Fraction(1)]
        raise NonPoly(f"variable '{e.name}' (not the fold binder)")
    if isinstance(e, A.Un) and e.op == "-":
        return _pscale(body_to_coeffs(e.operand, binder), Fraction(-1))
    if isinstance(e, A.Bin):
        if e.op == "+":
            return _padd(body_to_coeffs(e.lhs, binder), body_to_coeffs(e.rhs, binder))
        if e.op == "-":
            return _psub(body_to_coeffs(e.lhs, binder), body_to_coeffs(e.rhs, binder))
        if e.op == "*":
            return _pmul(body_to_coeffs(e.lhs, binder), body_to_coeffs(e.rhs, binder))
        if e.op == "/":
            rb = _ptrim(body_to_coeffs(e.rhs, binder))
            if len(rb) == 1 and rb[0] != 0:
                return _pscale(body_to_coeffs(e.lhs, binder), Fraction(1) / rb[0])
            raise NonPoly("division by a non-constant")
        if e.op == "**":
            if isinstance(e.rhs, A.Num) and not e.rhs.is_float:
                base = body_to_coeffs(e.lhs, binder)
                out = [Fraction(1)]
                for _ in range(int(e.rhs.value)):
                    out = _pmul(out, base)
                return out
            raise NonPoly("exponent is not a constant integer (non-polynomial, e.g. r^k)")
        raise NonPoly(f"operator '{e.op}'")
    if isinstance(e, A.Call):
        fname = e.func.name if isinstance(e.func, A.Var) else "?"
        raise NonPoly(f"call to '{fname}' (data-dependent / non-closed-form summand)")
    raise NonPoly(type(e).__name__)


# --------------------------------------------------------- JEFF fold engine bridge
def find_foldsum_binary() -> Optional[str]:
    here = os.path.dirname(os.path.abspath(__file__))
    repo = os.path.dirname(here)
    for sub in ("target/release/examples/jeff_foldsum", "target/debug/examples/jeff_foldsum"):
        p = os.path.join(repo, sub)
        if os.path.isfile(p) and os.access(p, os.X_OK):
            return p
    return None


def _fmt(c: Fraction) -> str:
    return str(c.numerator) if c.denominator == 1 else f"{c.numerator}/{c.denominator}"


def _run_foldsum(lo: int, coeffs: List[Fraction]) -> Optional[str]:
    binp = find_foldsum_binary()
    if not binp:
        return None
    try:
        return subprocess.run([binp, str(lo), ",".join(_fmt(c) for c in coeffs)],
                              capture_output=True, text=True, timeout=20).stdout.strip()
    except Exception:  # noqa: BLE001
        return None


def _jeff_coeffs_equal(closed_coeffs: List[str], ensures_coeffs: List[Fraction]) -> Optional[bool]:
    """closed_form ≡ ensures_RHS via the jeff_identity coeff-zero binary (real JEFF)."""
    binp = jeff_adapter.find_jeff_binary()
    if not binp:
        return None
    try:
        out = subprocess.run([binp, ",".join(closed_coeffs), ",".join(_fmt(c) for c in ensures_coeffs)],
                             capture_output=True, text=True, timeout=20).stdout.strip()
    except Exception:  # noqa: BLE001
        return None
    return out.startswith("PROVEN_EQUAL")


# --------------------------------------------------------- the collapser
def _const_int(e) -> Optional[int]:
    if isinstance(e, A.Num) and not e.is_float:
        return int(e.value)
    return None


def _block_return(b):
    if isinstance(b, A.Block) and b.stmts and isinstance(b.stmts[-1], A.ExprStmt):
        return b.stmts[-1].value
    return b


def _show(e) -> str:
    if isinstance(e, A.Bin):
        return f"{_show(e.lhs)}{e.op}{_show(e.rhs)}"
    if isinstance(e, A.Un):
        return f"{e.op}{_show(e.operand)}"
    if isinstance(e, A.Call):
        return f"{_show(e.func)}({', '.join(_show(a) for a in e.args)})"
    if isinstance(e, (A.Var, )):
        return e.name
    if isinstance(e, A.Num):
        return e.value
    return type(e).__name__


def collapse_fold(fold: A.Fold, ensures_rhs=None) -> FoldCollapseResult:
    if not isinstance(fold.domain, A.Range):
        return FoldCollapseResult("DEFER", None, "fold domain is not a range")
    lo = _const_int(fold.domain.lo)
    if lo is None:
        return FoldCollapseResult("DEFER", None, "fold lower bound is not a constant integer")
    hi = fold.domain.hi
    hi_var = hi.name if isinstance(hi, A.Var) else None
    body_expr = _block_return(fold.body)
    try:
        coeffs = _ptrim(body_to_coeffs(body_expr, fold.binder))
    except NonPoly as e:
        return FoldCollapseResult("DEFER", None,
                                  f"summand is not a polynomial in '{fold.binder}': {e}")
    out = _run_foldsum(lo, coeffs)
    if out is None:
        return FoldCollapseResult("DEFER", None, "jeff_foldsum engine not built")
    if not out.startswith("OK"):
        return FoldCollapseResult("DEFER", None, f"fold engine did not collapse: {out}")
    closed = re.search(r'closed="([^"]*)"', out).group(1)
    ccoeffs = re.search(r"coeffs=(\S+)", out).group(1).split(",")
    tele = "ZERO" if "telescope=ZERO" in out else "NONZERO"
    base = "ZERO" if "base=ZERO" in out else "NONZERO"
    cert = FoldCertificate(
        fold=f"Σ_{{{fold.binder}={lo}}}^{{{hi_var or 'n'}}} ({_show(body_expr)})",
        closed_form=closed,
        jeff_proof=f"telescope={tele}, base={base} (jeff-math poly coeff-zero)",
        equivalence=(f"∀{hi_var or 'n'}: fold = closed_form by induction "
                     f"(step S(n)−S(n−1)=body, base S({lo - 1})=0)"),
        speedup="O(n) → O(1)", verified_by="jeff-math", closed_coeffs=ccoeffs)
    matches = None
    if ensures_rhs is not None and hi_var:
        try:
            ecoeffs = _ptrim(body_to_coeffs(ensures_rhs, hi_var))
            matches = _jeff_coeffs_equal(ccoeffs, ecoeffs)
        except NonPoly:
            matches = None
    return FoldCollapseResult("COLLAPSED", cert, "collapsed to closed form", matches)


def collapse_fn_fold(fn: A.FnDecl) -> FoldCollapseResult:
    """Find the fold that is fn's return value and collapse it, matching fn.ensures if present."""
    ret = _block_return(fn.body) if isinstance(fn.body, A.Block) else fn.body
    if not isinstance(ret, A.Fold):
        return FoldCollapseResult("DEFER", None, "function body is not a fold")
    ensures_rhs = None
    if isinstance(fn.ensures, A.Bin) and fn.ensures.op in ("=", "==") \
            and isinstance(fn.ensures.lhs, A.Var) and fn.ensures.lhs.name == "result":
        ensures_rhs = fn.ensures.rhs
    return collapse_fold(ret, ensures_rhs)
