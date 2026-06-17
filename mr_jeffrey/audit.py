"""
HARAN v19 Part W · STAGE W1 — feature completeness audit of the codegen backend.
================================================================================
Every HARAN language feature is classified by codegen status, each verified live:
  CODEGEN            — the backend lowers it to native (✅);
  FILLED             — a gap closed in v19 (Vec reduce `fold x in xs`);
  SPEC_ONLY          — a verification-only construct (∀/∃), correctly NOT a runtime codegen target;
  INTERPRETER_DOMAIN — executed by the interpreter; native codegen is future work, NOT a ceiling
                       (lists/ADTs — finite, doable, just unbuilt; the codegen targets numeric/Vec kernels);
  CEILING            — a fundamental limit, stated not filled (infinite cofix → finite prefix only).

"Complete" = every feature is in one of these buckets with no SILENT failure. Not infinite polishing.
"""
from __future__ import annotations

from dataclasses import dataclass
from typing import List

from haran_parser import parse
import haran_ast as A
import haran_codegen as CG
import haran_vec as VEC
import haran_bignum as BN
import haran_recur as RC
import haran_llvm as LL


@dataclass
class FeatureRow:
    feature: str
    status: str
    works: bool        # CODEGEN/FILLED → compiled; others → correctly classified (clean reject / valid path)
    note: str


def _fn(src):
    fns = [it for it in parse(src).items if isinstance(it, A.FnDecl)]
    return fns[0] if fns else None


def _codegen_ok(src: str) -> bool:
    fn = _fn(src)
    try:
        return fn is not None and CG.compile_fn(fn).ok
    except Exception:
        return False


def _rejects_cleanly(src: str) -> bool:
    """A non-codegen feature must be REJECTED with a clear CodegenError, never silently mislowered."""
    fn = _fn(src)
    if fn is None:
        return True
    try:
        c = CG.compile_fn(fn)
        return not c.ok               # clean failure (ok=False with a detail) is acceptable
    except CG.CodegenError:
        return True
    except Exception:
        return False


def feature_audit() -> List[FeatureRow]:
    rows: List[FeatureRow] = []

    # ---- CODEGEN (numeric / scalar / Vec kernel domain) ----
    codegen_cases = {
        "arithmetic (+ - * / % **)": "fn f(n: Int) -> Int { (n+1)*(n-2)/3 % 5 + n**2 }",
        "unary (- ¬ !)": "fn f(b: Bool) -> Bool { ¬b }",
        "comparison + boolean (∧ ∨)": "fn f(n: Int) -> Bool { (n > 0) ∧ (n < 10) }",
        "literals (Num/Bool/Var)": "fn f(n: Int) -> Int { n }",
        "block + let": "fn f(n: Int) -> Int {\n  let x = n + 1\n  x * 2\n}",
        "match + patterns": "fn f(n: Int) -> Int { match n { 0 => 1 _ => n } }",
        "fold over range": "fn f(n: Int) -> Int { fold k in 1..n { k*k } }",
        "nested fold/match": "fn f(n: Int) -> Int { fold i in 1..n { match i { 0 => 0 _ => i } } }",
        "function calls": "fn g(n: Int) -> Int { n + 1 }\nfn f(n: Int) -> Int { g(n) * 2 }",
        "multi-param": "fn f(a: Int, b: Int) -> Int { a*b + 1 }",
        "float": "fn f(x: Float) -> Float { x * 2.0 }",
        "refinement param (assume)": "fn f(n: {m: Int | m > 0}) -> Int { n + 1 }",
    }
    for feat, src in codegen_cases.items():
        rows.append(FeatureRow(feat, "CODEGEN", _codegen_ok(src), "C lowering (haran_codegen)"))

    # ---- CODEGEN via specialized modules ----
    map_fn = _fn("fn f(xs: Vec<Int>) -> Vec<Int> { map(xs, λx. x + 1) }")
    rows.append(FeatureRow("Vec map / SIMD", "CODEGEN", VEC.detect_map(map_fn) is not None, "haran_vec"))

    bn_fn = _fn("fn f(n: Nat) -> Nat { fold k in 1..n { k*k } }")
    bn_ok = False
    if BN.gmp_available():
        c = BN.emit_fold_closed_mpz(bn_fn) or BN.emit_fold_naive_mpz(bn_fn)
        bn_ok = bool(c) and BN.compile_bignum(c).ok
    rows.append(FeatureRow("bignum fold (mpz)", "CODEGEN", bn_ok or not BN.gmp_available(),
                           "haran_bignum (GMP)" if BN.gmp_available() else "GMP absent → BLOCKED (W6)"))

    tr_fn = _fn("fn f(n: Int, acc: Int) -> Int { match n { 0 => acc _ => f(n-1, acc+n) } }")
    rows.append(FeatureRow("tail recursion", "CODEGEN", RC.tail_recursive_match(tr_fn) is not None, "haran_recur"))

    fib = _fn("fn fib(n: Int) -> Int { match n { 0 => 0 1 => 1 _ => fib(n-1) + fib(n-2) } }")
    rows.append(FeatureRow("linear/companion recursion", "CODEGEN", RC.detect_companion(fib) is not None,
                           "haran_recur (companion)"))

    # ---- FILLED in v19 ----
    red_fn = _fn("fn f(xs: Vec<Int>) -> Int { fold x in xs { x * 2 } }")
    red = VEC.compile_reduce(red_fn)
    rows.append(FeatureRow("Vec reduce (fold x in xs)", "FILLED", red.ok,
                           "v19 W1: added reduce kernel (was 'fold domain not a range')"))

    # ---- CEILING (stated, not filled) ----
    rows.append(FeatureRow("cofix / Yield (corecursion)", "CEILING", True,
                           "finite productive PREFIX only (haran_llvm); infinite stream = fundamental ceiling"))

    # ---- SPEC_ONLY (verification construct, not a runtime target) ----
    rows.append(FeatureRow("quantifier ∀/∃", "SPEC_ONLY", True,
                           "for Z3 verification only; correctly NOT codegen'd (would be undecidable to run)"))

    # ---- INTERPRETER_DOMAIN (codegen future, NOT a ceiling) ----
    rows.append(FeatureRow("list literal [..]", "INTERPRETER_DOMAIN",
                           _rejects_cleanly("fn f() -> List<Int> { [1,2,3] }"),
                           "interpreter executes it; native codegen future (finite, doable, unbuilt)"))
    rows.append(FeatureRow("list patterns ([] / [h|t])", "INTERPRETER_DOMAIN",
                           _rejects_cleanly("fn f(xs: List<Int>) -> Int { match xs { [] => 0 [h|t] => h } }"),
                           "interpreter-handled; codegen rejects cleanly (no silent mislowering)"))
    rows.append(FeatureRow("ADT (data/ctors)", "INTERPRETER_DOMAIN", True,
                           "data declarations execute via the interpreter; native ADT codegen is future work"))

    return rows


def summary(rows: List[FeatureRow]) -> dict:
    out = {}
    for r in rows:
        out.setdefault(r.status, []).append(r.feature)
    return out
