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

from dataclasses import dataclass
from typing import Optional

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
