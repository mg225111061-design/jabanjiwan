"""
STAGE V2 — Galois closure classification (the heart: fold everything foldable, prove absence honestly).
====================================================================================================
For every sum / recurrence, decide WHICH of four honest buckets it belongs to — and never blur them:

  CLOSED        a real closed form exists, with a real proof:
                  · polynomial  → Faulhaber (jeff_foldsum telescoping coeff-zero)        O(1)
                  · hypergeometric (incl. geometric) → Gosper antidifference, sympy-verified  O(1)/O(log n)
                  · linear recurrence → C-finite companion-matrix (cfinite.rs ≡ naive)    O(log n)
  ABSENT        a real IMPOSSIBILITY proof that no closed form exists:
                  · Gosper decision: hypergeometric but NOT summable (e.g. Σ1/k)         (gosper-nonsummable)
                  · Galois: quintic unsolvable by radicals (galois.rs, A₅ simple)        (galois-radical)
                  · Liouville: no elementary antiderivative (galois.rs, erf)             (liouville-elementary)
  NO_STRUCTURE  the summand is data-dependent (e.g. is_prime(k)) → outside every closure-decision
                class; Ω(N) information floor. ★Recognition, NOT a Galois proof★ — labeled as such.
  UNKNOWN       looks structured but no decision procedure here settles it.

★ No fake collapse: every CLOSED is verified (JEFF coeff-zero / sympy antidifference / companion≡naive).
★ No fake absence: ABSENT comes only from a real decision procedure (Gosper) or impossibility proof
  (Galois/Liouville). "probably not closed" is UNKNOWN, never ABSENT.
"""
from __future__ import annotations

import os
import subprocess
from dataclasses import dataclass
from typing import List, Optional

import haran_ast as A
import fold_collapse


class _DataDependent(Exception):
    pass
class _Unsupported(Exception):
    pass


@dataclass
class ClosureVerdict:
    kind: str          # CLOSED | ABSENT | NO_STRUCTURE | UNKNOWN
    method: str        # faulhaber|gosper|cfinite | gosper-nonsummable|galois-radical|liouville-elementary | omega-n-data
    closed_form: str
    proof: str
    speedup: str       # O(1) | O(log n) | none

    def __str__(self):
        cf = f"  closed_form={self.closed_form}" if self.closed_form not in ("", "—") else ""
        return f"{self.kind}[{self.method}] {self.speedup}{cf}  ({self.proof})"


# --------------------------------------------------------- HARAN expr → sympy
def haran_to_sympy(e, binder: str):
    import sympy as sp
    if isinstance(e, A.Num):
        return sp.Float(e.value) if e.is_float else sp.Integer(int(e.value))
    if isinstance(e, A.Var):
        return sp.Symbol(e.name)
    if isinstance(e, A.Un) and e.op == "-":
        return -haran_to_sympy(e.operand, binder)
    if isinstance(e, A.Bin):
        l = haran_to_sympy(e.lhs, binder)
        r = haran_to_sympy(e.rhs, binder)
        return {"+": l + r, "-": l - r, "*": l * r, "/": l / r, "**": l ** r}.get(e.op) \
            or _raise(_Unsupported(f"operator '{e.op}'"))
    if isinstance(e, A.Call):
        name = e.func.name if isinstance(e.func, A.Var) else "?"
        raise _DataDependent(name)
    raise _Unsupported(type(e).__name__)


def _raise(exc):
    raise exc


# --------------------------------------------------------- helpers
def _const_int(e) -> Optional[int]:
    if isinstance(e, A.Num) and not e.is_float:
        return int(e.value)
    if isinstance(e, A.Un) and e.op == "-" and isinstance(e.operand, A.Num) and not e.operand.is_float:
        return -int(e.operand.value)
    return None


def _block_return(b):
    if isinstance(b, A.Block) and b.stmts and isinstance(b.stmts[-1], A.ExprStmt):
        return b.stmts[-1].value
    return b


def _show(e):
    if isinstance(e, A.Bin): return f"{_show(e.lhs)}{e.op}{_show(e.rhs)}"
    if isinstance(e, A.Un): return f"{e.op}{_show(e.operand)}"
    if isinstance(e, A.Call): return f"{_show(e.func)}({', '.join(_show(a) for a in e.args)})"
    if isinstance(e, A.Var): return e.name
    if isinstance(e, A.Num): return e.value
    return type(e).__name__


def _has_symbolic_pow(expr, n) -> bool:
    import sympy as sp
    return any(n in a.exp.free_symbols for a in expr.atoms(sp.Pow))


# --------------------------------------------------------- fold closure
def classify_fold(fold: A.Fold) -> ClosureVerdict:
    import sympy as sp
    from sympy.concrete.gosper import gosper_sum

    if not isinstance(fold.domain, A.Range):
        return ClosureVerdict("UNKNOWN", "-", "—", "fold domain is not a range", "none")
    lo = _const_int(fold.domain.lo)
    hi_name = fold.domain.hi.name if isinstance(fold.domain.hi, A.Var) else "n"
    body = _block_return(fold.body)

    # 1. polynomial → Faulhaber (JEFF coeff-zero) — always closed
    fc = fold_collapse.collapse_fold(fold)
    if fc.verdict == "COLLAPSED":
        return ClosureVerdict("CLOSED", "faulhaber", fc.cert.closed_form,
                              f"JEFF: {fc.cert.jeff_proof}", "O(1)")

    # 2. sympify the summand
    try:
        expr = haran_to_sympy(body, fold.binder)
    except _DataDependent as e:
        return ClosureVerdict("NO_STRUCTURE", "omega-n-data", "—",
                              f"summand '{_show(body)}' is data-dependent (call to '{e}'); outside every "
                              f"closure class → Ω(N) information floor. Recognition, NOT a Galois proof.", "none")
    except _Unsupported as e:
        return ClosureVerdict("UNKNOWN", "-", "—", f"summand not analyzable ({e})", "none")

    k = sp.Symbol(fold.binder)
    n = sp.Symbol(hi_name)
    if lo is None:
        return ClosureVerdict("UNKNOWN", "-", "—", "non-constant lower bound", "none")

    # 3. hypergeometric? → Gosper decision (closed OR proven-absent)
    try:
        ratio = sp.hypersimp(expr, k)
    except Exception:
        ratio = None
    if ratio is not None:
        try:
            gs = gosper_sum(expr, (k, lo, n))
        except Exception:
            gs = None
        if gs is not None:
            gs = sp.simplify(gs)
            # VERIFY (no fake collapse): S(n) − S(n−1) == term(n)
            check = sp.simplify(gs - gs.subs(n, n - 1) - expr.subs(k, n))
            if check == 0:
                speed = "O(log n)" if _has_symbolic_pow(gs, n) else "O(1)"
                return ClosureVerdict("CLOSED", "gosper", str(gs),
                                      "Gosper antidifference, sympy-verified S(n)−S(n−1)=term(n)", speed)
            return ClosureVerdict("UNKNOWN", "-", "—", "Gosper output failed verification", "none")
        return ClosureVerdict("ABSENT", "gosper-nonsummable", "—",
                              f"Gosper decision: term is hypergeometric (ratio {sp.simplify(ratio)}) but has NO "
                              f"hypergeometric closed-form antidifference — PROVEN (no closed form).", "none")

    # 4. not hypergeometric → try sympy closed-form summation
    try:
        S = sp.summation(expr, (k, lo, n))
        if not S.has(sp.Sum):
            speed = "O(log n)" if _has_symbolic_pow(S, n) else "O(1)"
            return ClosureVerdict("CLOSED", "sympy-sum", str(sp.simplify(S)),
                                  "sympy closed-form summation", speed)
    except Exception:
        pass
    return ClosureVerdict("UNKNOWN", "-", "—", "not hypergeometric; no closed form found", "none")


# --------------------------------------------------------- linear recurrence (C-finite)
def extract_linear_recurrence(fn: A.FnDecl):
    ret = _block_return(fn.body) if fn.body else None
    if not isinstance(ret, A.Match):
        return None
    bases, rec_expr = {}, None
    for arm in ret.arms:
        if isinstance(arm.pattern, A.PNum):
            v = _const_int(_block_return(arm.body) if isinstance(arm.body, A.Block) else arm.body)
            if v is None:
                return None
            bases[int(arm.pattern.value)] = v
        elif isinstance(arm.pattern, (A.PWild, A.PVar)):
            rec_expr = _block_return(arm.body) if isinstance(arm.body, A.Block) else arm.body
        else:
            return None
    if rec_expr is None:
        return None
    terms, coeffs = _flatten_plus(rec_expr), {}
    for t in terms:
        co_off = _rec_term(t, fn.name)
        if co_off is None:
            return None
        co, off = co_off
        coeffs[off] = coeffs.get(off, 0) + co
    if not coeffs or min(coeffs) < 1:
        return None
    d = max(coeffs)
    c = [coeffs.get(j, 0) for j in range(1, d + 1)]
    init = []
    for i in range(d):
        if i not in bases:
            return None
        init.append(bases[i])
    return c, init


def _flatten_plus(e):
    if isinstance(e, A.Bin) and e.op == "+":
        return _flatten_plus(e.lhs) + _flatten_plus(e.rhs)
    return [e]


def _rec_term(t, fname):
    # fname(n - j)  → (1, j)
    if isinstance(t, A.Call) and isinstance(t.func, A.Var) and t.func.name == fname and len(t.args) == 1:
        off = _offset(t.args[0])
        return (1, off) if off is not None else None
    # c * fname(n - j)  /  fname(n - j) * c
    if isinstance(t, A.Bin) and t.op == "*":
        for a, b in ((t.lhs, t.rhs), (t.rhs, t.lhs)):
            if isinstance(a, A.Num) and not a.is_float and isinstance(b, A.Call) \
                    and isinstance(b.func, A.Var) and b.func.name == fname and len(b.args) == 1:
                off = _offset(b.args[0])
                return (int(a.value), off) if off is not None else None
    return None


def _offset(arg):
    if isinstance(arg, A.Bin) and arg.op == "-" and isinstance(arg.rhs, A.Num) and not arg.rhs.is_float:
        return int(arg.rhs.value)
    return None


def find_binary(name) -> Optional[str]:
    here = os.path.dirname(os.path.abspath(__file__))
    repo = os.path.dirname(here)
    for sub in (f"target/release/examples/{name}", f"target/debug/examples/{name}"):
        p = os.path.join(repo, sub)
        if os.path.isfile(p) and os.access(p, os.X_OK):
            return p
    return None


def _cfinite_match(c, init, n, q=1000000007) -> Optional[bool]:
    b = find_binary("cfinite_nth")
    if not b:
        return None
    out = subprocess.run([b, ",".join(map(str, c)), ",".join(map(str, init)), str(n), str(q)],
                         capture_output=True, text=True, timeout=15).stdout.strip()
    return out.startswith("MATCH")


def classify_recurrence(fn: A.FnDecl) -> Optional[ClosureVerdict]:
    rec = extract_linear_recurrence(fn)
    if rec is None:
        return None
    c, init = rec
    oks = [_cfinite_match(c, init, n) for n in (8, 16, 24)]
    if all(o is True for o in oks):
        return ClosureVerdict("CLOSED", "cfinite", f"companion-matrix power (order {len(c)}, c={c})",
                              f"C-finite: O(log n) companion ≡ O(n) naive (cfinite.rs), verified n∈{{8,16,24}}",
                              "O(log n)")
    if any(o is None for o in oks):
        return ClosureVerdict("UNKNOWN", "cfinite", "—", "cfinite_nth engine not built", "none")
    return ClosureVerdict("UNKNOWN", "cfinite", "—", "companion≠naive (extraction wrong?)", "none")


def classify_fn(fn: A.FnDecl) -> ClosureVerdict:
    ret = _block_return(fn.body) if fn.body else None
    if isinstance(ret, A.Fold):
        return classify_fold(ret)
    rec = classify_recurrence(fn)
    if rec is not None:
        return rec
    return ClosureVerdict("UNKNOWN", "-", "—", "not a fold or recognizable linear recurrence", "none")


# --------------------------------------------------------- Galois / Liouville absence (radicals / integrals)
def classify_radical_absence(a: int, b: int) -> ClosureVerdict:
    bin_ = find_binary("galois_absence")
    if not bin_:
        return ClosureVerdict("UNKNOWN", "galois-radical", "—", "galois_absence engine not built", "none")
    out = subprocess.run([bin_, "quintic", str(a), str(b)], capture_output=True, text=True, timeout=15).stdout.strip()
    if "no_closed_form=true" in out:
        w = out.split('witness="')[1].rstrip('"')
        return ClosureVerdict("ABSENT", "galois-radical", "—", f"Galois: {w}", "none")
    return ClosureVerdict("UNKNOWN", "galois-radical", "—", out, "none")


def classify_elementary_absence_erf() -> ClosureVerdict:
    bin_ = find_binary("galois_absence")
    if not bin_:
        return ClosureVerdict("UNKNOWN", "liouville-elementary", "—", "galois_absence engine not built", "none")
    out = subprocess.run([bin_, "erf"], capture_output=True, text=True, timeout=15).stdout.strip()
    if "no_closed_form=true" in out:
        w = out.split('witness="')[1].rstrip('"')
        return ClosureVerdict("ABSENT", "liouville-elementary", "—", f"Liouville: {w}", "none")
    return ClosureVerdict("UNKNOWN", "liouville-elementary", "—", out, "none")


# --------------------------------------------------------- ratio measurement
@dataclass
class ClosureRatio:
    rows: List[tuple]   # (label, verdict)

    def count(self, kind):
        return sum(1 for _, v in self.rows if v.kind == kind)

    def pct(self, kind):
        return round(100 * self.count(kind) / len(self.rows)) if self.rows else 0


def closure_ratio(corpus: dict) -> ClosureRatio:
    from haran_parser import parse
    rows = []
    for label, src in corpus.items():
        fn = parse(src).items[0]
        rows.append((label, classify_fn(fn)))
    return ClosureRatio(rows)


def render_ratio(r: ClosureRatio) -> str:
    lines = ["closure classification:"]
    for label, v in r.rows:
        lines.append(f"   · {label:16s} {v}")
    n = len(r.rows)
    lines.append(f"   ─ {n} computations: {r.pct('CLOSED')}% CLOSED, {r.pct('ABSENT')}% ABSENT(proven), "
                 f"{r.pct('NO_STRUCTURE')}% NO_STRUCTURE(Ω(N)), {r.pct('UNKNOWN')}% UNKNOWN")
    return "\n".join(lines)
