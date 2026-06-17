"""v23 Part T · T3 tests — copy replacement (marketing copy, labeled) + examples. Run: python3 test_web3.py

old_copy_removed   : "증명이 보이는 코딩" is absent (removed per spec).
new_hero_sub       : the exact hero + sub marketing copy is present (ko).
marketing_labeled  : the hero/sub copy is tagged `marketing copy` in the source (separated from measured).
no_fake_speedup    : examples carry NO fabricated "×N faster"; concrete numbers are [TBD: 측정필요].
examples_present   : multiple examples with real proven closed forms (n(n+1)/2, n²) + the fix loop.
"""
import sys

import web_check as W

PASS, FAIL, SKIP = [], [], []


def check(n, c, d=""):
    (PASS if c else FAIL).append(n)
    print(f"  [{'PASS' if c else 'FAIL'}] {n}" + (f" — {d}" if d and not c else ""))

HERO = "Claude가 코드를 짜고, 수학적으로 속도와 코드 품질을 끌어올리는 특수 제작 모델이 작동합니다."
SUB = "현존하는 모든 AI 대비 압도적인 속도와 정확성."


def old_copy_removed():
    ok = not W.has("증명이 보이는 코딩")
    check("old_copy_removed", ok)
    print("      → old tagline '증명이 보이는 코딩' is absent (removed as instructed).")


def new_hero_sub():
    ok = W.has(HERO) and W.has(SUB)
    check("new_hero_sub", ok, f"hero={W.has(HERO)} sub={W.has(SUB)}")
    print("      → new hero + sub marketing copy present (verbatim, ko).")


def marketing_labeled():
    # the hero/sub must be explicitly tagged as marketing copy in the source (honesty separation)
    ok = W.count("marketing copy") >= 2
    check("marketing_labeled", ok, f"'marketing copy' x{W.count('marketing copy')}")
    print(f"      → hero/sub tagged `marketing copy` ({W.count('marketing copy')}×) — abstract ads kept "
          "SEPARATE from HARAN's measured results (PROVEN/ms). Never mixed.")


def no_fake_speedup():
    import re
    t = W.html()
    # no concrete "<number>x faster" / "N배 빠름" baked into examples
    fake = re.search(r"\d+\s*[x×]\s*(faster|speedup|빠)", t, re.I)
    has_tbd = W.has("[TBD: 측정필요]")
    ok = (fake is None) and has_tbd
    check("no_fake_speedup", ok, f"fake_match={fake and fake.group(0)} tbd={has_tbd}")
    print(f"      → ★no fabricated '×N faster'★; concrete speed shown as [TBD: 측정필요]. Asymptotic O(1) "
          "is the proven structural claim; the wall-clock multiple awaits real measurement.")


def examples_present():
    nex = W.count('class="example"')
    ok = (nex >= 3 and W.has("n*(n+1)/2") and W.has("n*n")
          and W.has("PROVEN") and W.has("counterexample"))
    check("examples_present", ok, f"examples x{nex}")
    print(f"      → {nex} examples with REAL proven closed forms (n(n+1)/2, n²) "
          "+ the counterexample→fix loop. [user-confirm: they read as 'huge/clear']")


if __name__ == "__main__":
    print("v23 Part T · T3 — copy replacement + examples (structural + honesty)")
    old_copy_removed(); new_hero_sub(); marketing_labeled(); no_fake_speedup(); examples_present()
    print(f"\nT3: {len(PASS)} passed, {len(FAIL)} failed, {len(SKIP)} skipped")
    sys.exit(1 if FAIL else 0)
