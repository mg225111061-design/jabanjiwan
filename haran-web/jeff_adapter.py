"""
STAGE 3 — JEFF EXACT engine adapter (the strongest verification tier).
======================================================================
Connects Mr. to the **real JEFF** exact engine (Rust `jeff-math`, the coefficient-zero mechanism)
via a thin CLI (`examples/jeff_identity`). sympy is the *parser front-end* (expression → exact
rational coefficients); JEFF performs the exact `is_zero` verdict over `BigRational`.

Fallback chain (each tier labeled, honest):
    JEFF (exact, proof-carrying, univariate poly)  →  sympy (exact CAS, general)  →  DEFER.

Honest scope (Stage 3.4): for a *polynomial identity*, JEFF and sympy give the **same exact**
verdict — JEFF is not "more" there; its distinctions are (a) an independent exact re-check by a
separate engine, and (b) proof-CARRYING (the verdict comes from JEFF's actual coefficient-zero
mechanism, the same one `jeff-verify` re-checks), versus sympy's black-box `simplify`. JEFF's reach
*beyond* sympy (telescoper / fold certificates for structured sums, Stage 26/35 in the Rust engine)
is real but not yet CLI-exposed for arbitrary sums — stated, not implied.
"""
from __future__ import annotations
import os
import subprocess
from dataclasses import dataclass

try:
    import sympy as sp
    from verify_exact import prove_equiv  # sympy tier (the prototype)
    _SYMPY = True
except Exception:  # pragma: no cover
    _SYMPY = False


@dataclass
class ExactResult:
    verdict: str   # "PROVEN" | "REFUTED" | "DEFER"
    backend: str   # "jeff" | "sympy" | "none"
    detail: str = ""
    witness: str = ""

    def __str__(self):
        if self.verdict == "PROVEN":
            return f"PROVEN (all inputs) by {self.backend.upper()} — {self.detail}"
        if self.verdict == "REFUTED":
            return f"REFUTED by {self.backend.upper()} — {self.detail}" + (f" (witness {self.witness})" if self.witness else "")
        return f"DEFER — {self.detail}"


def find_jeff_binary():
    """Locate the prebuilt `jeff_identity` binary in the JEFF Rust workspace, or None."""
    here = os.path.dirname(os.path.abspath(__file__))
    repo = os.path.dirname(here)  # mr_jeffrey/.. = repo root
    for sub in ("target/release/examples/jeff_identity", "target/debug/examples/jeff_identity"):
        p = os.path.join(repo, sub)
        if os.path.isfile(p) and os.access(p, os.X_OK):
            return p
    env = os.environ.get("JEFF_IDENTITY_BIN")
    return env if env and os.path.isfile(env) else None


def _uni_coeffs(expr_str, var):
    """sympy: expression → ascending exact rational coefficient strings ('num/den'), or None if it
    is not a univariate polynomial in `var`."""
    x = sp.symbols(var)
    try:
        e = sp.expand(sp.sympify(expr_str, locals={var: x}))
        p = sp.Poly(e, x)
    except Exception:
        return None
    if len(p.gens) != 1:
        return None
    asc = list(reversed(p.all_coeffs()))  # all_coeffs is highest-degree first
    out = []
    for c in asc:
        r = sp.nsimplify(c)
        fr = sp.Rational(r)
        out.append(f"{fr.p}/{fr.q}")
    return out


def prove_identity(cand_expr: str, ref_expr: str, variables: list) -> ExactResult:
    """Prove `cand ≡ ref` for all inputs, using the JEFF→sympy→DEFER chain."""
    if not _SYMPY:
        return ExactResult("DEFER", "none", "sympy not available")

    # --- Tier 1: JEFF (univariate polynomial identities) ---
    if len(variables) == 1:
        binp = find_jeff_binary()
        cc = _uni_coeffs(cand_expr, variables[0])
        rc = _uni_coeffs(ref_expr, variables[0])
        if binp and cc is not None and rc is not None:
            try:
                out = subprocess.run([binp, ",".join(cc), ",".join(rc)],
                                     capture_output=True, text=True, timeout=30).stdout.strip()
            except Exception as e:
                out = ""
            if out.startswith("PROVEN_EQUAL"):
                return ExactResult("PROVEN", "jeff", f"{cand_expr} ≡ {ref_expr} (coefficient-zero, exact)")
            if out.startswith("REFUTED"):
                w = out[len("REFUTED"):].strip()  # e.g. "degree=1 residual=-1/2"
                return ExactResult("REFUTED", "jeff",
                                   f"{cand_expr} ≢ {ref_expr} (exact disagreement: {w})", witness=w)
            # else fall through to sympy

    # --- Tier 2: sympy (general exact CAS) ---
    v = prove_equiv(cand_expr, ref_expr, variables)
    if v.verdict == "PROVEN_EQUAL":
        return ExactResult("PROVEN", "sympy", v.detail)
    if v.verdict == "PROVEN_UNEQUAL":
        return ExactResult("REFUTED", "sympy", v.detail, witness=v.witness)
    # --- Tier 3: DEFER ---
    return ExactResult("DEFER", "none", v.detail)


def backends_available():
    """Report which exact backends are live (for honest status output)."""
    return {"jeff": find_jeff_binary() is not None, "sympy": _SYMPY}
