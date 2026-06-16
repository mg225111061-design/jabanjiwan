#!/usr/bin/env python3
"""
STAGE 4.4 — THE SHOWCASE: 10 real AI bugs Mr. catches (and 1 it PROVES).
========================================================================
"보세요, 진짜 잡습니다."  Each item below is the kind of code an LLM actually writes.
Mr. gives a fast, decisive, HONEST verdict on every one — and tells you the exact
input that breaks it (or the exact algebraic witness).

Three honest tiers, never blurred:
  PROVEN   exact, ALL inputs            (JEFF coefficient-zero  →  sympy CAS)
  VERIFIED bounded — strong, NOT proof  (fuzz + edges + timeout guard; unseen input may bite)
  REFUTED  here is the breaking input   (a real counterexample, shrunk to its smallest form)

Run:  python3 examples_demo.py        (or:  python3 mr.py demo)
Importable: `run_all()` returns the structured results; `main()` returns an exit code.
"""
from __future__ import annotations

import inspect
import math
from dataclasses import dataclass

from verify_strong import StrongVerifier
from jeff_adapter import backends_available, prove_identity

# ----------------------------- tiny TTY-aware color -----------------------------
import os
import sys

_COLOR = sys.stdout.isatty() and os.environ.get("NO_COLOR") is None
def _c(code, s):
    return f"\033[{code}m{s}\033[0m" if _COLOR else s
def paint(verdict):
    return {"PROVEN": _c("1;32", "PROVEN"), "VERIFIED": _c("1;33", "VERIFIED"),
            "REFUTED": _c("1;31", "REFUTED")}.get(verdict, _c("1;90", verdict))


# =============================================================================
#  The 10 AI outputs.  Candidates are written exactly as a careless model would.
# =============================================================================

# 1) classic off-by-one: "sum of 1..n" that forgets the last term.
def sum_1_to_n(n):
    return sum(range(n))            # BUG: range(n) is 0..n-1 — misses n
def _ref_sum_1_to_n(n):
    return n * (n + 1) // 2

# 2) average that divides without guarding the empty list.
def average(xs):
    return sum(xs) / len(xs)        # BUG: ZeroDivisionError on []
def _ref_average(xs):
    return sum(xs) / len(xs) if xs else 0.0

# 3) "return a sorted copy" that secretly mutates the caller's list.
def sorted_copy(xs):
    xs.sort()                       # BUG: in-place — mutates the input argument
    return xs

# 4) sort that "cleans up" by deduping — silently loses elements.
def sort_values(xs):
    return sorted(set(xs))          # BUG: drops duplicates → not a permutation

# 5) palindrome check that ignores case/space normalization.
def is_palindrome(s):
    return s == s[::-1]             # BUG: "Madam" / "Race car" wrongly reported False
def _ref_is_palindrome(s):
    t = "".join(ch.lower() for ch in s if ch.isalnum())
    return t == t[::-1]

# 6) a hang: queue drainer whose sentinel branch spins instead of progressing.
def drain_queue(n):
    while n > 0:
        if n == 100:               # BUG: "full" sentinel never decremented → infinite loop
            pass
        else:
            n -= 1
    return n
def _ref_drain_queue(n):
    return 0

# 7) "count evens" that actually counts odds (truthiness flip).
def count_evens(xs):
    return len([x for x in xs if x % 2])   # BUG: x % 2 is truthy for ODD numbers
def _ref_count_evens(xs):
    return len([x for x in xs if x % 2 == 0])

# 8) max via a zero accumulator — wrong for all-negative input.
def find_max(xs):
    m = 0                           # BUG: seeds with 0; negatives never exceed it
    for x in xs:
        if x > m:
            m = x
    return m
def _ref_find_max(xs):
    return max(xs) if xs else 0

# 9) an ALGEBRAIC claim that is actually TRUE — Mr. doesn't hunt, it PROVES it.
#    (x+1)^2  ==  x^2 + 2x + 1   for all x.
PROVE_TRUE = ("(x + 1)**2", "x**2 + 2*x + 1", ["x"])

# 10) an ALGEBRAIC claim that is FALSE — Mr. proves it WRONG, with the exact witness.
#     sum_{i=1}^n i  claimed as  n*n/2   (correct is n*(n+1)/2).
PROVE_FALSE = ("n*n/2", "n*(n+1)/2", ["n"])


@dataclass
class Example:
    id: int
    title: str
    bug: str
    kind: str           # "verify" | "prove"
    expect: str         # "REFUTED" | "PROVEN"
    cand: object        # function (verify) or expr str (prove)
    # verify:
    kinds: list = None
    ref: object = None
    func_name: str = None
    vopts: dict = None
    # prove:
    ref_expr: str = None
    variables: list = None


EXAMPLES = [
    Example(1, "sum 1..n (off-by-one)",
            "model wrote sum(range(n)) — it drops the final term n.",
            "verify", "REFUTED", sum_1_to_n, kinds=["nonneg_int"], ref=_ref_sum_1_to_n),
    Example(2, "average (empty-list crash)",
            "no guard for the empty list → ZeroDivisionError.",
            "verify", "REFUTED", average, kinds=["list_float"], ref=_ref_average),
    Example(3, "sorted_copy (mutates input)",
            "uses list.sort() in place — the caller's list is silently changed.",
            "verify", "REFUTED", sorted_copy, kinds=["list_int"], ref=sorted),
    Example(4, "sort_values (drops duplicates)",
            "sorted(set(xs)) loses repeats — the result isn't a permutation of the input.",
            "verify", "REFUTED", sort_values, kinds=["list_int"], func_name="sort_values"),
    Example(5, "is_palindrome (case/space)",
            "compares raw string — 'Madam' and 'Race car' are wrongly rejected.",
            "verify", "REFUTED", is_palindrome, kinds=["str"], ref=_ref_is_palindrome),
    Example(6, "drain_queue (infinite loop)",
            "the 'full' sentinel branch never decrements — it hangs forever.",
            "verify", "REFUTED", drain_queue, kinds=["nonneg_int"], ref=_ref_drain_queue,
            vopts={"timeout": 0.5, "fuzz": 0}),
    Example(7, "count_evens (counts odds)",
            "x % 2 is truthy for ODD numbers — the predicate is inverted.",
            "verify", "REFUTED", count_evens, kinds=["list_int"], ref=_ref_count_evens),
    Example(8, "find_max (zero seed)",
            "seeds the max with 0, so an all-negative list returns 0.",
            "verify", "REFUTED", find_max, kinds=["list_int"], ref=_ref_find_max,
            func_name="find_max"),
    Example(9, "(x+1)^2 == x^2+2x+1  [TRUE]",
            "a correct algebraic claim — Mr. PROVES it for all x (not just fuzzing).",
            "prove", "PROVEN", PROVE_TRUE[0], ref_expr=PROVE_TRUE[1], variables=PROVE_TRUE[2]),
    Example(10, "sum 1..n == n*n/2  [FALSE]",
            "a wrong closed form — Mr. proves it false and hands you the exact witness.",
            "prove", "REFUTED", PROVE_FALSE[0], ref_expr=PROVE_FALSE[1], variables=PROVE_FALSE[2]),
]


# ----------------------------- runner -----------------------------
@dataclass
class Result:
    id: int
    title: str
    expect: str
    actual: str
    ok: bool
    backend: str
    evidence: str


def _src(fn, max_lines=7):
    try:
        lines = inspect.getsource(fn).splitlines()
    except (OSError, TypeError):
        return ""
    # drop the decorator/def-leading blank, keep it short
    out = lines[:max_lines]
    return "\n".join(out)


def _run_verify(ex) -> Result:
    opts = ex.vopts or {}
    V = StrongVerifier(timeout=opts.get("timeout", 1.0),
                       fuzz_per_arg=opts.get("fuzz", 20),
                       max_tests=opts.get("max_tests", 200))
    rep = V.verify(ex.cand, ex.kinds, reference=ex.ref, func_name=ex.func_name)
    ev = str(rep)
    return Result(ex.id, ex.title, ex.expect, rep.verdict, rep.verdict == ex.expect, "fuzz", ev)


def _run_prove(ex) -> Result:
    res = prove_identity(ex.cand, ex.ref_expr, ex.variables)
    return Result(ex.id, ex.title, ex.expect, res.verdict, res.verdict == ex.expect, res.backend, str(res))


def run_all(verbose=True):
    """Run every showcase example; return a list[Result]. Used by main() and by test_stage4."""
    results = []
    for ex in EXAMPLES:
        r = _run_verify(ex) if ex.kind == "verify" else _run_prove(ex)
        results.append(r)
        if verbose:
            mark = _c("32", "✓") if r.ok else _c("31", "✗")
            print(f"\n{mark} #{ex.id:>2}  {ex.title}")
            print(f"     bug: {ex.bug}")
            if ex.kind == "verify":
                code = _src(ex.cand)
                if code:
                    print("     code:")
                    for ln in code.splitlines():
                        print(f"        {ln}")
            else:
                print(f"     claim: {ex.cand}  ==  {ex.ref_expr}   (vars {','.join(ex.variables)})")
            tag = f"{paint(r.actual)}" + (f"  [{r.backend}]" if ex.kind == "prove" else "")
            print(f"     Mr. → {tag}   (expected {ex.expect})")
            # show the decisive evidence line(s)
            for line in r.evidence.splitlines():
                print(f"        {line}")
    return results


def main():
    print("=" * 74)
    print("  Mr. Jeffrey — THE SHOWCASE: 10 AI outputs, 10 decisive verdicts")
    print("  PROVEN (exact, all inputs) · VERIFIED (bounded) · REFUTED (with witness)")
    print("=" * 74)
    b = backends_available()
    print(f"  exact backends live: JEFF={b['jeff']}  sympy={b['sympy']}"
          + ("" if b["jeff"] else "   (JEFF binary not built → sympy tier; honest fallback)"))

    results = run_all(verbose=True)

    ok = sum(1 for r in results if r.ok)
    proven = sum(1 for r in results if r.actual == "PROVEN")
    refuted = sum(1 for r in results if r.actual == "REFUTED")
    print("\n" + "=" * 74)
    print(f"  RESULT: {ok}/{len(results)} adjudicated correctly  "
          f"({refuted} bugs caught/refuted, {proven} proven for all inputs)")
    if ok == len(results):
        print("  Every AI output got the right verdict — and the exact reason why.")
    else:
        bad = [r.id for r in results if not r.ok]
        print(f"  MISMATCH on examples {bad} — investigate (do NOT ship a fake pass).")
    print("=" * 74)
    return 0 if ok == len(results) else 1


if __name__ == "__main__":
    sys.exit(main())
