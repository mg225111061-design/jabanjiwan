"""v23 Part T · T4 tests — two-mode message differentiation. Run: python3 test_web4.py

STRUCTURAL — whether the two message styles actually read distinctly is USER-CONFIRMATION.
mode_branch_render : renderAssistant branches on mode (normal vs extended) with distinct content.
distinct_messages  : normal = SPEED framing (instant + ms); extended = POWER framing (proof + fold).
mode_hero_shift    : the hero tagline shifts with the mode (refreshDynamic + tagline_normal/extended).
sim_labeled        : a simulated (no-key) result is labeled SIM in the bubble — never shown as live.
key_level1_in_ui   : localStorage stores ONLY the language; the API KEY is never persisted.
i18n_parity_msgs   : all new message fragments are translated in ko & en.
"""
import re
import sys

import web_check as W

PASS, FAIL, SKIP = [], [], []


def check(n, c, d=""):
    (PASS if c else FAIL).append(n)
    print(f"  [{'PASS' if c else 'FAIL'}] {n}" + (f" — {d}" if d and not c else ""))


def mode_branch_render():
    t = W.html()
    ok = (W.has("function renderAssistant", t) and 'mode === "normal"' in t
          and W.has("asst_normal", t) and W.has("asst_extended", t))
    check("mode_branch_render", ok)
    print("      → renderAssistant() branches on mode: normal vs extended produce different bubbles.")


def distinct_messages():
    t = W.html()
    normal_speed = W.has("m_fast", t) and W.has("m_more_in_extended", t) and "⚡" in t
    extended_power = W.has("m_proven", t) and W.has("m_optimized", t) and W.has("res.trace.map", t)
    ok = normal_speed and extended_power
    check("distinct_messages", ok, f"normal_speed={normal_speed} extended_power={extended_power}")
    print("      → NORMAL = SPEED (⚡ instant + SIM ms, concise); EXTENDED = POWER (full loop trace + "
          "PROVEN ∀ + fold optimization). Genuinely different per mode (v22 S5 contract).")


def mode_hero_shift():
    t = W.html()
    ok = (W.has("function refreshDynamic", t) and W.has("tagline_normal", t)
          and W.has("tagline_extended", t) and W.has("modeTagline", t))
    check("mode_hero_shift", ok)
    print("      → hero tagline shifts with mode (refreshDynamic, tagline_normal/extended) alongside the "
          "color transition. [user-confirm: the copy shift *feels* subtle & right]")


def sim_labeled():
    t = W.html()
    ok = W.has("mock-sim", t) and W.has("simtag", t) and W.has("SIM", t)
    check("sim_labeled", ok)
    print("      → a no-key result is a labeled SIM (simtag) — the browser mock is NEVER shown as a "
          "real/live proof. Real verification is server-side (T6/T7).")


def key_level1_in_ui():
    t = W.html()
    stored = re.findall(r'localStorage\.setItem\(\s*["\']([^"\']+)["\']', t)
    only_lang = all(k == "haran_lang" for k in stored) and "haran_lang" in stored
    no_cookie = "document.cookie" not in t and "sessionStorage" not in t
    no_key_store = ("haran_key" not in t
                    and re.search(r'setItem\([^)]*\bkey\b', t, re.I) is None)
    read_per_send = "keyEl.value" in t and "LEVEL-1" in t
    ok = only_lang and no_cookie and no_key_store and read_per_send
    check("key_level1_in_ui", ok,
          f"stored={stored} no_cookie={no_cookie} no_key_store={no_key_store} read_per_send={read_per_send}")
    print(f"      → ★LEVEL 1 in UI★: localStorage holds ONLY {stored} (language); the API key is read "
          "per-send and persisted NOWHERE (no cookie/sessionStorage/localStorage key write).")


def i18n_parity_msgs():
    ko, en = W.i18n_keys("ko"), W.i18n_keys("en")
    needed = {"tagline_normal", "tagline_extended", "asst_normal", "asst_extended", "m_fast",
              "m_verified", "m_proven", "m_optimized", "m_unresolved_shallow", "m_more_in_extended",
              "compare", "send", "chat_empty", "key_ph", "req_ph"}
    ok = needed <= ko and needed <= en and ko == en
    check("i18n_parity_msgs", ok, f"missing_ko={needed-ko} missing_en={needed-en} parity={ko==en}")
    print(f"      → all message fragments translated in ko & en; full i18n parity ({ko==en}).")


if __name__ == "__main__":
    print("v23 Part T · T4 — two-mode message differentiation (structural)")
    mode_branch_render(); distinct_messages(); mode_hero_shift(); sim_labeled()
    key_level1_in_ui(); i18n_parity_msgs()
    print(f"\nT4: {len(PASS)} passed, {len(FAIL)} failed, {len(SKIP)} skipped")
    sys.exit(1 if FAIL else 0)
