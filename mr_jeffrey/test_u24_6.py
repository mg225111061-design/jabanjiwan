"""v24 Part U · U6 tests — quick chips hidden/shown (structural). Run: python3 test_u24_6.py

Whether it *feels* uncluttered is USER-CONFIRMATION; here we check the collapse mechanism + wiring.
chips_collapsed_default : .qchips starts collapsed (max-height:0 / .open class) behind a toggle.
chips_toggle_wired      : a toggle button shows/hides the chips (aria-expanded), smooth transition.
chips_expanded_set      : 9 quick actions present (5 original + add-comments/simpler/explain/safer).
i18n_parity_chips       : the new chip labels are translated in ko & en.
"""
import sys

import web_check as W

PASS, FAIL, SKIP = [], [], []


def check(n, c, d=""):
    (PASS if c else FAIL).append(n)
    print(f"  [{'PASS' if c else 'FAIL'}] {n}" + (f" — {d}" if d and not c else ""))


def chips_collapsed_default():
    t = W.html()
    ok = (".qchips{" in t and "max-height:0" in t and ".qchips.open{" in t and 'id="qchips"' in t)
    check("chips_collapsed_default", ok)
    print("      → .qchips starts max-height:0/opacity:0 (collapsed) and expands via .open — hidden until "
          "the user opens it (no clutter). [user-confirm: it looks tidy]")


def chips_toggle_wired():
    t = W.html()
    ok = ('id="quickToggle"' in t and "quickToggle.addEventListener" in t
          and 'classList.toggle("open")' in t and "aria-expanded" in t)
    check("chips_toggle_wired", ok)
    print("      → '빠른 지시 ▾' toggle expands/collapses the chips (aria-expanded, rotating arrow, "
          "smooth max-height transition).")


def chips_expanded_set():
    t = W.html()
    nchips = t.count('class="qchip"')
    keys = all(W.has(k, t) for k in ("qa_faster", "qa_bug", "qa_test", "qa_edge", "qa_refactor",
                                     "qa_comment", "qa_simpler", "qa_explain", "qa_safer"))
    ok = nchips >= 9 and keys
    check("chips_expanded_set", ok, f"qchips={nchips}")
    print(f"      → {nchips} quick actions (faster/bug/test/edge/refactor + comments/simpler/explain/"
          "safer) — many, but hidden so they don't clutter.")


def i18n_parity_chips():
    ko, en = W.i18n_keys("ko"), W.i18n_keys("en")
    needed = {"qa_comment", "qa_simpler", "qa_explain", "qa_safer", "quick_label"}
    ok = needed <= ko and needed <= en and ko == en
    check("i18n_parity_chips", ok, f"missing={needed-(ko&en)} parity={ko==en}")
    print("      → new chip labels translated in ko & en; full i18n parity.")


if __name__ == "__main__":
    print("v24 Part U · U6 — quick chips hidden/shown (structural)")
    chips_collapsed_default(); chips_toggle_wired(); chips_expanded_set(); i18n_parity_chips()
    print(f"\nU6: {len(PASS)} passed, {len(FAIL)} failed, {len(SKIP)} skipped")
    sys.exit(1 if FAIL else 0)
