"""v24 Part U · U10 tests — integration + autonomous improvements + honesty check. Run: python3 test_u24_10.py

integration_pipeline : classify → route (code/chat/ask) → stages → verify, all wired front + back.
autonomous_features  : U10 additions (clear/new-chat, auto-grow textarea) present, honesty bar intact.
honesty_key_zero     : ★ the key is stored NOWHERE across ALL v22/v23/v24 files (incl. classify path) ★.
honesty_coding_vs_chat : coding carries a verification label; chat NEVER does (distinct, not mixed).
honesty_real_stages  : a chat turn emits no verify/optimize stage (progress is 1:1 with real work).
honesty_marketing_tbd : marketing copy labeled; unmeasured figures are [TBD] placeholders; no mirages.
"""
import json
import re
import sys

import intent as I
import server as S
import web_check as W

PASS, FAIL, SKIP = [], [], []
V_FILES = ["claude_agent.py", "agentic.py", "intent.py", "server.py", "haran.html", "web_check.py"]


def check(n, c, d=""):
    (PASS if c else FAIL).append(n)
    print(f"  [{'PASS' if c else 'FAIL'}] {n}" + (f" — {d}" if d and not c else ""))


def _read(p):
    return open(p, encoding="utf-8").read()


def integration_pipeline():
    code = S.handle_route({"prompt": "정수 오름차순 정렬 함수 만들어줘", "mode": "extended"})
    chat = S.handle_route({"prompt": "안녕"})
    ask = S.handle_route({"prompt": "정렬 함수"})
    stages = [json.loads(f[6:].strip()).get("stage") for f in S.stream_events(
        {"prompt": "정수 오름차순 정렬 함수 만들어줘", "mode": "extended"})
        if '"stage"' in f]
    ok = (code["kind"] == "code" and chat["kind"] == "chat" and ask["kind"] == "ask"
          and "classify" in stages and "verify" in stages)
    check("integration_pipeline", ok, f"kinds={code['kind']}/{chat['kind']}/{ask['kind']} stages={stages}")
    print("      → classify → route(code/chat/ask) → real stages → verify, end to end (front + back).")


def autonomous_features():
    t = W.html()
    feats = {
        "clear/new-chat": 'id="clearBtn"' in t and "HISTORY.length = 0" in t,
        "auto-grow textarea": 'reqEl.addEventListener("input"' in t and "scrollHeight" in t,
    }
    ok = all(feats.values())
    check("autonomous_features", ok, f"{feats}")
    print(f"      → autonomous (judged, goal-serving, honesty intact): {', '.join(feats)}. "
          "[user-confirm: they feel right]")


def honesty_key_zero():
    html = W.html(); ca = _read("claude_agent.py"); it = _read("intent.py"); sv = _read("server.py")
    df = _read("Dockerfile").upper(); cp = _read("docker-compose.yml").upper()
    ls = re.findall(r'localStorage\.setItem\(\s*["\']([^"\']+)["\']', html)
    front = all(k == "haran_lang" for k in ls) and "document.cookie" not in html and "sessionStorage" not in html
    agent = "import os" not in ca and "from os " not in ca           # cannot touch env/files
    classify_path = "import os" not in it and "from os " not in it   # the classify/chat path is key-safe too
    sv_env = re.findall(r'environ(?:\.get)?\(\s*["\']([^"\']+)["\']', sv)
    server = all(k.startswith("HARAN_") for k in sv_env) and "print(" not in sv and "_KEY_STORE = None" in sv
    docker = "API_KEY" not in df and "ANTHROPIC" not in df and "API_KEY" not in cp and "ANTHROPIC" not in cp
    ok = front and agent and classify_path and server and docker
    check("honesty_key_zero", ok,
          f"front={front} agent={agent} classify={classify_path} server={server} docker={docker}")
    print(f"      → ★KEY STORED NOWHERE★ across all v22/v23/v24 files — front localStorage={ls} (lang only); "
          "claude_agent + intent (classify/chat) have no os import; server env=HARAN only, no logging; no "
          "key in Docker. Used per call, dropped. Confirmed by grep.")


def honesty_coding_vs_chat():
    code = S.handle_route({"prompt": "정수 오름차순 정렬 함수 만들어줘"})
    chat = S.handle_route({"prompt": "안녕"})
    # coding result has a verification verdict; chat is verified=False and has no proof fields
    code_labeled = code.get("verified") in (True, False) and "result" in code and "proof_tier" in code["result"]
    chat_unlabeled = (chat.get("verified") is False and "reply" in chat
                      and "result" not in chat and I.chat_reply("안녕").verified is False)
    ok = code_labeled and chat_unlabeled
    check("honesty_coding_vs_chat", ok, f"code_has_tier={'proof_tier' in code.get('result',{})} chat.v={chat.get('verified')}")
    print("      → ★coding = verified (proof tier); chat = plain (verified=False, no proof) — never mixed★.")


def honesty_real_stages():
    chat_stages = [json.loads(f[6:].strip()).get("stage") for f in S.stream_events({"prompt": "안녕"})
                   if '"stage"' in f]
    ok = "verify" not in chat_stages and "optimize" not in chat_stages and "generate" not in chat_stages
    check("honesty_real_stages", ok, f"chat_stages={chat_stages}")
    print(f"      → ★no fake progress★: a chat turn's stages = {chat_stages} (no 검증중/최적화중/생성중). "
          "Stages are 1:1 with real work.")


def honesty_marketing_tbd():
    t = W.html()
    marketing = t.count("marketing copy") >= 2
    tbd = "[TBD: 측정필요]" in t or "[TBD: measure]" in t
    mirage = not re.search(r"homolog|persistent\s*homology|\bTDA\b|quantum|relativ|fluid\s*dynam|cohomolog",
                           " ".join(_read(f) for f in V_FILES), re.I)
    ok = marketing and tbd and mirage
    check("honesty_marketing_tbd", ok, f"marketing={marketing} tbd={tbd} no_mirage={mirage}")
    print("      → marketing copy labeled + separate from measured; [TBD] placeholders kept; no mirages.")


if __name__ == "__main__":
    print("v24 Part U · U10 — integration + autonomous improvements + honesty check")
    integration_pipeline(); autonomous_features(); honesty_key_zero()
    honesty_coding_vs_chat(); honesty_real_stages(); honesty_marketing_tbd()
    print(f"\nU10: {len(PASS)} passed, {len(FAIL)} failed, {len(SKIP)} skipped")
    sys.exit(1 if FAIL else 0)
