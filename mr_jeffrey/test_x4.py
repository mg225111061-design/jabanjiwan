"""
STAGE X4 tests — AI write→verify→fix loop (Qwen3-32B live or honest sim).  Run: python3 test_x4.py
"""
from llm_adapters import get_writer_verifier, Qwen3Adapter
from ai_loop import write_verify_fix, verify_haran

PASS, FAIL = [], []
def check(name, cond, detail=""):
    (PASS if cond else FAIL).append(name)
    print(f"  [{'PASS' if cond else 'FAIL'}] {name}" + (f" — {detail}" if detail and not cond else ""))


TASK = ("Write a HARAN function sum_squares(n: Nat) -> Nat with "
        "ensures result = n*(n+1)*(2*n+1)/6, effects pure.")

WRONG = """\
fn sum_squares(n: Nat) -> Nat
  ensures result = n*(n+1)*(2*n+1)/6
  effects pure
{ fold k in 1..n { k } }
"""
WRONG2 = """\
fn sum_squares(n: Nat) -> Nat
  ensures result = n*(n+1)*(2*n+1)/6
  effects pure
{ fold k in 1..n { k + k } }
"""
CORRECT = """\
fn sum_squares(n: Nat) -> Nat
  ensures result = n*(n+1)*(2*n+1)/6
  effects pure
{ fold k in 1..n { k*k } }
"""


def qwen3_adapter_or_sim():
    w, v, mode = get_writer_verifier(prefer="qwen3", scripted_writer=[WRONG], scripted_verifier=[CORRECT])
    ok = mode in ("live", "sim") and callable(w) and callable(v)
    # the adapter class exists and supports thinking on/off
    a_off = Qwen3Adapter(thinking=False)
    a_on = Qwen3Adapter(thinking=True)
    ok = ok and a_off.thinking is False and a_on.thinking is True and a_off.name == "qwen3"
    check("qwen3_adapter_or_sim", ok, f"mode={mode}")
    print(f"      → backend mode = {mode.upper()} (Qwen3-32B adapter present; thinking on/off slots ready)")


def write_verify_fix_loop():
    w, v, mode = get_writer_verifier(prefer="qwen3", scripted_writer=[WRONG], scripted_verifier=[CORRECT],
                                     verbose=False)
    res = write_verify_fix(TASK, w, v, verbose=True)
    ok = res.converged and res.iters == 2
    check("write_verify_fix_loop", ok, f"converged={res.converged} iters={res.iters}")
    print(f"      → converged in {res.iters} iterations (mode={mode})")


def ai_fixes_from_counterexample():
    w, v, _ = get_writer_verifier(prefer="qwen3", scripted_writer=[WRONG], scripted_verifier=[CORRECT],
                                  verbose=False)
    res = write_verify_fix(TASK, w, v, verbose=False)
    step1, step2 = res.trace[0], res.trace[1]
    # iter1 failed WITH a real counterexample; that cx was fed into iter2's prompt; iter2 verified
    cx = step1.verdict.counterexample
    # v7: fix prompt now carries a MINIMAL counterexample (smallest failing input + mismatch)
    ok = (step1.verdict.status == "FAILED" and cx is not None
          and "counterexample" in step2.prompt.lower() and str(cx.get("inputs")) in step2.prompt
          and step2.verdict.status == "VERIFIED")
    check("ai_fixes_from_counterexample", ok,
          f"iter1={step1.verdict.status} cx={cx}; cx_in_fix_prompt={'counterexample' in step2.prompt.lower()}; "
          f"iter2={step2.verdict.status}")
    print(f"      → iter1 FAILED with real cx {cx}; cx fed into fix prompt; iter2 VERIFIED")


def weak_model_converges_in_more_steps():
    # a weaker writer is wrong twice; Mr hands a fresh counterexample each round → still converges
    w, v, _ = get_writer_verifier(prefer="qwen3", scripted_writer=[WRONG],
                                  scripted_verifier=[WRONG2, CORRECT], verbose=False)
    res = write_verify_fix(TASK, w, v, verbose=False)
    ok = res.converged and res.iters == 3
    check("weak_model_converges_in_more_steps", ok, f"iters={res.iters}")
    print(f"      → weak model converged in {res.iters} iters (vs 2 for the stronger one) — "
          f"Mr's per-round counterexamples drive convergence")


if __name__ == "__main__":
    print("STAGE X4 — AI write→verify→fix loop")
    qwen3_adapter_or_sim()
    write_verify_fix_loop()
    ai_fixes_from_counterexample()
    weak_model_converges_in_more_steps()
    print(f"\nStage X4: {len(PASS)} passed, {len(FAIL)} failed")
    import sys
    sys.exit(1 if FAIL else 0)
