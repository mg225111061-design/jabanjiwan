"""v17 Part E · E2 tests — A upgrade: Coq automation + fold class. Run: python3 test_e2.py

E2.1 more unbounded ∀ theorems proven by pure automation; sort still needs manual (honest boundary).
E2.2 fold-class boundary probed: poly+hypergeometric close; harmonic/factorial DEFER; next classes DEFER.
"""
import sys

import upgrade_a as UA
import haran_coq

PASS, FAIL, SKIP = [], [], []


def check(n, c, d=""):
    (PASS if c else FAIL).append(n)
    print(f"  [{'PASS' if c else 'FAIL'}] {n}" + (f" — {d}" if d and not c else ""))


def skip(n, w):
    SKIP.append(n)
    print(f"  [SKIP] {n} — {w}")


def coq_more_automated():
    if not haran_coq.coq_available():
        skip("coq_more_automated", "coqc absent → BLOCKED"); return
    s = UA.automation_summary()
    # v16 had 3 auto theorems; v17 E2.1 adds app_length/map_map/oddsum → ≥6; sort still manual
    more = s.auto_count >= 6 and {"app_length", "map_map", "oddsum"} <= set(s.auto_proven)
    sort_manual = (not s.sort_auto_closes) and {"isort_sorted", "isort_perm"} <= set(s.manual_proven)
    check("coq_more_automated", more and sort_manual, f"auto={s.auto_count} sort_auto={s.sort_auto_closes}")
    print(f"      → auto-proven unbounded ∀: {s.auto_proven} ({s.auto_count}, was 3 in v16); "
          f"sort still NEEDS manual lemmas (pure automation closes it? {s.sort_auto_closes}). "
          f"Semi-automatic, honestly.")


def fold_class_attempted():
    fb = UA.fold_boundary()
    # poly + hypergeometric close; harmonic is ABSENT (non-summable); factorial NO_STRUCTURE — ceiling respected
    kinds = {label: kind for label, kind, _ in fb.rows}
    ok = (kinds["poly Σk³"] == "CLOSED" and kinds["hypergeom Σk·2^k"] == "CLOSED"
          and kinds["harmonic Σ1/k"] == "ABSENT" and kinds["factorial Σk!"] == "NO_STRUCTURE"
          and "higher-order holonomic" in fb.deferred)
    check("fold_class_attempted", ok, f"kinds={kinds}")
    for label, kind, method in fb.rows:
        print(f"      → {label:20} {kind:13} {method}")
    print(f"      → DEFERRED next classes (honest): "
          f"{'; '.join(fb.deferred)}.")
    print("        Gröbner/Mayr–Meyer EXPSPACE is a FUNDAMENTAL ceiling; Kovacic is an ODE domain — no "
          "fake closes (harmonic correctly ABSENT, factorial NO_STRUCTURE).")


if __name__ == "__main__":
    print("v17 Part E · E2 — A upgrade: Coq automation + fold class")
    coq_more_automated(); fold_class_attempted()
    print(f"\nE2: {len(PASS)} passed, {len(FAIL)} failed, {len(SKIP)} skipped")
    sys.exit(1 if FAIL else 0)
