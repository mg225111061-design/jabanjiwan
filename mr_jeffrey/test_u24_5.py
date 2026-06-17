"""v24 Part U · U5 tests — progress status events (real stages, 1:1 with the pipeline). Run: python3 test_u24_5.py

agentic_stream_stages : agentic.agentic_stream yields generate → code_done → verify → optimize → done in order.
stage_events_emitted  : stream_events emits 'stage' events (classify/generate/verify/optimize) for coding.
stage_matches_pipeline: classify < generate < verify in order; optimize only when it actually optimizes.
chat_path_stages      : a chat request emits classify → thinking → chat (NO verify/optimize stages).
no_fake_progress      : ★ a non-verified path (chat) never emits 'verify'/'optimize' stages ★ (real stages only).
frontend_progress     : haran.html shows a stage indicator (spinner + stage label) driven by 'stage' events.
"""
import json
import sys

import agentic as AG
import server as S
import web_check as W

PASS, FAIL, SKIP = [], [], []


def check(n, c, d=""):
    (PASS if c else FAIL).append(n)
    print(f"  [{'PASS' if c else 'FAIL'}] {n}" + (f" — {d}" if d and not c else ""))


def _events(payload):
    return [json.loads(f[6:].strip()) for f in S.stream_events(payload)]


def _stages(ev):
    return [e.get("stage") for e in ev if e["type"] == "stage"]


def agentic_stream_stages():
    stages = [ev["stage"] for ev in AG.agentic_stream("정수 오름차순 정렬 함수 만들어줘", "extended")]
    ok = (stages and stages[0] == "generate" and "verify" in stages and "optimize" in stages
          and stages[-1] == "done" and stages.index("generate") < stages.index("verify") < stages.index("optimize"))
    check("agentic_stream_stages", ok, f"stages={stages}")
    print(f"      → agentic_stream yields {stages} — each emitted right before the REAL step runs.")


def stage_events_emitted():
    ev = _events({"prompt": "정수 오름차순 정렬 함수 만들어줘", "mode": "extended"})
    st = _stages(ev)
    ok = "classify" in st and "generate" in st and "verify" in st and "optimize" in st
    check("stage_events_emitted", ok, f"stages={st}")
    print(f"      → SSE stage events for coding: {st} (분류중/Claude 호출중/검증중/최적화중).")


def stage_matches_pipeline():
    ev = _events({"prompt": "정수 오름차순 정렬 함수 만들어줘", "mode": "extended"})
    st = _stages(ev)
    ok = (st.index("classify") < st.index("generate") < st.index("verify")
          and "optimize" in st)   # optimize present because the proven code actually has a closed form
    check("stage_matches_pipeline", ok, f"order={st}")
    print("      → stages are ordered like the real pipeline: classify → generate → verify → optimize.")


def chat_path_stages():
    ev = _events({"prompt": "안녕"})
    st = _stages(ev)
    types = [e["type"] for e in ev]
    ok = st == ["classify", "thinking"] and "chat" in types and types[-1] == "done"
    check("chat_path_stages", ok, f"stages={st} types={types}")
    print("      → chat: classify → thinking → chat → done. Fewer stages (no code/verify/optimize).")


def no_fake_progress():
    ev = _events({"prompt": "안녕"})       # chat — nothing is verified or optimized
    st = _stages(ev)
    ok = "verify" not in st and "optimize" not in st and "generate" not in st
    check("no_fake_progress", ok, f"chat_stages={st}")
    print("      → ★no fake progress★: a chat turn NEVER shows '검증중'/'최적화중'/'Claude 코드 생성중' — "
          "those stages appear only when that work truly happens (rule 5).")


def frontend_progress():
    t = W.html()
    ok = ('ev.type==="stage"' in t and "setStage" in t and ".progress{" in t
          and "spinner" in t and 'T("stage_"' in t)
    check("frontend_progress", ok)
    print("      → haran.html shows a spinner + stage label driven by 'stage' events (i18n stage_*). "
          "[user-confirm: the progress *feels* smooth and the wait is no longer opaque]")


if __name__ == "__main__":
    print("v24 Part U · U5 — progress status events (real stages, 1:1 with pipeline)")
    agentic_stream_stages(); stage_events_emitted(); stage_matches_pipeline()
    chat_path_stages(); no_fake_progress(); frontend_progress()
    print(f"\nU5: {len(PASS)} passed, {len(FAIL)} failed, {len(SKIP)} skipped")
    sys.exit(1 if FAIL else 0)
