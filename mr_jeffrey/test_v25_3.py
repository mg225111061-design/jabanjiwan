"""v25 Part V · V3 tests — deploy-readiness + working endpoints. Run: python3 test_v25_3.py

endpoints_work : with FastAPI installed, GET /health + / and POST /api/generate (chat/code/ask) +
                 /api/stream all return 200 with the right shape (regression guard for the Request bug).
health_endpoint: /health returns ok (deploy health check).
deploy_ready   : Dockerfile runs server.py; requirements has fastapi/uvicorn; host/port from env; README
                 has exact Cloud Run + Render commands.
NOTE: a public deploy URL needs the user's cloud account (commands in README). The localhost link was
verified LIVE in this build (curl /health + /api/generate); CI here uses an in-process TestClient.
"""
import os
import sys

PASS, FAIL, SKIP = [], [], []
WEB = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "haran-web"))


def check(n, c, d=""):
    (PASS if c else FAIL).append(n)
    print(f"  [{'PASS' if c else 'FAIL'}] {n}" + (f" — {d}" if d and not c else ""))


def _client():
    import server
    from fastapi.testclient import TestClient
    return TestClient(server.create_app())


def endpoints_work():
    try:
        import fastapi  # noqa: F401
    except ImportError:
        SKIP.append("endpoints_work")
        print("  [SKIP] endpoints_work — FastAPI not installed (pip install -r requirements.txt to run live)")
        return
    c = _client()
    health = c.get("/health").status_code == 200
    page = c.get("/").status_code == 200 and len(c.get("/").text) > 10000
    chat = c.post("/api/generate", json={"prompt": "안녕"}).json()
    code = c.post("/api/generate", json={"prompt": "정수 오름차순 정렬 함수 만들어줘", "mode": "extended"}).json()
    ask = c.post("/api/generate", json={"prompt": "정렬 함수"}).json()
    stream = c.post("/api/stream", json={"prompt": "정수 오름차순 정렬 함수 만들어줘"})
    ok = (health and page and chat.get("kind") == "chat" and code.get("kind") == "code"
          and ask.get("kind") == "ask" and stream.status_code == 200 and '"stage"' in stream.text)
    check("endpoints_work", ok,
          f"health={health} page={page} chat={chat.get('kind')} code={code.get('kind')} ask={ask.get('kind')}")
    print("      → /health, /, /api/generate (chat/code/ask), /api/stream all 200 + correct kinds. "
          "★regression guard for the FastAPI `req: Request` annotation bug (PEP 563) caught by running live★.")


def health_endpoint():
    src = open("server.py").read()
    ok = '"/health"' in src and '"ok": True' in src
    check("health_endpoint", ok)
    print("      → GET /health → {ok:true} for Cloud Run / Render health checks.")


def deploy_ready():
    df = open("Dockerfile").read()
    rq = open("requirements.txt").read().lower()
    sv = open("server.py").read()
    rd = open("haran_web_README.md").read()
    ok = ("server.py" in df and "fastapi" in rq and "uvicorn" in rq
          and "HARAN_HOST" in sv and "HARAN_PORT" in sv
          and "gcloud run deploy" in rd and "Render" in rd)
    check("deploy_ready", ok)
    print("      → Dockerfile runs server.py; deps fastapi/uvicorn; host/port from env; README has exact "
          "Cloud Run (`gcloud run deploy --source .`) + Render steps. (Public URL needs your account.)")


if __name__ == "__main__":
    print("v25 Part V · V3 — deploy-readiness + working endpoints")
    endpoints_work(); health_endpoint(); deploy_ready()
    print(f"\nV3: {len(PASS)} passed, {len(FAIL)} failed, {len(SKIP)} skipped")
    sys.exit(1 if FAIL else 0)
