# HARAN v23 — the web product (front-end + backend + integration)

v22's `agentic_code()` (Claude writes → HARAN verifies → fixes via counterexamples → fold-optimizes)
turned into a conversational web product: Claude codes, a purpose-built model strengthens it, HARAN
proves it, and **you keep instructing** to refine. Single-file front-end (`haran.html`) + FastAPI
backend (`server.py`) wrapping v22.

> NOTE: no prior `haran.html` existed in this freshly-cloned container (the earlier mock was never
> committed / lost with its ephemeral session), so this front-end was built fresh to the spec — not an
> overhaul. If you have your original, share it and I'll integrate to it instead.

## Run it (you, now, free — localhost)
**Option A — Docker (one image: engine + FastAPI + front-end):**
```
cd mr_jeffrey
docker compose up            # → http://localhost:8000
# unbounded-∀ proofs (heavier): docker compose build --build-arg INSTALL_COQ=true
```
**Option B — uvicorn (no Docker):**
```
cd mr_jeffrey
pip install -r requirements.txt
python server.py             # reads HARAN_HOST/HARAN_PORT → http://localhost:8000
```
Then open the page, (optionally) paste your Claude API key, pick a mode, and start instructing.
Without a key everything runs as a **labeled simulation** (`SIM`) so the flow works offline.

## Deploy later (when you're ready — form only, not done here)
The same image deploys unchanged to **Cloud Run / Render / Fly** — only env vars differ
(`HARAN_PORT`, `HARAN_HOST`). No code change. Not deployed here; the shape is ready.

## Config (env vars — never the key)
| var | default | meaning |
|---|---|---|
| `HARAN_HOST` | `0.0.0.0` (Docker) / `127.0.0.1` (local) | bind host |
| `HARAN_PORT` | `8000` | bind port |
| `HARAN_MODE` | `normal` | default mode hint |

**★ The Claude API key is NEVER an env var, file, log, cache, or image layer.** It is entered per
request in the browser, sent for exactly one call, and dropped (level-1). Verified by grep in
`test_web9` (no key in Dockerfile/compose/server env) and `test_web6` (never stored/echoed server-side).

## What's auto-verified vs. needs your eyes
**Auto-verified (tests, this environment):**
- v22 pipeline S1–S7 (`test_s1`–`test_s7`).
- Backend: `/api/generate` + handle_generate (T6), SSE `stream_events` shape + order + fix-loop + scope
  (T7), history threading + **incremental re-verify** (measured ×5.1 one-edit / ×20.3 unchanged, T8),
  Docker/compose/requirements structure + **key-never-in-env grep** (T9).
- Front-end structure: KO/EN parity, mode theme swap, copy/chips, message differentiation, feedback-loop
  wiring, SSE consumer (`test_web1`–`test_web8`). Inline JS validated with `node --check`.

**Needs your eyes (USER-CONFIRMATION — not claimed done):**
- The screen's actual look: rounding, the cool↔warm theme **color feel** + transition smoothness.
- KO/EN copy **tone**; the streaming **feel**; the feedback-loop **flow**.
- A **real Claude API connection** (paste a key — the live path needs `pip install anthropic` + network).
- `docker build` + localhost serving **here**: the docker **daemon is not reachable in this sandbox**, so
  the image was written + structurally validated but NOT built/run here. Build it on your machine.
- Final marketing copy; the `[TBD: 측정필요]` comparison numbers (unmeasured → placeholder).

## Honesty line
Mirages: none. Marketing copy ("압도적인 …") is tagged `// marketing copy` and never mixed with measured
results. Unmeasured figures are `[TBD: 측정필요]`. Verification is **against the spec, not intent**
(whole-program requests get an honest scope reply — Rice — never a fake "verified backend"). Key stored
nowhere. The screen is a user-confirmation subject: this is **"v22 logic done + v23 backend done + screen
awaiting your confirmation,"** not a blanket "complete."
