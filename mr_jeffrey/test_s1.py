"""v22 Part S · S1 tests — Claude integration (mock) + key security LEVEL 1. Run: python3 test_s1.py

claude_call_mock  : no key → deterministic labeled simulation (never a fake "live").
key_not_stored    : the key is stored NOWHERE (env / globals / attrs / result) and `os` isn't imported.
mock_mode_works   : mock output drives the real downstream substrate (parses as HARAN) + is deterministic.
streaming_mock    : optional streaming path emits deltas that reconstruct the full text.
"""
import os
import sys

import claude_agent as CA
from haran_parser import parse

PASS, FAIL, SKIP = [], [], []


def check(n, c, d=""):
    (PASS if c else FAIL).append(n)
    print(f"  [{'PASS' if c else 'FAIL'}] {n}" + (f" — {d}" if d and not c else ""))


def claude_call_mock():
    r = CA.claude_generate("write a function that sums 1..n")
    ok = (not r.live) and r.source == "mock-sim" and "fn " in r.text and r.model == CA.DEFAULT_MODEL
    check("claude_call_mock", ok, f"live={r.live} source={r.source} model={r.model}")
    print(f"      → no key → SIM (live={r.live}, source='{r.source}'); model default '{r.model}'. "
          f"Honest: a mock is NEVER labeled live.")


def key_not_stored():
    SENTINEL = "sk-ant-TESTSENTINEL-DEADBEEF-d0nut-store-me"
    # Drive the key through the LIVE path (where it's actually used). Here the SDK isn't installed, so
    # this raises ClaudeError — but the key still flowed through `_live_generate` and was dropped by
    # the `finally`. The storage discipline must hold either way; the error must not leak the key.
    r, err_text = None, ""
    try:
        r = CA.claude_generate("sum 1..n", api_key=SENTINEL)
    except CA.ClaudeError as e:
        err_text = str(e)

    in_env = any(SENTINEL in str(v) for v in os.environ.values())
    in_globals = any(SENTINEL in str(v) for v in vars(CA).values())
    store_none = CA._KEY_STORE is None
    in_result = (r is not None) and SENTINEL in repr(r)
    in_error = SENTINEL in err_text
    no_attr = not hasattr(CA.claude_generate, "api_key")

    # structural guarantee: claude_agent.py does not import `os` (cannot touch env/files) and never
    # writes the key to env / a file, nor logs it.
    src = open("claude_agent.py").read()
    no_os_import = ("import os" not in src) and ("from os " not in src)
    no_env_write = "environ[" not in src
    no_file_write = "open(" not in src
    no_key_log = "print(api_key" not in src and "log(api_key" not in src

    ok = (not in_env and not in_globals and store_none and not in_result and not in_error
          and no_attr and no_os_import and no_env_write and no_file_write and no_key_log)
    check("key_not_stored", ok,
          f"env={in_env} globals={in_globals} store_none={store_none} result={in_result} "
          f"error={in_error} no_os={no_os_import} no_env_write={no_env_write}")
    print(f"      → ★LEVEL 1★ key absent from env({not in_env})/globals({not in_globals})/"
          f"result({not in_result})/error({not in_error}); _KEY_STORE is None({store_none}); "
          f"no os import({no_os_import}); received per-call & dropped. Stored: NOWHERE.")


def mock_mode_works():
    r1 = CA.claude_generate("sum 1..n")
    r2 = CA.claude_generate("sum 1..n")
    deterministic = r1.text == r2.text
    prog = parse(r1.text)
    parses = not prog.errors
    ok = deterministic and parses
    check("mock_mode_works", ok, f"deterministic={deterministic} parses={parses} errs={prog.errors[:1]}")
    print(f"      → mock is deterministic({deterministic}) and the generated HARAN PARSES({parses}) — "
          f"so S2/S3's write→verify→fix loop runs with zero network/secrets.")


def streaming_mock():
    chunks = []
    r = CA.claude_generate("sum 1..n", stream=True, on_delta=chunks.append)
    reconstructed = "".join(chunks)
    ok = len(chunks) >= 1 and reconstructed == r.text
    check("streaming_mock", ok, f"chunks={len(chunks)} reconstructed==text={reconstructed == r.text}")
    print(f"      → optional streaming: {len(chunks)} deltas reconstruct the full text "
          f"(same path shape as the live SDK stream).")


if __name__ == "__main__":
    print("v22 Part S · S1 — Claude integration (mock) + key security LEVEL 1")
    claude_call_mock(); key_not_stored(); mock_mode_works(); streaming_mock()
    print(f"\nS1: {len(PASS)} passed, {len(FAIL)} failed, {len(SKIP)} skipped")
    sys.exit(1 if FAIL else 0)
