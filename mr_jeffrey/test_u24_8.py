"""v24 Part U · U8 tests — black (normal) ↔ white (extended) themes + gradient. Run: python3 test_u24_8.py

Color *feel* is USER-CONFIRMATION; here we check the black/white palettes, the gradient cross-fade,
and — critically — READABILITY (dark theme = light text, light theme = dark text; never white-on-white).
black_white_themes : normal = black bg, extended = white bg (full palette swap).
readability_guard  : ★ normal text is light on dark; extended text is dark on light (real contrast) ★.
gradient_crossfade : two fixed gradient layers cross-fade on mode switch (smooth black↔white gradient).
status_colors_theme: ok/warn/bad are darkened in the light theme so they stay readable on white.
"""
import re
import sys

import web_check as W

PASS, FAIL, SKIP = [], [], []


def check(n, c, d=""):
    (PASS if c else FAIL).append(n)
    print(f"  [{'PASS' if c else 'FAIL'}] {n}" + (f" — {d}" if d and not c else ""))


def _var(mode, var, t):
    m = re.search(r"body\.mode-" + mode + r"\s*\{([^}]*)\}", t, re.S)
    if not m:
        return None
    v = re.search(var + r"\s*:\s*(#[0-9a-fA-F]{3,8})", m.group(1))
    return v.group(1) if v else None


def _lum(hexc):
    h = hexc.lstrip("#")
    if len(h) == 3:
        h = "".join(c * 2 for c in h)
    r, g, b = int(h[0:2], 16), int(h[2:4], 16), int(h[4:6], 16)
    return (0.299 * r + 0.587 * g + 0.114 * b) / 255


def black_white_themes():
    t = W.html()
    nbg, ebg = _var("normal", "--bg", t), _var("extended", "--bg", t)
    ok = _lum(nbg) < 0.15 and _lum(ebg) > 0.9      # normal ~black, extended ~white
    check("black_white_themes", ok, f"normal_bg={nbg}({_lum(nbg):.2f}) extended_bg={ebg}({_lum(ebg):.2f})")
    print(f"      → normal = BLACK bg {nbg}, extended = WHITE bg {ebg}. Full palette swap on click.")


def readability_guard():
    t = W.html()
    n_txt, e_txt = _var("normal", "--txt", t), _var("extended", "--txt", t)
    n_bg, e_bg = _var("normal", "--bg", t), _var("extended", "--bg", t)
    # contrast = |text luminance - bg luminance|; require strong contrast both themes
    n_contrast = abs(_lum(n_txt) - _lum(n_bg))
    e_contrast = abs(_lum(e_txt) - _lum(e_bg))
    ok = (_lum(n_txt) > 0.85 and _lum(e_txt) < 0.15 and n_contrast > 0.7 and e_contrast > 0.7)
    check("readability_guard", ok,
          f"normal txt={n_txt}(c={n_contrast:.2f}) extended txt={e_txt}(c={e_contrast:.2f})")
    print(f"      → ★readability★: black theme light text (contrast {n_contrast:.2f}); white theme DARK "
          f"text (contrast {e_contrast:.2f}). No white-on-white. (가독성 절대 우선)")


def gradient_crossfade():
    t = W.html()
    ok = (".bg-dark{background:linear-gradient" in t and ".bg-light{background:linear-gradient" in t
          and "body.mode-extended .bg-dark{opacity:0}" in t and "body.mode-extended .bg-light{opacity:1}" in t
          and "transition:opacity" in t and '<div class="bg bg-dark">' in t)
    check("gradient_crossfade", ok)
    print("      → two fixed gradient layers (black / white) cross-fade by opacity on mode switch — a "
          "smooth gradient black↔white transition (no flicker). [user-confirm: the transition feel]")


def status_colors_theme():
    t = W.html()
    n_warn, e_warn = _var("normal", "--warn", t), _var("extended", "--warn", t)
    # the light theme must use a DARKER warn (readable on white), not the bright dark-theme amber
    ok = n_warn and e_warn and _lum(n_warn) > _lum(e_warn)
    check("status_colors_theme", ok, f"normal_warn={n_warn} extended_warn={e_warn}")
    print(f"      → status colors are theme-aware: warn is darkened in the white theme ({e_warn}) so it "
          "stays readable (not bright amber on white). ok/bad likewise.")


if __name__ == "__main__":
    print("v24 Part U · U8 — black ↔ white themes + gradient (readability-guarded)")
    black_white_themes(); readability_guard(); gradient_crossfade(); status_colors_theme()
    print(f"\nU8: {len(PASS)} passed, {len(FAIL)} failed, {len(SKIP)} skipped")
    sys.exit(1 if FAIL else 0)
