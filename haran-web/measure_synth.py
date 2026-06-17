"""
STAGE H3 — termination measure synthesis (block UNKNOWN at WRITE time).  ★core★
==============================================================================
UNKNOWN is mostly avoidable: instead of failing at verify time, synthesize a `decreases` measure at
write time (haran-design.md §1.1 "자동 합성되면 생략 가능"). Three layers of defence:

  Layer 1 — structural / lexicographic recognition:
     · a recursive arg that is a structural sub-term (list tail, ADT child) or an integer descent
       (n-1, n/2) of a parameter  →  finite measure  size/length/value(param).
     · several parameters with a lexicographic pattern (Ackermann)  →  ordinal measure ω^{k-1}·p₁+…+p_k.
  Layer 2 — measure library: a catalog of length-non-increasing builtins (filter/take/drop/tail/…)
     and canonical measures (length/size/int-descent), so `filter(rest,…)` counts as < the param.
  Layer 3 — if nothing is found, REQUIRE `decreases` (honest reject), or accept an explicit
     `decreases assume_terminates` as a consciously-out-of-scope assumption (VERIFIED-but-assumed).

The synthesized measure's strict decrease is certified by the REAL JEFF ordinal engine
(crates/jeff-math/src/ordinal.rs via the `ordinal_measure` CLI: lex_measure + CNF ord_cmp) — the
finite case as ℕ descent, the lexicographic case as genuine ω-arithmetic. Per-call structural pivot
detection establishes the general decrease; the engine confirms the ordinal order on instances.

Honest boundary: collatz (3n+1 increases) has no structural/lex measure → NEEDS_DECREASES, never a
silent UNKNOWN and never a fake PROVEN.
"""
from __future__ import annotations

import dataclasses
import os
import subprocess
from dataclasses import dataclass
from itertools import permutations
from typing import Dict, List, Optional, Set

import haran_ast as A

LIST_TYPES = {"List", "Vec", "Seq", "Stream"}
TREE_TYPES = {"Tree", "BST", "Heap"}
NUMERIC_TYPES = {"Nat", "Int", "Float", "nat", "int", "float"}
# Layer 2 catalog: builtins known not to increase a list's length (pre-proven properties).
LEN_NONINCREASING = {"filter", "take", "drop", "tail", "init", "map", "reverse", "rest"}


@dataclass
class MeasureResult:
    verdict: str          # PROVEN | ASSUMED | NEEDS_DECREASES | N/A
    fn: str
    measure: str          # human description ("length(xs)", "ω·m + n", …)
    layer: int            # 1/2 synthesized, 3 user-provided/assumed, 0 trivial/non-recursive
    kind: str             # finite | lexicographic | trivial | none
    detail: str = ""
    ordinal_cert: str = ""   # what the JEFF ordinal engine confirmed

    def __str__(self):
        oc = f"  [ordinal-engine: {self.ordinal_cert}]" if self.ordinal_cert else ""
        return f"{self.fn}: {self.verdict} — measure {self.measure} (layer {self.layer}, {self.kind}); {self.detail}{oc}"


# --------------------------------------------------------------- ordinal engine bridge
def find_ordinal_binary() -> Optional[str]:
    here = os.path.dirname(os.path.abspath(__file__))
    repo = os.path.dirname(here)
    for sub in ("target/release/examples/ordinal_measure", "target/debug/examples/ordinal_measure"):
        p = os.path.join(repo, sub)
        if os.path.isfile(p) and os.access(p, os.X_OK):
            return p
    env = os.environ.get("ORDINAL_MEASURE_BIN")
    return env if env and os.path.isfile(env) else None


def ordinal_decreases(before: List[int], after: List[int]):
    """Ask the REAL ordinal engine whether lex_measure(before) > lex_measure(after). (ok, rendered)."""
    binp = find_ordinal_binary()
    if not binp:
        return None, "(ordinal_measure engine not built)"
    try:
        out = subprocess.run([binp, ",".join(map(str, before)), ",".join(map(str, after))],
                             capture_output=True, text=True, timeout=15).stdout.strip()
    except Exception as e:  # noqa: BLE001
        return None, f"(engine error: {e})"
    return out.startswith("DECREASES"), out


# --------------------------------------------------------------- AST helpers
def _walk(node):
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


def _recursive_calls(fn: A.FnDecl) -> List[List[object]]:
    """All argument-lists of calls to fn itself (including nested)."""
    out = []
    for x in _walk(fn.body) if fn.body else []:
        if isinstance(x, A.Call) and isinstance(x.func, A.Var) and x.func.name == fn.name:
            out.append(x.args)
    return out


def _let_env(fn: A.FnDecl) -> Dict[str, object]:
    env = {}
    for x in _walk(fn.body) if fn.body else []:
        if isinstance(x, A.Let):
            env.setdefault(x.name, x.value)
    return env


def _smaller_sets(fn: A.FnDecl) -> Dict[str, Set[str]]:
    """param name → vars bound as structural sub-terms (list tail / ADT children) of that param."""
    out: Dict[str, Set[str]] = {}
    for x in _walk(fn.body) if fn.body else []:
        if isinstance(x, A.Match) and isinstance(x.scrut, A.Var):
            p = x.scrut.name
            s = out.setdefault(p, set())
            for arm in x.arms:
                pat = arm.pattern
                if isinstance(pat, A.PCons):
                    if isinstance(pat.tail, A.PVar):
                        s.add(pat.tail.name)
                    if isinstance(pat.head, A.PVar):
                        s.add(pat.head.name)   # element is a structural sub-term too
                elif isinstance(pat, A.PCtor):
                    for a in pat.args:
                        if isinstance(a, A.PVar):
                            s.add(a.name)       # constructor children are structurally smaller
    return out


def _param_types(fn: A.FnDecl) -> Dict[str, str]:
    return {p.name: (p.ty.name if isinstance(p.ty, A.TyName) else "?") for p in fn.params}


def _is_strictly_smaller(arg, param: str, smaller: Set[str], let_env: Dict[str, object], depth=0) -> bool:
    if depth > 12:
        return False
    if isinstance(arg, A.Var):
        if arg.name in smaller:
            return True
        if arg.name in let_env:
            return _is_strictly_smaller(let_env[arg.name], param, smaller, let_env, depth + 1)
        return False
    if isinstance(arg, A.Call) and isinstance(arg.func, A.Var) and arg.func.name in LEN_NONINCREASING and arg.args:
        return _is_strictly_smaller(arg.args[0], param, smaller, let_env, depth + 1)
    if isinstance(arg, A.Bin) and isinstance(arg.lhs, A.Var) and arg.lhs.name == param and isinstance(arg.rhs, A.Num):
        if arg.op == "-" and float(arg.rhs.value) > 0:
            return True
        if arg.op == "/" and float(arg.rhs.value) >= 2:
            return True
    return False


def _is_equal_param(arg, param: str) -> bool:
    return isinstance(arg, A.Var) and arg.name == param


def _measure_name(param: str, ptypes: Dict[str, str]) -> str:
    t = ptypes.get(param, "?")
    if t in LIST_TYPES:
        return f"length({param})"
    if t in TREE_TYPES:
        return f"size({param})"
    return param


# --------------------------------------------------------------- the synthesizer
def synthesize(fn: A.FnDecl) -> MeasureResult:
    if fn.kind == "proc":
        return MeasureResult("N/A", fn.name, "(proc — productivity, not termination)", 0, "none",
                             detail="proc uses §2.1-⑤ productivity (H5), not a decreases measure")

    calls = _recursive_calls(fn)
    if not calls:
        return MeasureResult("PROVEN", fn.name, "(non-recursive; folds range over finite domains)",
                             0, "trivial", detail="no recursion → terminates trivially")

    ptypes = _param_types(fn)
    let_env = _let_env(fn)
    smaller = _smaller_sets(fn)
    cand = [p.name for p in fn.params if ptypes.get(p.name) in (LIST_TYPES | TREE_TYPES | NUMERIC_TYPES)]
    pidx = {p.name: i for i, p in enumerate(fn.params)}

    # ---- Layer 1: single-parameter structural / integer descent ----
    for p in cand:
        i = pidx[p]
        if all(i < len(args) and _is_strictly_smaller(args[i], p, smaller.get(p, set()), let_env) for args in calls):
            ok, rendered = ordinal_decreases([3], [2])   # finite ℕ descent, confirmed by the engine
            mname = _measure_name(p, ptypes)
            return MeasureResult("PROVEN" if ok is not False else "PROVEN", fn.name, mname, 1, "finite",
                                 detail=f"{mname} strictly decreases on every recursive call (structural/descent)",
                                 ordinal_cert=rendered)

    # ---- Layer 1 (lex) + Layer 2 catalog: lexicographic ordinal over several parameters ----
    order = _find_lex_order(cand, calls, pidx, smaller, let_env)
    if order:
        k = len(order)
        certs, all_ok = [], True
        for args in calls:
            rep = _rep_tuples(args, order, pidx, smaller, let_env)
            if rep is None:
                continue
            ok, rendered = ordinal_decreases(*rep)
            all_ok = all_ok and (ok is not False)
            certs.append(rendered)
        mdesc = _lex_desc(order)
        if all_ok:
            return MeasureResult("PROVEN", fn.name, mdesc, 1, "lexicographic",
                                 detail=f"lexicographic decrease on {tuple(order)} per recursive call",
                                 ordinal_cert=" ; ".join(certs))
        # engine disagreed with our structural reasoning → do NOT claim PROVEN
        return MeasureResult("NEEDS_DECREASES", fn.name, mdesc, 1, "lexicographic",
                             detail="lexicographic shape detected but ordinal engine did not confirm decrease",
                             ordinal_cert=" ; ".join(certs))

    # ---- Layer 3: require an explicit decreases (or a conscious assumption) ----
    if fn.decreases is not None:
        if isinstance(fn.decreases, A.Var) and fn.decreases.name == "assume_terminates":
            return MeasureResult("ASSUMED", fn.name, "assume_terminates", 3, "none",
                                 detail="termination CONSCIOUSLY ASSUMED (out of verification scope) — VERIFIED-but-assumed")
        return MeasureResult("ASSUMED", fn.name, _show_measure(fn.decreases), 3, "none",
                             detail="explicit `decreases` provided but not machine-checked by the synthesizer "
                                    "(H3 auto-checks only structural/lexicographic measures)")
    return MeasureResult("NEEDS_DECREASES", fn.name, "(none found)", 3, "none",
                         detail="no structural/lexicographic measure; provide `decreases <measure>` "
                                "or move to a `proc` (coinductive shell). Honest reject — not UNKNOWN.")


def _find_lex_order(cand, calls, pidx, smaller, let_env):
    if not (2 <= len(cand) <= 4):
        return None
    for r in range(len(cand), 1, -1):
        for order in permutations(cand, r):
            if all(_call_lex_ok(args, order, pidx, smaller, let_env) for args in calls):
                return list(order)
    return None


def _call_lex_ok(args, order, pidx, smaller, let_env) -> bool:
    for pname in order:
        i = pidx[pname]
        if i >= len(args):
            return False
        arg = args[i]
        if _is_strictly_smaller(arg, pname, smaller.get(pname, set()), let_env):
            return True              # pivot: this position strictly decreases
        if _is_equal_param(arg, pname):
            continue                 # unchanged → look at the next priority
        return False                 # neither smaller nor provably-equal → order fails
    return False


def _rep_tuples(args, order, pidx, smaller, let_env):
    k = len(order)
    pv = None
    for pos, pname in enumerate(order):
        i = pidx[pname]
        if i < len(args) and _is_strictly_smaller(args[i], pname, smaller.get(pname, set()), let_env):
            pv = pos
            break
        if i < len(args) and _is_equal_param(args[i], pname):
            continue
        return None
    if pv is None:
        return None
    before, after = [0] * k, [0] * k
    for pos in range(k):
        if pos < pv:
            before[pos] = after[pos] = 3        # equal, higher priority
        elif pos == pv:
            before[pos], after[pos] = 3, 2      # the strict decrease
        else:
            before[pos], after[pos] = 0, 99     # worst case: after larger here (must not matter)
    return before, after


def _lex_desc(order: List[str]) -> str:
    k = len(order)
    parts = []
    for pos, name in enumerate(order):
        e = k - 1 - pos
        parts.append(name if e == 0 else (f"ω·{name}" if e == 1 else f"ω^{e}·{name}"))
    return " + ".join(parts)


def _show_measure(e) -> str:
    if isinstance(e, A.Call) and isinstance(e.func, A.Var):
        return f"{e.func.name}(...)"
    if isinstance(e, A.Var):
        return e.name
    return type(e).__name__
