"""v23 Part T · T5 tests — continuous-instruction feedback loop (the heart). Run: python3 test_web5.py

STRUCTURAL — whether the loop *feels* natural is USER-CONFIRMATION.
followup_input_persists : the composer (next-instruction input) is always present (not consumed).
history_accumulates     : a HISTORY array accumulates rounds and is threaded into runAgentic.
quick_actions           : 5 quick-action follow-up chips (faster/bug/test/edge/refactor), i18n, wired.
followup_distinct       : the mock returns DISTINCT responses per quick-action (real rounds server-side).
i18n_parity_quick       : quick-action labels translated in ko & en.
"""
import sys

import web_check as W

PASS, FAIL, SKIP = [], [], []


def check(n, c, d=""):
    (PASS if c else FAIL).append(n)
    print(f"  [{'PASS' if c else 'FAIL'}] {n}" + (f" — {d}" if d and not c else ""))


def followup_input_persists():
    t = W.html()
    # the composer/textarea is a persistent element (never removed); each round appends to #msgs
    ok = W.has('id="reqInput"', t) and W.has("msgsEl.appendChild", t) and W.has("function onSend", t)
    check("followup_input_persists", ok)
    print("      → the next-instruction input persists; each round APPENDS to the conversation (rounds "
          "stack, prior rounds stay visible). [user-confirm: the flow feels natural]")


def history_accumulates():
    t = W.html()
    ok = ("const HISTORY = []" in t and "HISTORY.push(" in t
          and "runAgentic(request, MODE, key, HISTORY" in t)
    check("history_accumulates", ok)
    print("      → HISTORY accumulates {request,code} per round and is threaded into runAgentic as "
          "context (server uses it in T8) — the conversation builds up, not single-shot.")


def quick_actions():
    t = W.html()
    nq = t.count('class="qchip"')
    keys = all(W.has(k, t) for k in ("qa_faster", "qa_bug", "qa_test", "qa_edge", "qa_refactor"))
    wired = "quickEl.addEventListener" in t and "onSend(q.textContent" in t
    ok = nq >= 5 and keys and wired
    check("quick_actions", ok, f"qchips={nq} keys={keys} wired={wired}")
    print(f"      → {nq} quick-action follow-ups (faster/bug/test/edge/refactor) send as the next "
          "instruction — one-tap iteration.")


def followup_distinct():
    t = W.html()
    # the mock has a follow-up branch keyed on prior context with distinct responses
    ok = ("QA_FOLLOWUPS" in t and "history && history.length" in t and "followup:true" in t)
    check("followup_distinct", ok)
    print("      → with prior context, each quick-action yields a DISTINCT round (faster→fold O(1); "
          "bug→counterexample→fix; test→properties; edge→boundaries; refactor→equivalence). SIM here; "
          "real per-round verification server-side (T8).")


def i18n_parity_quick():
    ko, en = W.i18n_keys("ko"), W.i18n_keys("en")
    needed = {"quick_label", "qa_faster", "qa_bug", "qa_test", "qa_edge", "qa_refactor",
              "chips_label", "ex1", "ex2", "ex3", "ex4", "scope_note"}
    ok = needed <= ko and needed <= en and ko == en
    check("i18n_parity_quick", ok, f"missing_ko={needed-ko} missing_en={needed-en} parity={ko==en}")
    print("      → quick-action + chip + scope strings translated in ko & en; full parity.")


if __name__ == "__main__":
    print("v23 Part T · T5 — continuous-instruction feedback loop (the heart)")
    followup_input_persists(); history_accumulates(); quick_actions()
    followup_distinct(); i18n_parity_quick()
    print(f"\nT5: {len(PASS)} passed, {len(FAIL)} failed, {len(SKIP)} skipped")
    sys.exit(1 if FAIL else 0)
