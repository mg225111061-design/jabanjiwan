"""v16 Part B · B6 tests — Caesar digit interrogation (overconfidence moat). Run: python3 test_b6.py

B6.1 Hoeffding/rule-of-three sample check (claim 10⁻ᵏ only if N supports it; else discount).
B6.2 independence discount (correlated properties collapse to one factor — no fake digits).
B6.3 Caesar bound proof (PROVEN-BOUND, or honest BLOCKED if absent).
B6.4 digit certificate (the PROVEN bound, discounted + sample-ceiling noted — digits only when proven).
"""
import sys

import hir
import properties as PR
import property_test as PT
import digit_proof as DP

PASS, FAIL, SKIP = [], [], []


def check(n, c, d=""):
    (PASS if c else FAIL).append(n)
    print(f"  [{'PASS' if c else 'FAIL'}] {n}" + (f" — {d}" if d and not c else ""))


BUG_DROP = "def sort1(xs):\n a=sorted(xs)\n if len(a)>1:\n  a.pop()\n return a\n"
BUG_CMP = ("def sort1(xs):\n a=list(xs)\n for i in range(len(a)):\n  for j in range(len(a)-1):\n"
           "   if a[j] < a[j+1]:\n    a[j],a[j+1]=a[j+1],a[j]\n return a\n")


def _viol(src, n=500):
    f = hir.to_hir(src, "s.py").module.fn("sort1")
    fn = PR.compile_callable(f)
    rep = PT.test_properties(fn, PR.extract_properties(f), PT.gen_int_lists(n))
    return {k: v for k, v in rep.violations.items() if v}


def hoeffding_sample_check():
    short = DP.hoeffding_sample_check(1e-6, 2000)     # claim too strong for the sample
    enough = DP.hoeffding_sample_check(1e-3, 100000)  # claim within reach
    ok = (not short.sufficient) and short.needed_n > 10 ** 6 and enough.sufficient and enough.provable <= 1e-3
    check("hoeffding_sample_check", ok, f"1e-6@2k suff={short.sufficient} needs={short.needed_n}; 1e-3@1e5 suff={enough.sufficient}")
    print(f"      → claim 1e-6 @ N=2000 → INSUFFICIENT (needs N≥{short.needed_n:,}); provable ≤ "
          f"{short.provable:.1e}. claim 1e-3 @ N=1e5 → sufficient. Digits gated by sample size.")


def independence_discount():
    drop = DP.independence_discount(_viol(BUG_DROP))
    cmp_ = DP.independence_discount(_viol(BUG_CMP))
    # drop-bug: length/permutation/idempotence fail on the SAME inputs → 1 effective factor (not 3)
    ok = drop.raw_factors >= 3 and drop.effective_independent == 1 and len(drop.correlated_pairs) >= 1 \
        and cmp_.effective_independent == 1
    check("independence_discount", ok, f"drop raw={drop.raw_factors}→eff={drop.effective_independent}")
    print(f"      → drop-bug: {drop.raw_factors} violated properties fail on the SAME inputs ⇒ "
          f"effective independent = {drop.effective_independent} (raw 16³ would be a FAKE digit). "
          f"Correlated ⇒ NOT multiplied independently.")


def caesar_bound_proof():
    cb = DP.caesar_digit_bound()
    # PROVEN-BOUND if Caesar present, else honest BLOCKED — both acceptable, never a fake claim
    ok = (cb.available and cb.verdict.startswith("PROVEN")) or (not cb.available and cb.verdict == "BLOCKED")
    check("caesar_bound_proof", ok, f"available={cb.available} verdict={cb.verdict}")
    print(f"      → Caesar {'PROVED the probabilistic (expectation) bound (HeyVL+Z3)' if cb.available else 'absent → Hoeffding-only (honest BLOCKED)'}; "
          f"(ε,δ) tail stays DEFERRED.")


def digit_certificate():
    viol = _viol(BUG_DROP)
    over = DP.digit_certificate("pop", tentative=1e-6, n_samples=2000, violation_inputs=viol)   # over-optimistic
    fair = DP.digit_certificate("pop", tentative=1e-2, n_samples=2000, violation_inputs=viol)   # within reach
    ok = (over.discounted and over.proven_upper > 1e-6 and not fair.discounted
          and over.effective_independent == 1)
    check("digit_certificate", ok, f"over→{over.proven_upper:.1e}(disc={over.discounted}) fair→{fair.proven_upper:.1e}")
    print("      → " + over.render())
    print("      → " + fair.render())
    print("        digits CLAIMED only when proven; over-optimistic ones discounted to sample support.")


if __name__ == "__main__":
    print("v16 Part B · B6 — Caesar digit interrogation (overconfidence moat)")
    hoeffding_sample_check(); independence_discount(); caesar_bound_proof(); digit_certificate()
    print(f"\nB6: {len(PASS)} passed, {len(FAIL)} failed, {len(SKIP)} skipped")
    sys.exit(1 if FAIL else 0)
