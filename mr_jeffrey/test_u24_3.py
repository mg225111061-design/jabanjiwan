"""v24 Part U · U3 tests — chat reply (non-coding → plain answer, never verified). Run: python3 test_u24_3.py

chat_reply_greeting : "안녕" → a warm greeting reply.
question_reply      : "MR.JEFFREY가 뭐야?" → an explanation.
chat_no_verify_label: ★ chat replies carry NO verification label (verified=False, no PROVEN/반례) ★.
mock_chat           : no key → deterministic SIM reply (source='mock-sim').
coding_nudge        : a generic chat turn gently offers to build verified code.
"""
import sys

import intent as I

PASS, FAIL, SKIP = [], [], []


def check(n, c, d=""):
    (PASS if c else FAIL).append(n)
    print(f"  [{'PASS' if c else 'FAIL'}] {n}" + (f" — {d}" if d and not c else ""))


def chat_reply_greeting():
    r = I.chat_reply("안녕")
    ok = r.kind == "chat" and ("안녕" in r.text or "Hi" in r.text) and not r.verified
    check("chat_reply_greeting", ok, f"text={r.text[:30]!r} verified={r.verified}")
    print(f"      → '안녕' → '{r.text[:40]}…' (warm greeting). kind=chat, verified={r.verified}.")


def question_reply():
    r = I.chat_reply("MR.JEFFREY가 뭐야?")
    ok = ("MR.JEFFREY" in r.text or "검증" in r.text or "verify" in r.text) and not r.verified
    check("question_reply", ok, f"text={r.text[:40]!r}")
    print("      → a product question → an explanation (still NOT a verified claim).")


def chat_no_verify_label():
    r = I.chat_reply("심심해")
    # the result object must not expose any proof/verification label
    no_label = (r.verified is False and not hasattr(r, "proof_tier")
                and not hasattr(r, "counterexample") and "PROVEN" not in r.text and "반례" not in r.text)
    check("chat_no_verify_label", no_label, f"verified={r.verified}")
    print("      → ★chat is NEVER labeled verified★ — no PROVEN/반례/proof_tier. We didn't verify it, so "
          "we don't claim we did. (Coding answers get the proof labels; chat does not.)")


def mock_chat():
    a, b = I.chat_reply("안녕"), I.chat_reply("안녕")
    ok = a.source == "mock-sim" and a.text == b.text
    check("mock_chat", ok, f"source={a.source} deterministic={a.text==b.text}")
    print("      → no key → SIM reply (source='mock-sim'), deterministic; with a key it's a real Claude answer.")


def coding_nudge():
    r = I.chat_reply("오늘 날씨 좋다")
    ok = ("코드" in r.text or "code" in r.text)
    check("coding_nudge", ok, f"text={r.text[:40]!r}")
    print("      → after smalltalk, gently offers to build verified code (suggestion, not forced).")


if __name__ == "__main__":
    print("v24 Part U · U3 — chat reply (non-coding → plain answer, never verified)")
    chat_reply_greeting(); question_reply(); chat_no_verify_label(); mock_chat(); coding_nudge()
    print(f"\nU3: {len(PASS)} passed, {len(FAIL)} failed, {len(SKIP)} skipped")
    sys.exit(1 if FAIL else 0)
