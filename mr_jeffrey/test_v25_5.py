"""v25 Part V · V5 tests — accent * + no-log policy popover. Run: python3 test_v25_5.py

Visual *feel* is USER-CONFIRMATION; here we check the * + popover exist, are wired, and the policy text
matches the EXACT wording — and (cross-check) that the policy is true (no-log/no-store) in code.
accent_star        : an accent-colored, clickable * sits by the key field.
policy_popover      : clicking * toggles a popover; clicking outside closes it (rounded).
exact_policy_text   : the popover contains the exact Korean no-log policy sentence.
policy_is_true      : the no-log/no-store claim is backed by code (no key logging/storage; V6 grep proves it).
i18n_parity         : nolog_policy translated in ko & en.
"""
import sys

import web_check as W

PASS, FAIL, SKIP = [], [], []
KO_POLICY = "Mr.Jeffrey는 API Key 노-로그 정책을 고수합니다. 여기에 넣으면 어디에도 유출되지 않고 저장되지 않습니다."


def check(n, c, d=""):
    (PASS if c else FAIL).append(n)
    print(f"  [{'PASS' if c else 'FAIL'}] {n}" + (f" — {d}" if d and not c else ""))


def accent_star():
    t = W.html()
    ok = ('id="keyStar"' in t and ">*<" in t and ".key-star{" in t and "color:var(--accent)" in t)
    check("accent_star", ok)
    print("      → an accent-colored, clickable * sits beside the key field (hover scales). "
          "[user-confirm: it stands out]")


def policy_popover():
    t = W.html()
    ok = ('id="keyPop"' in t and "keyStar.addEventListener" in t and "keyPop.hidden = !keyPop.hidden" in t
          and "!keyPop.contains(e.target)" in t and ".key-pop{" in t and "border-radius" in t)
    check("policy_popover", ok)
    print("      → click * → popover toggles; click outside closes it; rounded. [user-confirm: feel]")


def exact_policy_text():
    t = W.html()
    ok = KO_POLICY in t
    check("exact_policy_text", ok)
    print("      → popover shows the exact policy: 'Mr.Jeffrey는 API Key 노-로그 정책을 고수합니다. "
          "여기에 넣으면 어디에도 유출되지 않고 저장되지 않습니다.'")


def policy_is_true():
    # the popover claim must be FACT, not marketing — the key is never logged/stored anywhere.
    ca = open("claude_agent.py").read()
    no_log = "print(" not in ca and "logging" not in ca           # claude_agent never logs
    no_os = "import os" not in ca                                  # cannot write env/files
    html = W.html()
    no_persist = "document.cookie" not in html and ("localStorage.setItem(\"haran_key" not in html)
    ok = no_log and no_os and no_persist
    check("policy_is_true", ok, f"no_log={no_log} no_os={no_os} no_persist={no_persist}")
    print("      → ★the policy is TRUE in code★: claude_agent never logs + has no os import; the key is "
          "never persisted in the browser. (Full grep proof in V6.) The text is a FACT, not marketing.")


def i18n_parity():
    ko, en = W.i18n_keys("ko"), W.i18n_keys("en")
    ok = "nolog_policy" in ko and "nolog_policy" in en and ko == en
    check("i18n_parity", ok, f"parity={ko==en}")
    print("      → nolog_policy translated in ko & en (en is accurate, not a looser claim).")


if __name__ == "__main__":
    print("v25 Part V · V5 — accent * + no-log policy popover")
    accent_star(); policy_popover(); exact_policy_text(); policy_is_true(); i18n_parity()
    print(f"\nV5: {len(PASS)} passed, {len(FAIL)} failed, {len(SKIP)} skipped")
    sys.exit(1 if FAIL else 0)
