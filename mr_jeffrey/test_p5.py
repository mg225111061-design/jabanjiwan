"""v20 Part P · P5 tests — MaxSAT multi-bug + ranking-function termination. Run: python3 test_p5.py

P5.1 MaxSAT multiple bugs. P5.2 minimal diagnosis (bug count + suspects). P5.2b ranking function.
P5.3 termination demo (terminating → ranking; infinite → unknown).
"""
import sys

import maxsat_fl as M
import termination as T

PASS, FAIL, SKIP = [], [], []


def check(n, c, d=""):
    (PASS if c else FAIL).append(n)
    print(f"  [{'PASS' if c else 'FAIL'}] {n}" + (f" — {d}" if d and not c else ""))


def maxsat_multi_bug():
    # two INDEPENDENT wrong outputs (c-path and e-path) → minimal correction needs 2 statements
    prog = [("a", "x + 2"), ("c", "a + 1"), ("d", "x * 10"), ("e", "d + 1")]
    d = M.diagnose(prog, {"x": 2}, {"c": 4, "e": 22})   # correct: a=3,c=4,d=20,e=22
    ok = d.min_bugs == 2 and len(d.sample_set) == 2
    check("maxsat_multi_bug", ok, f"min_bugs={d.min_bugs} set={d.sample_set}")
    print(f"      → two independent wrong outputs → MaxSAT min_bugs={d.min_bugs} (sample {d.sample_set}); "
          f"SBFL/diffusion assume ONE fault — MaxSAT pinpoints SEVERAL minimally.")


def minimal_diagnosis():
    single = [("a", "x + 1"), ("b", "a * 2"), ("c", "b + a")]   # bug: b should be a*3
    d = M.diagnose(single, {"x": 2}, {"c": 12})                 # correct b=9,c=12
    clean = M.diagnose([("a", "x + 1"), ("b", "a * 3"), ("c", "b + a")], {"x": 2}, {"c": 12})
    real_bug_in = any("b=a * 2" in s for s in d.suspects)
    ok = d.min_bugs == 1 and real_bug_in and clean.min_bugs == 0
    check("minimal_diagnosis", ok, f"min_bugs={d.min_bugs} real_bug∈suspects={real_bug_in} clean={clean.min_bugs}")
    print(f"      → single bug → min_bugs=1; the real faulty statement (b=a*2) is in the suspect set "
          f"{d.suspects}; a correct program → 0 bugs.")


def ranking_function():
    up = T.synthesize_ranking(["i", "n"], "i < n", {"i": "i + 1"})
    down = T.synthesize_ranking(["i"], "i > 0", {"i": "i - 1"})
    ok = up.terminates is True and up.ranking and down.terminates is True
    check("ranking_function", ok, f"up={up.terminates} ranking='{up.ranking}'")
    print(f"      → `while i<n: i++` → ranking '{up.ranking}' (≈ n−i, decreasing, ≥0) ⇒ TERMINATES; "
          f"`while i>0: i--` → ranking '{down.ranking}'. Z3 ∃coef.∀state synthesis.")


def termination_demo():
    term = T.synthesize_ranking(["i", "n"], "i < n", {"i": "i + 1"})
    inf = T.synthesize_ranking(["i", "n"], "i < n", {"i": "i - 1"})        # i moves away → infinite
    noop = T.synthesize_ranking(["i", "n"], "i < n", {})                    # i unchanged → infinite
    ok = term.terminates is True and inf.terminates is None and noop.terminates is None
    check("termination_demo", ok, f"term={term.terminates} infinite={inf.terminates}/{noop.terminates}")
    print(f"      → terminating loop → ranking found (TERMINATES); `while i<n: i--` and no-op loop → no "
          f"linear ranking → termination UNKNOWN (honest: may not terminate / needs lexicographic — DEFER).")


if __name__ == "__main__":
    print("v20 Part P · P5 — MaxSAT multi-bug + ranking-function termination")
    maxsat_multi_bug(); minimal_diagnosis(); ranking_function(); termination_demo()
    print(f"\nP5: {len(PASS)} passed, {len(FAIL)} failed, {len(SKIP)} skipped")
    sys.exit(1 if FAIL else 0)
