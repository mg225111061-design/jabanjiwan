"""
HARAN v22 Part S · STAGE S1 — Claude API integration (key-security LEVEL 1 + mock mode).
=========================================================================================
This is v22's *front door* to the model: Claude writes code, HARAN verifies it (S2+). The model is
swappable; HARAN's verification is the product. Two things define this module:

  ★ KEY SECURITY — LEVEL 1 ★ (the hard rule for v22):
    The Claude API key is *entered every call, stored NOWHERE* — not in a file, not in env, not in a
    log, not in a cache, not in a global, not on any object. It is received as an argument, used for
    exactly one Claude call, and dropped. This module therefore does **not even import `os`** — it
    structurally cannot read or write the environment or the filesystem. (This is a deliberate, stricter
    departure from `llm_adapters.AnthropicAdapter`, which reads `ANTHROPIC_API_KEY` from env; that is
    fine for a CLI, but the v22 product hands the key in fresh each time and keeps zero copies.)

  ★ MOCK MODE (no key) ★:
    With no key we run a deterministic *labeled simulation* (`live=False, source="mock-sim"`) so the
    whole write→verify→fix loop (S2/S3) is testable with zero network and zero secrets. We NEVER claim
    a mock result is "live".

Real calls use the official Anthropic SDK (lazy-imported, so this module loads without it). Default
model is `claude-opus-4-8` with adaptive thinking; streaming is supported for long generations.
"""
from __future__ import annotations

from dataclasses import dataclass
from typing import Callable, Optional

# NOTE: `os` is intentionally NOT imported (level-1: the key/secrets never touch env or files).

DEFAULT_MODEL = "claude-opus-4-8"   # claude-api skill default; user may override per call.

SYSTEM_PROMPT = (
    "You are a careful engineer writing HARAN, a small verified language. Return ONLY one HARAN "
    "function and nothing else (no prose, no markdown fences). Shape: "
    "`fn name(p: T) -> R ensures <expr> effects pure { <body> }`. Bodies use match / "
    "fold k in lo..hi { e } / let / arithmetic. The function must satisfy its `ensures` spec for ALL "
    "valid inputs. If told a previous attempt failed on a specific input, fix exactly that."
)

# A fixed, parseable HARAN function used as the deterministic mock generation. It both parses and
# verifies (Σ 1..n = n(n+1)/2), so S1's mock exercises the real downstream substrate end-to-end.
_MOCK_HARAN = (
    "fn triangular(n: Nat) -> Nat\n"
    "  ensures result = n*(n+1)/2\n"
    "{ fold k in 1..n { k } }"
)


class ClaudeError(Exception):
    """Raised when a *real* Claude call cannot be made/completed. Never carries the key."""


@dataclass
class GenResult:
    text: str            # the generated code (HARAN), text blocks only
    live: bool           # True only for a real Claude API response; mock is always False
    model: str           # model id used (or requested, for mock)
    source: str          # "claude-live" | "mock-sim"  (honest provenance — never a fake "live")
    usage: Optional[dict] = None   # token usage for a live call, when available


# --- key hygiene -----------------------------------------------------------------------------------
# Module invariant: keys are NEVER stored here. This stays None for the life of the process; the
# key_not_stored test asserts it, and asserts no global/env/attr ever holds a key.
_KEY_STORE = None


def redact_key(s: str) -> str:
    """Mask anything that looks like an API key, for safe diagnostics. (We never log the key, but any
    text that might echo one is run through this first — belt and suspenders.)"""
    if not s:
        return s
    out, i = [], 0
    while i < len(s):
        # Anthropic keys look like `sk-ant-...`; mask from that marker to the next whitespace.
        if s.startswith("sk-ant-", i) or s.startswith("sk-", i):
            j = i
            while j < len(s) and not s[j].isspace():
                j += 1
            out.append("sk-***REDACTED***")
            i = j
        else:
            out.append(s[i])
            i += 1
    return "".join(out)


def _friendly_error(e: Exception) -> str:
    """A user-understandable, KEY-SAFE message for a failed Claude call (U9.3). Never includes the raw
    SDK message verbatim (avoids any chance of leaking the key); maps common cases to clear text."""
    name = type(e).__name__
    low = redact_key(str(e)).lower()
    if "authentication" in name.lower() or "401" in low or "invalid x-api-key" in low or "api key" in low:
        return "API 키가 올바르지 않습니다 (invalid API key)."
    if "ratelimit" in name.lower() or "429" in low or "rate limit" in low:
        return "요청 한도를 초과했습니다 — 잠시 후 다시 시도해 주세요 (rate limited)."
    if "connection" in name.lower() or "timeout" in name.lower() or "network" in low:
        return "네트워크 오류 — 연결을 확인해 주세요 (network error)."
    return f"Claude 호출에 실패했습니다 (call failed: {name})."   # type only — never the raw message


def _mock_generate(prompt: str, model: str, stream: bool,
                   on_delta: Optional[Callable[[str], None]],
                   mock_response: Optional[str]) -> GenResult:
    """Deterministic, network-free, secret-free simulation. Clearly labeled source='mock-sim'."""
    text = mock_response if mock_response is not None else _MOCK_HARAN
    if stream and on_delta:
        # emit in a few chunks so the streaming path is exercised without a network
        step = max(1, len(text) // 4)
        for k in range(0, len(text), step):
            on_delta(text[k:k + step])
    return GenResult(text=text, live=False, model=model, source="mock-sim")


def _live_generate(prompt: str, api_key: str, model: str, system: Optional[str],
                   max_tokens: int, thinking: bool, stream: bool,
                   on_delta: Optional[Callable[[str], None]]) -> GenResult:
    """One real Claude call via the official SDK. The key is used here and dropped on return; it is
    never stored on the client beyond this scope, never logged, never returned."""
    try:
        import anthropic   # lazy: module imports fine without the SDK (mock mode needs none)
    except ImportError as e:
        raise ClaudeError(
            "anthropic SDK not installed — `pip install anthropic` to make real Claude calls "
            "(mock mode needs no SDK and no key)."
        ) from e

    client = anthropic.Anthropic(api_key=api_key)
    kwargs = {
        "model": model,
        "max_tokens": max_tokens,
        "messages": [{"role": "user", "content": prompt}],
        "system": system or SYSTEM_PROMPT,
    }
    if thinking:
        kwargs["thinking"] = {"type": "adaptive"}   # Opus 4.8 / skill default for non-trivial work
    try:
        if stream:
            with client.messages.stream(**kwargs) as s:
                for chunk in s.text_stream:
                    if on_delta:
                        on_delta(chunk)
                final = s.get_final_message()
            blocks, usage = final.content, getattr(final, "usage", None)
        else:
            msg = client.messages.create(**kwargs)
            blocks, usage = msg.content, getattr(msg, "usage", None)
    except Exception as e:   # noqa: BLE001 — normalize SDK errors; never leak the key in the message
        raise ClaudeError(_friendly_error(e)) from None
    finally:
        # drop the client (and with it the key it captured) as soon as the call is done
        del client

    text = "".join(getattr(b, "text", "") for b in blocks
                   if getattr(b, "type", None) == "text").strip()
    if not text:
        raise ClaudeError("Claude returned no text content")
    usage_d = {"input_tokens": getattr(usage, "input_tokens", None),
               "output_tokens": getattr(usage, "output_tokens", None)} if usage else None
    return GenResult(text=text, live=True, model=model, source="claude-live", usage=usage_d)


def claude_generate(prompt: str, api_key: Optional[str] = None, *,
                    model: str = DEFAULT_MODEL, system: Optional[str] = None,
                    max_tokens: int = 4096, thinking: bool = True, stream: bool = False,
                    on_delta: Optional[Callable[[str], None]] = None,
                    mock_response: Optional[str] = None) -> GenResult:
    """Generate code with Claude (real) or a labeled simulation (mock).

    LEVEL-1 KEY RULE: `api_key` is used for exactly one call and then dropped — never stored in env, a
    file, a log, a cache, a global, or on any returned object. Pass it fresh every call.

    • api_key truthy  → one real Claude call (official SDK, model default `claude-opus-4-8`).
    • api_key falsy   → deterministic mock (`source='mock-sim'`, `live=False`); zero network/secrets.

    `stream=True` + `on_delta(chunk)` streams text deltas (recommended for long output). `mock_response`
    overrides the canned mock text (used by S2/S3 to script writer/fixer turns)."""
    if api_key:
        try:
            result = _live_generate(prompt, api_key, model, system, max_tokens,
                                    thinking, stream, on_delta)
        finally:
            # explicit hygiene: forget our binding to the key the instant we're done with it. The
            # caller still owns its copy (it re-supplies per call); WE keep nothing.
            api_key = None
        return result
    return _mock_generate(prompt, model, stream, on_delta, mock_response)
