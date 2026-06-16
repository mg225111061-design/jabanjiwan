"""v17 Part C · C5 tests — multilang dispatcher + per-language measurement. Run: python3 test_c5.py

C5.1 dispatcher: code → detect language → frontend → HIR → shared engine.
C5.2 per-language measured: top-1 op, speed; typed vs dynamic; BLOCKED parsers listed honestly.
"""
import sys

import multilang as ML

PASS, FAIL, SKIP = [], [], []


def check(n, c, d=""):
    (PASS if c else FAIL).append(n)
    print(f"  [{'PASS' if c else 'FAIL'}] {n}" + (f" — {d}" if d and not c else ""))


_M = ML.measure_all(40)


def multilang_dispatcher():
    # the SAME bug, written in each language, all route to a frontend and produce a runnable HIR
    covered = _M.covered()
    ok = "python" in covered and len(covered) >= 4   # python + several native languages
    check("multilang_dispatcher", ok, f"covered={covered}")
    print(f"      → one dispatcher (detect → frontend → HIR → engine) covered {len(covered)} languages: "
          f"{covered}. BLOCKED: {_M.blocked() or 'none'}.")


def per_language_measured():
    print("      per-language (same descending-sort bug):")
    for r in _M.results:
        typ = "typed " if r.lang in ML.TYPED else "dynamic"
        status = (f"out={r.out} violated={r.violated} top1={r.top1}@{r.top1_lines} {r.elapsed_s*1e3:.0f}ms"
                  if r.supported else f"BLOCKED ({r.detail[:40]})")
        print(f"        {r.lang:11} {typ}  {status}")
    # every covered language must localize the bug to `compare` (engine reuse holds across languages)
    hits = _M.top1_hits()
    cov = len(_M.covered())
    ok = cov >= 4 and hits == cov
    check("per_language_measured", ok, f"top1=compare {hits}/{cov}")
    print(f"      → top-1 = compare in {hits}/{cov} covered languages — ONE engine, many frontends. "
          f"Typed languages (C/Rust/Go/Java/TS) give explicit shape; dynamic (Python/JS) inferred.")


if __name__ == "__main__":
    print("v17 Part C · C5 — multilang dispatcher + measurement")
    multilang_dispatcher(); per_language_measured()
    print(f"\nC5: {len(PASS)} passed, {len(FAIL)} failed, {len(SKIP)} skipped")
    sys.exit(1 if FAIL else 0)
