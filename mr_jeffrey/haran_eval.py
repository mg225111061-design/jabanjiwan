"""
STAGE H6 (support) — a small tree-walking evaluator for HARAN's pure fragment.
=============================================================================
Needed so the CORRECTNESS obligation of a *general proposition* (e.g. sort's `sorted(result) ∧
permutation(result, xs)`) can be discharged by BOUNDED FUZZING (§2.1 단계3 third engine) — run the
HARAN implementation on random inputs and check `ensures`. This is honestly TESTED, not proven: a
counterexample is definitive (FAILED), but passing N cases is evidence, not a ∀-proof (Boundary 1).

Supported pure fragment: match (list/cons/num/bool/wildcard patterns), let, fold, λ + application,
list literals, ++, arithmetic/comparison/boolean ops, recursion, and a small builtin catalog
(filter, map, length, sorted, permutation). Anything else → EvalError → the caller reports UNKNOWN.
"""
from __future__ import annotations

import random
from typing import List, Optional

import haran_ast as A


class EvalError(Exception):
    pass


class Closure:
    __slots__ = ("params", "body", "env")
    def __init__(self, params, body, env):
        self.params, self.body, self.env = params, body, dict(env)


class Interp:
    def __init__(self, ftab: dict, max_steps: int = 400_000):
        self.ftab = ftab
        self.steps = 0
        self.max_steps = max_steps

    def _tick(self):
        self.steps += 1
        if self.steps > self.max_steps:
            raise EvalError("step limit exceeded (possible non-termination)")

    def call_fn(self, fn: A.FnDecl, argvals: List[object]):
        self._tick()
        env = {p.name: v for p, v in zip(fn.params, argvals)}
        return self.eval(fn.body, env)

    def apply_closure(self, clo: Closure, *vals):
        env = dict(clo.env)
        for p, v in zip(clo.params, vals):
            env[p] = v
        return self.eval(clo.body, env)

    def eval(self, node, env):
        self._tick()
        if isinstance(node, A.Num):
            return float(node.value) if node.is_float else int(node.value)
        if isinstance(node, A.BoolLit):
            return node.value
        if isinstance(node, A.Var):
            if node.name in env:
                return env[node.name]
            raise EvalError(f"unbound variable '{node.name}'")
        if isinstance(node, A.ListLit):
            return [self.eval(e, env) for e in node.elems]
        if isinstance(node, A.Lambda):
            return Closure(node.params, node.body, env)
        if isinstance(node, A.Un):
            v = self.eval(node.operand, env)
            if node.op == "-":
                return -v
            if node.op in ("¬", "!"):
                return not v
            raise EvalError(f"unary op '{node.op}'")
        if isinstance(node, A.Bin):
            return self._binop(node, env)
        if isinstance(node, A.Call):
            return self._call(node, env)
        if isinstance(node, A.Block):
            return self._block(node, env)
        if isinstance(node, A.Match):
            return self._match(node, env)
        if isinstance(node, A.Fold):
            return self._fold(node, env)
        if isinstance(node, A.Range):
            return ("range", self.eval(node.lo, env), self.eval(node.hi, env))
        raise EvalError(f"cannot evaluate {type(node).__name__}")

    def _binop(self, node, env):
        op = node.op
        if op in ("∧", "&&"):
            return bool(self.eval(node.lhs, env)) and bool(self.eval(node.rhs, env))
        if op in ("∨", "||"):
            return bool(self.eval(node.lhs, env)) or bool(self.eval(node.rhs, env))
        a = self.eval(node.lhs, env)
        b = self.eval(node.rhs, env)
        if op == "+":
            return a + b
        if op == "-":
            return a - b
        if op == "*":
            return a * b
        if op == "/":
            return a // b if isinstance(a, int) and isinstance(b, int) and b != 0 and a % b == 0 else a / b
        if op == "%":
            return a % b
        if op == "**":
            return a ** b
        if op == "++":
            return list(a) + list(b)
        if op in ("=", "=="):
            return a == b
        if op in ("≠", "!="):
            return a != b
        if op == "<":
            return a < b
        if op in ("≤", "<="):
            return a <= b
        if op == ">":
            return a > b
        if op in ("≥", ">="):
            return a >= b
        raise EvalError(f"binary op '{op}'")

    def _call(self, node, env):
        if not isinstance(node.func, A.Var):
            raise EvalError("only named calls supported")
        name = node.func.name
        args = [self.eval(a, env) for a in node.args]
        b = _BUILTINS.get(name)
        if b is not None:
            return b(self, args)
        if name in self.ftab:
            return self.call_fn(self.ftab[name], args)
        raise EvalError(f"unknown function '{name}'")

    def _block(self, node, env):
        env = dict(env)
        val = None
        for st in node.stmts:
            if isinstance(st, A.Let):
                env[st.name] = self.eval(st.value, env)
            elif isinstance(st, A.ExprStmt):
                val = self.eval(st.value, env)
            elif isinstance(st, A.Yield):
                val = self.eval(st.value, env)
            else:
                raise EvalError(f"stmt {type(st).__name__}")
        return val

    def _match(self, node, env):
        scrut = self.eval(node.scrut, env)
        for arm in node.arms:
            bound = {}
            if _match_pat(arm.pattern, scrut, bound):
                e2 = dict(env)
                e2.update(bound)
                return self.eval(arm.body, e2)
        raise EvalError("no matching arm (non-exhaustive at runtime)")

    def _fold(self, node, env):
        dom = self.eval(node.domain, env)
        if not (isinstance(dom, tuple) and dom[0] == "range"):
            raise EvalError("fold domain is not a range")
        _, lo, hi = dom
        acc = 0
        for k in range(int(lo), int(hi) + 1):   # inclusive (matches H4)
            e2 = dict(env)
            e2[node.binder] = k
            acc += self.eval(node.body, e2)
        return acc


def _match_pat(pat, value, bound: dict) -> bool:
    if isinstance(pat, A.PWild):
        return True
    if isinstance(pat, A.PVar):
        bound[pat.name] = value
        return True
    if isinstance(pat, A.PNum):
        return value == int(pat.value)
    if isinstance(pat, A.PBool):
        return value == pat.value
    if isinstance(pat, A.PListEmpty):
        return isinstance(value, list) and len(value) == 0
    if isinstance(pat, A.PCons):
        if isinstance(value, list) and len(value) >= 1:
            return _match_pat(pat.head, value[0], bound) and _match_pat(pat.tail, value[1:], bound)
        return False
    if isinstance(pat, A.PList):
        if isinstance(value, list) and len(value) == len(pat.elems):
            return all(_match_pat(p, v, bound) for p, v in zip(pat.elems, value))
        return False
    raise EvalError(f"pattern {type(pat).__name__}")


# ---- builtin catalog (pure) ----
def _b_filter(it: Interp, args):
    lst, clo = args[0], args[1]
    return [x for x in lst if it.apply_closure(clo, x)]
def _b_map(it: Interp, args):
    lst, clo = args[0], args[1]
    return [it.apply_closure(clo, x) for x in lst]
def _b_length(it, args):
    return len(args[0])
def _b_sorted(it, args):
    l = args[0]
    return all(l[i] <= l[i + 1] for i in range(len(l) - 1))
def _b_permutation(it, args):
    return sorted(args[0]) == sorted(args[1])

_BUILTINS = {"filter": _b_filter, "map": _b_map, "length": _b_length,
             "sorted": _b_sorted, "permutation": _b_permutation}


# ---- input generation + bounded fuzzing ----
def _gen_input(ty, rng: random.Random):
    if isinstance(ty, A.TyName):
        if ty.name in ("List", "Vec", "Seq"):
            return [rng.randint(-9, 9) for _ in range(rng.randint(0, 6))]
        if ty.name in ("Nat",):
            return rng.randint(0, 20)
        if ty.name in ("Int",):
            return rng.randint(-20, 20)
        if ty.name in ("Bool",):
            return rng.choice([True, False])
        if ty.name in ("Float",):
            return rng.uniform(-10, 10)
    raise EvalError(f"cannot generate inputs for type {getattr(ty, 'name', ty)}")


def bounded_fuzz(fn: A.FnDecl, ftab: dict, ensures, n: int = 200, seed: int = 20260616):
    """Run fn on n random inputs; check `ensures`. Returns (status, detail, counterexample).
    status ∈ {PASS, FAIL, UNKNOWN}; deterministic (fixed seed)."""
    rng = random.Random(seed)
    try:
        for _ in range(n):
            args = [_gen_input(p.ty, rng) for p in fn.params]
            it = Interp(ftab)
            result = it.call_fn(fn, args)
            env = {p.name: a for p, a in zip(fn.params, args)}
            env["result"] = result
            ok = it.eval(ensures, env)
            if not bool(ok):
                inputs = {p.name: a for p, a in zip(fn.params, args)}
                return ("FAIL", f"ensures false on a random input", {"inputs": inputs, "result": result})
    except EvalError as e:
        return ("UNKNOWN", f"cannot evaluate ({e})", None)
    return ("PASS", f"no counterexample in {n} random cases (tested, not proven)", None)
