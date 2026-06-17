"""v24 Part U · U7 tests — expected-questions collapse/expand + proceed-anyway. Run: python3 test_u24_7.py

Whether the fold *feels* tidy is USER-CONFIRMATION; here we check the panel structure + the force flow.
panel_collapsible   : renderAsks builds a collapsible head/body (collapsed by default, .open toggles).
proceed_anyway_force: 'proceed anyway' (force) skips the clarity gate → runs the pipeline (server + route).
answer_flow_wired   : the panel carries the request + a proceed button → onSend(request,{force:true}).
i18n_parity         : proceed_anyway + asks_hint translated in ko & en.
"""
import sys

import intent as I
import server as S
import web_check as W

PASS, FAIL, SKIP = [], [], []


def check(n, c, d=""):
    (PASS if c else FAIL).append(n)
    print(f"  [{'PASS' if c else 'FAIL'}] {n}" + (f" — {d}" if d and not c else ""))


def panel_collapsible():
    t = W.html()
    ok = ("asks-head" in t and "asks-body" in t and ".asks-body.open" in t
          and 'classList.toggle("open")' in t and "function renderAsks" in t)
    check("panel_collapsible", ok)
    print("      → expected-questions is a collapsible panel: '예상 질문 (N) ▾' header toggles the body "
          "(collapsed by default, smooth max-height). [user-confirm: tidy fold]")


def proceed_anyway_force():
    # route: vague → ask; force → code (proceed despite missing details)
    r_ask = I.route("정렬 함수")
    r_force = I.route("정렬 함수", force=True)
    # server: same via handle_route
    s_ask = S.handle_route({"prompt": "정렬 함수"})
    s_force = S.handle_route({"prompt": "정렬 함수", "force": True})
    ok = (r_ask.kind == "ask" and r_force.kind == "code"
          and s_ask["kind"] == "ask" and s_force["kind"] == "code")
    check("proceed_anyway_force", ok, f"route {r_ask.kind}->{r_force.kind} server {s_ask['kind']}->{s_force['kind']}")
    print("      → vague → expected questions; ★proceed anyway (force) skips the gate → runs the verified "
          "pipeline★ (the questions are suggestions, never a hard block).")


def answer_flow_wired():
    t = W.html()
    ok = ("renderAsks(steps, ev.asks, request)" in t          # panel gets the original request
          and "onSend(request, {force: true})" in t           # proceed button forces the pipeline
          and "asks-hint" in t)
    check("answer_flow_wired", ok)
    print("      → answer in the box (follow-up, history carries the original) OR click 'proceed anyway'. "
          "Hint shown; suggestions, not forced.")


def i18n_parity():
    ko, en = W.i18n_keys("ko"), W.i18n_keys("en")
    needed = {"proceed_anyway", "asks_hint", "asks_label"}
    ok = needed <= ko and needed <= en and ko == en
    check("i18n_parity", ok, f"missing={needed-(ko&en)} parity={ko==en}")
    print("      → proceed_anyway + asks_hint translated in ko & en.")


if __name__ == "__main__":
    print("v24 Part U · U7 — expected-questions collapse/expand + proceed-anyway")
    panel_collapsible(); proceed_anyway_force(); answer_flow_wired(); i18n_parity()
    print(f"\nU7: {len(PASS)} passed, {len(FAIL)} failed, {len(SKIP)} skipped")
    sys.exit(1 if FAIL else 0)
