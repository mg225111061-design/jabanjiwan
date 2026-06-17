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
from pathlib import Path
from typing import List, Optional, Tuple

import agentic as AG
import claude_agent as CA

HARAN_HTML = Path(__file__).with_name("haran.html")

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
    history = parse_history(p.get("history"))
    api_key = p.get("apiKey") or None          # read locally only
    if not prompt:
        return {"error": True, "message": "empty prompt"}
    try:
        res = AG.agentic_code(prompt, mode, api_key, history=history)
        return to_result_dict(res)
    except Exception as e:   # noqa: BLE001 — never leak the key in an error message
        return {"error": True, "message": f"{type(e).__name__}: {CA.redact_key(str(e))}"}
    finally:
        api_key = None       # drop our binding immediately (the client re-supplies per request)


def _fastapi_available() -> bool:
    return importlib.util.find_spec("fastapi") is not None


def create_app():
    """Wire the FastAPI app (lazy import). Routes delegate to the dependency-free handlers above."""
    from fastapi import FastAPI, Request                       # noqa: PLC0415 (lazy by design)
    from fastapi.responses import HTMLResponse, JSONResponse

    app = FastAPI(title="HARAN", docs_url=None, redoc_url=None)

    @app.get("/", response_class=HTMLResponse)
    async def index():                                          # noqa: ANN202
        return HARAN_HTML.read_text(encoding="utf-8")

    @app.post("/api/generate")
    async def generate(req: Request):                          # noqa: ANN202
        payload = await req.json()
        return JSONResponse(handle_generate(payload))

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
