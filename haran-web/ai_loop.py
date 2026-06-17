"""
STAGE X4 — AI write→verify→fix loop (Qwen3-32B live, else honest simulation).
============================================================================
A model (Qwen3-32B, thinking OFF) writes HARAN → Mr.Jeffrey verifies (proven / tested / closure /
error-bound) → on failure returns a CONCRETE counterexample (어디·왜·반례) → the model (thinking ON)
reasons over the counterexample and fixes → re-verify. A weak model just loops more; Mr hands it a
fresh counterexample each round, so it converges. Mr's verification IS the product; the model is swappable.

★ The loop and the counterexamples are REAL. If no local Qwen3 endpoint answers, the model text is a
labeled SIMULATION (ScriptedLLM) — never a fake "live" claim. ★
"""
from __future__ import annotations

from dataclasses import dataclass, field
from typing import Callable, List, Optional

import mr_haran
from haran_parser import parse


@dataclass
class LoopVerdict:
    ok: bool
    status: str                       # VERIFIED | FAILED | UNKNOWN | PARSE_ERROR
    feedback: str
    counterexample: Optional[dict] = None


def verify_haran(code: str) -> LoopVerdict:
    """Run a single HARAN function through Mr.Jeffrey; return the loop verdict + any counterexample."""
    prog = parse(code)
    if prog.errors:
        e = prog.errors[0]
        return LoopVerdict(False, "PARSE_ERROR", f"parse error at {e.line}:{e.col}: {e.message}")
    reports = mr_haran.verify_program(code)
    rep = reports[0]
    if rep.verdict == "VERIFIED":
        return LoopVerdict(True, "VERIFIED", "all obligations satisfied (명세 대비)")
    failing = next((o for o in rep.obligations if o.status == "FAIL"), None)
    if failing:
        return LoopVerdict(False, "FAILED", f"{failing.kind}: {failing.why}", failing.counterexample)
    unk = next((o for o in rep.obligations if o.status == "UNKNOWN"), None)
    return LoopVerdict(False, rep.verdict, (unk.why if unk else "not verified"))


@dataclass
class LoopStep:
    iteration: int
    mode: str
    prompt: str
    code: str
    verdict: LoopVerdict


@dataclass
class LoopResult:
    converged: bool
    iters: int
    trace: List[LoopStep] = field(default_factory=list)


def minimal_feedback(v: LoopVerdict) -> str:
    """Structural MINIMAL counterexample (TraceCoder-style): strip to the single smallest failing
    input + the exact mismatch, removing noise so the fix is targeted."""
    cx = v.counterexample or {}
    inputs = cx.get("inputs", {})
    impl = cx.get("impl_value", cx.get("result"))
    spec = cx.get("spec_value")
    if inputs and impl is not None and spec is not None:
        return f"minimal counterexample: at {inputs}, your output = {impl} but the spec requires {spec}."
    if inputs:
        return f"minimal counterexample: fails at {inputs}."
    return v.feedback


def _fix_prompt(task: str, code: str, v: LoopVerdict) -> str:
    return (f"{task}\n\nYour previous attempt was REJECTED:\n{code}\n\n"
            f"Mr.Jeffrey [{v.status}]: {minimal_feedback(v)}\n"
            f"Fix exactly this one issue and return the corrected HARAN function only.")


def write_verify_fix(task: str, writer: Callable[[str], str], verifier: Callable[[str], str],
                     max_iters=3, verbose=True) -> LoopResult:
    """Drive the loop. iter 0 uses the writer (think-off); later iters use the verifier (think-on)."""
    prompt = task
    res = LoopResult(False, 0)
    for it in range(max_iters):
        model = writer if it == 0 else verifier
        mode = "write(think-off)" if it == 0 else "fix(think-on)"
        code = model(prompt).strip()
        v = verify_haran(code)
        res.trace.append(LoopStep(it + 1, mode, prompt, code, v))
        if verbose:
            cx = f"  cx={v.counterexample}" if v.counterexample else ""
            print(f"      iter {it + 1} [{mode}] → {v.status}{cx}")
        if v.ok:
            res.converged, res.iters = True, it + 1
            return res
        prompt = _fix_prompt(task, code, v)
    res.iters = max_iters
    return res


# --- P3: grammar guidance + parse-failure measurement (Claude API has no GBNF; we guide + measure) ---
HARAN_GRAMMAR_HINT = (
    "Output ONLY a HARAN function: `fn name(p: T) -> R ensures <expr> effects pure { <body> }`. "
    "Bodies use match / fold k in lo..hi { e } / let / arithmetic. No prose, no markdown fences."
)

def measure_parse_failures(codes):
    from haran_parser import parse
    fails = sum(1 for c in codes if parse(c).errors)
    return {"n": len(codes), "parse_failures": fails,
            "rate": (fails / len(codes)) if codes else 0.0}
