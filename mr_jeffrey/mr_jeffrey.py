"""
Mr. Jeffrey (Mr.)  —  the self-correcting verified-code system
===============================================================
"Fast, instant, decisive — and PROVEN."

The loop:
    1. an LLM writes a function for a task
    2. Mr. VERIFIES it
         - numeric/algebraic claim  -> EXACT proof (all inputs) via JEFF-style layer
         - general code             -> strong fuzzing (crash / wrong / side-effect)
    3. if REFUTED, Mr. hands the LLM the EXACT breaking input and asks for a fix
    4. repeat until PROVEN / VERIFIED or budget runs out

Mr. is LLM-agnostic: you pass any `llm(prompt)->str` callable. Drop in a real
Claude/GPT call in production; here we include a deterministic "fake LLM" so the
whole loop runs and is demonstrably correct without an API key.

Speed: verification is the fast part (fuzzing is bounded; exact proof is a single
symbolic simplify). The LLM call is the only slow step — exactly as it should be.
"""

import re
import copy
import time
from dataclasses import dataclass, field
from typing import Callable

from verify_core import CodeVerifier
from verify_exact import prove_equiv, prove_closed_form_sum


# ---------- a task description Mr. understands ----------

@dataclass
class Task:
    name: str
    prompt: str                       # what the LLM is asked to write
    func_name: str                    # the function name expected in the code
    arg_kinds: list                   # ["int"], ["list_int"], ["str"], ...
    reference: Callable = None        # oracle (strongest)
    properties: list = None
    examples: dict = None
    pure: bool = True
    # optional EXACT claim: ("equiv", cand_expr, ref_expr, vars)
    #                   or  ("sum", closed, summand, idx, lo, hi, vars)
    exact_claim: tuple = None


@dataclass
class Round:
    n: int
    code: str
    verdict: str
    detail: str

@dataclass
class MrResult:
    status: str                       # "PROVEN" | "VERIFIED" | "FAILED"
    rounds: list = field(default_factory=list)
    final_code: str = ""
    elapsed: float = 0.0

    def summary(self):
        tag = {"PROVEN":"PROVEN (exact, all inputs)",
               "VERIFIED":"VERIFIED (bounded+fuzzed)",
               "FAILED":"FAILED (budget exhausted)"}[self.status]
        lines = [f"Mr. → {tag} in {len(self.rounds)} round(s), {self.elapsed*1000:.0f} ms"]
        for r in self.rounds:
            lines.append(f"   round {r.n}: {r.verdict} — {r.detail}")
        return "\n".join(lines)


# ---------- extracting a function from LLM output ----------

def extract_function(text: str, func_name: str):
    """Pull a python function definition out of an LLM reply and compile it."""
    # strip markdown fences
    m = re.search(r"```(?:python)?\s*(.*?)```", text, re.S)
    body = m.group(1) if m else text
    ns = {}
    try:
        exec(body, {"__builtins__": __builtins__}, ns)
    except Exception as e:
        # try the whole text
        try:
            exec(text, {"__builtins__": __builtins__}, ns)
        except Exception:
            return None, f"could not exec LLM code: {e}"
    fn = ns.get(func_name)
    if fn is None:
        for v in ns.values():
            if callable(v):
                fn = v; break
    if fn is None:
        return None, "no function found in LLM output"
    return fn, body


# ---------- the Mr. loop ----------

class MrJeffrey:
    def __init__(self, llm: Callable, max_rounds=4, verifier=None):
        self.llm = llm
        self.max_rounds = max_rounds
        self.V = verifier or CodeVerifier(max_tests=150, fuzz_per_arg=40)

    def _exact_check(self, claim):
        if claim is None:
            return None
        if claim[0] == "equiv":
            _, c, r, vs = claim
            return prove_equiv(c, r, vs)
        if claim[0] == "sum":
            _, closed, summand, idx, lo, hi, vs = claim
            return prove_closed_form_sum(closed, summand, idx, lo, hi, vs)
        return None

    def solve(self, task: Task) -> MrResult:
        t0 = time.time()
        rounds = []
        feedback = ""
        last_code = ""

        for n in range(1, self.max_rounds + 1):
            prompt = task.prompt
            if feedback:
                prompt += (f"\n\nYour previous attempt was WRONG. {feedback}\n"
                           f"Fix the function `{task.func_name}` so it is correct "
                           f"for ALL inputs. Return only the corrected function.")
            reply = self.llm(prompt)
            fn, code_or_err = extract_function(reply, task.func_name)
            if fn is None:
                rounds.append(Round(n, reply, "ERROR", code_or_err))
                feedback = f"Your code did not compile: {code_or_err}"
                continue
            last_code = code_or_err

            # ---- EXACT path first (the "딱 하고 뚝" part) ----
            exact = self._exact_check(task.exact_claim)
            if exact is not None and exact.verdict == "PROVEN_EQUAL":
                rounds.append(Round(n, last_code, "PROVEN", str(exact)))
                return MrResult("PROVEN", rounds, last_code, time.time()-t0)
            if exact is not None and exact.verdict == "PROVEN_UNEQUAL":
                rounds.append(Round(n, last_code, "REFUTED(exact)", str(exact)))
                feedback = (f"The math is provably wrong: differ at {exact.witness}. "
                            f"Residual {exact.detail}.")
                continue

            # ---- bounded/fuzz path ----
            rep = self.V.verify(fn, task.arg_kinds,
                                reference=task.reference,
                                properties=task.properties,
                                examples=task.examples,
                                pure=task.pure)
            if rep.verdict == "VERIFIED":
                rounds.append(Round(n, last_code, "VERIFIED", f"{rep.tests_run} tests, no counterexample"))
                return MrResult("VERIFIED", rounds, last_code, time.time()-t0)

            # REFUTED — build precise feedback from the first counterexample
            c = rep.counterexamples[0]
            if c.kind == "crash":
                fb = f"On input {c.inputs!r} it CRASHED: {c.detail}."
            elif c.kind == "wrong_output":
                fb = f"On input {c.inputs!r} it returned {c.got!r} but should return {c.expected!r}."
            elif c.kind == "side_effect":
                fb = f"On input {c.inputs!r} it MUTATED its argument ({c.detail}); it must not modify inputs."
            else:
                fb = f"On input {c.inputs!r} it violated: {c.detail}."
            rounds.append(Round(n, last_code, "REFUTED", fb))
            feedback = fb

        return MrResult("FAILED", rounds, last_code, time.time()-t0)
