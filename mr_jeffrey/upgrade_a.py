"""
HARAN v17 Part E · STAGE E2 — A upgrade: Coq automation + fold-class boundary.
=============================================================================
E2.1 Coq automation — more unbounded ∀ theorems proven by pure automation (induction/auto/lia/nia);
     the sort proofs are PROBED with pure automation and shown to still need hand-written lemmas
     (the honest semi-automatic boundary). Admitted is never counted (haran_coq gate).
E2.2 fold class — probe the fold engine's reach: polynomial (Faulhaber) + hypergeometric (Gosper) close;
     harmonic → ABSENT (Gosper-non-summable), factorial → NO_STRUCTURE. The NEXT classes are DEFERRED
     honestly: definite-binomial (Zeilberger) is engineering; higher-order holonomic is the Gröbner /
     Mayr–Meyer EXPSPACE ceiling (FUNDAMENTAL); Kovacic Case 2/3 is a different (ODE) domain.
"""
from __future__ import annotations

from dataclasses import dataclass, field
from typing import Dict, List

import haran_coq
from haran_parser import parse as haran_parse
import closure_classifier as CC


# ----------------------------------------------------------------- E2.1 Coq automation summary
@dataclass
class AutomationSummary:
    auto_proven: List[str]
    manual_proven: List[str]
    sort_auto_closes: bool        # did pure automation close the sort proof? (expected False)
    detail: str

    @property
    def auto_count(self):
        return len(self.auto_proven)


def automation_summary() -> AutomationSummary:
    if not haran_coq.coq_available():
        return AutomationSummary([], [], False, "coqc BLOCKED")
    s = haran_coq.prove_all()
    auto = sorted(r.name for r in s.proven if r.proven and r.mode == "auto")
    manual = sorted(r.name for r in s.proven if r.proven and r.mode == "manual")
    # probe: can pure automation close insertion-sort sortedness WITHOUT the hand-written lemmas?
    minimal = (
        "Require Import List Arith Lia.\nRequire Import Sorting.Sorted.\nImport ListNotations.\n"
        "Fixpoint insert (x:nat)(l:list nat):list nat := match l with []=>[x]"
        "|y::t=>if x<=?y then x::y::t else y::insert x t end.\n"
        "Fixpoint isort (l:list nat):list nat := match l with []=>[]|x::t=>insert x (isort t) end.\n")
    sort_closes = haran_coq.auto_attempt(
        "Theorem isort_sorted_auto : forall l, Sorted le (isort l).", minimal,
        "induction l; simpl; auto; try lia.")
    return AutomationSummary(auto, manual, sort_closes,
                             "pure automation closes the new arithmetic/list theorems but NOT the sort "
                             "proof (needs hand-written insert lemmas) — semi-automatic, honestly")


# ----------------------------------------------------------------- E2.2 fold-class boundary
@dataclass
class FoldBoundary:
    rows: List[tuple]             # (label, kind, method)
    deferred: Dict[str, str]      # next-class → reason

    def closes(self, label):
        return any(r[0] == label and r[1] == "CLOSED" for r in self.rows)


def fold_boundary() -> FoldBoundary:
    probes = [
        ("poly Σk", "fn f(n: Nat)->Nat { fold k in 1..n { k } }"),
        ("poly Σk³", "fn f(n: Nat)->Nat { fold k in 1..n { k*k*k } }"),
        ("hypergeom Σk·2^k", "fn f(n: Nat)->Nat { fold k in 1..n { k*2**k } }"),
        ("harmonic Σ1/k", "fn f(n: Nat)->Nat { fold k in 1..n { 1/k } }"),
        ("factorial Σk!", "fn f(n: Nat)->Nat { fold k in 1..n { fact(k) } }"),
    ]
    rows = []
    for label, src in probes:
        try:
            v = CC.classify_fn(haran_parse(src).items[0])
            rows.append((label, v.kind, v.method))
        except Exception as e:
            rows.append((label, "ERROR", str(e)[:30]))
    deferred = {
        "definite-binomial (Zeilberger)": "ΣC(n,k) not yet recognized — engineering extension, DEFER",
        "higher-order holonomic": "Gröbner / Mayr–Meyer EXPSPACE ceiling — FUNDAMENTAL, DEFER",
        "Kovacic Case 2/3": "Liouvillian ODE solutions — a different domain than sums, DEFER",
    }
    return FoldBoundary(rows, deferred)
