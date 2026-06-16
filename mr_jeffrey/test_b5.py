"""v16 Part B · B5 tests — probabilistic narrowing + digit shaving. Run: python3 test_b5.py

B5.1 Bayesian update (posterior over operations; top-1 = the actual bug op).
B5.2 digit-narrowing layers (property-bundle, input-bombardment, causal-mutation, dataflow cross-val).
B5.3 SmartFL-style Bayesian (valid distribution built from per-op likelihoods).
B5.4 narrowing measured (top-1/top-5 + TENTATIVE innocent digit — NOT claimed; B6 proves it).
"""
import sys

import hir
import properties as PR
import property_test as PT
import narrow as N

PASS, FAIL, SKIP = [], [], []


def check(n, c, d=""):
    (PASS if c else FAIL).append(n)
    print(f"  [{'PASS' if c else 'FAIL'}] {n}" + (f" — {d}" if d and not c else ""))


CORRECT = ("def sort1(xs):\n a=list(xs)\n for i in range(len(a)):\n  for j in range(len(a)-1):\n"
           "   if a[j] > a[j+1]:\n    a[j],a[j+1]=a[j+1],a[j]\n return a\n")
BUG_CMP = CORRECT.replace("a[j] > a[j+1]", "a[j] < a[j+1]")
BUG_DROP = "def sort1(xs):\n a=sorted(xs)\n if len(a)>1:\n  a.pop()\n return a\n"


def _hfn(src):
    return hir.to_hir(src, "s.py").module.fn("sort1")


def _violated(hfn, n=400):
    fn = PR.compile_callable(hfn)
    props = PR.extract_properties(hfn)
    rep = PT.test_properties(fn, props, PT.gen_int_lists(n))
    return [p for p in props if p.name in rep.violated_properties()]


def bayesian_update():
    h = _hfn(BUG_CMP)
    res = N.bayesian_narrow(h, _violated(h))
    top = res.ranked[0]
    # the actual bug (comparison @ line 5) is the top suspect; least-suspect has much lower posterior
    ok = top.op_kind == "compare" and 5 in top.lines and res.ranked[-1].posterior < top.posterior
    check("bayesian_update", ok, f"top={top.op_kind}@{top.lines} p={top.posterior:.3f}")
    print(f"      → posterior top = compare@{top.lines} (p={top.posterior:.3f}); innocent ops carry the "
          f"residual mass. Posterior odds ∝ prior × ∏ violated-property likelihood ratios.")


def digit_narrowing_layers():
    h = _hfn(BUG_CMP)
    m = N.narrow(h, n_random=400)
    # the causal mutation layer must fire for an operator bug (mutating < restores ordered_output)
    ok = ("L1 property-bundle" in m.layers_used and "L2 input-bombardment" in m.layers_used
          and "L4 causal-mutation" in m.layers_used and m.causal_recovered)
    check("digit_narrowing_layers", ok, f"layers={m.layers_used}")
    print(f"      → layers fired: {m.layers_used}; causal mutation at line {m.top1_lines} RECOVERS the "
          f"violated property ⇒ the comparison is causally the bug (the decisive shaver).")


def smartfl_on_z3():
    h = _hfn(BUG_DROP)
    res = N.bayesian_narrow(h, _violated(h))
    total = sum(s.posterior for s in res.ranked)
    valid_dist = abs(total - 1.0) < 1e-9
    # SmartFL-style: posterior built from per-op likelihoods; top reflects the value-corruption op (pop)
    ok = valid_dist and res.ranked[0].op_kind == "pop"
    check("smartfl_on_z3", ok, f"Σposterior={total:.6f} top={res.ranked[0].op_kind}")
    print(f"      → SmartFL-style Bayesian: posterior is a valid distribution (Σ={total:.4f}); top=pop "
          f"(value/length corruption). Deeper per-statement Z3 value-modeling is partial → B6/DEFER.")


def narrowing_measured():
    for label, src in [("cmp-bug", BUG_CMP), ("drop-bug", BUG_DROP)]:
        m = N.narrow(_hfn(src), n_random=400)
        print(f"      → {label}: top1={m.top1}@{m.top1_lines}  top5={m.top5}  "
              f"innocent-digit(TENTATIVE)={m.innocent_digit_tentative:.2e}  causal={m.causal_recovered}")
    # we only ASSERT we measured top-k + a tentative digit; we do NOT claim the digit is proven (that's B6)
    m = N.narrow(_hfn(BUG_CMP), n_random=400)
    ok = m.top1 == "compare" and len(m.top5) >= 1 and 0 < m.innocent_digit_tentative < 1
    check("narrowing_measured", ok, f"digit tentative={m.innocent_digit_tentative:.2e}")
    print("        DISCIPLINE: the digit is TENTATIVE (raw ∏ may include correlated properties); "
          "B6 discounts correlation and bounds it by sample (Hoeffding) before any claim.")


if __name__ == "__main__":
    print("v16 Part B · B5 — probabilistic narrowing + digit shaving")
    bayesian_update(); digit_narrowing_layers(); smartfl_on_z3(); narrowing_measured()
    print(f"\nB5: {len(PASS)} passed, {len(FAIL)} failed, {len(SKIP)} skipped")
    sys.exit(1 if FAIL else 0)
