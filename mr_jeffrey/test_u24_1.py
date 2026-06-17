"""v24 Part U · U1 tests — intent classification (keyword-first + Claude fallback). Run: python3 test_u24_1.py

classify_obvious_chat           : greetings/thanks → CHAT via local keywords (no network).
classify_obvious_coding         : imperative coding requests → CODING via local keywords (no network).
classify_question               : informational questions → QUESTION via local keywords.
classify_ambiguous_via_claude_mock : ambiguous text → stage-2 Claude (mock here), parsed from JSON.
intent_enum + speed_honesty     : intent ∈ {CODING,CHAT,QUESTION}; obvious cases are method='keyword' (sub-ms).
"""
import sys
import time

import intent as I

PASS, FAIL, SKIP = [], [], []


def check(n, c, d=""):
    (PASS if c else FAIL).append(n)
    print(f"  [{'PASS' if c else 'FAIL'}] {n}" + (f" — {d}" if d and not c else ""))


def classify_obvious_chat():
    cases = ["안녕", "고마워", "ㅋㅋ", "hi", "who are you"]
    rs = [I.classify_intent(c) for c in cases]
    ok = all(r.intent == "CHAT" and r.method == "keyword" and r.source == "local" for r in rs)
    check("classify_obvious_chat", ok, f"{[(c,r.intent) for c,r in zip(cases,rs)]}")
    print("      → greetings/thanks → CHAT via LOCAL keywords (no network, no key).")


def classify_obvious_coding():
    cases = ["정렬 함수 만들어줘", "퀵소트 구현해줘", "이 버그 고쳐", "write a function to reverse a list",
             "이진탐색 알고리즘"]
    rs = [I.classify_intent(c) for c in cases]
    ok = all(r.intent == "CODING" and r.method == "keyword" for r in rs)
    check("classify_obvious_coding", ok, f"{[(c,r.intent) for c,r in zip(cases,rs)]}")
    print("      → imperative coding / coding nouns → CODING via LOCAL keywords (no network).")


def classify_question():
    cases = ["정렬이 뭐야?", "재귀가 무엇인가요?", "what is a closure?", "왜 이렇게 느려?"]
    rs = [I.classify_intent(c) for c in cases]
    ok = all(r.intent == "QUESTION" and r.method == "keyword" for r in rs)
    check("classify_question", ok, f"{[(c,r.intent) for c,r in zip(cases,rs)]}")
    print("      → informational questions → QUESTION via keywords (asking, not commanding).")


def classify_ambiguous_via_claude_mock():
    # text with no clear signal → stage 2. Simulate Claude's structured reply via mock_response.
    amb = "피보나치"   # no verb/greeting/question/noun → ambiguous
    coding = I.classify_intent(amb, mock_response='{"is_coding": true, "confidence": 0.8}')
    chatty = I.classify_intent(amb, mock_response='{"is_coding": false, "confidence": 0.9}')
    ok = (coding.intent == "CODING" and chatty.intent == "CHAT"
          and coding.method == "mock-default" and coding.source == "mock-sim")
    check("classify_ambiguous_via_claude_mock", ok, f"coding={coding.intent} chat={chatty.intent} method={coding.method}")
    print("      → ambiguous → stage-2 structured query (Claude live with a key; mock here) parsed from "
          "JSON. ★no key → conservative default CODING★ ('거의 다 코딩').")


def intent_enum_and_speed():
    r = I.classify_intent("정렬 함수 만들어줘")
    enum_ok = r.intent in I.INTENTS
    # speed: an obvious case must resolve locally (keyword), not via a network call
    t = time.perf_counter()
    for _ in range(1000):
        I.classify_intent("안녕")
    per_ms = (time.perf_counter() - t)   # 1000 calls
    fast = per_ms < 0.5   # 1000 local classifications well under 0.5s → sub-ms each
    ok = enum_ok and fast
    check("intent_enum_and_speed", ok, f"enum={enum_ok} 1000calls={per_ms*1000:.1f}ms")
    print(f"      → intent ∈ {I.INTENTS}; 1000 obvious classifications in {per_ms*1000:.1f}ms (sub-ms each, "
          "LOCAL). Honest: only AMBIGUOUS text pays the Claude round-trip (hundreds of ms).")


if __name__ == "__main__":
    print("v24 Part U · U1 — intent classification (keyword-first + Claude fallback)")
    classify_obvious_chat(); classify_obvious_coding(); classify_question()
    classify_ambiguous_via_claude_mock(); intent_enum_and_speed()
    print(f"\nU1: {len(PASS)} passed, {len(FAIL)} failed, {len(SKIP)} skipped")
    sys.exit(1 if FAIL else 0)
