"""
STAGE H6 tests — integrated HARAN→Mr pipeline, §2.2 three-verdict output.  Run: python3 test_h6.py
"""
from mr_haran import verify_program, render

PASS, FAIL = [], []
def check(name, cond, detail=""):
    (PASS if cond else FAIL).append(name)
    print(f"  [{'PASS' if cond else 'FAIL'}] {name}" + (f" — {detail}" if detail and not cond else ""))


SUM_SQUARES = """\
fn sum_squares(n: Nat) -> Nat
  ensures result = n*(n+1)*(2*n+1)/6
  effects pure
{ fold k in 1..n { k*k } }
"""

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

SERVER = """\
proc server() -> Stream<Response>
  produces ∀ req. eventually(response_to(req))
  effects io
{
  cofix loop {
    let req = receive()
    let resp = compute(req)
    yield resp
    loop
  }
}
"""

WRONG = """\
fn sum_squares(n: Nat) -> Nat
  ensures result = n*n/2
  effects pure
{ fold k in 1..n { k*k } }
"""

COLLATZ = """\
fn collatz(n: Nat) -> Nat
  effects pure
{ match n { 1 => 0  _ => match n % 2 { 0 => collatz(n / 2)  _ => collatz(3 * n + 1) } } }
"""


def _one(src):
    return verify_program(src)[0]


def _ob(rep, kind):
    return next((o for o in rep.obligations if o.kind == kind), None)


def end_to_end_sum_squares_verified():
    r = _one(SUM_SQUARES)
    ok = r.verdict == "VERIFIED" and _ob(r, "correctness").status == "PASS" and "exact" in _ob(r, "correctness").method
    check("end_to_end_sum_squares_verified", ok, render(r))


def end_to_end_sort_verified():
    r = _one(SORT)
    c, t, e = _ob(r, "correctness"), _ob(r, "termination"), _ob(r, "exhaustiveness")
    ok = (r.verdict == "VERIFIED" and c.status == "PASS" and "bounded" in c.method
          and t.status == "PASS" and "length(xs)" in t.method
          and e is not None and e.status == "PASS")
    check("end_to_end_sort_verified", ok, render(r))
    print(f"      → sort correctness: {c.method}")
    print(f"      → sort termination: {t.method}")


def end_to_end_server_verified():
    r = _one(SERVER)
    p = _ob(r, "productivity")
    check("end_to_end_server_verified", r.verdict == "VERIFIED" and p.status == "PASS", render(r))


def wrong_spec_failed_with_cx():
    r = _one(WRONG)
    c = _ob(r, "correctness")
    ok = r.verdict == "FAILED" and c.status == "FAIL" and c.counterexample is not None
    check("wrong_spec_failed_with_cx", ok, render(r))


def collatz_unknown_with_options():
    r = _one(COLLATZ)
    t = _ob(r, "termination")
    ok = r.verdict == "UNKNOWN" and t.status == "UNKNOWN" and len(t.options) == 3
    check("collatz_unknown_with_options", ok, render(r))


def three_verdict_format():
    out_v = render(_one(SUM_SQUARES))
    out_f = render(_one(WRONG))
    out_u = render(_one(COLLATZ))
    ok = (out_v.startswith("✅") and "명세 대비" in out_v
          and out_f.startswith("❌") and "어디(where)" in out_f and "반례" in out_f and "무엇을(do)" in out_f
          and out_u.startswith("⚠️") and "선택지 1" in out_u and "선택지 3" in out_u)
    check("three_verdict_format", ok)


def showcase():
    print("\n  ===== HARAN × Mr.Jeffrey — end-to-end showcase =====")
    for src in (SUM_SQUARES, SORT, SERVER, WRONG, COLLATZ):
        for r in verify_program(src):
            print(render(r))
            print()


if __name__ == "__main__":
    print("STAGE H6 — integrated pipeline (§2.2 three verdicts)")
    end_to_end_sum_squares_verified()
    end_to_end_sort_verified()
    end_to_end_server_verified()
    wrong_spec_failed_with_cx()
    collatz_unknown_with_options()
    three_verdict_format()
    showcase()
    print(f"\nStage H6: {len(PASS)} passed, {len(FAIL)} failed")
    import sys
    sys.exit(1 if FAIL else 0)
