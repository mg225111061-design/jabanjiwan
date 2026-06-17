"""
HARAN v24 Part U — intent classification + clarity + chat + routing.
====================================================================
Turns the engine into a conversational product that knows what you mean:

  • U1  classify_intent(text, key?)  — CODING / CHAT / QUESTION   (keyword-first, Claude only if unsure)
  • U2  assess_clarity(req, key?)    — clear → proceed, vague → expected questions
  • U3  chat_reply(text, key?)       — non-coding → a plain Claude answer (NO verification label)
  • U4  route(text, mode, key?)      — classify → coding pipeline | chat reply | ask

★ SPEED HONESTY ★ — stage 1 is LOCAL keyword matching (no network, sub-ms); only genuinely ambiguous
  text falls through to a Claude call (an internet round-trip = hundreds of ms). We never claim a Claude
  call is "0.5ms". The method/source on every result says exactly which path ran.

★ KEY LEVEL 1 ★ — every Claude call here (classify / clarity / chat) goes through claude_agent, which
  takes the key per-call and drops it (never stored/logged). No key → labeled mock (source='mock-sim').

★ CODING vs CHAT honesty ★ — coding answers are HARAN-verified (carry proof labels elsewhere); chat
  answers are a plain LLM reply and carry NO verification label (we didn't verify them).
"""
from __future__ import annotations

import json
import re
from dataclasses import dataclass, field
from typing import List, Optional

import claude_agent as CA

# ── U1: keyword signals (local, no LLM) ─────────────────────────────────────────────────────────
# imperative "make/implement/fix" → almost certainly a coding request
_CODING_VERBS = [
    "만들어", "만들어줘", "구현", "짜줘", "짜봐", "작성", "코딩", "고쳐", "고쳐줘", "리팩토", "최적화",
    "implement", "write a", "write me", "create", "build", "code up", "fix", "refactor", "optimize",
    "generate",
]
_CODING_NOUNS = [
    "함수", "코드", "알고리즘", "정렬", "클래스", "메서드", "버그", "자료구조", "재귀", "반복문", "배열",
    "function", "code", "algorithm", "sort", "class", "method", "bug", "data structure", "recursion",
    "loop", "array", "parser", "regex",
]
_CHAT_KW = [
    "안녕", "하이", "헬로", "반가", "고마", "감사", "잘 가", "잘가", "ㅋㅋ", "ㅎㅎ", "누구야", "누구니",
    "뭐해", "심심", "hi", "hello", "hey", "thanks", "thank you", "bye", "who are you", "what's up",
]
_QUESTION_KW = ["뭐야", "무엇", "뭔지", "어떻게", "왜", "설명", "알려줘", "what is", "what's", "what are",
                "how do", "how does", "how can", "why", "explain", "tell me about", "difference"]

INTENTS = ("CODING", "CHAT", "QUESTION")


@dataclass
class IntentResult:
    intent: str               # CODING | CHAT | QUESTION
    method: str               # "keyword" (local, sub-ms) | "claude" (live) | "mock-default" (no key)
    confidence: float         # 0..1
    source: str = "local"     # "local" | "claude-live" | "mock-sim"


def _has(text: str, kws) -> bool:
    return any(k in text for k in kws)


def _keyword_intent(text: str) -> Optional[IntentResult]:
    """Stage 1 — local, no network. Returns a confident result or None (→ ambiguous)."""
    t = text.lower().strip()
    has_verb = _has(t, _CODING_VERBS)
    has_noun = _has(t, _CODING_NOUNS)
    has_chat = _has(t, _CHAT_KW)
    is_question = t.endswith("?") or _has(t, _QUESTION_KW)

    # 1) explicit coding command wins (imperative verb) — "정렬 함수 만들어줘"
    if has_verb:
        return IntentResult("CODING", "keyword", 0.95)
    # 2) clear smalltalk (greeting/thanks) with no coding noun — "안녕", "고마워"
    if has_chat and not has_noun:
        return IntentResult("CHAT", "keyword", 0.9)
    # 3) an informational question with no coding command — "정렬이 뭐야?", "HARAN이 뭐야?"
    if is_question and not has_verb:
        return IntentResult("QUESTION", "keyword", 0.85)
    # 4) a coding noun, no question/greeting — "퀵소트 코드", "이진탐색 알고리즘"
    if has_noun:
        return IntentResult("CODING", "keyword", 0.8)
    return None   # ambiguous → stage 2


def _extract_json(text: str) -> dict:
    m = re.search(r"\{.*\}", text, re.S)
    if not m:
        return {}
    try:
        return json.loads(m.group(0))
    except Exception:   # noqa: BLE001
        return {}


# conservative default for the no-key path: "거의 다 코딩" → assume CODING when unsure
_CLASSIFY_MOCK = '{"is_coding": true, "confidence": 0.55}'


def classify_intent(text: str, api_key: Optional[str] = None, *,
                    mock_response: Optional[str] = None) -> IntentResult:
    """U1: CODING / CHAT / QUESTION. Stage 1 = local keywords (sub-ms, no network). Stage 2 (only when
    ambiguous) = a Claude call (hundreds of ms) or, with no key, a conservative mock default (CODING)."""
    if not (text or "").strip():
        return IntentResult("CHAT", "keyword", 0.5)
    kw = _keyword_intent(text)
    if kw is not None:
        return kw
    # stage 2 — ambiguous: ask Claude (real round-trip) or fall back to a conservative mock default
    prompt = ('Classify this user message. Reply ONLY JSON {"is_coding": bool, "confidence": 0..1}. '
              'is_coding=true for requests to write/fix/optimize code; false for smalltalk or questions.\n'
              f"Message: {text}")
    gen = CA.claude_generate(prompt, api_key, mock_response=mock_response or _CLASSIFY_MOCK)
    obj = _extract_json(gen.text)
    is_coding = bool(obj.get("is_coding", True))      # default true (conservative)
    conf = float(obj.get("confidence", 0.55)) if isinstance(obj.get("confidence", 0.55), (int, float)) else 0.55
    method = "claude" if api_key else "mock-default"
    intent = "CODING" if is_coding else "CHAT"
    return IntentResult(intent, method, conf, source=gen.source)
