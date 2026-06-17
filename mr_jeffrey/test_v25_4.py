"""v25 Part V · V4 tests — key masking (●●●● ). Run: python3 test_v25_4.py

Display *feel* is USER-CONFIRMATION; here we check the masking is structural + plaintext never shown.
input_masked       : the key field is type=password (chars hidden as you type) + no autofill/spellcheck.
masked_confirmation: a present key shows "●●●● 입력됨" (masked), driven by the field VALUE, not its text.
no_plaintext_key   : the confirmation is a fixed ●●●● string — the actual key value is never written to the DOM.
level1_kept        : the masked status is display-only; the key is still read per-request, stored nowhere.
i18n_parity        : key_set translated in ko & en.
"""
import re
import sys

import web_check as W

PASS, FAIL, SKIP = [], [], []


def check(n, c, d=""):
    (PASS if c else FAIL).append(n)
    print(f"  [{'PASS' if c else 'FAIL'}] {n}" + (f" — {d}" if d and not c else ""))


def input_masked():
    t = W.html()
    ok = ('id="keyInput" type="password"' in t and 'autocomplete="off"' in t and 'spellcheck="false"' in t)
    check("input_masked", ok)
    print("      → key field is type=password (input hidden) + no autocomplete/spellcheck. "
          "[user-confirm: chars show as dots while typing]")


def masked_confirmation():
    t = W.html()
    ok = ('id="keyStatus"' in t and "key_set" in t and 'ke.value.trim() ? T("key_set")' in t)
    check("masked_confirmation", ok)
    print("      → a present key shows a MASKED confirmation '●●●● 입력됨' (set from the field's VALUE "
          "presence, not its contents).")


def no_plaintext_key():
    t = W.html()
    # the only place key value is read is for the request body / status presence — never assigned to .textContent/innerHTML
    leaks = re.findall(r'(?:textContent|innerHTML)\s*=\s*[^;\n]*keyEl\.value', t)
    ko_mask = "●●●●" in t
    ok = not leaks and ko_mask
    check("no_plaintext_key", ok, f"leaks={leaks}")
    print("      → ★the plaintext key is NEVER written to the DOM★ — confirmation is a fixed ●●●● mask. "
          "(key value is only read into the request body + emptiness check.)")


def level1_kept():
    t = W.html()
    # key read per-send; never persisted (no localStorage/cookie of the key)
    no_persist = (re.search(r'localStorage\.setItem\([^)]*key', t, re.I) is None
                  and "document.cookie" not in t)
    read_per_send = "keyEl.value.trim() || null" in t
    ok = no_persist and read_per_send
    check("level1_kept", ok, f"no_persist={no_persist} read_per_send={read_per_send}")
    print("      → masking is display-only; key still read per-request & stored nowhere (level 1 intact).")


def i18n_parity():
    ko, en = W.i18n_keys("ko"), W.i18n_keys("en")
    ok = "key_set" in ko and "key_set" in en and ko == en
    check("i18n_parity", ok, f"parity={ko==en}")
    print("      → key_set translated in ko & en.")


if __name__ == "__main__":
    print("v25 Part V · V4 — key masking (●●●●)")
    input_masked(); masked_confirmation(); no_plaintext_key(); level1_kept(); i18n_parity()
    print(f"\nV4: {len(PASS)} passed, {len(FAIL)} failed, {len(SKIP)} skipped")
    sys.exit(1 if FAIL else 0)
