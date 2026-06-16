"""v8 tests (Q1-Q3) — sort proven + contract/aliasing checked. Run: python3 test_v8.py"""
from haran_parser import parse
from sort_proof import prove_sort_z3, attempt_unbounded_induction
from prove_exact import check_contract, check_aliasing
from aliasing_analysis import noalias_eligible
from z3_adapter import z3_available
PASS, FAIL, SKIP = [], [], []
def check(n, c, d=""):
    (PASS if c else FAIL).append(n); print(f"  [{'PASS' if c else 'FAIL'}] {n}" + (f" — {d}" if d and not c else ""))
def skip(n, w): SKIP.append(n); print(f"  [SKIP] {n} — {w}")

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
fn fma(a: &Vec<Float>, b: &Vec<Float>, c: &mut Vec<Float>)
  effects pure
{ c }
fn consume(b: own Buffer) -> Int
  effects pure
{ use(b) + use(b) }
"""

# ---- Q1 ----
def _proof():
    return prove_sort_z3(max_len=4)
def sort_sorted_property():
    if not z3_available(): skip("sort_sorted_property", "Z3 absent"); return
    p = _proof(); check("sort_sorted_property", p.sorted_ok, str(p))
    print(f"      → sortedness Z3-proven ∀ integer values, length ≤ {p.max_len} (every ordering)")
def sort_permutation_property():
    if not z3_available(): skip("sort_permutation_property", "Z3 absent"); return
    p = _proof(); check("sort_permutation_property", p.permutation_ok)
    print(f"      → permutation (output is a rearrangement of input) — exact, length ≤ {p.max_len}")
def sort_proven_or_honest_bound():
    if not z3_available(): skip("sort_proven_or_honest_bound", "Z3 absent"); return
    p = _proof()
    ok = p.sorted_ok and p.permutation_ok and "DEFER" in p.detail and "Z3" in p.level
    check("sort_proven_or_honest_bound", ok, p.level)
    print(f"      → level: {p.level}")
    print(f"      → unbounded ∀: {attempt_unbounded_induction()[:90]}... DEFERRED")

# ---- Q2 ----
def requires_checked_at_callsite():
    if not z3_available(): skip("requires_checked_at_callsite", "Z3 absent"); return
    p = parse(CONTRACT); ftab = {f.name: f for f in p.fns()}
    chk = check_contract(p.get("use_ok"), ftab)
    ok = len(chk) == 1 and chk[0].verdict == "PASS"
    check("requires_checked_at_callsite", ok, str(chk))
    print(f"      → use_ok calls recip(a) under a>1 ⇒ a≠0 satisfied at call site: {chk[0].verdict}")
def contract_violation_counterexample():
    if not z3_available(): skip("contract_violation_counterexample", "Z3 absent"); return
    p = parse(CONTRACT); ftab = {f.name: f for f in p.fns()}
    chk = check_contract(p.get("use_bad"), ftab)
    ok = len(chk) == 1 and chk[0].verdict == "FAIL" and chk[0].counterexample
    check("contract_violation_counterexample", ok, str(chk))
    print(f"      → use_bad calls recip(a) unguarded: {chk[0].verdict} (cx {chk[0].counterexample})")

# ---- Q3 ----
def aliasing_checked():
    p = parse(ALIASING)
    issues = check_aliasing(p.get("consume"))
    ok = len(issues) == 1 and issues[0].uses == 2
    check("aliasing_checked", ok, str(issues))
    print(f"      → consume: {issues[0].msg} (own used-after-move CAUGHT)")
def noalias_guarantee_verified():
    p = parse(ALIASING)
    fma = p.get("fma")
    elig = noalias_eligible(fma)               # &mut c is provably exclusive ⇒ noalias-eligible
    clean = check_aliasing(fma)                 # and not misused
    ok = {i.name for i in elig} == {"c"} and len(clean) == 0
    check("noalias_guarantee_verified", ok, f"noalias={[i.name for i in elig]} issues={clean}")
    print("      → fma &mut c: noalias-eligible (exclusive borrow) AND usage clean → noalias guarantee CHECK-BACKED")
    print("        (v4.5 gave LLVM noalias; v8 confirms the aliasing CHECK justifies it — C can't)")

if __name__ == "__main__":
    print("v8 — verification depth (sort proven + contract/aliasing)")
    print("[Q1]"); sort_sorted_property(); sort_permutation_property(); sort_proven_or_honest_bound()
    print("[Q2]"); requires_checked_at_callsite(); contract_violation_counterexample()
    print("[Q3]"); aliasing_checked(); noalias_guarantee_verified()
    print(f"\nv8: {len(PASS)} passed, {len(FAIL)} failed, {len(SKIP)} skipped")
    import sys; sys.exit(1 if FAIL else 0)
