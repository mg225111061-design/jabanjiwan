"""v24 Part U · U2 tests — clarity assessment (proceed vs ask). Run: python3 test_u24_2.py

clarity_clear_proceeds      : a request stating constraints → clear=True (proceed straight to coding).
clarity_vague_asks          : a bare ambiguous request → clear=False with expected questions.
expected_questions_generated: the questions are topic-specific & useful (e.g. asc/desc, type).
clarity_undecided_via_claude: an undecided request → Claude/mock decides (JSON parsed).
suggestions_not_forced      : asks are SUGGESTIONS — clear requests carry none.
"""
import sys

import intent as I

PASS, FAIL, SKIP = [], [], []


def check(n, c, d=""):
    (PASS if c else FAIL).append(n)
    print(f"  [{'PASS' if c else 'FAIL'}] {n}" + (f" — {d}" if d and not c else ""))


def clarity_clear_proceeds():
    cases = ["1부터 n까지의 합 (오름차순 정수)", "정렬 함수 — 내림차순, 정수 리스트", "reverse an integer list"]
    rs = [I.assess_clarity(c) for c in cases]
    ok = all(r.clear and not r.asks for r in rs)
    check("clarity_clear_proceeds", ok, f"{[(c, r.clear) for c,r in zip(cases,rs)]}")
    print("      → requests stating constraints (asc/desc, type, range) → clear=True → straight to coding.")


def clarity_vague_asks():
    r = I.assess_clarity("정렬 함수")
    ok = (not r.clear) and len(r.asks) >= 1 and r.method == "keyword"
    check("clarity_vague_asks", ok, f"clear={r.clear} asks={r.asks}")
    print(f"      → bare '정렬 함수' → clear=False + expected questions {r.asks} (building blind would be "
          "wrong). Detected LOCALLY (no network).")


def expected_questions_generated():
    r = I.assess_clarity("sort function")
    text = " ".join(r.asks).lower()
    ok = (not r.clear) and ("ascending" in text or "오름차순" in text) and ("type" in text or "타입" in text)
    check("expected_questions_generated", ok, f"asks={r.asks}")
    print("      → topic-specific questions (asc/desc?, element type?, stable?) — useful, not generic noise.")


def clarity_undecided_via_claude():
    # a request with no constraint kw and no known-ambiguous topic → stage 2
    vague = I.assess_clarity("a thing that processes user records",
                             mock_response='{"clear": false, "asks": ["what fields?", "what output?"]}')
    clear = I.assess_clarity("a thing that processes user records",
                             mock_response='{"clear": true, "asks": []}')
    ok = (not vague.clear) and vague.asks and clear.clear and vague.source == "mock-sim"
    check("clarity_undecided_via_claude", ok, f"vague={vague.clear}/{vague.asks} clear={clear.clear}")
    print("      → undecided locally → Claude decides (mock here, JSON parsed): clear or a question list.")


def suggestions_not_forced():
    clear = I.assess_clarity("1부터 100까지 정수 오름차순 합")
    ok = clear.clear and clear.asks == []
    check("suggestions_not_forced", ok, f"clear={clear.clear} asks={clear.asks}")
    print("      → a fully-specified request carries NO questions — the asks are suggestions only.")


if __name__ == "__main__":
    print("v24 Part U · U2 — clarity assessment (proceed vs ask)")
    clarity_clear_proceeds(); clarity_vague_asks(); expected_questions_generated()
    clarity_undecided_via_claude(); suggestions_not_forced()
    print(f"\nU2: {len(PASS)} passed, {len(FAIL)} failed, {len(SKIP)} skipped")
    sys.exit(1 if FAIL else 0)
