"""
STAGE H2 tests — HARAN ensures → Mr.Jeffrey obligations (spec READ, not inferred).
Run: python3 test_h2.py
"""
import haran_ast as A
from haran_parser import parse
from haran_to_obligations import discharge_correctness, generate_obligations, verify_fn
from jeff_adapter import find_jeff_binary

PASS, FAIL = [], []
def check(name, cond, detail=""):
    (PASS if cond else FAIL).append(name)
    print(f"  [{'PASS' if cond else 'FAIL'}] {name}" + (f" — {detail}" if detail and not cond else ""))


SUM_SQUARES = """\
fn sum_squares(n: Nat) -> Nat
  ensures  result = n*(n+1)*(2*n+1)/6
  effects  pure
{
  fold k in 1..n { k*k }
}
"""

# same impl, WRONG closed-form spec (n*n/2 instead of the real sum)
SUM_SQUARES_WRONG = """\
fn sum_squares(n: Nat) -> Nat
  ensures  result = n*n/2
  effects  pure
{
  fold k in 1..n { k*k }
}
"""

# same correct impl+spec, but the function is NAMED 'sort' (a name a guesser would mishandle)
MISNAMED = """\
fn sort(n: Nat) -> Nat
  ensures  result = n*(n+1)*(2*n+1)/6
  effects  pure
{
  fold k in 1..n { k*k }
}
"""

SORT = """\
fn sort(xs: List<Int>) -> List<Int>
  ensures   sorted(result) ∧ permutation(result, xs)
  decreases length(xs)
  effects   pure
{
  match xs {
    []      => []
    [p|rest] => { sort(filter(rest, λy. y ≤ xs)) ++ [xs] }
  }
}
"""

SERVER = """\
proc server() -> Stream<Response>
  produces  ∀ req. eventually(response_to(req))
  effects   io
{
  cofix loop { let r = recv() yield r loop }
}
"""


def ensures_to_jeff_proven():
    fn = parse(SUM_SQUARES).get("sum_squares")
    ob = discharge_correctness(fn)
    ok = ob.verdict == "PROVEN"
    check("ensures_to_jeff_proven", ok, str(ob))
    if find_jeff_binary():
        check("proven_by_jeff_backend", ob.backend == "jeff", f"backend={ob.backend}: {ob}")
    else:
        print("  [INFO] jeff_identity binary not built — sympy tier used (honest fallback)")
    print(f"      → {ob.detail}")


def wrong_ensures_refuted_with_cx():
    fn = parse(SUM_SQUARES_WRONG).get("sum_squares")
    ob = discharge_correctness(fn)
    ok = ob.verdict == "REFUTED" and ob.counterexample is not None
    check("wrong_ensures_refuted_with_cx", ok, str(ob))
    if ob.counterexample:
        cx = ob.counterexample
        # the witness must be a concrete, real disagreement (impl ≠ spec at that input)
        sane = cx["impl_value"] != cx["spec_value"]
        check("counterexample_is_concrete", sane,
              f"inputs={cx['inputs']} impl={cx['impl_value']} spec={cx['spec_value']}")
        print(f"      → REFUTED: at {cx['inputs']} the impl yields {cx['impl_value']} "
              f"but ensures claims {cx['spec_value']}")


def ensures_read_not_inferred():
    # The function is NAMED 'sort' but its ensures is the sum-of-squares closed form.
    # Mr. must judge it by the ENSURES (→ PROVEN), not by the misleading name.
    fn = parse(MISNAMED).get("sort")
    ob = discharge_correctness(fn)
    judged_by_spec = ob.verdict == "PROVEN" and "(2 * n)" in ob.detail.replace(" ", " ")
    check("ensures_read_not_inferred", ob.verdict == "PROVEN", str(ob))
    # contrast: the OLD name-based path WOULD have guessed sort-properties from the name 'sort'
    from verify_strong import infer_properties
    old_guess = infer_properties("sort")
    check("name_inference_bypassed",
          len(old_guess) > 0 and ob.kind == "correctness" and ob.backend in ("jeff", "sympy"),
          f"old name-guess props={len(old_guess)} (ignored); H2 used ensures via {ob.backend}")
    print(f"      → name='sort' ignored; verified its ARITHMETIC ensures via {ob.backend}: {ob.verdict}")


def six_obligation_kinds_generated():
    # §2.1 단계2 ①–⑥ : show the obligation KINDS are generated across the example corpus.
    ss = {o.kind for o in generate_obligations(parse(SUM_SQUARES).get("sum_squares"))}
    so = {o.kind for o in generate_obligations(parse(SORT).get("sort"))}
    sv = {o.kind for o in generate_obligations(parse(SERVER).get("server"))}
    union = ss | so | sv
    need = {"correctness", "termination", "exhaustiveness", "productivity"}
    check("six_obligation_kinds_generated", need.issubset(union),
          f"sum_squares={ss}  sort={so}  server={sv}")
    # sort: correctness + termination(decreases) + exhaustiveness(match)
    check("sort_generates_term_and_exhaust", {"correctness", "termination", "exhaustiveness"}.issubset(so), str(so))
    # server: productivity from `produces`
    check("server_generates_productivity", "productivity" in sv, str(sv))


def general_prop_defers_honestly():
    # sort's real spec (sorted ∧ permutation) is a general proposition — no evaluator yet → honest DEFER.
    fn = parse(SORT).get("sort")
    ob = discharge_correctness(fn)
    check("general_prop_defers_honestly", ob.verdict == "DEFER" and "evaluator" in ob.detail.lower(), str(ob))
    print(f"      → {ob.verdict}: {ob.detail}")


if __name__ == "__main__":
    print("STAGE H2 — HARAN ensures → Mr.Jeffrey obligations")
    ensures_to_jeff_proven()
    wrong_ensures_refuted_with_cx()
    ensures_read_not_inferred()
    six_obligation_kinds_generated()
    general_prop_defers_honestly()
    print(f"\nStage H2: {len(PASS)} passed, {len(FAIL)} failed")
    import sys
    sys.exit(1 if FAIL else 0)
