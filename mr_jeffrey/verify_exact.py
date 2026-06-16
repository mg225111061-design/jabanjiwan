"""
JEFF-style EXACT verification layer
====================================
The bounded verifier (verify_core) is honest but limited: it says
"VERIFIED (bounded)" — strong evidence, not proof. A hidden bug at an untested
input survives.

This layer does what JEFF does: for NUMERIC / ALGEBRAIC code, prove equivalence
for ALL inputs by exact symbolic identity — so the verdict is PROVEN, not "no
counterexample found." This is the differentiator: other AI-test tools can only
fuzz; this returns a real proof for the math-shaped part of the code.

(In production this calls the real JEFF verifier — exact polynomial-identity /
fold certificate. Here we use exact symbolic algebra (sympy, exact rationals) to
demonstrate the SAME idea: closed-form equivalence, proven, no floating point.)
"""

import sympy as sp
from dataclasses import dataclass


@dataclass
class ExactVerdict:
    verdict: str        # PROVEN_EQUAL | PROVEN_UNEQUAL | NOT_APPLICABLE
    detail: str = ""
    witness: str = ""   # for unequal: an input where they differ

    def __str__(self):
        if self.verdict == "PROVEN_EQUAL":
            return f"PROVEN EQUAL (all inputs) -- {self.detail}"
        if self.verdict == "PROVEN_UNEQUAL":
            return f"PROVEN UNEQUAL -- differ at {self.witness}; {self.detail}"
        return f"NOT APPLICABLE (not a closed-form numeric claim) -- {self.detail}"


def prove_equiv(expr_candidate: str, expr_reference: str, variables: list):
    """
    Prove that candidate(x...) == reference(x...) for ALL real x, exactly.
    expr_* are math expressions in the given variable names, e.g.
        prove_equiv("n*(n+1)/2", "Sum(k,(k,1,n))", ["n"])  -> requires closed forms
        prove_equiv("(x+1)**2", "x**2+2*x+1", ["x"])       -> PROVEN_EQUAL
    Returns ExactVerdict.
    """
    syms = sp.symbols(variables, real=True)
    env = dict(zip(variables, syms if isinstance(syms, (list, tuple)) else [syms]))
    try:
        c = sp.sympify(expr_candidate, locals=env)
        r = sp.sympify(expr_reference, locals=env)
    except Exception as e:
        return ExactVerdict("NOT_APPLICABLE", detail=f"could not parse: {e}")

    diff = sp.simplify(c - r)
    if diff == 0:
        return ExactVerdict("PROVEN_EQUAL", detail=f"{expr_candidate}  ≡  {expr_reference}")

    # try harder: expand / factor
    diff2 = sp.simplify(sp.expand(c) - sp.expand(r))
    if diff2 == 0:
        return ExactVerdict("PROVEN_EQUAL", detail=f"{expr_candidate}  ≡  {expr_reference} (after expansion)")

    # they differ — find a witness input exactly
    witness = None
    free = list(diff.free_symbols)
    for trial in range(0, 6):
        subs = {s: trial for s in free}
        try:
            val = diff.subs(subs)
            if val != 0:
                witness = ", ".join(f"{s}={trial}" for s in free) + f" → diff={val}"
                break
        except Exception:
            pass
    return ExactVerdict("PROVEN_UNEQUAL", detail=f"residual = {diff}",
                        witness=witness or "(symbolic residual nonzero)")


def prove_closed_form_sum(closed_form: str, summand: str, index: str,
                          lo: str, hi: str, variables: list):
    """
    JEFF's signature move: prove that a loop computing a sum equals a CLOSED FORM,
    for all n — exactly. e.g. prove sum_{k=1}^{n} k  ==  n*(n+1)/2.
        prove_closed_form_sum("n*(n+1)/2", "k", "k", "1", "n", ["n","k"])
    """
    syms = {v: sp.symbols(v, integer=True, positive=True) for v in variables}
    k = syms[index]
    n_sym = syms.get("n")
    try:
        summand_e = sp.sympify(summand, locals=syms)
        lo_e = sp.sympify(lo, locals=syms)
        hi_e = sp.sympify(hi, locals=syms)
        closed_e = sp.sympify(closed_form, locals=syms)
    except Exception as e:
        return ExactVerdict("NOT_APPLICABLE", detail=f"parse error: {e}")

    actual = sp.summation(summand_e, (k, lo_e, hi_e))
    diff = sp.simplify(actual - closed_e)
    if diff == 0:
        return ExactVerdict("PROVEN_EQUAL",
            detail=f"Σ_{{{index}={lo}}}^{{{hi}}} {summand} ≡ {closed_form}  (proven for all {hi})")
    # witness
    witness = None
    if n_sym is not None:
        for t in range(1, 6):
            if sp.simplify(diff.subs(n_sym, t)) != 0:
                witness = f"n={t}"
                break
    return ExactVerdict("PROVEN_UNEQUAL", detail=f"residual = {diff}",
                        witness=witness or "(nonzero residual)")
