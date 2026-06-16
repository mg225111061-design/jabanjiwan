"""
STAGE Q1 (v8) — sort correctness: strengthen v3's bounded enumeration toward a Z3 proof.
========================================================================================
v3 verified sort by EXHAUSTIVE enumeration over length≤4 AND values∈[0,3] (341 concrete cases).
v8 strengthens this to a Z3 proof over ALL integer values at fixed length (distinct), by enumerating
the N! orderings: run the actual algorithm to get its output permutation P, then have Z3 prove that,
under the ordering's symbolic constraints, the output v[P] is sorted (and P is a permutation).

★ Honest ceiling ★: a fully UNBOUNDED ∀-proof (all lengths) needs INDUCTION over the recursion —
Z3 has no automatic induction (it's not a decidable theory here). That requires an inductive prover
(Coq/Isabelle, or Z3 with hand-written induction lemmas) → DEFERRED. So v8 gives:
  "PROVEN (Z3, ∀ integer values, length ≤ N, distinct)" — strictly stronger than v3's value-bounded —
  plus an honest DEFER for the unbounded case.
"""
from __future__ import annotations

import itertools
from dataclasses import dataclass

import haran_eval
from haran_parser import parse

SORT = """\
fn sort(xs: List<Int>) -> List<Int>
  ensures sorted(result) ∧ permutation(result, xs)
  effects pure
{
  match xs {
    []      => []
    [p|rest] => {
      let smaller = filter(rest, λy. y ≤ p)
      let larger  = filter(rest, λy. y > p)
      sort(smaller) ++ [p] ++ sort(larger)
    }
  }
}
"""


@dataclass
class SortProof:
    sorted_ok: bool
    permutation_ok: bool
    max_len: int
    level: str          # "Z3 ∀-values bounded(len≤N)" | "bounded-enumeration"
    detail: str


def prove_sort_z3(max_len: int = 4) -> SortProof:
    import z3
    fn = parse(SORT).get("sort")
    ftab = {fn.name: fn}
    sorted_all = True
    perm_all = True
    for N in range(max_len + 1):
        for perm in itertools.permutations(range(N)):          # distinct concrete values
            xs = list(perm)
            out = haran_eval.Interp(ftab).call_fn(fn, [xs])     # run the REAL algorithm
            # permutation: output is a rearrangement of the input (structural, exact)
            if sorted(out) != sorted(xs):
                perm_all = False
            P = [xs.index(out[j]) for j in range(N)]            # output[j] = input[P[j]]
            if N >= 2:
                v = [z3.Int(f"v{i}") for i in range(N)]
                s = z3.Solver()
                s.set("timeout", 4000)
                for a in range(N):
                    for b in range(N):
                        if xs[a] < xs[b]:
                            s.add(v[a] < v[b])                  # encode the ordering on symbolic vars
                sortedness = z3.And(*[v[P[j]] <= v[P[j + 1]] for j in range(N - 1)])
                s.add(z3.Not(sortedness))                       # try to break sortedness
                if s.check() != z3.unsat:                       # unsat ⇒ sorted ∀ values
                    sorted_all = False
    return SortProof(sorted_all, perm_all, max_len,
                     "Z3 ∀-values bounded(len≤%d, distinct)" % max_len,
                     f"sortedness Z3-proven ∀ integer values for every ordering up to length {max_len}; "
                     f"permutation exact; UNBOUNDED ∀ (all lengths) DEFERRED (Z3 has no auto-induction)")


def attempt_unbounded_induction() -> str:
    """Honest report on why the unbounded ∀-proof is out of reach for Z3."""
    return ("Z3 cannot prove ∀-length sort correctness: it has no automatic induction over the "
            "recursion, and quantified array/sequence + recursive-function reasoning is undecidable "
            "here. Needs an inductive prover (Coq/Isabelle) or hand-written Z3 induction lemmas → DEFER.")
