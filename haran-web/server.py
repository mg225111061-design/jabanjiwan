"""
HARAN v23 Part T — web backend (FastAPI wrapping the v22 agentic pipeline).
===========================================================================
Serves haran.html and exposes the agentic pipeline over HTTP:

  • T6  POST /api/generate        — JSON: {prompt, mode, apiKey?, history?} → agentic_code(...) result
  • T7  POST /api/stream          — SSE: token → code_done → verify → (fix) → done  (added in T7)
  • T8  history (conversation)    — threaded into agentic_code as context; incremental on follow-ups

★ KEY SECURITY — LEVEL 1 (server side) ★: the API key is read from the request body for exactly one
  call, passed to Claude, and dropped. It is NEVER stored (no env, file, cache, DB, global), NEVER
  logged, and NEVER echoed in a response or error (errors are redacted). Confirmed by test_web6.

Design: all logic lives in plain, dependency-free functions (handle_generate, to_result_dict,
parse_history) so it is testable WITHOUT FastAPI installed. create_app() lazily imports FastAPI and
wires the routes — so this module imports fine in any environment (mirrors claude_agent's SDK-less path).
"""
from __future__ import annotations

import importlib.util
import json
import re
from pathlib import Path
from typing import Iterator, List, Optional, Tuple

import agentic as AG
import claude_agent as CA
import haran_cache as HC
import intent as IN

# Expose `Request` at MODULE scope so FastAPI can resolve the route handlers' string annotations
# (PEP 563 / `from __future__ import annotations` turns `req: Request` into the string "Request",
# which FastAPI resolves against this module's globals — not create_app's locals). Guarded so the
# module still imports when FastAPI isn't installed.
try:
    from fastapi import Request
except Exception:   # noqa: BLE001
    Request = None

HARAN_HTML = Path(__file__).with_name("haran.html")

# ── intent-gap / scope honesty (rule 5) ─────────────────────────────────────────────────────────
# Whole-program requests can't be generated-from-nothing and verified (Rice). HARAN verifies small~
# medium code AGAINST A SPEC. Detect whole-program asks by their nouns and return an honest scope reply
# instead of fake-verifying. (Keyword-based — NOT length-based; long but tractable requests are fine.)
_SCOPE_RE = re.compile(r"백엔드|서버|backend|server|큐|queue|상태\s*머신|state\s*machine|"
                       r"\bapi\b|jwt|결제|payment", re.I)
SCOPE_MESSAGE = ("scope: HARAN verifies & optimizes small~medium code AGAINST A SPEC (Rice: full "
                 "generation from nothing is impossible). Provide the core logic + a spec (ensures).")


def is_scope(prompt: str) -> bool:
    return bool(_SCOPE_RE.search(prompt or ""))


def _scope_result(prompt: str, mode: str) -> dict:
    return {"request": prompt, "mode": mode, "source": "mock-sim", "scope": True, "converged": False,
            "status": "SCOPE", "code": None, "proof_tier": "(scope)", "optimization": None,
            "ms": 0.0, "history_len": 0, "trace": [], "message": SCOPE_MESSAGE}

# Module invariant: the key is NEVER stored here. Stays None for the process lifetime (test asserts it).
_KEY_STORE = None


def parse_history(raw) -> List[Tuple[str, str]]:
    """Accept history as a list of {request, code} dicts or [request, code] pairs → list of tuples."""
    out: List[Tuple[str, str]] = []
    if not raw:
        return out
    for item in raw:
        if isinstance(item, dict):
            req, code = item.get("request", ""), item.get("code", "")
        elif isinstance(item, (list, tuple)) and len(item) >= 2:
            req, code = item[0], item[1]
        else:
            continue
        if code:
            out.append((str(req), str(code)))
    return out


def to_result_dict(res: AG.AgenticResult) -> dict:
    """Serialize an AgenticResult to a JSON-able dict (never includes the key)."""
    opt = None
    if res.optimization is not None:
        o = res.optimization
        opt = {"optimized": o.optimized, "kind": o.kind, "method": o.method,
               "closed_form": o.closed_form, "speedup": o.speedup}
    trace = [{"iter": s.iteration, "mode": s.mode, "status": s.verdict.status,
              "counterexample": s.verdict.counterexample} for s in res.trace]
    return {"request": res.request, "mode": res.mode, "source": res.source,
            "converged": res.converged, "iters": res.iters, "status": res.status,
            "code": res.final_code, "proof_tier": res.proof_tier, "optimization": opt,
            "ms": round(res.ms, 2), "history_len": res.history_len, "trace": trace}


def handle_generate(payload: Optional[dict]) -> dict:
    """T6 core: run the agentic pipeline for one request. LEVEL-1 key — used once, dropped, never
    stored/logged/echoed. No key → labeled mock (v22). Errors return a redacted dict (never raise)."""
    p = payload or {}
    prompt = str(p.get("prompt", "")).strip()
    mode = p.get("mode", "normal")
    if not prompt:
        return {"error": True, "message": "empty prompt"}
    if is_scope(prompt):                         # intent-gap honesty: don't fake-verify a whole program
        return _scope_result(prompt, mode)
    history = parse_history(p.get("history"))
    api_key = p.get("apiKey") or None          # read locally only
    try:
        res = AG.agentic_code(prompt, mode, api_key, history=history)
        return to_result_dict(res)
    except Exception as e:   # noqa: BLE001 — never leak the key in an error message
        return {"error": True, "message": f"{type(e).__name__}: {CA.redact_key(str(e))}"}
    finally:
        api_key = None       # drop our binding immediately (the client re-supplies per request)


# ---------------------------------------------------------------------------------------------------
# T7 — SSE streaming. The verification RESULTS are real (from agentic_code); the event sequence is
# emitted around them so the UI shows a live token→code_done→verify→(fix)→done flow. Hard cases emit a
# 'verifying' (⏳) status first, then resolve (v21 R5 background → non-blocking status; honest: here the
# pipeline is fast/synchronous, so 'verifying' frames the resolved result rather than truly deferring).
# ---------------------------------------------------------------------------------------------------

def sse_event(obj: dict) -> str:
    """One SSE message: a single JSON object on a `data:` line, terminated by a blank line."""
    return f"data: {json.dumps(obj, ensure_ascii=False)}\n\n"


def _chunks(s: str, n: int = 16) -> List[str]:
    return [s[i:i + n] for i in range(0, len(s), n)] or [""]


def stream_events(payload: Optional[dict]) -> Iterator[str]:
    """T7/U5 core: route the message, then stream REAL progress stages + results over SSE. LEVEL-1 key
    (used once, dropped, never stored/logged/echoed). Each 'stage' is emitted only when that work runs:
      classify → (chat: thinking → chat) | (scope: note) | (vague: ask) | (code: generate → verify →
      [refuted → fix] → optimize → proven/optimized → done). Stages map 1:1 to the real pipeline."""
    p = payload or {}
    prompt = str(p.get("prompt", "")).strip()
    mode = p.get("mode", "normal")
    history = parse_history(p.get("history"))
    api_key = p.get("apiKey") or None
    if not prompt:
        yield sse_event({"type": "error", "message": "empty prompt"})
        return
    try:
        yield sse_event({"type": "stage", "stage": "classify"})           # 분류중 (local, ~instant)
        it = IN.classify_intent(prompt, api_key)

        if it.intent != "CODING":                                          # chat / question
            yield sse_event({"type": "stage", "stage": "thinking"})        # 생각중
            cr = IN.chat_reply(prompt, api_key, history)
            yield sse_event({"type": "chat", "reply": cr.text})            # plain answer, NO verify label
            yield sse_event({"type": "done", "summary": {"kind": "chat", "verified": False,
                                                         "source": cr.source, "intent": it.intent}})
            return

        if IN.is_scope(prompt):                                            # whole-program → scope note
            yield sse_event({"type": "note", "text": IN.SCOPE_REPLY})
            yield sse_event({"type": "done", "summary": {"kind": "chat", "scope": True,
                                                         "verified": False, "source": "local"}})
            return

        if not p.get("force"):                                            # U7: 'proceed anyway' skips it
            clarity = IN.assess_clarity(prompt, api_key)
            if not clarity.clear:                                          # vague → expected questions
                yield sse_event({"type": "ask", "asks": clarity.asks})
                yield sse_event({"type": "done", "summary": {"kind": "ask", "asks": clarity.asks,
                                                             "verified": False, "source": clarity.source}})
                return

        # coding pipeline — emit each real stage as it runs
        for ev in AG.agentic_stream(prompt, mode, api_key, history=history):
            st = ev["stage"]
            if st in ("generate", "fix", "verify", "optimize"):
                d = {"type": "stage", "stage": st}
                if "iter" in ev:
                    d["iter"] = ev["iter"]
                yield sse_event(d)
            elif st == "code_done":
                for chunk in _chunks(ev["code"]):
                    yield sse_event({"type": "token", "text": chunk})
                yield sse_event({"type": "code_done", "code": ev["code"]})
            elif st == "refuted":
                yield sse_event({"type": "verify", "status": "refuted",
                                 "counterexample": ev.get("counterexample")})
            elif st == "done":
                res = ev["result"]
                rd = to_result_dict(res)
                rd["kind"] = "code"
                yield sse_event({"type": "verify",
                                 "status": "proven" if res.proof_tier == "PROVEN" else "shallow",
                                 "tier": res.proof_tier})
                opt = rd["optimization"]
                if opt and opt["optimized"]:
                    yield sse_event({"type": "optimized", "closed_form": opt["closed_form"],
                                     "speedup": opt["speedup"]})
                yield sse_event({"type": "done", "summary": rd})
    except Exception as e:   # noqa: BLE001 — redact, never leak the key
        yield sse_event({"type": "error", "message": f"{type(e).__name__}: {CA.redact_key(str(e))}"})
    finally:
        api_key = None


# ---------------------------------------------------------------------------------------------------
# T8 — follow-up rounds: conversation history is threaded into agentic_code as context (handle_generate
# / stream_events already pass `history`); with a real key Claude reflects the prior code+instructions.
# Incremental re-verify (v21 R4): when a follow-up changes part of a codebase, ONLY the changed function
# (+ its dependents) re-verifies — so follow-up rounds are perceived-zero. Real measured speedup.
# ---------------------------------------------------------------------------------------------------

def handle_route(payload: Optional[dict]) -> dict:
    """U4: classify + route a message → kind-tagged JSON. CODING → verified pipeline (kind='code',
    carries proof labels); CHAT/QUESTION → plain reply (kind='chat', NO verification label); CODING but
    vague → expected questions (kind='ask'). LEVEL-1 key: used once, dropped, never stored/logged/echoed."""
    p = payload or {}
    text = str(p.get("prompt", "")).strip()
    mode = p.get("mode", "normal")
    history = parse_history(p.get("history"))
    api_key = p.get("apiKey") or None
    if not text:
        return {"error": True, "message": "empty prompt"}
    try:
        rr = IN.route(text, mode, api_key, history, force=bool(p.get("force")))
        out = {"kind": rr.kind, "intent": rr.intent, "source": rr.source, "verified": rr.verified}
        if rr.kind == "code":
            out["result"] = to_result_dict(rr.code_result)
        elif rr.kind == "chat":
            out["reply"] = rr.reply                  # plain answer — NO verification label
        elif rr.kind == "ask":
            out["asks"] = rr.asks                    # expected questions (suggestions)
        return out
    except Exception as e:   # noqa: BLE001 — never leak the key
        return {"error": True, "message": f"{type(e).__name__}: {CA.redact_key(str(e))}"}
    finally:
        api_key = None


def reverify_incremental(prev_src: str, new_src: str) -> dict:
    """Re-verify a follow-up edit incrementally (v21 Merkle cache): returns which functions actually
    re-verified + measured timings/speedup. Unchanged functions are served from cache (not re-proved)."""
    m = HC.measure_edit_loop(prev_src, new_src)
    return {"reverified": m.reverified_after_edit,
            "cold_ms": round(m.cold_s * 1000, 2),
            "warm_one_edit_ms": round(m.warm_one_edit_s * 1000, 2),
            "speedup_one_edit": round(m.speedup_one_edit, 1),
            "speedup_unchanged": round(m.speedup_unchanged, 1)}


def _fastapi_available() -> bool:
    return importlib.util.find_spec("fastapi") is not None


def create_app():
    """Wire the FastAPI app (lazy import). Routes delegate to the dependency-free handlers above."""
    from fastapi import FastAPI, Request                       # noqa: PLC0415 (lazy by design)
    from fastapi.responses import HTMLResponse, JSONResponse, StreamingResponse

    app = FastAPI(title="HARAN", docs_url=None, redoc_url=None)

    @app.get("/", response_class=HTMLResponse)
    async def index():                                          # noqa: ANN202
        return HARAN_HTML.read_text(encoding="utf-8")

    @app.get("/health")                                        # deploy health check (Cloud Run/Render)
    async def health():                                        # noqa: ANN202
        return {"ok": True, "service": "mrjeffrey"}

    @app.post("/api/generate")                                 # routes through intent (U4): code|chat|ask
    async def generate(req: Request):                          # noqa: ANN202
        payload = await req.json()
        return JSONResponse(handle_route(payload))

    @app.post("/api/stream")                                    # T7: SSE
    async def stream(req: Request):                            # noqa: ANN202
        payload = await req.json()
        return StreamingResponse(stream_events(payload), media_type="text/event-stream",
                                 headers={"Cache-Control": "no-cache", "X-Accel-Buffering": "no"})

    return app


# Create the ASGI app only when FastAPI is installed (so `uvicorn server:app` works in deployment);
# importing this module without FastAPI is fine (app=None) — the logic is tested via handle_generate.
app = create_app() if _fastapi_available() else None


if __name__ == "__main__":   # pragma: no cover - manual local run
    import os
    if app is None:
        raise SystemExit("FastAPI not installed — `pip install -r requirements.txt` to run the server.")
    import uvicorn
    uvicorn.run(app, host=os.environ.get("HARAN_HOST", "127.0.0.1"),
                port=int(os.environ.get("HARAN_PORT", "8000")))
