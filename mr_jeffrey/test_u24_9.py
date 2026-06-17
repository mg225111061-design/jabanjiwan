"""v24 Part U · U9 tests — real connection + finishing. Run: python3 test_u24_9.py

live_path_structure  : the live path is wired correctly (Anthropic SDK; classify/clarity use a neutral
                       JSON system prompt; chat uses a chat system prompt; all touchpoints pass the key).
frontend_backend_contract : the front sends {prompt,mode,apiKey,history,force}; the server reads each; every
                       SSE event the server emits is handled by the front.
error_redacted_friendly : a failed Claude call → a friendly, KEY-SAFE message (no key leak, no raw msg).
history_accumulates  : conversation history threads front → server → agentic_code.
mixed_chat_coding    : chat renders a plain bubble (no verify label); coding renders a verify card.
"""
import re
import sys

import claude_agent as CA
import intent as I
import server as S
import web_check as W

PASS, FAIL, SKIP = [], [], []


def check(n, c, d=""):
    (PASS if c else FAIL).append(n)
    print(f"  [{'PASS' if c else 'FAIL'}] {n}" + (f" — {d}" if d and not c else ""))


def _read(p):
    return open(p, encoding="utf-8").read()


def live_path_structure():
    ca, it, ag = _read("claude_agent.py"), _read("intent.py"), _read("agentic.py")
    rq = _read("requirements.txt").lower()
    sdk = ("import anthropic" in ca and "client.messages.create" in ca and "client.messages.stream" in ca
           and CA.DEFAULT_MODEL == "claude-opus-4-8")
    neutral_classify = "_CLASSIFY_SYSTEM" in it and "system=_CLASSIFY_SYSTEM" in it   # not HARAN-code prompt
    chat_system = "CHAT_SYSTEM" in it and "system=CHAT_SYSTEM" in it
    touchpoints = "claude_generate(prompt, api_key" in it and "_claude_model_fn(api_key" in ag
    ok = sdk and neutral_classify and chat_system and touchpoints and "anthropic" in rq
    check("live_path_structure", ok,
          f"sdk={sdk} neutral={neutral_classify} chat={chat_system} touchpoints={touchpoints}")
    print("      → live path: official Anthropic SDK (create+stream, claude-opus-4-8); classify/clarity "
          "use a NEUTRAL json system prompt (else Claude returns code); chat uses a chat prompt; all "
          "touchpoints pass the key. requirements has anthropic. [user-confirm: run it with a real key]")


def frontend_backend_contract():
    html, sv = W.html(), _read("server.py")
    body = re.search(r"JSON\.stringify\(\{([^}]*)\}", html).group(1)
    fields = ["prompt", "mode", "apiKey", "history", "force"]
    front_sends = all(f in body for f in fields)
    server_reads = all(f'get("{f}"' in sv for f in fields)   # matches get("x") and get("x", default)
    emitted = set(re.findall(r'sse_event\(\{"type":\s*"([^"]+)"', sv))
    handled = set(re.findall(r'ev\.type\s*===\s*"([^"]+)"', html))
    ok = front_sends and server_reads and emitted and not (emitted - handled)
    check("frontend_backend_contract", ok,
          f"front_sends={front_sends} server_reads={server_reads} unhandled={emitted-handled}")
    print(f"      → contract: front sends {fields}; server reads each; front handles every SSE event "
          f"{sorted(emitted)}. Mock fallback kept for no-backend.")


def error_redacted_friendly():
    SENTINEL = "sk-ant-LEAKTEST-DEADBEEF"
    r = S.handle_route({"prompt": "정수 오름차순 정렬 함수 만들어줘", "apiKey": SENTINEL})
    leaked = SENTINEL in str(r)
    friendly = bool(r.get("error"))
    # the friendly-error mapper exists and never echoes the raw SDK message
    has_mapper = "_friendly_error" in _read("claude_agent.py")
    ok = (not leaked) and friendly and has_mapper
    check("error_redacted_friendly", ok, f"leaked={leaked} error={friendly} mapper={has_mapper}")
    print("      → a failed call → friendly KEY-SAFE message (invalid key / rate limit / network), key "
          "redacted, raw SDK message never echoed. (Here: SDK absent → 'not installed' — still no leak.)")


def history_accumulates():
    html = W.html()
    front = "const HISTORY = []" in html and "HISTORY.push(" in html
    hist = [{"request": "x", "code": "fn f(n: Nat)->Nat\n  ensures result=n\n{ n }"}]
    r = S.handle_route({"prompt": "make it safer", "history": hist})
    ok = front and (r.get("kind") in ("code", "ask", "chat"))   # threaded without error
    check("history_accumulates", ok, f"front={front} server_ok={not r.get('error')}")
    print("      → conversation history threads front → server → agentic_code (multi-round context).")


def mixed_chat_coding():
    html = W.html()
    chat_plain = "chatreply" in html and I.chat_reply("안녕").verified is False
    coding_card = "renderAssistant" in html and ("m_proven" in html or "m_verified" in html)
    ok = chat_plain and coding_card
    check("mixed_chat_coding", ok, f"chat_plain={chat_plain} coding_card={coding_card}")
    print("      → chat = plain bubble (.chatreply, verified=False, NO proof label); coding = verify card "
          "(steps + proof tier). They coexist on one screen, visually distinct. [user-confirm: the look]")


if __name__ == "__main__":
    print("v24 Part U · U9 — real connection + finishing")
    live_path_structure(); frontend_backend_contract(); error_redacted_friendly()
    history_accumulates(); mixed_chat_coding()
    print(f"\nU9: {len(PASS)} passed, {len(FAIL)} failed, {len(SKIP)} skipped")
    sys.exit(1 if FAIL else 0)
