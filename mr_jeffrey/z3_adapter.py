"""
STAGE X1 — proof-engine core: Z3 relational backend (foundation for error proofs).
=================================================================================
v1/v2 could prove polynomial identities (JEFF coeff-zero) and decide closure (Gosper/Galois), and
TEST general props (bounded fuzz). What was missing — and what "unstructured conquest" (X2) needs —
is proving INEQUALITIES like |approx − exact| ≤ ε for ALL inputs. This wires Z3 for that.

Extended fallback chain (X1.2):
    jeff (polynomial identity, exact)  →  sympy (exact identity)  →  Z3 (relational / inequality / FOL)
    →  bounded fuzz (last resort, TESTED not PROVEN)

★ Honesty: a Z3 `unsat` of the negation is a real ∀-proof (PROVEN). `sat` → REFUTED with a concrete
counterexample (the model). `unknown`/timeout → UNKNOWN (never upgraded to PROVEN). Boundary 1 holds:
everything proven is *명세 대비*.
"""
from __future__ import annotations

from dataclasses import dataclass
from fractions import Fraction
from typing import Dict, List, Optional

import haran_ast as A

try:
    import z3
    _Z3 = True
except Exception:   # noqa: BLE001
    _Z3 = False


def z3_available() -> bool:
    return _Z3


@dataclass
class ProofResult:
    verdict: str            # PROVEN | REFUTED | UNKNOWN | UNAVAILABLE
    backend: str            # z3 | none
    detail: str
    counterexample: Optional[dict] = None

    def __str__(self):
        cx = f"  cx={self.counterexample}" if self.counterexample else ""
        return f"{self.verdict} via {self.backend} — {self.detail}{cx}"


class _Unsupported(Exception):
    pass


def _to_z3(e, env: Dict[str, object], real=True):
    """HARAN bool/arith expr → a Z3 term. `env` maps free names to typed Z3 consts."""
    if isinstance(e, A.Num):
        if e.is_float or real:
            f = Fraction(e.value)
            return z3.RealVal(f"{f.numerator}/{f.denominator}")
        return z3.IntVal(int(e.value))
    if isinstance(e, A.BoolLit):
        return z3.BoolVal(e.value)
    if isinstance(e, A.Var):
        if e.name not in env:
            raise _Unsupported(f"free variable '{e.name}'")
        return env[e.name]
    if isinstance(e, A.Un):
        if e.op == "-":
            return -_to_z3(e.operand, env, real)
        if e.op in ("¬", "!"):
            return z3.Not(_to_z3(e.operand, env, real))
        raise _Unsupported(f"unary '{e.op}'")
    if isinstance(e, A.Call) and isinstance(e.func, A.Var) and e.func.name == "abs" and len(e.args) == 1:
        a = _to_z3(e.args[0], env, real)
        return z3.If(a >= 0, a, -a)
    if isinstance(e, A.Call) and isinstance(e.func, A.Var) and e.func.name in ("min", "max") and len(e.args) == 2:
        a, b = _to_z3(e.args[0], env, real), _to_z3(e.args[1], env, real)
        return z3.If(a <= b, a, b) if e.func.name == "min" else z3.If(a >= b, a, b)
    if isinstance(e, A.Call):
        raise _Unsupported(f"opaque call '{getattr(e.func, 'name', '?')}' (Z3 cannot model it)")
    if isinstance(e, A.Quant):
        zvars = [z3.Real(v) if real else z3.Int(v) for v in e.vars]
        inner_env = dict(env)
        inner_env.update({v: zv for v, zv in zip(e.vars, zvars)})
        body = _to_z3(e.body, inner_env, real)
        return z3.ForAll(zvars, body) if e.kind == "∀" else z3.Exists(zvars, body)
    if isinstance(e, A.Bin):
        l = _to_z3(e.lhs, env, real)
        r = _to_z3(e.rhs, env, real)
        op = e.op
        if op == "+": return l + r
        if op == "-": return l - r
        if op == "*": return l * r
        if op == "/": return l / r
        if op == "%": return l % r
        if op == "**":
            if isinstance(e.rhs, A.Num) and not e.rhs.is_float:
                out = z3.RealVal(1) if real else z3.IntVal(1)
                for _ in range(int(e.rhs.value)):
                    out = out * l
                return out
            raise _Unsupported("non-constant exponent")
        if op in ("=", "=="): return l == r
        if op in ("≠", "!="): return l != r
        if op == "<": return l < r
        if op in ("≤", "<="): return l <= r
        if op == ">": return l > r
        if op in ("≥", ">="): return l >= r
        if op in ("∧", "&&"): return z3.And(l, r)
        if op in ("∨", "||"): return z3.Or(l, r)
        raise _Unsupported(f"operator '{op}'")
    raise _Unsupported(type(e).__name__)


def prove_forall(goal, var_types: Dict[str, str], assumptions: List = ()) -> ProofResult:
    """Prove ∀ (free vars). goal holds under assumptions, by checking unsat of the negation in Z3."""
    if not _Z3:
        return ProofResult("UNAVAILABLE", "none", "Z3 not installed")
    real = all(t == "Real" for t in var_types.values()) or not var_types
    env = {n: (z3.Real(n) if t == "Real" else z3.Int(n)) for n, t in var_types.items()}
    try:
        s = z3.Solver()
        s.set("timeout", 5000)
        for a in assumptions:
            s.add(_to_z3(a, env, real))
        s.add(z3.Not(_to_z3(goal, env, real)))
    except _Unsupported as e:
        return ProofResult("UNKNOWN", "z3", f"could not encode for Z3: {e}")
    r = s.check()
    if r == z3.unsat:
        return ProofResult("PROVEN", "z3", "∀-proof: negation is unsatisfiable")
    if r == z3.sat:
        m = s.model()
        cx = {}
        for n in var_types:
            v = m.eval(env[n], model_completion=True)
            cx[n] = str(v)
        return ProofResult("REFUTED", "z3", "negation satisfiable — counterexample found", cx)
    return ProofResult("UNKNOWN", "z3", f"Z3 returned {r}")


# --------------------------------------------------------- convenience: parse a predicate string
def parse_predicate(expr_str: str, params: Dict[str, str]):
    """Parse a HARAN boolean expression (the ensures) given param types. Returns the ensures AST."""
    from haran_parser import parse
    decls = ", ".join(f"{n}: {t}" for n, t in params.items())
    src = f"fn _p({decls}) -> Bool\n  ensures {expr_str}\n  effects pure\n{{ true }}\n"
    prog = parse(src)
    if prog.errors:
        raise ValueError("; ".join(str(e) for e in prog.errors))
    return prog.get("_p").ensures


def prove_predicate(expr_str: str, params: Dict[str, str], assumptions: List[str] = ()) -> ProofResult:
    """Parse + prove a HARAN inequality/FOL predicate over the given typed params."""
    goal = parse_predicate(expr_str, params)
    asm = [parse_predicate(a, params) for a in assumptions]
    # Z3 type tag: HARAN 'Float'→Real, 'Int'/'Nat'→Int (default Real for proving over ℝ)
    var_types = {n: ("Real" if t in ("Float", "Real", "rat") else "Int") for n, t in params.items()}
    return prove_forall(goal, var_types, asm)


# --------------------------------------------------------- the extended fallback chain (X1.2)
FALLBACK_CHAIN = ["jeff", "sympy", "z3", "fuzz"]
_SAFE_FNS = {"abs", "min", "max"}
_INEQ = {"<", "≤", "<=", ">", "≥", ">="}


def choose_backend(ensures) -> str:
    """Route a spec to the first capable backend: jeff (poly identity) → sympy → z3 (inequality/FOL)
    → fuzz (opaque predicates). Demonstrates the extended chain."""
    import dataclasses
    from spec_fragment import is_exact_arith

    def walk(n):
        yield n
        if dataclasses.is_dataclass(n) and not isinstance(n, A.Span):
            for f in dataclasses.fields(n):
                v = getattr(n, f.name)
                if isinstance(v, list):
                    for x in v:
                        if dataclasses.is_dataclass(x):
                            yield from walk(x)
                elif dataclasses.is_dataclass(v):
                    yield from walk(v)

    if is_exact_arith(ensures):
        return "jeff"
    opaque = [x.func.name for x in walk(ensures)
              if isinstance(x, A.Call) and isinstance(x.func, A.Var) and x.func.name not in _SAFE_FNS]
    if opaque:
        return "fuzz"                       # references predicates Z3 can't model → bounded fuzz
    has_ineq = any(isinstance(x, A.Bin) and x.op in _INEQ for x in walk(ensures))
    has_quant = any(isinstance(x, A.Quant) for x in walk(ensures))
    if has_ineq or has_quant:
        return "z3"                          # inequality / first-order over arithmetic → Z3
    return "fuzz"
