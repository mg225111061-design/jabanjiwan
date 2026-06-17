# MR.JEFFREY v24 — intent + conversation + progress + real connection

v24 makes the product *understand what you mean* and *show what it's doing*:

- **Intent (U1):** keyword-first, local, sub-ms — `CODING` / `CHAT` / `QUESTION`. Only genuinely
  ambiguous text pays a Claude round-trip (hundreds of ms — shown honestly, never claimed sub-ms).
- **Clarity (U2):** a vague coding ask (`정렬 함수`) → expected questions (asc/desc? type?); a specified
  one proceeds. Suggestions, not a block.
- **Chat (U3):** non-coding → a plain Claude answer with **no verification label** (coding answers are
  proven; chat answers are not — we never mix them).
- **Router (U4):** `intent.route()` → `code` | `chat` | `ask`. `server.handle_route` serializes it;
  `/api/generate` and `/api/stream` route through it.
- **Progress (U5):** SSE `stage` events — **분류중 → Claude 호출중 → 검증중 → 반례 수정중 → 최적화중** —
  each emitted only when that work actually runs (no fake progress). A spinner + label shows the wait.
- **UI (U6/U7):** quick-action chips hidden behind a toggle; expected questions in a collapsible panel
  with "proceed anyway".
- **Color (U8):** normal = **black** theme, extended = **white** theme; two gradient layers cross-fade
  smoothly on switch; text contrast guaranteed both ways (no white-on-white).
- **Real connection (U9):** the live path uses the official Anthropic SDK for every Claude touchpoint
  (classify / clarity / chat / generate). Errors are friendly and **key-safe** (redacted). Mock
  fallback keeps the standalone page working with no key.

## Run it yourself (you enter the key; this build makes it *possible*)
```
cd mr_jeffrey
pip install -r requirements.txt          # fastapi, uvicorn, anthropic, sympy, z3-solver
python server.py                         # → http://localhost:8000   (HARAN_HOST / HARAN_PORT env)
# or: docker compose up
```
Open the page → (optionally) paste your Claude API key → pick a mode → type anything.
- No key → everything runs as a labeled **SIM** (the flow works offline; classification/clarity/chat
  are local or canned).
- With a key → real Claude generation + HARAN verification; you'll see the live stages and a `LIVE` tag.

## ★ Key security — LEVEL 1 (unchanged) ★
The Claude API key is entered **every request**, used for that one call, and dropped. It is **never**
stored (env / file / log / cache / DB / localStorage), never echoed, never in the Docker image. The
classification, clarity, chat, and generation calls all obey this. Verified by grep across every
v22/v23/v24 file (`claude_agent` doesn't even import `os`).

## Auto-verified vs. needs your eyes
**Auto-verified (tests):** intent classification (U1), clarity (U2), chat split + no-verify-label (U3),
routing (U4), real stage events 1:1 with the pipeline (U5), chips/questions structure (U6/U7), black↔white
palettes + readability contrast (U8), live-path wiring + front↔back contract + error redaction (U9).
Inline JS validated with `node --check`.

**Needs your eyes (USER-CONFIRMATION — not claimed done):** the actual look; the black↔white **color feel,
gradient & transition smoothness, readability**; KO/EN tone; the **progress-indicator feel**; the chips /
expected-questions feel; the conversation flow; and **running it live with your own key** (needs network +
`pip install anthropic`). Docker build/serve needs a daemon (not reachable in this sandbox).

## Honesty line
Coding = verified (PROVEN/반례 labels); chat = plain answer (no label). Progress stages are real (a chat
turn never shows 검증중/최적화중). Speed is honest (local classify sub-ms; Claude calls are hundreds of ms,
shown as progress). Marketing copy stays `// marketing copy`; unmeasured figures are `[TBD: 측정필요]`. No
mirages. Verification is spec-relative, not intent. **"Logic + backend + connection-ready are done; the
screen and the live run are yours to confirm"** — not a blanket "complete".
