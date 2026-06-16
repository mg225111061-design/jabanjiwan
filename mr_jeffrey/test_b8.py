"""v16 Part B · B8 tests — structured report + AI fix loop. Run: python3 test_b8.py

B8.1 structured minimal report (where/why/minimal-cx/limit/grade/safe-zone/fix).
B8.2 slice minimized (suspect line + def header, not the whole function).
B8.3 fix loop re-verifies via an INDEPENDENT module; structural bug escalates honestly (no fake success).
B8.4 context separation: the fixer proposes, the verifier (separate) gates — a fake fix cannot pass.
"""
import sys

import hir
import report as RPT
import fix_loop as FL

PASS, FAIL, SKIP = [], [], []


def check(n, c, d=""):
    (PASS if c else FAIL).append(n)
    print(f"  [{'PASS' if c else 'FAIL'}] {n}" + (f" — {d}" if d and not c else ""))


CORRECT = ("def sort1(xs):\n a=list(xs)\n for i in range(len(a)):\n  for j in range(len(a)-1):\n"
           "   if a[j] > a[j+1]:\n    a[j],a[j+1]=a[j+1],a[j]\n return a\n")
BUG_CMP = CORRECT.replace("a[j] > a[j+1]", "a[j] < a[j+1]")
BUG_DROP = "def sort1(xs):\n a=sorted(xs)\n if len(a)>1:\n  a.pop()\n return a\n"


def _fn(src):
    return hir.to_hir(src, "s.py").module.fn("sort1")


def structured_report():
    r = RPT.build_report(_fn(BUG_CMP))
    ok = (r is not None and 5 in r.where_lines and "ordered_output" in r.why
          and len(r.counterexample) <= 3 and r.grade == "A" and r.safe_zone and "line 5" in r.fix_direction)
    check("structured_report", ok, f"grade={r.grade} cx={r.counterexample} where={r.where_lines}")
    print(f"      → grade {r.grade}: where=line {r.where_lines}, minimal c-ex={r.counterexample}, "
          f"safe-zone={r.safe_zone[:3]}..., fix points at the causal operator. Minimal & precise.")


def slice_minimized():
    r = RPT.build_report(_fn(BUG_CMP))
    # the slice keeps the def header + suspect line, not all 7 lines
    lines_shown = [ln for ln in r.slice_text.splitlines() if ln.strip()]
    ok = any("def sort1" in ln for ln in lines_shown) and any("a[j] <" in ln for ln in lines_shown) \
        and len(lines_shown) < 7
    check("slice_minimized", ok, f"slice lines={len(lines_shown)}")
    print(f"      → slice = {len(lines_shown)} lines (def + suspect), not the full 7 — ProbDD/AST minimized.")


def ai_fix_loop_reverifies():
    res = FL.ai_fix_loop(_fn(BUG_CMP))
    cmp_ok = res.fixed and res.rounds <= 3 and "re-verified" in res.detail
    resd = FL.ai_fix_loop(_fn(BUG_DROP))
    drop_ok = (not resd.fixed) and "escalate" in resd.detail   # structural bug → honest escalation
    check("ai_fix_loop_reverifies", cmp_ok and drop_ok, f"cmp fixed={res.fixed} drop fixed={resd.fixed}")
    print(f"      → cmp-bug: repaired in round {res.rounds} ({res.fixer}), INDEPENDENTLY re-verified. "
          f"drop-bug: NOT faked — honestly escalates (structural).")


def context_separation():
    # the fixer cannot make the verifier pass by fiat: feeding the verifier the UNCHANGED buggy code fails
    all_hold, violated = FL.reverify(BUG_CMP, "sort1")
    fixed_src = FL.ai_fix_loop(_fn(BUG_CMP)).fixed_source
    fixed_hold, _ = FL.reverify(fixed_src, "sort1")
    ok = (not all_hold) and "ordered_output" in violated and fixed_hold   # buggy fails, repaired passes
    check("context_separation", ok, f"buggy_holds={all_hold} fixed_holds={fixed_hold}")
    print("      → verifier (property re-test) is a SEPARATE module: buggy code fails it, only a genuinely "
          "repaired code passes. The fixer can't self-certify (수학적 분리).")


if __name__ == "__main__":
    print("v16 Part B · B8 — structured report + AI fix loop")
    structured_report(); slice_minimized(); ai_fix_loop_reverifies(); context_separation()
    print(f"\nB8: {len(PASS)} passed, {len(FAIL)} failed, {len(SKIP)} skipped")
    sys.exit(1 if FAIL else 0)
