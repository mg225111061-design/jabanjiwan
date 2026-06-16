"""
STAGE H3 tests — termination measure synthesis (UNKNOWN blocked at write time).
Run: python3 test_h3.py
"""
from haran_parser import parse
from measure_synth import synthesize, find_ordinal_binary

PASS, FAIL = [], []
def check(name, cond, detail=""):
    (PASS if cond else FAIL).append(name)
    print(f"  [{'PASS' if cond else 'FAIL'}] {name}" + (f" — {detail}" if detail and not cond else ""))


# sort WITHOUT a manual `decreases` — synthesis must find length(xs)
SORT_NODEC = """\
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

FACTORIAL = """\
fn factorial(n: Nat) -> Nat
  effects pure
{ match n { 0 => 1  _ => n * factorial(n - 1) } }
"""

FIB = """\
fn fib(n: Nat) -> Nat
  effects pure
{ match n { 0 => 0  1 => 1  _ => fib(n - 1) + fib(n - 2) } }
"""

TREE_SUM = """\
fn tree_sum(t: Tree) -> Int
  effects pure
{ match t { Leaf(x) => x  Node(l, r) => tree_sum(l) + tree_sum(r) } }
"""

ACKERMANN = """\
fn ack(m: Nat, n: Nat) -> Nat
  effects pure
{
  match m {
    0 => n + 1
    _ => match n {
      0 => ack(m - 1, 1)
      _ => ack(m - 1, ack(m, n - 1))
    }
  }
}
"""

COLLATZ = """\
fn collatz(n: Nat) -> Nat
  effects pure
{
  match n {
    1 => 0
    _ => match n % 2 {
      0 => collatz(n / 2)
      _ => collatz(3 * n + 1)
    }
  }
}
"""

COLLATZ_ASSUMED = """\
fn collatz(n: Nat) -> Nat
  decreases assume_terminates
  effects pure
{
  match n {
    1 => 0
    _ => match n % 2 { 0 => collatz(n / 2)  _ => collatz(3 * n + 1) }
  }
}
"""

SUM_SQUARES = """\
fn sum_squares(n: Nat) -> Nat
  ensures result = n*(n+1)*(2*n+1)/6
  effects pure
{ fold k in 1..n { k*k } }
"""


def synth_struct_decrease():
    s = synthesize(parse(SORT_NODEC).get("sort"))
    check("synth_struct_decrease_sort", s.verdict == "PROVEN" and s.layer == 1 and "length(xs)" in s.measure, str(s))
    f = synthesize(parse(FACTORIAL).get("factorial"))
    check("synth_struct_decrease_factorial", f.verdict == "PROVEN" and f.measure == "n", str(f))
    t = synthesize(parse(TREE_SUM).get("tree_sum"))
    check("synth_struct_decrease_tree", t.verdict == "PROVEN" and "size(t)" in t.measure, str(t))
    print(f"      → sort: {s.measure} (layer {s.layer}); factorial: {f.measure}; tree_sum: {t.measure}")


def synth_nested_loop_ordinal():
    a = synthesize(parse(ACKERMANN).get("ack"))
    ok = a.verdict == "PROVEN" and a.kind == "lexicographic" and "ω" in a.measure
    check("synth_nested_loop_ordinal", ok, str(a))
    # the ordinal engine must have CONFIRMED the decrease (real ω-arithmetic)
    if find_ordinal_binary():
        check("ordinal_engine_confirmed", "DECREASES" in a.ordinal_cert, a.ordinal_cert)
    else:
        print("  [INFO] ordinal_measure binary not built — engine cert skipped")
    print(f"      → ack: measure {a.measure}; engine: {a.ordinal_cert}")


def collatz_synth_fails_honestly():
    c = synthesize(parse(COLLATZ).get("collatz"))
    ok = c.verdict == "NEEDS_DECREASES" and ("decreases" in c.detail.lower() or "proc" in c.detail.lower())
    check("collatz_synth_fails_honestly", ok, str(c))
    print(f"      → collatz: {c.verdict} — {c.detail}")


def decreases_required_else_reject():
    # no measure + no decreases → honest reject
    c = synthesize(parse(COLLATZ).get("collatz"))
    check("decreases_required", c.verdict == "NEEDS_DECREASES", str(c))
    # explicit `decreases assume_terminates` → consciously ASSUMED (out of scope), not a fake PROVEN
    a = synthesize(parse(COLLATZ_ASSUMED).get("collatz"))
    check("assume_terminates_is_assumed", a.verdict == "ASSUMED" and "assum" in a.detail.lower(), str(a))
    print(f"      → assume_terminates → {a.verdict}: {a.detail}")


def ordinal_engine_connected():
    check("ordinal_engine_connected", find_ordinal_binary() is not None,
          "ordinal_measure binary not built — build with: cargo build -p jeff-math --example ordinal_measure")
    print(f"      → engine binary: {find_ordinal_binary()}")


def unknown_rate_before_after():
    corpus = {"sum_squares": SUM_SQUARES, "sort": SORT_NODEC, "factorial": FACTORIAL,
              "fib": FIB, "tree_sum": TREE_SUM, "ack": ACKERMANN, "collatz": COLLATZ}
    results = {}
    for name, src in corpus.items():
        prog = parse(src)
        fn = prog.items[0]
        results[name] = synthesize(fn)
    total = len(corpus)
    # "before" synthesis: a recursive function has no machine-checked measure → silent UNKNOWN.
    from measure_synth import _recursive_calls
    before_unknown = sum(1 for n, s in corpus.items() if _recursive_calls(parse(s).items[0]))
    # "after" synthesis:
    proven = sum(1 for r in results.values() if r.verdict == "PROVEN")
    needs = sum(1 for r in results.values() if r.verdict == "NEEDS_DECREASES")
    silent_unknown_after = 0  # synthesis emits PROVEN or an ACTIONABLE NEEDS_DECREASES — never silent UNKNOWN
    print("\n      UNKNOWN-rate, before vs after synthesis:")
    print(f"        corpus: {total} functions")
    print(f"        BEFORE: {before_unknown}/{total} recursive → silent UNKNOWN ({100*before_unknown//total}%)")
    print(f"        AFTER : {proven}/{total} PROVEN automatically, {needs}/{total} actionable NEEDS_DECREASES, "
          f"{silent_unknown_after}/{total} silent UNKNOWN (0%)")
    for n, r in results.items():
        print(f"          · {n:12s} {r.verdict:16s} {r.measure}")
    ok = (proven == total - 1 and needs == 1 and silent_unknown_after == 0 and before_unknown >= 5)
    check("unknown_rate_before_after", ok, f"proven={proven} needs={needs} before_unknown={before_unknown}")


if __name__ == "__main__":
    print("STAGE H3 — termination measure synthesis")
    ordinal_engine_connected()
    synth_struct_decrease()
    synth_nested_loop_ordinal()
    collatz_synth_fails_honestly()
    decreases_required_else_reject()
    unknown_rate_before_after()
    print(f"\nStage H3: {len(PASS)} passed, {len(FAIL)} failed")
    import sys
    sys.exit(1 if FAIL else 0)
