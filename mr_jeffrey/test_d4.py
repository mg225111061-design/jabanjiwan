"""v17 Part D · D4 tests — fused pipeline + measurement. Run: python3 test_d4.py

D4.1 fused: ordinary code → B detect → A (fold/Z3/Coq) → closed-form | verified | refuted | unbounded | bug.
D4.2 measured: fold-close rate (foldable only), Z3 verify/refute, Coq unbounded, bugs localized, speed.
"""
import sys

import fusion_pipeline as FP

PASS, FAIL, SKIP = [], [], []


def check(n, c, d=""):
    (PASS if c else FAIL).append(n)
    print(f"  [{'PASS' if c else 'FAIL'}] {n}" + (f" — {d}" if d and not c else ""))


_M = FP.measure_fusion()
_BY = {name: (klass, cat) for name, klass, cat, _ in _M.rows}


def fusion_pipeline():
    want = {
        "sum_k2": "closed-form", "sum_k3": "closed-form",
        "recurrence": "no-structure", "sum_spec_ok": "verified", "sum_spec_bad": "refuted",
        "sort_ok": "unbounded-proven", "sort_bug": "bug-localized",
    }
    got = {k: _BY[k][1] for k in want}
    ok = all(got[k] == v for k, v in want.items())
    check("fusion_pipeline", ok, f"got={got}")
    for name, klass, cat, head in _M.rows:
        print(f"      → {name:13} [{klass:11}] → {cat}")


def fusion_measured():
    # fold closes ONLY fold-able loops (3/3 here); Z3 proves & refutes; Coq does the unbounded sort
    ok = (_M.fold_close_rate() == 1.0 and _M.fold_attempted == 3
          and _M.z3_verified == 1 and _M.z3_refuted == 1
          and _M.coq_unbounded == 1 and _M.bugs_localized == 1)
    check("fusion_measured", ok,
          f"fold {_M.fold_closed}/{_M.fold_attempted} z3v={_M.z3_verified} z3r={_M.z3_refuted} "
          f"coq={_M.coq_unbounded} bugs={_M.bugs_localized}")
    print(f"      → fold-closed {_M.fold_closed}/{_M.fold_attempted} ({_M.fold_close_rate():.0%} of FOLD-ABLE), "
          f"Z3 verified={_M.z3_verified}/refuted={_M.z3_refuted}, Coq unbounded={_M.coq_unbounded}, "
          f"bugs localized={_M.bugs_localized}, avg {_M.avg_s*1e3:.0f}ms.")
    print("        HONEST: the A-side digit lands only on fold-able/spec'd/recognized code; the "
          "recurrence got NO_STRUCTURE — 'general language' did not make fold work more (rule 6).")


if __name__ == "__main__":
    print("v17 Part D · D4 — fused pipeline + measurement")
    fusion_pipeline(); fusion_measured()
    print(f"\nD4: {len(PASS)} passed, {len(FAIL)} failed, {len(SKIP)} skipped")
    sys.exit(1 if FAIL else 0)
