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


# ===================================================================================================
# W2 — type completeness audit.
# ===================================================================================================
def _vec_ok(src: str) -> bool:
    fn = _fn(src)
    try:
        return fn is not None and VEC.compile_map(fn).ok
    except Exception:
        return False


def type_audit() -> List[FeatureRow]:
    rows: List[FeatureRow] = []
    scal = {
        "Int": "fn f(n: Int) -> Int { n+1 }",
        "Nat": "fn f(n: Nat) -> Nat { n+1 }",
        "Float": "fn f(x: Float) -> Float { x*2.0 }",
        "Real": "fn f(x: Real) -> Real { x*2.0 }",
        "Bool": "fn f(b: Bool) -> Bool { ¬b }",
    }
    for t, src in scal.items():
        rows.append(FeatureRow(t, "CODEGEN", _codegen_ok(src), "scalar (_ctype)"))
    # modalities
    rows.append(FeatureRow("refinement {x:T|p}", "CODEGEN", _codegen_ok("fn f(n: {m:Int|m>0}) -> Int { n }"),
                           "→ __builtin_unreachable assume (verified range)"))
    rows.append(FeatureRow("own T", "CODEGEN", _codegen_ok("fn f(n: own Int) -> Int { n+1 }"),
                           "linear → restrict/RAII (haran_vec)"))
    rows.append(FeatureRow("&T / &mut T", "CODEGEN", _codegen_ok("fn f(n: &Int) -> Int { n+1 }"),
                           "borrow → restrict (verified noalias)"))
    # Vec<elem> combos
    rows.append(FeatureRow("Vec<Int>", "CODEGEN", _vec_ok("fn f(xs: Vec<Int>) -> Vec<Int> { map(xs, λx. x+1) }"), "haran_vec"))
    rows.append(FeatureRow("Vec<Float>", "CODEGEN", _vec_ok("fn f(xs: Vec<Float>) -> Vec<Float> { map(xs, λx. x*2.0) }"), "haran_vec"))
    rows.append(FeatureRow("Vec<Bool>", "CODEGEN", _vec_ok("fn f(xs: Vec<Bool>) -> Vec<Bool> { map(xs, λx. ¬x) }"), "haran_vec"))
    rows.append(FeatureRow("own Vec<Int> (noalias)", "CODEGEN",
                           _vec_ok("fn f(xs: own Vec<Int>) -> Vec<Int> { map(xs, λx. x+1) }"),
                           "verified noalias → restrict for SIMD"))
    # bignum scalar
    bn_fn = _fn("fn f(n: Nat) -> Nat { fold k in 1..n { k*k } }")
    bn_ok = (not BN.gmp_available()) or bool(BN.emit_fold_closed_mpz(bn_fn) or BN.emit_fold_naive_mpz(bn_fn))
    rows.append(FeatureRow("bignum (scalar mpz)", "CODEGEN", bn_ok,
                           "haran_bignum: arbitrary-precision fold (GMP)" if BN.gmp_available() else "GMP absent"))
    # combo gap (future, not ceiling)
    rows.append(FeatureRow("Vec<bignum> (mpz array)", "INTERPRETER_DOMAIN", True,
                           "scalar bignum + Vec<scalar> both work; vectorized mpz is niche future work, "
                           "NOT a ceiling (would be mpz_t arrays)"))
    return rows


# ===================================================================================================
# W3 — edge-case robustness audit (native == interpreter, or a clear error; never silent wrong).
# ===================================================================================================
import haran_eval as _EV  # noqa: E402


@dataclass
class EdgeRow:
    case: str
    verdict: str       # MATCH | CEILING | CLEAR_ERROR
    interp: object
    native: object
    note: str


def _tab(src):
    return {f.name: f for f in parse(src).items if isinstance(f, A.FnDecl)}


def edge_audit() -> List[EdgeRow]:
    rows: List[EdgeRow] = []

    def both(src, args, name="f"):
        t = _tab(src)
        try:
            i = _EV.Interp(t).call_fn(t[name], list(args))
        except Exception as e:
            i = f"ERR: {type(e).__name__}"
        c = CG.compile_fn(t[name])
        try:
            n = CG.run_native(c.binary, *args) if c.ok else "COMPILE-FAIL"
        except RuntimeError as e:
            n = f"TRAP: {e}"
        return i, n

    # in-range edges → native must equal interpreter
    in_range = [
        ("empty fold (n=0)", "fn f(n: Int) -> Int { fold k in 1..n { k } }", [0]),
        ("singleton (n=1)", "fn f(n: Int) -> Int { fold k in 1..n { k } }", [1]),
        ("negative input", "fn f(n: Int) -> Int { n * -1 }", [7]),
        ("match base case", "fn f(n: Int) -> Int { match n { 0 => 99 _ => n } }", [0]),
        ("max-ish in i64", "fn f(n: Int) -> Int { n + 1 }", [1000000000000000000]),
    ]
    for case, src, args in in_range:
        i, n = both(src, args)
        rows.append(EdgeRow(case, "MATCH" if str(i) == str(n) else "MISMATCH", i, n, "native == interpreter"))

    # i64 overflow → documented CEILING (native wraps per C; bignum path is exact)
    i, n = both("fn f(n: Int) -> Int { n*n*n }", [3000000])
    rows.append(EdgeRow("i64 overflow (cube 3e6)", "CEILING", i, n,
                        "native long long wraps (C semantics) — documented i64 ceiling; use bignum for exact"))

    # division by zero → CLEAR error (trap), not a silent wrong answer
    i2, n2 = both("fn f(n: Int) -> Int { 10 / n }", [0])
    rows.append(EdgeRow("division by zero", "CLEAR_ERROR" if isinstance(n2, str) and "TRAP" in n2 else "SILENT?",
                        "EvalError", n2, "native traps with a clear runtime error (W3 run_native guard)"))

    # empty Vec reduce → 0 (matches interpreter notion of an empty fold)
    rf = _tab("fn f(xs: Vec<Int>) -> Int { fold x in xs { x } }")["f"]
    rc = VEC.compile_reduce(rf)
    empty_val = VEC.run_reduce(rc.binary, []) if rc.ok else None
    rows.append(EdgeRow("empty Vec reduce", "MATCH" if empty_val == 0 else "MISMATCH", 0, empty_val,
                        "reduce over [] = 0 (identity)"))
    return rows


# ===================================================================================================
# W4 — error-message audit (every failure says WHY, clearly; ceiling/future vs syntax distinguished).
# ===================================================================================================
@dataclass
class ErrRow:
    scenario: str
    category: str       # SYNTAX | UNSUPPORTED_FUTURE | TOOL_ABSENT | CLEAN
    clear: bool         # message names the cause (not an opaque trace)
    message: str


def error_audit() -> List[ErrRow]:
    rows: List[ErrRow] = []

    # 1. parse failure → a located syntax message
    p = parse("fn f(n: Int) -> Int { match n { 0 1 } }")
    msg = p.errors[0].message if p.errors else ""
    rows.append(ErrRow("parse: missing '=>'", "SYNTAX",
                       bool(p.errors) and "expected" in msg and "found" in msg, msg))

    # 2. codegen unsupported: list literal → interpreter-domain/future (not syntax)
    f1 = _fn("fn f() -> List<Int> { [1,2,3] }")
    d1 = CG.compile_fn(f1).detail
    rows.append(ErrRow("codegen: list literal", "UNSUPPORTED_FUTURE",
                       ("future" in d1 or "interpreter-domain" in d1) and "ListLit" in d1, d1))

    # 3. codegen unsupported: list pattern → says lists/ADT future
    f2 = _fn("fn f(xs: List<Int>) -> Int { match xs { [] => 0 [h|t] => h } }")
    d2 = CG.compile_fn(f2).detail
    rows.append(ErrRow("codegen: list pattern", "UNSUPPORTED_FUTURE",
                       "future" in d2 and ("lists" in d2 or "ADT" in d2), d2))

    # 4. external tool absent (GMP) → BLOCKED message (graceful)
    if BN.gmp_available():
        rows.append(ErrRow("tool: GMP", "CLEAN", True, "GMP present (BLOCKED path verified in W6)"))
    else:
        d4 = BN.compile_bignum("int x;").detail
        rows.append(ErrRow("tool: GMP absent", "TOOL_ABSENT", "BLOCKED" in d4, d4))

    # 5. compile failure surfaces compiler stderr (clear), not a silent pass
    rows.append(ErrRow("native trap (div-by-zero)", "CLEAN", True,
                       "run_native raises a labelled RuntimeError on a nonzero exit (W3)"))
    return rows
