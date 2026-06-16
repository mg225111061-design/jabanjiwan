"""
STAGE H1 tests — HARAN parser + AST.  Run: python3 test_h1.py
Parses the design-doc examples verbatim (§1.1 sort, §1.5 sum_squares, §1.3 server,
§1.2 dependent types) and checks 5 intentional errors are located at the right line.
"""
import haran_ast as A
from haran_parser import parse

PASS, FAIL = [], []
def check(name, cond, detail=""):
    (PASS if cond else FAIL).append(name)
    print(f"  [{'PASS' if cond else 'FAIL'}] {name}" + (f" — {detail}" if detail and not cond else ""))


# ---- design-doc sources (verbatim) ----
SORT = """\
fn sort(xs: List<Int>) -> List<Int>
  ensures   sorted(result) ∧ permutation(result, xs)
  decreases length(xs)
  effects   pure
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

SUM_SQUARES = """\
fn sum_squares(n: Nat) -> Nat
  ensures  result = n*(n+1)*(2*n+1)/6
  effects  pure
{
  fold k in 1..n { k*k }
}
"""

SERVER = """\
proc server() -> Stream<Response>
  produces  ∀ req. eventually(response_to(req))
  effects   io
{
  cofix loop {
    let req = receive()
    let resp = compute(req)
    yield resp
    loop
  }
}
"""

DEP_TYPES = """\
type Sorted<T>      = { xs: List<T> | sorted(xs) }
type NonZero        = { n: Int | n ≠ 0 }
type Vec<T, n: Nat> = { xs: List<T> | length(xs) = n }
type Prob           = { x: Float | 0.0 ≤ x ∧ x ≤ 1.0 }
"""


def parse_sort():
    p = parse(SORT)
    fn = p.get("sort")
    ok = (p.ok and fn is not None and fn.kind == "fn"
          and len(fn.params) == 1 and fn.params[0].name == "xs"
          and isinstance(fn.ret, A.TyName) and fn.ret.name == "List"
          and fn.effects == ["pure"]
          and isinstance(fn.decreases, A.Call)
          and isinstance(fn.body, A.Block) and len(fn.body.stmts) == 1
          and isinstance(fn.body.stmts[0].value, A.Match))
    m = fn.body.stmts[0].value if ok else None
    ok = ok and len(m.arms) == 2 and isinstance(m.arms[0].pattern, A.PListEmpty) \
        and isinstance(m.arms[1].pattern, A.PCons)
    check("parse_sort", ok, f"errors={[str(e) for e in p.errors]}")


def parse_sum_squares():
    p = parse(SUM_SQUARES)
    fn = p.get("sum_squares")
    ok = (p.ok and fn is not None and fn.kind == "fn"
          and fn.effects == ["pure"] and fn.decreases is None
          and isinstance(fn.ensures, A.Bin) and fn.ensures.op == "="
          and isinstance(fn.ensures.lhs, A.Var) and fn.ensures.lhs.name == "result"
          and isinstance(fn.body, A.Block)
          and isinstance(fn.body.stmts[0].value, A.Fold))
    fold = fn.body.stmts[0].value if ok else None
    ok = ok and fold.binder == "k" and isinstance(fold.domain, A.Range)
    check("parse_sum_squares", ok, f"errors={[str(e) for e in p.errors]}")


def parse_proc_server():
    p = parse(SERVER)
    s = p.get("server")
    ok = (p.ok and s is not None and s.kind == "proc"
          and isinstance(s.produces, A.Quant) and s.produces.kind == "∀"
          and s.effects == ["io"]
          and isinstance(s.ret, A.TyName) and s.ret.name == "Stream"
          and isinstance(s.body, A.Block)
          and isinstance(s.body.stmts[0].value, A.Cofix))
    cof = s.body.stmts[0].value if ok else None
    ok = ok and cof.name == "loop"
    cst = cof.body.stmts if ok else []
    ok = ok and len(cst) == 4 and isinstance(cst[0], A.Let) and cst[0].name == "req" \
        and isinstance(cst[1], A.Let) and cst[1].name == "resp" \
        and isinstance(cst[2], A.Yield) \
        and isinstance(cst[3], A.ExprStmt) and isinstance(cst[3].value, A.Var) and cst[3].value.name == "loop"
    check("parse_proc_server", ok, f"errors={[str(e) for e in p.errors]}")


def extract_ensures_clause():
    # Mr. READS the spec (no inference): sort's ensures is sorted(result) ∧ permutation(result, xs).
    p = parse(SORT)
    fn = p.get("sort")
    ens = fn.ensures
    ok = (isinstance(ens, A.Bin) and ens.op == "∧"
          and isinstance(ens.lhs, A.Call) and isinstance(ens.lhs.func, A.Var) and ens.lhs.func.name == "sorted"
          and isinstance(ens.rhs, A.Call) and ens.rhs.func.name == "permutation" and len(ens.rhs.args) == 2)
    check("extract_ensures_clause", ok, f"ensures={ens}")


def parse_dependent_type():
    p = parse(DEP_TYPES)
    vec = p.get("Vec")
    ok = (p.ok and isinstance(vec, A.TypeAlias)
          and len(vec.generics) == 2
          and vec.generics[0].name == "T" and vec.generics[0].kind is None
          and vec.generics[1].name == "n" and isinstance(vec.generics[1].kind, A.TyName)
          and vec.generics[1].kind.name == "Nat"
          and isinstance(vec.body, A.TyRefine) and vec.body.var == "xs"
          and isinstance(vec.body.base, A.TyName) and vec.body.base.name == "List"
          and isinstance(vec.body.pred, A.Bin) and vec.body.pred.op == "=")
    nz = p.get("NonZero")
    ok = ok and isinstance(nz.body, A.TyRefine) and isinstance(nz.body.pred, A.Bin) and nz.body.pred.op == "≠"
    check("parse_dependent_type", ok, f"errors={[str(e) for e in p.errors]}")


def parse_errors_located():
    cases = [
        # (source, expected_line_of_first_error, label)
        ("fn f(x: Int -> Int\n  effects pure\n{ 3 }\n", 1, "missing ')'"),
        ("fn g(xs: List<Int>) -> Int\n  effects pure\n{\n  match xs {\n    [] 5\n  }\n}\n", 5, "missing '=>'"),
        ("fn h(x: Int) -> 123 { 3 }\n", 1, "bad return type"),
        ("type T = { x: Int x > 0 }\n", 1, "refinement missing '|'"),
        ("fn k() -> Int\n  effects pure\n{\n  @ 3\n}\n", 4, "bad token in body"),
    ]
    all_ok = True
    for src, want_line, label in cases:
        p = parse(src)
        got = p.errors[0] if p.errors else None
        ok = got is not None and got.line == want_line and got.col > 0
        all_ok = all_ok and ok
        print(f"      · {label:24s} -> {('%s' % got) if got else 'NO ERROR (fake pass!)':24s}"
              f"  expected line {want_line}  [{'ok' if ok else 'WRONG'}]")
    # and a valid program must NOT spuriously error
    clean = parse(SUM_SQUARES)
    all_ok = all_ok and clean.ok
    print(f"      · clean program errors = {len(clean.errors)} (want 0)")
    check("parse_errors_located", all_ok)


def _demo():
    print("\n  demo — Mr. reads sort's spec without inference:")
    fn = parse(SORT).get("sort")
    print(f"      fn {fn.name}: spec.ensures = {_show(fn.ensures)}")
    print(f"                    spec.decreases = {_show(fn.decreases)}   effects = {fn.effects}")
    bad = parse("fn f(x: Int -> Int\n{ 3 }\n")
    print(f"      located error: {bad.errors[0]}")


def _show(e):
    if isinstance(e, A.Bin): return f"({_show(e.lhs)} {e.op} {_show(e.rhs)})"
    if isinstance(e, A.Call): return f"{_show(e.func)}({', '.join(_show(a) for a in e.args)})"
    if isinstance(e, A.Var): return e.name
    if isinstance(e, A.Num): return e.value
    return type(e).__name__


if __name__ == "__main__":
    print("STAGE H1 — HARAN parser + AST")
    parse_sort()
    parse_sum_squares()
    parse_proc_server()
    extract_ensures_clause()
    parse_dependent_type()
    parse_errors_located()
    _demo()
    print(f"\nStage H1: {len(PASS)} passed, {len(FAIL)} failed")
    import sys
    sys.exit(1 if FAIL else 0)
