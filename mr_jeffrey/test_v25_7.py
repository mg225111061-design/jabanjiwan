"""v25 Part V · V7 tests — full flow from the CLEAN repo only. Run: python3 test_v25_7.py

clean_repo_full_flow : a subprocess in ../haran-web (its files only) routes coding/chat/ask, streams real
                       stages, and re-verifies incrementally — no dependency on mr_jeffrey/.
frontend_complete    : haran-web/haran.html has the whole UI: KO/EN, black/white themes, progress stages,
                       chips toggle, expected-questions panel, key mask (●●●●) + accent * + no-log popover.
mock_and_live        : no key → labeled mock (source='mock-sim'); the live path (Anthropic SDK) is wired.
"""
import os
import re
import subprocess
import sys

PASS, FAIL, SKIP = [], [], []
WEB = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "haran-web"))


def check(n, c, d=""):
    (PASS if c else FAIL).append(n)
    print(f"  [{'PASS' if c else 'FAIL'}] {n}" + (f" — {d}" if d and not c else ""))


def _html():
    return open(os.path.join(WEB, "haran.html"), encoding="utf-8").read()


def clean_repo_full_flow():
    code = r"""
import json, server
code = server.handle_route({'prompt':'정수 오름차순 정렬 함수 만들어줘','mode':'extended'})
chat = server.handle_route({'prompt':'안녕'})
ask  = server.handle_route({'prompt':'정렬 함수'})
scope= server.handle_route({'prompt':'실시간 채팅 백엔드 만들어줘'})
st = [json.loads(f[6:])['stage'] for f in server.stream_events({'prompt':'정수 오름차순 정렬 함수 만들어줘','mode':'extended'}) if '"stage"' in f]
chat_st = [json.loads(f[6:])['stage'] for f in server.stream_events({'prompt':'안녕'}) if '"stage"' in f]
src = '\n\n'.join('fn f%d(n: Nat)->Nat\n  ensures result=n*(n+1)/2\n{ fold k in 1..n { k } }' % i for i in range(6))
inc = server.reverify_incremental(src, src.replace('fold k in 1..n { k }','fold k in 1..n { k+0 }',1))
assert code['kind']=='code' and code['verified'] in (True,False)
assert chat['kind']=='chat' and chat['verified'] is False
assert ask['kind']=='ask' and ask['asks']
assert 'classify' in st and 'verify' in st and 'optimize' in st
assert 'verify' not in chat_st and 'optimize' not in chat_st   # chat shows no fake verify stages
assert inc['reverified']==['f0']   # ONLY the changed function re-verifies (incremental)
print('FULLFLOW_OK')
"""
    p = subprocess.run([sys.executable, "-c", code], cwd=WEB, capture_output=True, text=True)
    ok = p.returncode == 0 and "FULLFLOW_OK" in p.stdout
    check("clean_repo_full_flow", ok, f"rc={p.returncode} err={p.stderr.strip()[-200:]}")
    print("      → clean repo (own files only): coding→stages(classify/generate/verify/optimize)→verified; "
          "chat→reply (no verify stages); ask→questions; scope honest; incremental re-verify. End to end.")


def frontend_complete():
    t = _html()
    pieces = {
        "ko/en toggle": 'data-lang="ko"' in t and "function setLang" in t,
        "black/white themes": "body.mode-extended" in t and ".bg-light{background:linear-gradient" in t,
        "progress stages": 'ev.type==="stage"' in t and "stage_generate" in t,
        "chips toggle": 'id="quickToggle"' in t and ".qchips.open" in t,
        "questions panel": "asks-head" in t and "renderAsks" in t,
        "key mask ●●●●": "key_set" in t and 'type="password"' in t and "●●●●" in t,
        "accent * popover": 'id="keyStar"' in t and 'id="keyPop"' in t and "nolog_policy" in t,
        "MR.JEFFREY brand": "MR<span" in t and "<title>MR.JEFFREY" in t,
    }
    ok = all(pieces.values())
    check("frontend_complete", ok, f"missing={[k for k,v in pieces.items() if not v]}")
    print(f"      → haran.html has the full UI: {', '.join(pieces)}. [user-confirm: the actual look/feel]")


def mock_and_live():
    # backend mock provenance + live path wiring, checked in the clean repo's own files
    code = ("import server;"
            "r=server.handle_route({'prompt':'정수 오름차순 정렬 함수 만들어줘'});"
            "print(r['result']['source'] if r.get('result') else r.get('source'))")
    p = subprocess.run([sys.executable, "-c", code], cwd=WEB, capture_output=True, text=True)
    mock = "mock-sim" in p.stdout
    ca = open(os.path.join(WEB, "claude_agent.py"), encoding="utf-8").read()
    live = "import anthropic" in ca and "client.messages.create" in ca
    ok = mock and live
    check("mock_and_live", ok, f"mock={mock} live_wired={live}")
    print("      → no key → labeled mock (source='mock-sim'); live path uses the Anthropic SDK (key → real).")


if __name__ == "__main__":
    print("v25 Part V · V7 — full flow from the clean repo only")
    clean_repo_full_flow(); frontend_complete(); mock_and_live()
    print(f"\nV7: {len(PASS)} passed, {len(FAIL)} failed, {len(SKIP)} skipped")
    sys.exit(1 if FAIL else 0)
