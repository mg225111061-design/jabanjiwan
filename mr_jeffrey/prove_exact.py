"""
STAGE X3 — proof-power completion (exact paths proven too).
==========================================================
v1 confessed "sort = tested-not-proven". Here we strengthen exact verification and fill the
obligations that v1 only *generated*:

  correctness tiers (kept distinct, never relabeled):
    PROVEN          exact arithmetic identity, ∀ unbounded (JEFF/sympy).
    PROVEN-BOUNDED  EXHAUSTIVE enumeration over a finite domain (ALL |xs|≤N, values∈[0,V)) — a real
                    proof for that domain, strictly stronger than random fuzz. (sort lands here.)
    TESTED          random bounded fuzz (last resort).
  contract     each call to a `requires`-function: args satisfy the callee's requires (Z3).
  aliasing     `own` values used at most once (use-after-move check).
  adt          a match on a `data` type covers all constructors (or has a wildcard).

Honesty: PROVEN-BOUNDED is NOT a full ∀-proof — a Z3/array-induction proof of unbounded sorting is
out of scope and not claimed. The tier name says exactly what was shown.
"""
from __future__ import annotations

from dataclasses import dataclass
from itertools import product
from typing import List, Optional

import haran_ast as A
import haran_eval
import z3_adapter
from haran_to_obligations import discharge_correctness


@dataclass
class Verdict:
    tier: str               # PROVEN | PROVEN-BOUNDED | TESTED | FAILED | UNKNOWN
    detail: str
    counterexample: Optional[dict] = None
    def proven(self) -> bool:
        return self.tier in ("PROVEN", "PROVEN-BOUNDED")
    def __str__(self):
        cx = f"  cx={self.counterexample}" if self.counterexample else ""
        return f"{self.tier} — {self.detail}{cx}"


def _walk(node):
    import dataclasses
    yield node
    if dataclasses.is_dataclass(node) and not isinstance(node, A.Span):
        for f in dataclasses.fields(node):
            v = getattr(node, f.name)
            if isinstance(v, list):
                for x in v:
                    if dataclasses.is_dataclass(x):
                        yield from _walk(x)
            elif dataclasses.is_dataclass(v):
                yield from _walk(v)


# --------------------------------------------------------- X3.1 correctness (proven where possible)
def _list_params(fn):
    return [p.name for p in fn.params if isinstance(p.ty, A.TyName) and p.ty.name in ("List", "Vec", "Seq")]


def exhaustive_list_check(fn, ftab, ens, max_len=4, vals=4) -> Optional[Verdict]:
    lps = _list_params(fn)
    if len(lps) != 1:
        return None
    pname = lps[0]
    cases = 0
    for L in range(max_len + 1):
        for combo in product(range(vals), repeat=L):
            cases += 1
            xs = list(combo)
            try:
                it = haran_eval.Interp(ftab)
                result = it.call_fn(fn, [xs])
                ok = it.eval(ens, {pname: xs, "result": result})
            except haran_eval.EvalError:
                return None    # cannot evaluate exhaustively → caller falls back to fuzz
            if not bool(ok):
                return Verdict("FAILED", f"ensures false at {pname}={xs}",
                               {"inputs": {pname: xs}, "result": result})
    return Verdict("PROVEN-BOUNDED",
                   f"EXHAUSTIVE: all |{pname}|≤{max_len}, values∈[0,{vals - 1}] ({cases} cases) satisfy ensures")


def prove_correctness(fn, ftab) -> Verdict:
    if fn.ensures is None:
        return Verdict("UNKNOWN", "no ensures")
    # 1. exact arithmetic identity → JEFF/sympy ∀
    d = discharge_correctness(fn)
    if d.verdict == "PROVEN":
        return Verdict("PROVEN", f"exact ∀ via {d.backend}: {d.detail}")
    if d.verdict == "REFUTED":
        return Verdict("FAILED", d.detail, d.counterexample)
    # 2. predicate over a single list → exhaustive enumeration (upgrade from random fuzz)
    ex = exhaustive_list_check(fn, ftab, fn.ensures)
    if ex is not None:
        return ex
    # 3. random fuzz (honest last resort)
    status, detail, cx = haran_eval.bounded_fuzz(fn, ftab, fn.ensures)
    if status == "PASS":
        return Verdict("TESTED", f"bounded fuzz: {detail}")
    if status == "FAIL":
        return Verdict("FAILED", "ensures false on a random input", cx)
    return Verdict("UNKNOWN", detail)


# --------------------------------------------------------- X3.2 contract (requires)
def _subst(e, m):
    if isinstance(e, A.Var):
        return m.get(e.name, e)
    if isinstance(e, A.Bin):
        return A.Bin(e.op, _subst(e.lhs, m), _subst(e.rhs, m), e.span)
    if isinstance(e, A.Un):
        return A.Un(e.op, _subst(e.operand, m), e.span)
    if isinstance(e, A.Call):
        return A.Call(e.func, [_subst(a, m) for a in e.args], e.span)
    return e


def _numeric_var_types(fn):
    out = {}
    for p in fn.params:
        if isinstance(p.ty, A.TyName) and p.ty.name in ("Float", "Real", "rat"):
            out[p.name] = "Real"
        elif isinstance(p.ty, A.TyName) and p.ty.name in ("Int", "Nat"):
            out[p.name] = "Int"
    return out


@dataclass
class ContractCheck:
    callee: str
    verdict: str            # PASS | FAIL | UNKNOWN
    counterexample: Optional[dict] = None


def check_contract(fn, ftab) -> List[ContractCheck]:
    out = []
    var_types = _numeric_var_types(fn)
    assumptions = [fn.requires] if fn.requires is not None else []
    for x in _walk(fn.body) if fn.body else []:
        if isinstance(x, A.Call) and isinstance(x.func, A.Var):
            g = ftab.get(x.func.name)
            if g is not None and g.requires is not None and len(x.args) == len(g.params):
                sub = _subst(g.requires, {p.name: a for p, a in zip(g.params, x.args)})
                r = z3_adapter.prove_forall(sub, var_types, assumptions)
                out.append(ContractCheck(g.name, "PASS" if r.verdict == "PROVEN"
                                         else ("FAIL" if r.verdict == "REFUTED" else "UNKNOWN"),
                                         r.counterexample))
    return out


# --------------------------------------------------------- X3.3 aliasing (own/&)
@dataclass
class AliasIssue:
    name: str
    uses: int
    msg: str


def check_aliasing(fn) -> List[AliasIssue]:
    issues = []
    for p in fn.params:
        if isinstance(p.ty, A.TyOwn):
            uses = sum(1 for x in _walk(fn.body) if isinstance(x, A.Var) and x.name == p.name) if fn.body else 0
            if uses > 1:
                issues.append(AliasIssue(p.name, uses, f"own value '{p.name}' used {uses}× (use-after-move)"))
    return issues


# --------------------------------------------------------- X3.4 ADT exhaustiveness
def build_data_index(prog):
    ctors_of = {}
    for it in prog.items:
        if isinstance(it, A.DataDecl):
            ctors_of[it.name] = {c.name for c in it.ctors}
    return ctors_of


def check_adt_exhaustiveness(fn, ctors_of) -> List[tuple]:
    """Returns [(scrut, verdict, missing)]. verdict ∈ PASS|FAIL|UNKNOWN."""
    ptypes = {p.name: (p.ty.name if isinstance(p.ty, A.TyName) else None) for p in fn.params}
    out = []
    for m in (x for x in _walk(fn.body) if isinstance(x, A.Match)) if fn.body else []:
        if not isinstance(m.scrut, A.Var):
            continue
        tname = ptypes.get(m.scrut.name)
        if tname not in ctors_of:
            continue
        if any(isinstance(a.pattern, (A.PWild, A.PVar)) for a in m.arms):
            out.append((m.scrut.name, "PASS", set()))
            continue
        covered = {a.pattern.name for a in m.arms if isinstance(a.pattern, A.PCtor)}
        missing = ctors_of[tname] - covered
        out.append((m.scrut.name, "PASS" if not missing else "FAIL", missing))
    return out


# --------------------------------------------------------- X3.4 proven ratio
def proven_ratio(corpus: dict):
    from haran_parser import parse
    rows = []
    for label, src in corpus.items():
        prog = parse(src)
        fns = [it for it in prog.items if isinstance(it, A.FnDecl) and it.ensures is not None]
        ftab = {f.name: f for f in prog.items if isinstance(f, A.FnDecl)}
        for f in fns:
            rows.append((label, prove_correctness(f, ftab)))
    return rows
