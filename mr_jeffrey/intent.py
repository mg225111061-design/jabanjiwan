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

import agentic as AG
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

# neutral system prompt for the structured (classify/clarity) Claude calls — must NOT be the HARAN-code
# system prompt, or Claude would return code instead of the requested JSON.
_CLASSIFY_SYSTEM = ("You are a precise classifier. Reply with ONLY the requested JSON object — no prose, "
                    "no code, no markdown fences.")


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
    gen = CA.claude_generate(prompt, api_key, system=_CLASSIFY_SYSTEM,
                             mock_response=mock_response or _CLASSIFY_MOCK)
    obj = _extract_json(gen.text)
    is_coding = bool(obj.get("is_coding", True))      # default true (conservative)
    conf = float(obj.get("confidence", 0.55)) if isinstance(obj.get("confidence", 0.55), (int, float)) else 0.55
    method = "claude" if api_key else "mock-default"
    intent = "CODING" if is_coding else "CHAT"
    return IntentResult(intent, method, conf, source=gen.source)


# ── U2: clarity assessment ──────────────────────────────────────────────────────────────────────
# A bare "정렬 함수" is ambiguous (asc/desc? type?) — building it blind would be wrong. A request that
# already states constraints proceeds immediately. Known-ambiguous topics get topic-specific questions
# locally; otherwise (rare) Claude decides. The questions are SUGGESTIONS — the user may answer or ignore.
_CONSTRAINT_KW = ["오름차순", "내림차순", "ascending", "descending", "정수", "integer", "int", "float",
                  "문자열", "string", "리스트", "list", "1부터", "1 to", "0부터", "ensures", "명세",
                  "반환", "returns", "given", "입력은", "input is"]
_TOPIC_ASKS = {
    "sort": ["오름차순/내림차순? (ascending or descending?)", "원소 타입? (int / float / string?)",
             "안정 정렬이 필요한가요? (stable?)"],
    "search": ["입력이 정렬돼 있나요? (sorted input?)", "원소 타입? (type?)",
               "못 찾으면 어떻게? (not-found behavior?)"],
    "parse": ["입력 형식은? (input format?)", "잘못된 입력 처리? (error handling?)"],
}
_TOPIC_KW = {
    "sort": ["정렬", "sort", "소트"],
    "search": ["탐색", "검색", "search", "find"],
    "parse": ["파싱", "파서", "parse", "parser"],
}


@dataclass
class ClarityResult:
    clear: bool                       # True → proceed straight to agentic_code
    asks: List[str] = field(default_factory=list)   # expected questions when vague (suggestions)
    method: str = "keyword"           # "keyword" (local) | "claude" | "mock-default"
    source: str = "local"


def _detect_topic(t: str) -> Optional[str]:
    for topic, kws in _TOPIC_KW.items():
        if _has(t, kws):
            return topic
    return None


_CLARITY_MOCK = '{"clear": true, "asks": []}'


def assess_clarity(request: str, api_key: Optional[str] = None, *,
                   mock_response: Optional[str] = None) -> ClarityResult:
    """U2: is a coding request specific enough to build, or should we ask first? Local first (constraint
    keywords → clear; known-ambiguous topic w/o constraints → topic questions); Claude only if unsure."""
    t = (request or "").lower()
    if _has(t, _CONSTRAINT_KW):                 # already states constraints → proceed
        return ClarityResult(True, [], "keyword", "local")
    topic = _detect_topic(t)
    if topic:                                    # known-ambiguous topic, no constraints → ask (local)
        return ClarityResult(False, _TOPIC_ASKS[topic], "keyword", "local")
    # undecided locally → Claude (or conservative mock = proceed)
    prompt = ('Is this coding request specific enough to implement, or are key details missing? Reply '
              'ONLY JSON {"clear": bool, "asks": ["q1","q2"]} (asks = the questions to ask if unclear).\n'
              f"Request: {request}")
    gen = CA.claude_generate(prompt, api_key, system=_CLASSIFY_SYSTEM,
                             mock_response=mock_response or _CLARITY_MOCK)
    obj = _extract_json(gen.text)
    clear = bool(obj.get("clear", True))
    asks = [str(a) for a in obj.get("asks", [])] if isinstance(obj.get("asks", []), list) else []
    return ClarityResult(clear, asks if not clear else [], "claude" if api_key else "mock-default", gen.source)


# ── U3: chat reply (non-coding → a plain answer, NEVER verified) ─────────────────────────────────
# Chat/questions get a normal LLM answer. ★ It carries NO verification label ★ — we did not prove it,
# so we never stamp it "PROVEN/반례". (Coding answers are HARAN-verified; chat answers are not. Honest.)
CHAT_SYSTEM = ("You are MR.JEFFREY, a friendly assistant for a verified-coding product. Answer briefly "
               "and warmly. After smalltalk, gently offer to build & verify some code.")


@dataclass
class ChatReply:
    text: str
    source: str = "mock-sim"          # "claude-live" | "mock-sim"
    kind: str = "chat"
    verified: bool = False            # ALWAYS False — chat is never verified (no proof label)


def _canned_chat(text: str) -> str:
    t = (text or "").lower()
    if _has(t, ["안녕", "하이", "헬로", "반가", "hi", "hello", "hey", "what's up"]):
        return "안녕하세요! 무엇을 만들어 드릴까요? (Hi! What should we build & verify?)"
    if _has(t, ["고마", "감사", "thanks", "thank you"]):
        return "천만에요! 코드가 필요하면 언제든 말씀해 주세요. (Anytime — say the word for code.)"
    if _has(t, ["누구", "who are you", "mr.jeffrey", "mr. jeffrey", "haran", "뭐야", "what is", "뭔지"]):
        return ("저는 MR.JEFFREY예요 — Claude가 코드를 짜면 제가 수학적으로 검증·최적화합니다. "
                "함수나 알고리즘을 요청해 보세요. (I'm MR.JEFFREY: Claude writes code, I verify & "
                "optimize it mathematically. Ask for a function or algorithm.)")
    return ("음 — 코드로 만들어 검증해 드릴까요? 예: '정렬 함수' 같은 걸요. "
            "(Want me to turn that into verified code? e.g. a sort function.)")


def chat_reply(text: str, api_key: Optional[str] = None, history=None, *,
               mock_response: Optional[str] = None) -> ChatReply:
    """U3: a plain conversational answer for CHAT/QUESTION. With a key → a real Claude reply (general
    system prompt). No key → a canned SIM reply. NEVER carries a verification label (verified=False)."""
    if api_key:
        gen = CA.claude_generate(text, api_key, system=CHAT_SYSTEM)   # general answer, not HARAN code
        return ChatReply(gen.text, gen.source)
    return ChatReply(mock_response or _canned_chat(text), "mock-sim")


# ── U4: unified router ──────────────────────────────────────────────────────────────────────────
# input → classify → CODING (clear → agentic_code | vague → ask) | CHAT/QUESTION → chat_reply.
# Whole-program coding asks are out of scope (Rice) → an honest plain reply (NOT a fake verification).
# keep the keyword set in sync with server._SCOPE_RE.
_SCOPE_RE = re.compile(r"백엔드|서버|backend|server|큐|queue|상태\s*머신|state\s*machine|\bapi\b|jwt|"
                       r"결제|payment", re.I)
SCOPE_REPLY = ("이 요청은 큰 프로그램입니다. MR.JEFFREY는 작은~중간 코드를 *명세 대비* 검증·최적화합니다 "
               "(Rice: 무에서 전체 생성 불가). 핵심 로직과 명세(ensures)를 주세요. / This is a large "
               "program — give the core logic + a spec; MR.JEFFREY verifies small~medium code against it.")


def is_scope(text: str) -> bool:
    return bool(_SCOPE_RE.search(text or ""))


@dataclass
class RouteResult:
    kind: str                          # "code" | "chat" | "ask"
    intent: str                        # CODING | CHAT | QUESTION
    request: str
    source: str
    verified: bool = False             # True ONLY for a verified coding result; chat/ask = False
    code_result: object = None         # AgenticResult (kind="code")
    reply: Optional[str] = None        # (kind="chat")
    asks: List[str] = field(default_factory=list)  # (kind="ask")


def route(text: str, mode: str = "normal", api_key: Optional[str] = None, history=None,
          force: bool = False) -> RouteResult:
    """U4: classify the message and route it. CODING+clear → run the verified pipeline; CODING+vague →
    return expected questions (unless `force` → proceed anyway); CHAT/QUESTION → a plain (unverified)
    reply. The `kind` says which."""
    it = classify_intent(text, api_key)
    if it.intent == "CODING":
        if is_scope(text):                                   # whole-program ask → honest scope reply
            return RouteResult("chat", "CODING", text, "local", verified=False, reply=SCOPE_REPLY)
        if not force:                                        # U7: 'proceed anyway' skips the clarity gate
            clarity = assess_clarity(text, api_key)
            if not clarity.clear:                            # missing details → ask first (suggestions)
                return RouteResult("ask", "CODING", text, clarity.source, verified=False, asks=clarity.asks)
        res = AG.agentic_code(text, mode, api_key, history=history or [])   # the verified pipeline
        return RouteResult("code", "CODING", text, res.source, verified=res.converged, code_result=res)
    # CHAT / QUESTION → plain answer, never verified
    cr = chat_reply(text, api_key, history)
    return RouteResult("chat", it.intent, text, cr.source, verified=False, reply=cr.text)
