"""
STAGE V1 tests — structuring maximization (richer specs, honest fragment, coverage).
Run: python3 test_v1.py
"""
import haran_ast as A
from haran_parser import parse
from spec_fragment import classify, coverage, render_coverage, BUILTIN_PREDS
import spec_infer

PASS, FAIL = [], []
def check(name, cond, detail=""):
    (PASS if cond else FAIL).append(name)
    print(f"  [{'PASS' if cond else 'FAIL'}] {name}" + (f" — {detail}" if detail and not cond else ""))


def rich_refinement_type():
    # compound predicate, relational refinement, dependent function return type
    src = """\
type Prob = { x: Float | 0.0 ≤ x ∧ x ≤ 1.0 }
type Pos  = { n: Int | n > 0 ∧ n < 100 }
fn dup(xs: List<Int>) -> { ys: List<Int> | length(ys) = length(xs) }
  effects pure
{ xs }
"""
    p = parse(src)
    prob = p.get("Prob")
    pos = p.get("Pos")
    dup = p.get("dup")
    ok = (p.ok
          and isinstance(prob.body, A.TyRefine) and isinstance(prob.body.pred, A.Bin) and prob.body.pred.op == "∧"
          and isinstance(pos.body, A.TyRefine) and isinstance(pos.body.pred, A.Bin) and pos.body.pred.op == "∧"
          # dependent return type: result type's predicate relates to the argument xs
          and isinstance(dup.ret, A.TyRefine) and dup.ret.var == "ys"
          and isinstance(dup.ret.pred, A.Bin) and dup.ret.pred.op == "="
          and isinstance(dup.ret.pred.lhs, A.Call) and dup.ret.pred.lhs.func.name == "length")
    check("rich_refinement_type", ok, f"errors={[str(e) for e in p.errors]}")
    print(f"      → dependent return type: {{ ys | length(ys) = length(xs) }} parsed; compound refinements parsed")


def forall_exists_ensures():
    server = """\
proc server() -> Stream<Response>
  produces ∀ req. eventually(response_to(req))
  effects io
{ cofix loop { yield 1  loop } }
"""
    quant_eval = """\
fn f(xs: List<Int>) -> List<Int>
  ensures ∀ k. sorted(result)
  effects pure
{ xs }
"""
    ps = parse(server)
    pq = parse(quant_eval)
    eval_names = BUILTIN_PREDS
    sc_server = classify(ps.get("server").produces, eval_names)
    sc_quant = classify(pq.get("f").ensures, eval_names)
    # ∀ over a NON-evaluable temporal predicate (eventually) → outside the verifiable fragment
    ok1 = sc_server.fragment == "QUANTIFIED_FOL" and sc_server.verifiability == "OUTSIDE_FRAGMENT"
    # ∀ over an evaluable predicate → bounded-only, and crucially NOT claimed exactly provable (no Z3)
    ok2 = sc_quant.fragment == "QUANTIFIED_FOL" and sc_quant.verifiability == "BOUNDED_ONLY"
    check("forall_exists_ensures", ok1 and ok2, f"server={sc_server}; quant={sc_quant}")
    print(f"      → ∀ eventually(...) : {sc_server.verifiability} (honest: no Z3 → outside fragment)")
    print(f"      → ∀ sorted(result) : {sc_quant.verifiability} (tested, not an exact ∀-proof)")


def recursive_predicate_spec():
    # user-DEFINED recursive predicate all_pos used in another function's ensures
    src = """\
fn all_pos(xs: List<Int>) -> Bool
  effects pure
{ match xs { [] => true  [x|rest] => (x > 0) ∧ all_pos(rest) } }

fn pos_filter(xs: List<Int>) -> List<Int>
  ensures all_pos(result)
  effects pure
{ filter(xs, λy. y > 0) }
"""
    p = parse(src)
    eval_names = BUILTIN_PREDS | {f.name for f in p.fns()}
    sc = classify(p.get("pos_filter").ensures, eval_names)
    ok = sc.verifiability == "BOUNDED_ONLY" and "all_pos" in sc.predicates
    check("recursive_predicate_spec_classified", ok, str(sc))
    # and the integrated verifier actually discharges it via the evaluator calling the user predicate
    from mr_haran import verify_program
    reps = {r.name: r for r in verify_program(src)}
    check("recursive_predicate_spec_verified", reps["pos_filter"].verdict == "VERIFIED",
          str(reps["pos_filter"].verdict))
    print(f"      → user predicate all_pos recognized ({sc.verifiability}); pos_filter ⊨ all_pos(result): "
          f"{reps['pos_filter'].verdict}")


def spec_inference_proposes():
    # sort has NO ensures → inference proposes tested structural candidates (needs confirmation)
    sort = """\
fn sort(xs: List<Int>) -> List<Int>
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
    p = parse(sort)
    ftab = {f.name: f for f in p.fns()}
    cands = spec_infer.infer(p.get("sort"), ftab)
    texts = {c.text for c in cands}
    ok = any("permutation" in t for t in texts) and any("sorted(result)" in t for t in texts)
    check("spec_inference_proposes", ok, f"candidates={texts}")
    print(f"      → inferred (needs confirmation): {spec_infer.propose_ensures(p.get('sort'), ftab)}")


def spec_coverage_report():
    src = """\
fn sum_squares(n: Nat) -> Nat
  ensures result = n*(n+1)*(2*n+1)/6
  effects pure
{ fold k in 1..n { k*k } }

fn cube_sum(n: Nat) -> Nat
  ensures result = (n*(n+1)/2)*(n*(n+1)/2)
  effects pure
{ fold k in 1..n { k**3 } }

fn sort(xs: List<Int>) -> List<Int>
  effects pure
{ match xs { [] => []  [p|rest] => sort(filter(rest, λy. y ≤ p)) ++ [p] ++ sort(filter(rest, λy. y > p)) } }

fn evens(xs: List<Int>) -> List<Int>
  ensures length(result) <= length(xs)
  effects pure
{ filter(xs, λy. y % 2 == 0) }

proc server() -> Stream<Response>
  produces ∀ req. eventually(response_to(req))
  effects io
{ cofix loop { yield 1  loop } }

fn mystery(n: Nat) -> Nat
  effects pure
{ n }
"""
    cov = coverage(src)
    print("\n" + render_coverage(cov))
    have = {r.status for r in cov.rows}
    ok = ({"verifiable", "bounded", "inferred", "outside", "unspecified"}.issubset(have)
          and cov.pct("verifiable", "bounded", "inferred") >= 60)
    check("spec_coverage_report", ok, f"statuses={have} structuring={cov.pct('verifiable','bounded','inferred')}%")


if __name__ == "__main__":
    print("STAGE V1 — structuring maximization")
    rich_refinement_type()
    forall_exists_ensures()
    recursive_predicate_spec()
    spec_inference_proposes()
    spec_coverage_report()
    print(f"\nStage V1: {len(PASS)} passed, {len(FAIL)} failed")
    import sys
    sys.exit(1 if FAIL else 0)
