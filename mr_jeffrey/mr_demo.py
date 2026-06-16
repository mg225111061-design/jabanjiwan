"""
DEMO — Mr. Jeffrey's self-correcting loop, end to end.
A deterministic 'fake LLM' plays the role of the AI: it first writes BUGGY code,
then — after Mr. hands it the exact breaking input — writes the FIX. This proves
the loop works without needing an API key. Swap in a real Claude/GPT call and the
same loop drives a real model.
"""
import math
from mr_jeffrey import MrJeffrey, Task


# ---------- a scripted "LLM" : returns the next attempt each time it's called ----------
class ScriptedLLM:
    def __init__(self, attempts): self.attempts = attempts; self.i = 0
    def __call__(self, prompt):
        out = self.attempts[min(self.i, len(self.attempts)-1)]
        self.i += 1
        return out


print("="*70)
print("DEMO 1 — factorial. AI writes off-by-one bug, Mr. catches it, AI fixes.")
print("="*70)
llm1 = ScriptedLLM([
    # attempt 1: BUGGY (range(1,n))
    "```python\ndef factorial(n):\n    r = 1\n    for i in range(1, n):\n        r *= i\n    return r\n```",
    # attempt 2: FIXED (range(1,n+1))
    "```python\ndef factorial(n):\n    r = 1\n    for i in range(1, n+1):\n        r *= i\n    return r\n```",
])
task1 = Task(name="factorial",
             prompt="Write a python function factorial(n) for n>=0.",
             func_name="factorial", arg_kinds=["nonneg_int"],
             reference=math.factorial)
res1 = MrJeffrey(llm1).solve(task1)
print(res1.summary())
print()

print("="*70)
print("DEMO 2 — sum of list. AI writes empty-list crash, Mr. catches, AI fixes.")
print("="*70)
llm2 = ScriptedLLM([
    "```python\ndef mysum(xs):\n    total = xs[0]\n    for x in xs[1:]:\n        total += x\n    return total\n```",
    "```python\ndef mysum(xs):\n    total = 0\n    for x in xs:\n        total += x\n    return total\n```",
])
task2 = Task(name="mysum", prompt="Write mysum(xs) returning the sum of a list.",
             func_name="mysum", arg_kinds=["list_int"], reference=sum)
res2 = MrJeffrey(llm2).solve(task2)
print(res2.summary())
print()

print("="*70)
print("DEMO 3 — THE JEFF EDGE: AI claims a closed form. Mr. PROVES it (all n).")
print("="*70)
llm3 = ScriptedLLM([
    # AI claims sum 1..n via a closed form n*(n+1)/2
    "```python\ndef sum_to_n(n):\n    return n*(n+1)//2\n```",
])
task3 = Task(name="sum_to_n",
             prompt="Write sum_to_n(n) = 1+2+...+n, as a closed form.",
             func_name="sum_to_n", arg_kinds=["nonneg_int"],
             reference=lambda n: n*(n+1)//2,
             exact_claim=("sum", "n*(n+1)/2", "k", "k", "1", "n", ["n","k"]))
res3 = MrJeffrey(llm3).solve(task3)
print(res3.summary())
print("   ^ note: not 'tested' — PROVEN for every n. That's the differentiator.")
print()

print("="*70)
print("DEMO 4 — AI claims a WRONG closed form. Mr. PROVES it wrong, asks fix.")
print("="*70)
llm4 = ScriptedLLM([
    "```python\ndef sum_to_n(n):\n    return n*n//2\n```",          # WRONG
    "```python\ndef sum_to_n(n):\n    return n*(n+1)//2\n```",      # FIXED
])
task4 = Task(name="sum_to_n",
             prompt="Write sum_to_n(n) = 1+2+...+n as a closed form.",
             func_name="sum_to_n", arg_kinds=["nonneg_int"],
             reference=lambda n: n*(n+1)//2,
             exact_claim=("sum", "n*n/2", "k", "k", "1", "n", ["n","k"]))
# (round 1 uses the wrong claim; if refuted, Mr. re-checks the candidate's own code)
res4 = MrJeffrey(llm4).solve(task4)
print(res4.summary())
print()

print("="*70)
print("Mr. Jeffrey works: writes → verifies (proof or fuzz) → feeds back → fixes.")
print("Math claims get PROVEN. Bugs get caught with the exact breaking input.")
print("="*70)
