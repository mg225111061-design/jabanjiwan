"""
STAGE 1 — real LLM adapters for Mr. Jeffrey.
============================================
`mr_jeffrey.MrJeffrey` takes any `llm(prompt)->str` callable. This module supplies real ones:
Anthropic (Claude), OpenAI, and a local/OpenAI-compatible endpoint (Ollama, vLLM, …) — all over
**stdlib urllib** so there is NO SDK dependency. If no API key is present, `get_adapter()` falls
back to a scripted simulation so the *whole loop still runs* (Mr.'s identity: the verification is
the product, the model is swappable).

Philosophy (unchanged): we do NOT try to make the model perfect. We wrap whatever model you have in
perfect verification + self-correction. Adapters are deliberately thin.
"""
from __future__ import annotations
import json
import os
import time
import urllib.request
import urllib.error

# A code-focused system prompt: make the model return ONE python function in a ```python block,
# which `mr_jeffrey.extract_function` knows how to pull out.
SYSTEM_PROMPT = (
    "You are a careful Python engineer. Return ONLY a single self-contained Python function in a "
    "```python code block — no prose, no tests, no prints. The function must be correct for ALL "
    "valid inputs (edge cases included). If told a previous attempt was wrong on a specific input, "
    "fix exactly that and keep the rest correct."
)


class LLMError(Exception):
    pass


class LLMAdapter:
    """Common interface: `.complete(prompt) -> str`, with retry/timeout/error handling.
    Subclasses implement `_call(prompt) -> str`."""

    name = "base"

    def __init__(self, timeout=60, retries=3, backoff=2.0):
        self.timeout = timeout
        self.retries = retries
        self.backoff = backoff

    def _call(self, prompt: str) -> str:  # pragma: no cover - overridden
        raise NotImplementedError

    def complete(self, prompt: str) -> str:
        last = None
        for attempt in range(self.retries):
            try:
                return self._call(prompt)
            except (urllib.error.URLError, urllib.error.HTTPError, TimeoutError, LLMError) as e:
                last = e
                if attempt < self.retries - 1:
                    time.sleep(self.backoff * (attempt + 1))
        raise LLMError(f"{self.name}: failed after {self.retries} attempts: {last}")

    # callable so it drops straight into MrJeffrey(llm=...)
    def __call__(self, prompt: str) -> str:
        return self.complete(prompt)


def _post_json(url: str, headers: dict, payload: dict, timeout: int) -> dict:
    data = json.dumps(payload).encode("utf-8")
    req = urllib.request.Request(url, data=data, headers=headers, method="POST")
    with urllib.request.urlopen(req, timeout=timeout) as resp:
        return json.loads(resp.read().decode("utf-8"))


class AnthropicAdapter(LLMAdapter):
    """Claude via the Anthropic Messages API (stdlib urllib). Reads ANTHROPIC_API_KEY."""

    name = "anthropic"

    def __init__(self, model="claude-sonnet-4-6", max_tokens=2048, api_key=None, **kw):
        super().__init__(**kw)
        self.model = model
        self.max_tokens = max_tokens
        self.api_key = api_key or os.environ.get("ANTHROPIC_API_KEY")
        if not self.api_key:
            raise LLMError("ANTHROPIC_API_KEY not set")

    def _call(self, prompt: str) -> str:
        body = _post_json(
            "https://api.anthropic.com/v1/messages",
            {
                "x-api-key": self.api_key,
                "anthropic-version": "2023-06-01",
                "content-type": "application/json",
            },
            {
                "model": self.model,
                "max_tokens": self.max_tokens,
                "system": SYSTEM_PROMPT,
                "messages": [{"role": "user", "content": prompt}],
            },
            self.timeout,
        )
        # content is a list of blocks; concatenate the text blocks.
        parts = [b.get("text", "") for b in body.get("content", []) if b.get("type") == "text"]
        text = "".join(parts).strip()
        if not text:
            raise LLMError(f"anthropic: empty response: {body}")
        return text


class OpenAIAdapter(LLMAdapter):
    """OpenAI (or any OpenAI-compatible) chat-completions endpoint. Reads OPENAI_API_KEY."""

    name = "openai"

    def __init__(self, model="gpt-4o", api_key=None, base_url="https://api.openai.com/v1", **kw):
        super().__init__(**kw)
        self.model = model
        self.base_url = base_url.rstrip("/")
        self.api_key = api_key or os.environ.get("OPENAI_API_KEY")
        if not self.api_key:
            raise LLMError("OPENAI_API_KEY not set")

    def _call(self, prompt: str) -> str:
        body = _post_json(
            f"{self.base_url}/chat/completions",
            {"Authorization": f"Bearer {self.api_key}", "content-type": "application/json"},
            {
                "model": self.model,
                "messages": [
                    {"role": "system", "content": SYSTEM_PROMPT},
                    {"role": "user", "content": prompt},
                ],
            },
            self.timeout,
        )
        text = body["choices"][0]["message"]["content"].strip()
        if not text:
            raise LLMError("openai: empty response")
        return text


class LocalAdapter(LLMAdapter):
    """Local / open-source model via an OpenAI-compatible endpoint (Ollama, vLLM, LM Studio, …).
    Default targets Ollama's OpenAI-compat server at localhost:11434. No API key — free to run."""

    name = "local"

    def __init__(self, model="llama3", base_url="http://localhost:11434/v1", **kw):
        super().__init__(**kw)
        self.model = model
        self.base_url = base_url.rstrip("/")

    def _call(self, prompt: str) -> str:
        body = _post_json(
            f"{self.base_url}/chat/completions",
            {"content-type": "application/json"},
            {
                "model": self.model,
                "messages": [
                    {"role": "system", "content": SYSTEM_PROMPT},
                    {"role": "user", "content": prompt},
                ],
                "stream": False,
            },
            self.timeout,
        )
        return body["choices"][0]["message"]["content"].strip()


class ScriptedLLM:
    """Deterministic simulation: returns a fixed sequence of attempts (the fallback / for tests).
    The whole self-correction loop runs against this without any API key."""

    name = "scripted"

    def __init__(self, attempts):
        self.attempts = list(attempts)
        self.i = 0

    def complete(self, prompt: str) -> str:
        out = self.attempts[min(self.i, len(self.attempts) - 1)]
        self.i += 1
        return out

    def __call__(self, prompt: str) -> str:
        return self.complete(prompt)


class Qwen3Adapter(LocalAdapter):
    """Qwen3-32B (4-bit, Apache-2.0) via a local OpenAI-compatible endpoint (Ollama/vLLM).
    ONE model does both roles via Qwen3's thinking switch:
      • write (HARAN generation):     thinking OFF (`/no_think`) — fast.
      • verify-reasoning (fix from cx): thinking ON (`/think`)   — deep.
    Multi-adapter slots stay open: a writer/verifier can later be different models entirely."""

    name = "qwen3"

    def __init__(self, model="qwen3:32b", base_url="http://localhost:11434/v1", thinking=False, **kw):
        super().__init__(model=model, base_url=base_url, **kw)
        self.thinking = thinking

    def _call(self, prompt: str) -> str:
        # Qwen3 convention: append /think or /no_think to toggle the reasoning trace.
        return super()._call(prompt + ("\n/think" if self.thinking else "\n/no_think"))


def _endpoint_alive(base_url: str, timeout=2) -> bool:
    root = base_url.rstrip("/").removesuffix("/v1")
    for url in (root + "/api/tags", base_url.rstrip("/") + "/models"):
        try:
            urllib.request.urlopen(url, timeout=timeout)
            return True
        except Exception:   # noqa: BLE001
            continue
    return False


def get_writer_verifier(prefer="qwen3", base_url="http://localhost:11434/v1", model="qwen3:32b",
                        scripted_writer=None, scripted_verifier=None, verbose=True):
    """Return (writer, verifier, mode). Live Qwen3-32B if a local endpoint answers, else an HONEST
    ScriptedLLM SIMULATION (mode='sim') — the write→verify→fix loop and Mr's counterexamples are real
    regardless; only the model's text is simulated. Writer/verifier are separate slots (swappable to
    Qwen2.5-Coder-32B for writing, R1-Distill-Qwen-32B for verification, etc.)."""
    def warn(m):
        if verbose:
            print(f"[mr/llm] {m}")
    if prefer == "qwen3" and _endpoint_alive(base_url):
        warn(f"using LIVE Qwen3-32B ({model} @ {base_url}); write=think-off, verify=think-on")
        return (Qwen3Adapter(model=model, base_url=base_url, thinking=False),
                Qwen3Adapter(model=model, base_url=base_url, thinking=True), "live")
    warn("no local Qwen3-32B endpoint detected — SIMULATION (ScriptedLLM). "
         "★ The loop + Mr's counterexamples are REAL; only the model text is scripted. ★ "
         "Run Ollama with `ollama pull qwen3:32b` to go live.")
    return (ScriptedLLM(scripted_writer or [""]),
            ScriptedLLM(scripted_verifier or scripted_writer or [""]), "sim")


def get_claude_writer_verifier(scripted_writer=None, scripted_verifier=None, model="claude-sonnet-4-6",
                               verbose=True):
    """Return (writer, verifier, mode) for the AI loop. LIVE Claude if ANTHROPIC_API_KEY is set
    (read from env ONLY — never logged), else an honest ScriptedLLM SIMULATION. Writer and verifier
    are SEPARATE adapter instances ⇒ separate contexts (anti self-deception)."""
    def warn(m):
        if verbose:
            print(f"[mr/llm] {m}")
    if os.environ.get("ANTHROPIC_API_KEY"):
        try:
            writer = AnthropicAdapter(model=model)
            verifier = AnthropicAdapter(model=model)   # separate instance = separate context
            warn(f"using LIVE Claude ({model}); writer/verifier = separate contexts")
            return writer, verifier, "live"
        except LLMError as e:
            warn(f"Claude unavailable ({e}) — falling back to simulation")
    warn("no ANTHROPIC_API_KEY — SIMULATION (ScriptedLLM). The loop + Mr's counterexamples are REAL; "
         "only the model text is scripted. Set ANTHROPIC_API_KEY to go live.")
    return (ScriptedLLM(scripted_writer or [""]),
            ScriptedLLM(scripted_verifier or scripted_writer or [""]), "sim")


def get_adapter(prefer: str = "auto", scripted_attempts=None, verbose=True, **kw):
    """Factory. `prefer` ∈ {auto, anthropic, openai, local, scripted}. `auto` picks the first real
    backend with credentials, else falls back to ScriptedLLM with a printed warning. Returns an
    object usable directly as `MrJeffrey(llm=...)`."""

    def warn(msg):
        if verbose:
            print(f"[mr/llm] {msg}")

    order = (
        [prefer] if prefer in ("anthropic", "openai", "local", "scripted")
        else ["anthropic", "openai", "local"]
    )
    for backend in order:
        try:
            if backend == "anthropic":
                a = AnthropicAdapter(**{k: v for k, v in kw.items() if k in ("model", "max_tokens")})
                warn(f"using Anthropic ({a.model})")
                return a
            if backend == "openai":
                a = OpenAIAdapter(**{k: v for k, v in kw.items() if k in ("model", "base_url")})
                warn(f"using OpenAI ({a.model})")
                return a
            if backend == "local":
                # only use local if explicitly preferred (auto must not block on a dead localhost)
                if prefer == "local":
                    a = LocalAdapter(**{k: v for k, v in kw.items() if k in ("model", "base_url")})
                    warn(f"using local endpoint ({a.base_url}, {a.model})")
                    return a
            if backend == "scripted":
                warn("using scripted simulation (explicit)")
                return ScriptedLLM(scripted_attempts or [""])
        except LLMError as e:
            warn(f"{backend} unavailable: {e}")
    warn("no API key / backend available — using SIMULATION (ScriptedLLM). "
         "Set ANTHROPIC_API_KEY or OPENAI_API_KEY for a real model.")
    return ScriptedLLM(scripted_attempts or [""])
