# HARAN web — one image: Python engine (Z3 / sympy) + FastAPI + v22 agentic pipeline + front-end.
# Local-only by default (you, now, free). The SAME image deploys later (Cloud Run / Render) with zero
# code change — only env vars differ. See HARAN_v23_README.md.
FROM python:3.11-slim AS base
ENV PYTHONUNBUFFERED=1 \
    PIP_NO_CACHE_DIR=1
WORKDIR /app

# Optional: Coq enables unbounded-∀ proofs (extended mode, hard cases). It is HEAVY, so it's OFF by
# default to keep the image lean — without it those cases honestly DEFER (UNRESOLVED), never wrong.
# Enable with:  docker build --build-arg INSTALL_COQ=true .
ARG INSTALL_COQ=false
RUN if [ "$INSTALL_COQ" = "true" ]; then \
        apt-get update && apt-get install -y --no-install-recommends coq && \
        rm -rf /var/lib/apt/lists/* ; \
    fi

COPY requirements.txt .
RUN pip install -r requirements.txt
COPY . .

# Config via env (NOT hardcoded). ★ The Claude API key is NEVER an env var or baked into the image —
#   it is entered per request in the browser, used once for the call, and dropped (level-1). ★
ENV HARAN_HOST=0.0.0.0 \
    HARAN_PORT=8000
EXPOSE 8000

# server.py reads HARAN_HOST / HARAN_PORT from env and runs uvicorn.
CMD ["python", "server.py"]
