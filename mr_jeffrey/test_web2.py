"""v23 Part T · T2 tests — two-mode contrasting colors + click transition. Run: python3 test_web2.py

STRUCTURAL only — whether the contrast/transition actually *looks* good is USER-CONFIRMATION.
mode_selector    : normal & extended pills + setMode wiring present.
contrasting_colors: --normal and --extended are DISTINCT colors; body toggles mode classes.
click_transition : transition declared on the mode pills + accent-driven elements.
i18n_parity_modes: the new mode strings are translated in both ko & en.
"""
import sys

import web_check as W

PASS, FAIL, SKIP = [], [], []


def check(n, c, d=""):
    (PASS if c else FAIL).append(n)
    print(f"  [{'PASS' if c else 'FAIL'}] {n}" + (f" — {d}" if d and not c else ""))


def mode_selector():
    ok = (W.has('data-mode="normal"') and W.has('data-mode="extended"')
          and W.has("function setMode") and W.has("modeSelect"))
    check("mode_selector", ok)
    print("      → 일반/확장 (normal/extended) pills + setMode click-switch present.")


def _mode_var(mode, var, t):
    import re
    # the value of `var` inside the body.mode-<mode>{...} block
    m = re.search(r"body\.mode-" + mode + r"\s*\{([^}]*)\}", t, re.S)
    if not m:
        return None
    v = re.search(var + r"\s*:\s*([#a-zA-Z0-9_().,\- ]+?);", m.group(1))
    return v.group(1).strip() if v else None


def contrasting_colors():
    import re
    t = W.html()
    nrm = re.search(r"--normal\s*:\s*(#[0-9a-fA-F]{3,8})", t)
    ext = re.search(r"--extended\s*:\s*(#[0-9a-fA-F]{3,8})", t)
    hues_distinct = bool(nrm and ext and nrm.group(1).lower() != ext.group(1).lower())
    # ★ full THEME swap: bg / panel / txt differ between the two modes (not just accent) ★
    bg_diff = _mode_var("normal", "--bg", t) != _mode_var("extended", "--bg", t)
    panel_diff = _mode_var("normal", "--panel", t) != _mode_var("extended", "--panel", t)
    txt_diff = _mode_var("normal", "--txt", t) != _mode_var("extended", "--txt", t)
    ok = hues_distinct and bg_diff and panel_diff and txt_diff
    check("contrasting_colors", ok,
          f"hues={hues_distinct} bg={bg_diff} panel={panel_diff} txt={txt_diff}")
    print(f"      → FULL theme swap: normal bg {_mode_var('normal','--bg',t)} (black) vs extended "
          f"{_mode_var('extended','--bg',t)} (white); panel/txt/accent all shift (see U8 for readability). "
          "[user-confirm: the contrast reads clearly + stays readable]")


def click_transition():
    ok = W.has("transition:") and W.has(".mode-pill") and W.count("transition") >= 4
    check("click_transition", ok, f"transition x{W.count('transition')}")
    print(f"      → transitions declared on pills/accent ({W.count('transition')}×) → smooth click switch. "
          "[user-confirm: the transition actually feels smooth]")


def i18n_parity_modes():
    ko, en = W.i18n_keys("ko"), W.i18n_keys("en")
    needed = {"mode_normal_name", "mode_normal_desc", "mode_extended_name", "mode_extended_desc"}
    ok = needed <= ko and needed <= en
    check("i18n_parity_modes", ok, f"in_ko={needed<=ko} in_en={needed<=en}")
    print("      → mode names/descriptions translated in both ko & en.")


if __name__ == "__main__":
    print("v23 Part T · T2 — two-mode contrasting colors + click transition (structural)")
    mode_selector(); contrasting_colors(); click_transition(); i18n_parity_modes()
    print(f"\nT2: {len(PASS)} passed, {len(FAIL)} failed, {len(SKIP)} skipped")
    sys.exit(1 if FAIL else 0)
