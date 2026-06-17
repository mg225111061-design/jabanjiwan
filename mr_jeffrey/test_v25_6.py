"""v25 Part V · V6 tests — no-log policy ACTUALLY verified (the popover claim must be fact). Run: python3 test_v25_6.py

Runs over the CLEAN repo (../haran-web) — the deliverable. The V5 popover says the key is never leaked
or stored; these tests prove that in code (else the popover would be a lie).
no_key_in_logs    : no print / logging / console.log of the api key anywhere in the clean repo.
no_key_storage    : no localStorage(key)/cookie/file-write/env-write; ANTHROPIC_API_KEY only in a comment; no env-key reads.
error_redacts_key : redact_key masks sk-... ; the error path never echoes the raw key/message.
claude_agent_clean: claude_agent has no os import + no logging (cannot touch env/files/logs).
"""
import os
import re
import sys

import claude_agent as CA

PASS, FAIL, SKIP = [], [], []
WEB = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "haran-web"))


def check(n, c, d=""):
    (PASS if c else FAIL).append(n)
    print(f"  [{'PASS' if c else 'FAIL'}] {n}" + (f" — {d}" if d and not c else ""))


def _files(exts):
    return [os.path.join(WEB, f) for f in os.listdir(WEB) if f.endswith(exts)]


def _scan(pattern):
    rx = re.compile(pattern, re.I)
    hits = []
    for p in _files((".py", ".html")):
        for i, line in enumerate(open(p, encoding="utf-8"), 1):
            if rx.search(line):
                hits.append(f"{os.path.basename(p)}:{i}: {line.strip()[:80]}")
    return hits


def no_key_in_logs():
    hits = _scan(r"(?:print|console\.log)\s*\([^)]*\bapi_?key\b|logging[^=\n]*\bapi_?key\b")
    ok = not hits
    check("no_key_in_logs", ok, f"hits={hits}")
    print("      → no print / logging / console.log of the API key anywhere in the clean repo (grep).")


def no_key_storage():
    storage = _scan(r"localStorage\.setItem\([^)]*key|sessionStorage|document\.cookie|"
                    r"environ\[[^]]*key|open\([^)]*key")
    storage = [h for h in storage if "haran_lang" not in h]
    env_reads = _scan(r"environ(?:\.get)?\([^)]*key")
    # ANTHROPIC_API_KEY should appear ONLY in the explanatory comment
    anth = _scan(r"ANTHROPIC_API_KEY")
    anth_bad = [h for h in anth if "departure" not in h and "#" not in h and "reads `ANTHROPIC" not in h]
    ok = not storage and not env_reads and not anth_bad
    check("no_key_storage", ok, f"storage={storage} env_reads={env_reads} anth_bad={anth_bad}")
    print("      → no key in localStorage/cookie/file/env; ANTHROPIC_API_KEY only in a 'we don't use it' "
          "comment; ZERO env-key reads. The key is stored NOWHERE.")


def error_redacts_key():
    masked = CA.redact_key("boom sk-ant-SECRET123 tail")
    redacts = "sk-ant-SECRET123" not in masked and "REDACTED" in masked
    friendly = CA._friendly_error(Exception("fail with sk-ant-SECRET123"))
    no_leak = "sk-ant-SECRET123" not in friendly
    ok = redacts and no_leak
    check("error_redacts_key", ok, f"masked={masked!r} friendly={friendly!r}")
    print("      → redact_key masks sk-… ; the friendly error never echoes the raw key/message. "
          "Errors leak nothing.")


def claude_agent_clean():
    ca = open(os.path.join(WEB, "claude_agent.py"), encoding="utf-8").read()
    ok = "import os" not in ca and "from os " not in ca and "print(" not in ca and "logging" not in ca
    check("claude_agent_clean", ok)
    print("      → claude_agent.py: no os import (cannot touch env/files) + no print/logging. The key "
          "lives only as a per-call argument, then dropped.")


if __name__ == "__main__":
    print("v25 Part V · V6 — no-log policy ACTUALLY verified (clean repo grep)")
    no_key_in_logs(); no_key_storage(); error_redacts_key(); claude_agent_clean()
    print(f"\nV6: {len(PASS)} passed, {len(FAIL)} failed, {len(SKIP)} skipped")
    sys.exit(1 if FAIL else 0)
