"""
STAGE 2 — strengthened verification (Mr.'s real weapon).
========================================================
Extends `verify_core` (does NOT replace it) with:
  2.1 more input types: float, list_float, dict, tuple, 2d-list, bool, negative/big edges.
  2.2 performance guard: per-test TIMEOUT (the verifier must never hang) + size caps.
  2.3 auto property inference from the function name (sort/reverse/abs/…), so users needn't.
  2.4 sandbox: detect file/network side-effects (open/socket) and input mutation.
  2.5 shrinking: minimize a counterexample so the feedback is the SMALLEST breaking input.

Honesty unchanged: a clean run is "VERIFIED (bounded)" — strong evidence, NOT a proof; an unseen
input may still hold a bug. Exact proof (all inputs) comes only from the JEFF/sympy exact layer.
"""
from __future__ import annotations
import copy
import random
import string
import threading
from dataclasses import dataclass, field
from typing import Any, Callable

from verify_core import Counterexample, VerdictReport


# ----------------------------- 2.1 generators -----------------------------
def _int_edge():
    return [0, 1, -1, 2, -2, 3, 10, -10, 100, -100, 7, 13, 2**31, -(2**31), 2**31 - 1, 999983, -999983]
def _float_edge():
    return [0.0, 1.0, -1.0, 0.5, -0.5, 2.0, 3.14159, -3.14159, 1e9, -1e9, 1e-9, 100.0, -0.0]
def _str_structured():
    return ["", "a", "ab", "aba", "abc", "racecar", "Hello, World!", "  ", "AaBb", "12321",
            "A man a plan a canal Panama", "Not a palindrome", "Madam", "ab cd"]
def _list_int_structured():
    return [[], [0], [1], [-1], [1, 2, 3], [3, 1, 2], [5, 5, 5], list(range(20)),
            list(range(20, 0, -1)), [10**6, -10**6], [2, 2, 2], [-1, -2, -3]]
def _list_float_structured():
    return [[], [0.0], [1.5], [-1.5, 2.5], [3.14, 2.71, 1.41], [1e9, -1e9], [0.1, 0.2, 0.3]]
def _dict_structured():
    return [{}, {"a": 1}, {"x": 1, "y": 2}, {1: "a", 2: "b"}, {"k": [1, 2]}, {0: 0, 1: 1, 2: 4}]
def _tuple_structured():
    return [(), (0,), (1, 2), (-1, 1), (1, 2, 3), ("a", 1), (0, 0, 0)]
def _list2d_structured():
    return [[], [[]], [[1]], [[1, 2], [3, 4]], [[1], [2], [3]], [[0, 0], [0, 0]], [[1, 2, 3], [4, 5, 6]]]
def _bool_all():
    return [True, False]

def _rand(kind, n):
    out = []
    for _ in range(n):
        if kind == "int":
            out.append(random.randint(-10**12, 10**12))
        elif kind == "nonneg_int":
            out.append(random.randint(0, 300))
        elif kind == "float":
            out.append(random.uniform(-1e6, 1e6) * (10 ** random.randint(-6, 6)))
        elif kind == "str":
            L = random.randint(0, 12)
            s = "".join(random.choice(string.ascii_letters + " ,.!?") for _ in range(L))
            if random.random() < 0.3 and s:
                s = s + s[::-1]
            out.append(s)
        elif kind == "list_int":
            out.append([random.randint(-1000, 1000) for _ in range(random.randint(0, 15))])
        elif kind == "list_float":
            out.append([random.uniform(-1000, 1000) for _ in range(random.randint(0, 12))])
        elif kind == "dict":
            out.append({i: random.randint(-100, 100) for i in range(random.randint(0, 6))})
        elif kind == "tuple":
            out.append(tuple(random.randint(-100, 100) for _ in range(random.randint(0, 5))))
        elif kind == "list2d":
            r = random.randint(0, 4)
            out.append([[random.randint(-100, 100) for _ in range(random.randint(0, 4))] for _ in range(r)])
        elif kind == "bool":
            out.append(random.random() < 0.5)
    return out

_STRUCT = {
    "int": _int_edge, "nonneg_int": lambda: [0, 1, 2, 3, 5, 7, 10, 15, 20, 30, 50, 100],
    "float": _float_edge, "str": _str_structured, "list_int": _list_int_structured,
    "list_float": _list_float_structured, "dict": _dict_structured, "tuple": _tuple_structured,
    "list2d": _list2d_structured, "bool": _bool_all,
}

def samples(kind, fuzz):
    if kind not in _STRUCT:
        raise ValueError(f"unknown arg kind: {kind} (known: {sorted(_STRUCT)})")
    return _STRUCT[kind]() + _rand(kind, fuzz)


# ----------------------------- 2.2 performance guard -----------------------------
class _Timeout(Exception):
    pass

def call_with_timeout(fn, args, timeout):
    """Run fn(*args) with a hard wall-clock TIMEOUT so the verifier never hangs. On timeout the
    worker thread is abandoned (daemon — reaped at process exit); we return a timeout marker and
    move on. Returns (ok, result, error_str_or_None)."""
    box = {}
    def worker():
        try:
            box["r"] = fn(*args)
        except Exception as e:  # noqa: BLE001 - candidate may raise anything
            box["e"] = f"{type(e).__name__}: {e}"
    th = threading.Thread(target=worker, daemon=True)
    th.start()
    th.join(timeout)
    if th.is_alive():
        return False, None, f"TIMEOUT (> {timeout}s — likely infinite loop or explosive input)"
    if "e" in box:
        return False, None, box["e"]
    return True, box.get("r"), None


# ----------------------------- 2.3 auto property inference -----------------------------
def infer_properties(func_name: str):
    """Generate common properties from the function name so the user need not write them.
    Each property is `f(inputs_tuple, output) -> bool`. Returns a list (possibly empty)."""
    nm = func_name.lower()
    props = []
    def _p(fn, name):
        fn.__name__ = name
        return fn

    if "sort" in nm:
        props.append(_p(lambda i, o: isinstance(o, list) and all(o[k] <= o[k + 1] for k in range(len(o) - 1)),
                        "output_is_sorted"))
        props.append(_p(lambda i, o: isinstance(o, list) and sorted(o) == sorted(i[0]),
                        "output_is_a_permutation_of_input"))
    if "reverse" in nm:
        props.append(_p(lambda i, o: hasattr(o, "__len__") and len(o) == len(i[0]), "length_preserved"))
        props.append(_p(lambda i, o: list(o[::-1]) == list(i[0]), "double_reverse_is_identity"))
    if "abs" in nm or "magnitude" in nm:
        props.append(_p(lambda i, o: o >= 0, "result_nonnegative"))
    if nm.startswith("is_") or nm.startswith("has_"):
        props.append(_p(lambda i, o: isinstance(o, bool), "predicate_returns_bool"))
    if "max" in nm:
        props.append(_p(lambda i, o: (not i[0]) or o in i[0], "max_is_an_element"))
    if "min" in nm:
        props.append(_p(lambda i, o: (not i[0]) or o in i[0], "min_is_an_element"))
    return props


# ----------------------------- 2.4 sandbox (file/network) -----------------------------
class _Sandbox:
    """Context manager that blocks file open and socket creation, so a side-effecting candidate is
    caught (a verified function must be pure)."""
    def __enter__(self):
        import builtins
        import socket
        self._open = builtins.open
        self._sock = socket.socket
        def blocked_open(*a, **k):
            raise PermissionError("file access attempted")
        def blocked_socket(*a, **k):
            raise PermissionError("network access attempted")
        builtins.open = blocked_open
        socket.socket = blocked_socket
        self._b, self._s = builtins, socket
        return self
    def __exit__(self, *exc):
        self._b.open = self._open
        self._s.socket = self._sock
        return False


# ----------------------------- 2.5 shrinking -----------------------------
def _shrink_value(v):
    """Yield strictly-"smaller" candidates for one value."""
    if isinstance(v, bool):
        return []
    if isinstance(v, int):
        out = []
        if v != 0:
            out.append(0)
            out.append(v // 2)
            out.append(v - (1 if v > 0 else -1))
        return [x for x in out if x != v]
    if isinstance(v, float):
        return [x for x in (0.0, v / 2.0) if x != v]
    if isinstance(v, str):
        return [v[:-1], v[1:], v[: len(v) // 2]] if v else []
    if isinstance(v, list):
        if not v:
            return []
        out = [v[:-1], v[1:], v[: len(v) // 2]]
        # also try shrinking the first element
        for s in _shrink_value(v[0]):
            out.append([s] + v[1:])
        return out
    if isinstance(v, tuple):
        return [tuple(x) for x in _shrink_value(list(v))]
    return []

def shrink(inputs: tuple, still_fails: Callable[[tuple], bool], rounds=200) -> tuple:
    """Greedily minimize `inputs` while `still_fails(inputs)` stays True. Hypothesis-style."""
    cur = inputs
    for _ in range(rounds):
        improved = False
        for idx in range(len(cur)):
            for sv in _shrink_value(cur[idx]):
                cand = cur[:idx] + (sv,) + cur[idx + 1:]
                try:
                    if still_fails(cand):
                        cur = cand
                        improved = True
                        break
                except Exception:
                    pass
            if improved:
                break
        if not improved:
            break
    return cur


# ----------------------------- the strengthened verifier -----------------------------
@dataclass
class StrongVerifier:
    max_tests: int = 400
    fuzz_per_arg: int = 50
    seed: int = 12345
    timeout: float = 2.0
    sandbox: bool = True

    def verify(self, candidate, arg_kinds, reference=None, properties=None,
               examples=None, pure=True, func_name=None) -> VerdictReport:
        random.seed(self.seed)
        props = list(properties or [])
        if func_name:
            props += infer_properties(func_name)  # 2.3 auto-inferred
        if reference is None and not props and not examples:
            return VerdictReport("ERROR", note="give a reference, properties, or examples")
        sample_lists = [samples(k, self.fuzz_per_arg) for k in arg_kinds]
        test_inputs = self._combine(sample_lists)
        counters, run = [], 0

        def fails(inputs):
            """Re-evaluate the full check on `inputs`; return a Counterexample or None (for shrink)."""
            snapshot = copy.deepcopy(inputs)
            call_args = copy.deepcopy(inputs)
            runner = (lambda: self._sandboxed(candidate, call_args)) if self.sandbox else None
            if self.sandbox:
                ok, got, err = call_with_timeout(lambda: self._sandboxed(candidate, call_args), (), self.timeout)
            else:
                ok, got, err = call_with_timeout(candidate, tuple(call_args), self.timeout)
            _ = runner
            if not ok:
                kind = "side_effect" if err and ("file access" in err or "network access" in err) else "crash"
                return Counterexample(inputs, None, None, kind, err)
            if pure and call_args != snapshot:
                ch = [(snapshot[i], call_args[i]) for i in range(len(snapshot)) if call_args[i] != snapshot[i]]
                return Counterexample(inputs, None, got, "side_effect", f"arg changed {ch[0][0]!r} -> {ch[0][1]!r}")
            if reference is not None:
                try:
                    expected = reference(*copy.deepcopy(inputs))
                except Exception:
                    expected = "<reference crashed>"
                if got != expected:
                    return Counterexample(inputs, expected, got, "wrong_output")
            if examples and inputs in examples and got != examples[inputs]:
                return Counterexample(inputs, examples[inputs], got, "wrong_output")
            for prop in props:
                try:
                    ok2 = prop(inputs, got)
                except Exception as e:  # noqa: BLE001
                    ok2 = False
                    return Counterexample(inputs, None, got, "property_violated", f"{getattr(prop,'__name__','prop')} raised {e}")
                if not ok2:
                    return Counterexample(inputs, None, got, "property_violated", getattr(prop, "__name__", "property"))
            return None

        for inputs in test_inputs:
            run += 1
            c = fails(inputs)
            if c is not None:
                # 2.5 shrink the counterexample to the smallest still-failing input
                minimal = shrink(inputs, lambda x: fails(x) is not None)
                cm = fails(minimal) or c
                counters.append(cm)
                if len(counters) >= 6:
                    break
        if counters:
            return VerdictReport("REFUTED", tests_run=run, counterexamples=counters)
        return VerdictReport("VERIFIED", tests_run=run,
                             note="(bounded+fuzzed+timeout-guarded — strong evidence, NOT a proof; "
                                  "exact proof for numeric specs via JEFF/sympy)")

    def _sandboxed(self, candidate, call_args):
        with _Sandbox():
            return candidate(*call_args)

    def _combine(self, sample_lists):
        if len(sample_lists) == 1:
            return [(x,) for x in sample_lists[0]][: self.max_tests]
        import itertools
        out = []
        m = max(len(s) for s in sample_lists)
        for i in range(m):
            out.append(tuple(s[i % len(s)] for s in sample_lists))
        for combo in itertools.product(*[s[:5] for s in sample_lists]):
            out.append(combo)
            if len(out) >= self.max_tests:
                break
        seen, uniq = set(), []
        for t in out:
            k = repr(t)
            if k not in seen:
                seen.add(k)
                uniq.append(t)
        return uniq[: self.max_tests]
