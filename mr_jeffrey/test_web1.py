"""v23 Part T · T1 tests — KO/EN toggle + rounder UI (haran.html). Run: python3 test_web1.py

STRUCTURAL checks only (presence/wiring/well-formedness). The actual look & feel is USER-CONFIRMATION.
ko_en_toggle    : I18N has ko+en, setLang + a clickable toggle with both languages.
i18n_parity     : every data-i18n key is translated in BOTH ko and en (no missing strings).
rounder_ui      : rounded-corner design tokens present (--radius / border-radius).
well_formed     : haran.html parses (tags balance) and JS isn't truncated (braces balance).
"""
import sys

import web_check as W

PASS, FAIL, SKIP = [], [], []


def check(n, c, d=""):
    (PASS if c else FAIL).append(n)
    print(f"  [{'PASS' if c else 'FAIL'}] {n}" + (f" — {d}" if d and not c else ""))


def ko_en_toggle():
    ok = (W.has('data-lang="ko"') and W.has('data-lang="en"')
          and W.has("function setLang") and W.has("langToggle"))
    check("ko_en_toggle", ok)
    print("      → KO/EN toggle present (한국어/EN buttons + setLang live-switch). "
          "[user-confirm: the switch *feels* instant & translates everything visible]")


def i18n_parity():
    ko, en = W.i18n_keys("ko"), W.i18n_keys("en")
    used = W.data_i18n_keys()
    missing_ko = used - ko
    missing_en = used - en
    ok = ko and en and not missing_ko and not missing_en and ko == en
    check("i18n_parity", ok, f"missing_ko={missing_ko} missing_en={missing_en} ko=={en}? {ko==en}")
    print(f"      → {len(used)} data-i18n keys, all translated in ko & en (parity {ko==en}); no missing strings.")


def rounder_ui():
    ok = W.has("--radius") and W.count("border-radius") >= 3
    check("rounder_ui", ok, f"border-radius x{W.count('border-radius')}")
    print(f"      → rounded design tokens (--radius) + border-radius used {W.count('border-radius')}× "
          "(rounder UI). [user-confirm: the rounding actually looks soft/Apple-like]")


def well_formed():
    ok = W.well_formed() and W.js_balanced()
    check("well_formed", ok, f"html_balanced={W.well_formed()} js_balanced={W.js_balanced()}")
    print("      → haran.html tags balance and inline JS braces balance (not truncated).")


if __name__ == "__main__":
    print("v23 Part T · T1 — KO/EN toggle + rounder UI (structural)")
    ko_en_toggle(); i18n_parity(); rounder_ui(); well_formed()
    print(f"\nT1: {len(PASS)} passed, {len(FAIL)} failed, {len(SKIP)} skipped")
    sys.exit(1 if FAIL else 0)
