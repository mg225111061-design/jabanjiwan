"""
STAGE X3 tests — proof-power completion (proven where possible; contract/aliasing/ADT).
Run: python3 test_x3.py
"""
from haran_parser import parse
from prove_exact import (prove_correctness, check_contract, check_aliasing,
                         check_adt_exhaustiveness, build_data_index, proven_ratio)
from z3_adapter import z3_available

PASS, FAIL, SKIP = [], [], []
def check(name, cond, detail=""):
    (PASS if cond else FAIL).append(name)
    print(f"  [{'PASS' if cond else 'FAIL'}] {name}" + (f" — {detail}" if detail and not cond else ""))
def skip(name, why):
    SKIP.append(name); print(f"  [SKIP] {name} — {why}")


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

CONTRACT = """\
fn recip(x: Float) -> Float
  requires x ≠ 0
  effects pure
{ 1 / x }

fn use_ok(a: Float) -> Float
  requires a > 1
  effects pure
{ recip(a) }

fn use_bad(a: Float) -> Float
  effects pure
{ recip(a) }
"""

ALIASING = """\
fn consume(b: own Buffer) -> Int
  effects pure
{ use(b) + use(b) }

fn ok_use(b: own Buffer) -> Int
  effects pure
{ use(b) }
"""

ADT = """\
data Tree { Leaf(Int)  Node(Tree, Tree) }

fn tsum(t: Tree) -> Int
  effects pure
{ match t { Leaf(x) => x  Node(l, r) => tsum(l) + tsum(r) } }

fn tbad(t: Tree) -> Int
  effects pure
{ match t { Leaf(x) => x } }
"""


def sort_proven_if_possible():
    p = parse(SORT)
    ftab = {f.name: f for f in p.fns()}
    v = prove_correctness(p.get("sort"), ftab)
    # honest upgrade: exhaustive over all |xs|≤4 (vals 0..3) → PROVEN-BOUNDED (stronger than random fuzz)
    ok = v.tier == "PROVEN-BOUNDED"
    check("sort_proven_if_possible", ok, str(v))
    print(f"      → sort: {v.tier} — {v.detail}")
    print("        (honest: bounded-exhaustive proof, NOT a full ∀-proof — Z3 array-induction out of scope)")


def contract_checked():
    if not z3_available():
        skip("contract_checked", "Z3 absent")
        return
    p = parse(CONTRACT)
    ftab = {f.name: f for f in p.fns()}
    ok_checks = check_contract(p.get("use_ok"), ftab)
    bad_checks = check_contract(p.get("use_bad"), ftab)
    ok = (len(ok_checks) == 1 and ok_checks[0].verdict == "PASS"
          and len(bad_checks) == 1 and bad_checks[0].verdict == "FAIL" and bad_checks[0].counterexample)
    check("contract_checked", ok, f"use_ok={ok_checks}; use_bad={bad_checks}")
    print(f"      → use_ok calls recip(a) under a>1 ⇒ a≠0: {ok_checks[0].verdict}")
    print(f"      → use_bad calls recip(a) with no guard: {bad_checks[0].verdict} (cx {bad_checks[0].counterexample})")


def aliasing_checked():
    p = parse(ALIASING)
    bad = check_aliasing(p.get("consume"))
    good = check_aliasing(p.get("ok_use"))
    ok = len(bad) == 1 and bad[0].uses == 2 and len(good) == 0
    check("aliasing_checked", ok, f"consume={bad}; ok_use={good}")
    print(f"      → consume uses own 'b' twice: {bad[0].msg}; ok_use: clean")


def adt_exhaustive():
    p = parse(ADT)
    idx = build_data_index(p)
    good = check_adt_exhaustiveness(p.get("tsum"), idx)
    bad = check_adt_exhaustiveness(p.get("tbad"), idx)
    ok = (good and good[0][1] == "PASS" and bad and bad[0][1] == "FAIL" and "Node" in bad[0][2])
    check("adt_exhaustive", ok, f"tsum={good}; tbad={bad}")
    print(f"      → tsum covers {{Leaf,Node}}: PASS; tbad missing {bad[0][2]}: FAIL")


def proven_ratio_report():
    corpus = {
        "sum_squares": "fn sum_squares(n: Nat) -> Nat ensures result = n*(n+1)*(2*n+1)/6 effects pure { fold k in 1..n { k*k } }",
        "sort": SORT,
        "evens": "fn evens(xs: List<Int>) -> List<Int> ensures length(result) <= length(xs) effects pure { filter(xs, λy. y % 2 == 0) }",
    }
    rows = proven_ratio(corpus)
    print("\n      exact-correctness proof tiers:")
    for label, v in rows:
        print(f"        · {label:12s} {v.tier}")
    proven = sum(1 for _, v in rows if v.proven())
    total = len(rows)
    print(f"      ─ {proven}/{total} exact specs PROVEN/PROVEN-BOUNDED ({round(100*proven/total)}%), "
          f"{total - proven} TESTED-only")
    check("proven_ratio_report", proven >= 2 and total >= 3)


if __name__ == "__main__":
    print("STAGE X3 — proof-power completion")
    sort_proven_if_possible()
    contract_checked()
    aliasing_checked()
    adt_exhaustive()
    proven_ratio_report()
    print(f"\nStage X3: {len(PASS)} passed, {len(FAIL)} failed, {len(SKIP)} skipped")
    import sys
    sys.exit(1 if FAIL else 0)
