"""v23 Part T · T8 tests — follow-up (conversation accumulation) + incremental re-verify. Run: python3 test_web8.py

history_context        : agentic_code threads prior code+instructions into the task (with a key, Claude uses it).
history_threaded_backend: handle_generate accumulates history across rounds (history_len reflects context).
incremental_reverify   : a follow-up edit re-verifies ONLY the changed function (v21 Merkle), measured speedup.
unchanged_cached       : unchanged functions are served from cache (big speedup), not re-proved.
followup_multiround    : two backend rounds work (round 2 carries round 1 as context).
"""
import sys

import agentic as AG
import server as S

PASS, FAIL, SKIP = [], [], []


def check(n, c, d=""):
    (PASS if c else FAIL).append(n)
    print(f"  [{'PASS' if c else 'FAIL'}] {n}" + (f" — {d}" if d and not c else ""))


def history_context():
    task = AG._with_history("make it faster", [("sum 1..n", "CODE_BODY_X")])
    ok = "CODE_BODY_X" in task and "sum 1..n" in task and "make it faster" in task
    check("history_context", ok, f"task={task!r}")
    print("      → prior (request→code) is woven into the task ('# earlier:' … '# now:') — with a real "
          "key Claude reflects the conversation; verification stays spec-relative.")


def history_threaded_backend():
    hist = [{"request": "sum 1..n", "code": "fn s(n: Nat)->Nat\n  ensures result=n*(n+1)/2\n{ fold k in 1..n { k } }"}]
    r = S.handle_generate({"prompt": "add a test", "mode": "extended", "history": hist})
    ok = (not r.get("error")) and r.get("history_len") == 1
    check("history_threaded_backend", ok, f"history_len={r.get('history_len')}")
    print(f"      → backend threads conversation history (history_len={r.get('history_len')}) into the "
          "pipeline — the server side of T5's feedback loop.")


def incremental_reverify():
    src = "\n\n".join(f"fn f{i}(n: Nat)->Nat\n  ensures result = n*(n+1)/2\n{{ fold k in 1..n {{ k }} }}"
                      for i in range(6))
    edited = src.replace("fold k in 1..n { k }", "fold k in 1..n { k + 0 }", 1)   # change ONLY f0
    inc = S.reverify_incremental(src, edited)
    only_changed = inc["reverified"] == ["f0"]
    faster = inc["speedup_one_edit"] > 1.0
    ok = only_changed and faster
    check("incremental_reverify", ok, f"reverified={inc['reverified']} speedup={inc['speedup_one_edit']}")
    print(f"      → follow-up edit re-verifies ONLY {inc['reverified']} (the changed fn), "
          f"{inc['warm_one_edit_ms']}ms vs cold {inc['cold_ms']}ms → ×{inc['speedup_one_edit']} (measured).")


def unchanged_cached():
    src = "\n\n".join(f"fn f{i}(n: Nat)->Nat\n  ensures result = n*(n+1)/2\n{{ fold k in 1..n {{ k }} }}"
                      for i in range(6))
    inc = S.reverify_incremental(src, src)        # no change → everything cached
    ok = inc["reverified"] == [] and inc["speedup_unchanged"] > 1.0
    check("unchanged_cached", ok, f"reverified={inc['reverified']} speedup_unchanged={inc['speedup_unchanged']}")
    print(f"      → no change → 0 functions re-proved (all cached) → ×{inc['speedup_unchanged']} (measured). "
          "Follow-up rounds that don't touch a function don't re-verify it.")


def followup_multiround():
    r1 = S.handle_generate({"prompt": "sum 1..n", "mode": "normal"})
    hist = [{"request": "sum 1..n", "code": r1["code"]}]
    r2 = S.handle_generate({"prompt": "make it faster", "mode": "extended", "history": hist})
    ok = (not r1.get("error")) and (not r2.get("error")) and r2.get("history_len") == 1
    check("followup_multiround", ok, f"r1={r1.get('status')} r2={r2.get('status')} hist={r2.get('history_len')}")
    print("      → round 1 → round 2 (carrying round 1 as context) both succeed — multi-round backend.")


if __name__ == "__main__":
    print("v23 Part T · T8 — follow-up (history) + incremental re-verify")
    history_context(); history_threaded_backend(); incremental_reverify(); unchanged_cached(); followup_multiround()
    print(f"\nT8: {len(PASS)} passed, {len(FAIL)} failed, {len(SKIP)} skipped")
    sys.exit(1 if FAIL else 0)
