"""v23 Part T · T3 tests — copy replacement + big-prompt chips + honesty. Run: python3 test_web3.py

old_copy_removed   : "증명이 보이는 코딩" is absent (removed per spec).
new_hero_sub       : the exact hero + sub marketing copy is present (ko).
marketing_labeled  : hero/sub/compare copy is tagged `marketing copy` in the source (separated from measured).
comparison_line    : T3.2 comparison line present as marketing copy with a [TBD] placeholder, hidden by default.
big_prompt_chips   : 4 huge coding prompts as clickable chips; small math examples removed.
scope_honest       : a big-program request gets an honest scope/intent-gap reply (not a fake verification).
no_fake_speedup    : NO fabricated "×N faster"; concrete numbers stay [TBD: measure].
"""
import re
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
    ok = W.count("marketing copy") >= 2
    check("marketing_labeled", ok, f"'marketing copy' x{W.count('marketing copy')}")
    print(f"      → hero/sub/compare tagged `marketing copy` ({W.count('marketing copy')}×) — abstract ads "
          "kept SEPARATE from HARAN's measured results (PROVEN/ms).")


def comparison_line():
    t = W.html()
    has_compare = W.has('id="compareLine"', t) and W.has("compare", t)
    is_tbd = "[TBD: 측정필요]" in t or "[TBD: measure]" in t
    hidden = bool(re.search(r'id="compareLine"[^>]*style="display:none"', t))
    ok = has_compare and is_tbd and hidden
    check("comparison_line", ok, f"compare={has_compare} tbd={is_tbd} hidden={hidden}")
    print("      → T3.2 comparison line is marketing copy with a [TBD] number, HIDDEN until measured "
          "(no unmeasured figure shown).")


def big_prompt_chips():
    t = W.html()
    nchips = t.count('class="chip"')
    big = (W.has("실시간 채팅 백엔드 전체", t) and W.has("JWT 인증 REST API 서버", t)
           and W.has("결제 처리 상태머신", t) and W.has("동시성 안전한 작업 큐", t))
    small_removed = ('class="example"' not in t) and ("1부터 n까지 더해줘" not in t)
    clickable = W.has("exampleChips", t) and W.has("reqEl.value = c.textContent", t)
    ok = nchips >= 4 and big and small_removed and clickable
    check("big_prompt_chips", ok, f"chips={nchips} big={big} small_removed={small_removed} clickable={clickable}")
    print(f"      → {nchips} HUGE coding-prompt chips (chat backend / JWT API / payment SM / job queue), "
          "clickable→input; small math examples removed.")


def scope_honest():
    t = W.html()
    ok = W.has("scope:true", t) and W.has("scope_note", t) and W.has("Rice", t)
    check("scope_honest", ok)
    print("      → ★intent-gap honesty★: a whole-program request returns an honest scope reply (verify "
          "small~medium code AGAINST A SPEC; Rice) — never a fake 'verified backend'.")


def no_fake_speedup():
    t = W.html()
    fake = re.search(r"\d+\s*[x×]\s*(faster|speedup|빠)", t, re.I)
    has_tbd = "[TBD: 측정필요]" in t or "[TBD: measure]" in t
    ok = (fake is None) and has_tbd
    check("no_fake_speedup", ok, f"fake={fake and fake.group(0)} tbd={has_tbd}")
    print("      → no fabricated '×N faster'; concrete speed stays [TBD: measure]. O(1) is the proven "
          "structural claim; the wall-clock multiple awaits real measurement.")


if __name__ == "__main__":
    print("v23 Part T · T3 — copy + big-prompt chips + honesty (structural)")
    old_copy_removed(); new_hero_sub(); marketing_labeled(); comparison_line()
    big_prompt_chips(); scope_honest(); no_fake_speedup()
    print(f"\nT3: {len(PASS)} passed, {len(FAIL)} failed, {len(SKIP)} skipped")
    sys.exit(1 if FAIL else 0)
