"""v17 Part D · D1 tests — HIR loop → fold engine (Type A+B fusion). Run: python3 test_d1.py

D1.1 a slow loop flagged by B7 (perf) is handed to the fold engine.
D1.2 fold closes (closed form + cert) OR NO_STRUCTURE honestly (4-bucket).
D1.3 demo: slow Σk² loop (Python & C) → closed form n(n+1)(2n+1)/6, verified against the real loop.
"""
import sys

import hir
import perf
import fusion

PASS, FAIL, SKIP = [], [], []


def check(n, c, d=""):
    (PASS if c else FAIL).append(n)
    print(f"  [{'PASS' if c else 'FAIL'}] {n}" + (f" — {d}" if d and not c else ""))


PY_SQ = "def slow(n):\n s=0\n for i in range(1, n+1):\n  s += i*i\n return s\n"
PY_REC = "def slow(n):\n s=1\n for i in range(1, n+1):\n  s = s*31 + i\n return s\n"
PY_DATA = "def slow(a):\n s=0\n for i in range(1, len(a)):\n  s += a[i]\n return s\n"
C_SQ = "int slow(int n){ int s=0; int i; for(i=1;i<=n;i=i+1){ s = s + i*i; } return s; }\n"


def _fold(src, ext):
    return fusion.fold_inject(hir.to_hir(src, ext).module.functions[0])


def hir_to_fold():
    r = _fold(PY_SQ, "x.py")
    ok = r.kind == "CLOSED" and "n^3" in r.closed_form and r.verified
    check("hir_to_fold", ok, f"kind={r.kind} closed={r.closed_form}")
    print(f"      → a general-language loop (Σi²) → HARAN fold engine → {r.kind}: {r.closed_form} "
          f"(O(1)); the SAME Faulhaber/Z3 path B never had access to.")


def fold_closes_or_honest():
    closed = _fold(PY_SQ, "x.py")
    rec = _fold(PY_REC, "x.py")
    dat = _fold(PY_DATA, "x.py")
    ok = (closed.kind == "CLOSED" and closed.verified
          and rec.kind == "NO_STRUCTURE" and dat.kind == "NO_STRUCTURE")
    check("fold_closes_or_honest", ok, f"Σi²={closed.kind} recur={rec.kind} data={dat.kind}")
    print(f"      → Σi² CLOSES (verified); accumulator recurrence → {rec.kind}; data-dependent → "
          f"{dat.kind}. fold closes ONLY fold-able loops — being 'general code' changes nothing (rule 6).")


def general_lang_fold_demo():
    # D1.1 wire: perf flags the slow loop as the hotspot, then fold closes it
    mod = PY_SQ + "def driver(n):\n return slow(n)\n"
    hs = perf.top_hotspot(mod, "driver", [400] * 30)
    perf_flagged = hs is not None and hs.func in ("slow", "driver")
    py = _fold(PY_SQ, "x.py")
    c = _fold(C_SQ, "x.c")
    # both languages close to the SAME formula, each verified against its REAL runtime
    same_formula = py.closed_form == c.closed_form
    ok = perf_flagged and py.kind == "CLOSED" and py.verified and c.kind == "CLOSED" and c.verified and same_formula
    check("general_lang_fold_demo", ok, f"perf={hs.func if hs else None} py={py.kind} c={c.kind} same={same_formula}")
    print(f"      → B7 perf flags '{hs.func}' as hotspot → fold closes it: Python & C both → "
          f"{py.closed_form}, each verified against its real (interpreted / gcc-compiled) loop.")
    print("        This is the fusion: B finds the slow loop, A collapses it to O(1) with a proof.")


if __name__ == "__main__":
    print("v17 Part D · D1 — HIR loop → fold engine (fusion)")
    hir_to_fold(); fold_closes_or_honest(); general_lang_fold_demo()
    print(f"\nD1: {len(PASS)} passed, {len(FAIL)} failed, {len(SKIP)} skipped")
    sys.exit(1 if FAIL else 0)
