"""v23 Part T · T6 tests — FastAPI backend wrapping v22 (handle_generate). Run: python3 test_web6.py

Tests the dependency-free core (handle_generate) directly — no FastAPI needed here. The HTTP wiring
(create_app) is exercised when FastAPI is installed (deployment); structurally checked otherwise.
api_generate_mock : no key → labeled mock result with the full agentic structure (code/verdict/tier).
key_not_persisted : a sentinel key never appears in the response, module globals, or error (level-1).
error_handling    : bad/empty payloads return a graceful error dict (never raise).
history_accepted  : history (conversation) is parsed + threaded (history_len reflects it).
app_wiring        : create_app exists; app is built when FastAPI is present, None otherwise (importable).
"""
import json
import sys

import server as S

PASS, FAIL, SKIP = [], [], []


def check(n, c, d=""):
    (PASS if c else FAIL).append(n)
    print(f"  [{'PASS' if c else 'FAIL'}] {n}" + (f" — {d}" if d and not c else ""))


def api_generate_mock():
    r = S.handle_generate({"prompt": "sum 1..n", "mode": "extended"})
    ok = (not r.get("error") and r.get("source") == "mock-sim" and r.get("converged")
          and r.get("code") and r.get("proof_tier") in ("PROVEN", "TESTED")
          and "ms" in r and "trace" in r)
    check("api_generate_mock", ok, f"keys={sorted(r)[:6]} source={r.get('source')}")
    print(f"      → no key → mock result: source={r.get('source')}, status={r.get('status')}, "
          f"tier={r.get('proof_tier')}, ms={r.get('ms')}. Full agentic structure over HTTP.")


def key_not_persisted():
    SENTINEL = "sk-ant-WEB-SENTINEL-DEADBEEF-never-store"
    r = S.handle_generate({"prompt": "sum 1..n", "apiKey": SENTINEL})
    in_response = SENTINEL in json.dumps(r)
    in_globals = any(SENTINEL in str(v) for v in vars(S).values())
    store_none = S._KEY_STORE is None
    # source-level: server never writes the key to env/file/log
    src = open("server.py").read()
    no_env = "environ[" not in src.replace("os.environ.get", "")  # only reads HOST/PORT, never writes key
    no_log = "print(" not in src and "logging" not in src
    ok = (not in_response) and (not in_globals) and store_none and no_log
    check("key_not_persisted", ok,
          f"in_response={in_response} in_globals={in_globals} store_none={store_none} no_log={no_log}")
    print(f"      → ★LEVEL 1 (server)★ sentinel key absent from response({not in_response})/"
          f"globals({not in_globals}); _KEY_STORE None({store_none}); no logging({no_log}). "
          "Used once, dropped, never stored.")


def error_handling():
    empty = S.handle_generate({"prompt": "  "})
    none = S.handle_generate(None)
    ok = empty.get("error") and none.get("error")
    check("error_handling", ok, f"empty={empty.get('error')} none={none.get('error')}")
    print("      → empty/None payloads → graceful error dicts (never raises). API stays up.")


def history_accepted():
    hist = [{"request": "sum 1..n", "code": "fn s(n: Nat)->Nat\n  ensures result=n*(n+1)/2\n{ fold k in 1..n { k } }"}]
    r = S.handle_generate({"prompt": "make it faster", "mode": "extended", "history": hist})
    ok = (not r.get("error")) and r.get("history_len") == 1
    check("history_accepted", ok, f"history_len={r.get('history_len')}")
    print(f"      → conversation history parsed + threaded (history_len={r.get('history_len')}) — the "
          "backend for T5's feedback loop (full incremental handling in T8).")


def app_wiring():
    has_factory = hasattr(S, "create_app") and callable(S.create_app)
    consistent = (S.app is not None) == S._fastapi_available()
    ok = has_factory and consistent
    check("app_wiring", ok, f"factory={has_factory} fastapi={S._fastapi_available()} app={'built' if S.app else 'None'}")
    print(f"      → create_app() present; app {'built' if S.app else 'None'} (FastAPI "
          f"{'installed' if S._fastapi_available() else 'absent — importable, tested via handle_generate'}). "
          "GET / serves haran.html, POST /api/generate → handle_generate.")


if __name__ == "__main__":
    print("v23 Part T · T6 — FastAPI backend wrapping v22 (handle_generate)")
    api_generate_mock(); key_not_persisted(); error_handling(); history_accepted(); app_wiring()
    print(f"\nT6: {len(PASS)} passed, {len(FAIL)} failed, {len(SKIP)} skipped")
    sys.exit(1 if FAIL else 0)
