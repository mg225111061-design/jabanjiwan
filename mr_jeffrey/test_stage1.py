"""STAGE 1 tests — LLM adapters. Run: python3 test_stage1.py  (no pytest needed)."""
import math
from llm_adapters import get_adapter, ScriptedLLM, AnthropicAdapter, OpenAIAdapter
from mr_jeffrey import MrJeffrey, Task, extract_function

PASS, FAIL = [], []
def check(name, cond, detail=""):
    (PASS if cond else FAIL).append(name)
    print(f"  [{'PASS' if cond else 'FAIL'}] {name}" + (f" — {detail}" if detail and not cond else ""))


def adapter_extracts_code():
    # an adapter's text output (```python block) is extractable by mr_jeffrey.extract_function.
    reply = "Here you go:\n```python\ndef sq(x):\n    return x*x\n```\nDone."
    llm = ScriptedLLM([reply])
    fn, _ = extract_function(llm(""), "sq")
    check("adapter_extracts_code", fn is not None and fn(5) == 25, f"got {fn}")


def adapter_fallback_works():
    # no API key present ⇒ get_adapter('auto') must fall back to ScriptedLLM, not crash.
    a = get_adapter("auto", scripted_attempts=["```python\ndef f(n):\n    return n\n```"], verbose=False)
    check("adapter_fallback_works", isinstance(a, ScriptedLLM), f"got {type(a).__name__}")
    # and the real adapters raise cleanly without a key (so the factory can fall back).
    try:
        AnthropicAdapter(api_key=None)
        no_raise = True
    except Exception:
        no_raise = False
    check("anthropic_raises_without_key", not no_raise)


def real_loop_with_simulation_passes():
    # the full Mr. loop runs end-to-end against a scripted adapter (buggy → fix → VERIFIED).
    llm = get_adapter("scripted", scripted_attempts=[
        "```python\ndef factorial(n):\n    r=1\n    for i in range(1,n):\n        r*=i\n    return r\n```",  # buggy
        "```python\ndef factorial(n):\n    r=1\n    for i in range(1,n+1):\n        r*=i\n    return r\n```",  # fixed
    ], verbose=False)
    task = Task("factorial", "Write factorial(n) for n>=0.", "factorial", ["nonneg_int"], reference=math.factorial)
    res = MrJeffrey(llm).solve(task)
    check("real_loop_with_simulation_passes", res.status == "VERIFIED" and len(res.rounds) == 2, res.summary())
    # and a PROVEN path (closed form) drives through the same loop.
    llm2 = get_adapter("scripted", scripted_attempts=["```python\ndef s(n):\n    return n*(n+1)//2\n```"], verbose=False)
    t2 = Task("s", "sum 1..n", "s", ["nonneg_int"], reference=lambda n: n*(n+1)//2,
              exact_claim=("sum", "n*(n+1)/2", "k", "k", "1", "n", ["n", "k"]))
    res2 = MrJeffrey(llm2).solve(t2)
    check("real_loop_proven_path", res2.status == "PROVEN")


if __name__ == "__main__":
    print("STAGE 1 — LLM adapters")
    adapter_extracts_code()
    adapter_fallback_works()
    real_loop_with_simulation_passes()
    print(f"\nStage 1: {len(PASS)} passed, {len(FAIL)} failed")
    import sys
    sys.exit(1 if FAIL else 0)
