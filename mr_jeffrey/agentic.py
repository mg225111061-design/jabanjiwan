"""
HARAN v22 Part S — agentic coding pipeline (write → verify → fix → optimize).
=============================================================================
The product: Claude *writes* code; HARAN *verifies* it against its spec and, when wrong, hands back a
concrete counterexample to fix. This module grows stage by stage:

  • S2  write_verify(request, key)        — Claude proposes → HARAN verifies (VERIFIED / counterexample)
  • S3  write_verify_fix(...)             — the loop: feed the counterexample back until proven   (the heart)
  • S4  + fold optimization              — closed-form speedup on the proven result
  • S5  modes (normal / extended)        — reuse v21
  • S6  Type A (spec-embedded)           — verify against the embedded spec
  • S7  agentic_code(request, mode, key) — the integrated entry point + honest measurement

HONESTY (v22 bar):
  ★ Verification is **against the spec** (`ensures`), NOT against intent. "VERIFIED" means the code meets
    the stated spec — it does NOT mean Claude guessed what you meant. Intent gaps are reported, not hidden.
  ★ The model is swappable; the key is level-1 (entered per call, stored nowhere — see claude_agent).
  ★ Mock provenance is always labeled (source='mock-sim'); a mock is never reported as 'live'.
"""
from __future__ import annotations

from dataclasses import dataclass, field
from typing import Callable, List, Optional

import ai_loop
import claude_agent as CA


@dataclass
class WriteVerifyResult:
    request: str
    code: str
    source: str               # "mock-sim" | "claude-live"  (honest provenance)
    live: bool
    status: str               # VERIFIED | FAILED | UNKNOWN | PARSE_ERROR
    ok: bool                  # True iff VERIFIED against the spec
    counterexample: Optional[dict]
    feedback: str             # minimal, targeted feedback (concrete failing input + mismatch)


def write_verify(request: str, api_key: Optional[str] = None, *,
                 model: str = CA.DEFAULT_MODEL, system: Optional[str] = None,
                 mock_response: Optional[str] = None) -> WriteVerifyResult:
    """S2: Claude proposes code for `request`; HARAN verifies it against its `ensures` spec.

    Returns VERIFIED, or a concrete counterexample (the smallest failing input + the impl/spec
    mismatch) — which S3 will feed back to drive a fix. With no `api_key` this runs the labeled mock
    (deterministic), so the whole write→verify path is testable with zero network/secrets."""
    gen = CA.claude_generate(request, api_key, model=model, system=system, mock_response=mock_response)
    v = ai_loop.verify_haran(gen.text)
    return WriteVerifyResult(
        request=request, code=gen.text, source=gen.source, live=gen.live,
        status=v.status, ok=v.ok, counterexample=v.counterexample,
        feedback=ai_loop.minimal_feedback(v) if not v.ok else "verified against spec (명세 대비)",
    )


# ---------------------------------------------------------------------------------------------------
# S3 — write → verify → FIX (the heart). Claude writes; HARAN rejects with a concrete counterexample;
# that counterexample is fed back into the next prompt; repeat until PROVEN or the budget runs out.
# The loop mechanics (fix-prompt + minimal counterexample) are reused from v7 (ai_loop.write_verify_fix);
# v22 wires Claude (level-1 key) as the model and adds honest provenance.
#
# ★ The loop and HARAN's counterexamples are REAL. In mock mode the *model text* is a scripted
#   SIMULATION (wrong→fixed) — never a fake 'live'. A weak model just loops more; HARAN never
#   rubber-stamps, so there is NO false convergence. ★
# ---------------------------------------------------------------------------------------------------

@dataclass
class WVFResult:
    converged: bool
    iters: int
    source: str               # "mock-sim" | "claude-live"
    final_code: str
    final_status: str
    trace: List[ai_loop.LoopStep] = field(default_factory=list)


def _claude_model_fn(api_key: Optional[str], model: str,
                     mock_sequence: Optional[List[str]]) -> Callable[[str], str]:
    """A `prompt -> code` callable backed by Claude. Live: each call is a real Claude turn (so the fix
    prompt, which carries the counterexample, actually drives a fix). Mock: a deterministic scripted
    sequence (wrong → fixed) advancing one step per call — an honest SIMULATION of the model's turns."""
    state = {"i": 0}
    seq = mock_sequence or [CA._MOCK_HARAN]

    def model_fn(prompt: str) -> str:
        if api_key:
            return CA.claude_generate(prompt, api_key, model=model).text
        out = seq[min(state["i"], len(seq) - 1)]
        state["i"] += 1
        return out

    return model_fn


def write_verify_fix(request: str, api_key: Optional[str] = None, *,
                     model: str = CA.DEFAULT_MODEL, mock_sequence: Optional[List[str]] = None,
                     max_iters: int = 3, verbose: bool = False) -> WVFResult:
    """S3: drive Claude→HARAN→fix until the code is PROVEN against its spec (or budget exhausted).

    Returns convergence + the full trace (each iteration's code, verdict, and the counterexample that
    was fed back). With no key, `mock_sequence` scripts the model's turns (e.g. [WRONG, GOOD]); the
    loop and counterexamples are real regardless."""
    fn = _claude_model_fn(api_key, model, mock_sequence)
    loop = ai_loop.write_verify_fix(request, fn, fn, max_iters=max_iters, verbose=verbose)
    last = loop.trace[-1] if loop.trace else None
    return WVFResult(
        converged=loop.converged, iters=loop.iters,
        source="claude-live" if api_key else "mock-sim",
        final_code=last.code if last else "", final_status=last.verdict.status if last else "NONE",
        trace=loop.trace,
    )
