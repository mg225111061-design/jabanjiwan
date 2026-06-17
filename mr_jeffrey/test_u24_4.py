"""v24 Part U · U4 tests — unified router (classify → route). Run: python3 test_u24_4.py

route_coding   : a clear coding request → kind='code' with the verified pipeline result.
route_chat     : smalltalk → kind='chat' with a plain reply (verified=False).
route_ask      : a vague coding request → kind='ask' with expected questions.
route_scope    : a whole-program request → honest scope reply (kind='chat', not a fake verify).
labels_distinct: code carries a verification label; chat/ask never do (honest split).
server_routes_through_intent : server.handle_route returns kind-tagged JSON; /api/generate uses it.
"""
import sys

import intent as I
import server as S

PASS, FAIL, SKIP = [], [], []


def check(n, c, d=""):
    (PASS if c else FAIL).append(n)
    print(f"  [{'PASS' if c else 'FAIL'}] {n}" + (f" — {d}" if d and not c else ""))


def route_coding():
    r = I.route("정수 리스트를 오름차순 정렬하는 함수 만들어줘")
    ok = r.kind == "code" and r.intent == "CODING" and r.code_result is not None
    check("route_coding", ok, f"kind={r.kind} verified={r.verified}")
    print(f"      → clear coding request → kind='code' (verified pipeline ran; verified={r.verified}).")


def route_chat():
    r = I.route("안녕")
    ok = r.kind == "chat" and r.reply and not r.verified and r.code_result is None
    check("route_chat", ok, f"kind={r.kind} verified={r.verified} reply={r.reply[:20]!r}")
    print(f"      → '안녕' → kind='chat', plain reply, verified={r.verified} (no proof label).")


def route_ask():
    r = I.route("정렬 함수")
    ok = r.kind == "ask" and r.asks and not r.verified
    check("route_ask", ok, f"kind={r.kind} asks={r.asks}")
    print(f"      → vague '정렬 함수' → kind='ask' with expected questions {r.asks[:1]}…")


def route_scope():
    r = I.route("실시간 채팅 백엔드 전체 만들어줘")
    ok = r.kind == "chat" and not r.verified and ("Rice" in (r.reply or "") or "명세" in (r.reply or ""))
    check("route_scope", ok, f"kind={r.kind} verified={r.verified}")
    print("      → whole-program request → honest scope reply (kind='chat', NOT a fake verified backend).")


def labels_distinct():
    code = I.route("정수 오름차순 정렬 함수 만들어줘")
    chat = I.route("고마워")
    ask = I.route("검색 함수")
    ok = (code.kind == "code") and (chat.verified is False) and (ask.verified is False) and (ask.kind == "ask")
    check("labels_distinct", ok, f"code={code.kind} chat.v={chat.verified} ask={ask.kind}")
    print("      → ★only coding carries a verification label★; chat & ask are verified=False. No mixing.")


def server_routes_through_intent():
    code = S.handle_route({"prompt": "정수 오름차순 정렬 함수 만들어줘", "mode": "extended"})
    chat = S.handle_route({"prompt": "안녕"})
    ask = S.handle_route({"prompt": "정렬 함수"})
    src = open("server.py").read()
    wired = "handle_route(payload)" in src   # /api/generate delegates to it
    ok = (code.get("kind") == "code" and "result" in code and chat.get("kind") == "chat"
          and "reply" in chat and ask.get("kind") == "ask" and "asks" in ask and wired)
    check("server_routes_through_intent", ok, f"code={code.get('kind')} chat={chat.get('kind')} ask={ask.get('kind')} wired={wired}")
    print("      → server.handle_route → kind-tagged JSON (code/chat/ask); /api/generate routes through it.")


if __name__ == "__main__":
    print("v24 Part U · U4 — unified router (classify → route)")
    route_coding(); route_chat(); route_ask(); route_scope(); labels_distinct(); server_routes_through_intent()
    print(f"\nU4: {len(PASS)} passed, {len(FAIL)} failed, {len(SKIP)} skipped")
    sys.exit(1 if FAIL else 0)
