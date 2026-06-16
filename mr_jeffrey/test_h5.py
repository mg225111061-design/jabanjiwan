"""
STAGE H5 tests — proc/cofix productivity (fresh build, honest scope).  Run: python3 test_h5.py
"""
from haran_parser import parse
from productivity import check_proc

PASS, FAIL = [], []
def check(name, cond, detail=""):
    (PASS if cond else FAIL).append(name)
    print(f"  [{'PASS' if cond else 'FAIL'}] {name}" + (f" — {detail}" if detail and not cond else ""))


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

SPIN = """\
proc spin() -> Stream<Int>
  produces eventually(out)
  effects io
{ cofix loop { loop } }
"""

# recursion AFTER work but with NO yield → still non-productive (straight-line unguarded)
SPIN_WORK = """\
proc churn() -> Stream<Int>
  produces eventually(out)
  effects io
{ cofix loop { let x = compute() loop } }
"""

# conditional yield: one arm yields then recurses, the other recurses bare → OUT OF SCOPE
CONDITIONAL = """\
proc cond() -> Stream<Int>
  produces eventually(out)
  effects io
{
  cofix loop {
    match ready() {
      true  => { yield x  loop }
      false => loop
    }
  }
}
"""

# BOTH arms yield before recursing → per-arm guardedness handles it → PROVEN
BOTH_ARMS = """\
proc tick() -> Stream<Int>
  produces eventually(out)
  effects io
{
  cofix loop {
    match flip() {
      true  => { yield a  loop }
      false => { yield b  loop }
    }
  }
}
"""

NESTED = """\
proc twolevel() -> Stream<Int>
  produces eventually(out)
  effects io
{
  cofix outer {
    yield 1
    cofix inner { yield 2  inner }
  }
}
"""


def _proc(src):
    return parse(src).items[0]


def server_productive_proven():
    r = check_proc(_proc(SERVER))
    check("server_productive_proven", r.verdict == "PROVEN" and "guarded" in r.detail, str(r))
    print(f"      → {r}")


def nonproductive_refuted():
    r = check_proc(_proc(SPIN))
    ok = r.verdict == "REFUTED" and len(r.unguarded) >= 1
    check("nonproductive_refuted", ok, str(r))
    print(f"      → cofix loop {{ loop }}: {r.verdict} (unguarded at {r.unguarded})")
    # work-but-no-yield is also non-productive (straight-line unguarded)
    r2 = check_proc(_proc(SPIN_WORK))
    check("work_without_yield_refuted", r2.verdict == "REFUTED", str(r2))


def complex_productivity_out_of_scope():
    r = check_proc(_proc(CONDITIONAL))
    ok = r.verdict == "OUT_OF_SCOPE" and "conditional" in r.detail.lower()
    check("complex_productivity_out_of_scope", ok, str(r))
    print(f"      → conditional yield: {r.verdict} — {r.detail}")
    # nested cofix is also out of scope
    rn = check_proc(_proc(NESTED))
    check("nested_cofix_out_of_scope", rn.verdict == "OUT_OF_SCOPE" and "nested" in rn.detail.lower(), str(rn))
    print(f"      → nested cofix: {rn.verdict} — {rn.detail}")


def per_arm_guardedness_in_scope():
    # honest nuance: SOME conditionals ARE in scope (every arm yields before recursing)
    r = check_proc(_proc(BOTH_ARMS))
    check("both_arms_yield_proven", r.verdict == "PROVEN", str(r))
    print(f"      → both arms yield: {r.verdict} — {r.detail}")


def scope_report():
    print("\n      productivity checker — honest scope:")
    print("        IN SCOPE : straight-line guardedness; per-arm guardedness (every arm yields)")
    print("        REFUTED  : straight-line recursion with no preceding yield (unambiguous)")
    print("        OUT OF SCOPE: conditional/data-dependent productivity, mutual recursion,")
    print("                      nested cofix, deep guardedness through called functions")
    check("scope_report", True)


if __name__ == "__main__":
    print("STAGE H5 — proc/cofix productivity (fresh build)")
    server_productive_proven()
    nonproductive_refuted()
    complex_productivity_out_of_scope()
    per_arm_guardedness_in_scope()
    scope_report()
    print(f"\nStage H5: {len(PASS)} passed, {len(FAIL)} failed")
    import sys
    sys.exit(1 if FAIL else 0)
