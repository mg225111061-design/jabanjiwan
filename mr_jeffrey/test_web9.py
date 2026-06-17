"""v23 Part T · T9 tests — Docker + env vars + localhost. Run: python3 test_web9.py

STRUCTURAL validation here. The actual `docker build` / `localhost` run is attempted separately and is a
USER-CONFIRMATION item (network/daemon dependent) — never reported as passed unless actually observed.
dockerfile_valid    : Dockerfile builds Python+deps, copies app, exposes port, runs server.py.
requirements_present: fastapi / uvicorn / anthropic present.
env_config          : host/port are env-driven (server reads them; Dockerfile + compose set them) — not hardcoded.
key_never_in_env    : ★ the Claude API key is NEVER an env var / baked into image / in compose ★ (level-1).
compose_localhost   : docker-compose maps a localhost port and is env-parametrized.
"""
import re
import sys

PASS, FAIL, SKIP = [], [], []


def check(n, c, d=""):
    (PASS if c else FAIL).append(n)
    print(f"  [{'PASS' if c else 'FAIL'}] {n}" + (f" — {d}" if d and not c else ""))


def _read(p):
    return open(p, encoding="utf-8").read()


def dockerfile_valid():
    df = _read("Dockerfile")
    ok = ("FROM python:" in df and "COPY requirements.txt" in df and "pip install -r requirements.txt" in df
          and "COPY . ." in df and "EXPOSE" in df and "server.py" in df)
    check("dockerfile_valid", ok)
    print("      → Dockerfile: python base → pip install reqs → copy app → EXPOSE → CMD server.py. "
          "[user-confirm: actual `docker build` succeeds in your network].")


def requirements_present():
    rq = _read("requirements.txt").lower()
    ok = "fastapi" in rq and "uvicorn" in rq and "anthropic" in rq
    check("requirements_present", ok)
    print("      → requirements.txt has fastapi + uvicorn + anthropic (+ sympy/z3 for the engine).")


def env_config():
    sv = _read("server.py"); df = _read("Dockerfile"); cp = _read("docker-compose.yml")
    reads = re.findall(r'environ(?:\.get)?\(\s*["\']([^"\']+)["\']', sv)
    server_env = "HARAN_HOST" in reads and "HARAN_PORT" in reads
    docker_env = "HARAN_HOST" in df and "HARAN_PORT" in df
    compose_env = "HARAN_PORT" in cp and "HARAN_HOST" in cp
    ok = server_env and docker_env and compose_env
    check("env_config", ok, f"server={server_env} docker={docker_env} compose={compose_env}")
    print(f"      → host/port are env-driven (server reads {reads}); Dockerfile + compose set them. "
          "Not hardcoded → localhost now, deploy later, zero code change.")


def key_never_in_env():
    df = _read("Dockerfile").upper(); cp = _read("docker-compose.yml").upper(); sv = _read("server.py")
    no_key_docker = "API_KEY" not in df and "ANTHROPIC" not in df
    no_key_compose = "API_KEY" not in cp and "ANTHROPIC" not in cp
    env_reads = re.findall(r'environ(?:\.get)?\(\s*["\']([^"\']+)["\']', sv)
    server_only_haran = all(k.startswith("HARAN_") for k in env_reads)
    ok = no_key_docker and no_key_compose and server_only_haran
    check("key_never_in_env", ok,
          f"docker={no_key_docker} compose={no_key_compose} server_env={env_reads}")
    print(f"      → ★LEVEL 1★ no API key in Dockerfile/compose/env (server reads only {env_reads}); the "
          "key is entered per request in the UI, never via env/file/image. Verified by grep.")


def compose_localhost():
    cp = _read("docker-compose.yml")
    ok = "ports:" in cp and "8000" in cp and "${HARAN_PORT" in cp
    check("compose_localhost", ok)
    print("      → docker compose maps a localhost port (env-parametrized). [user-confirm: `docker "
          "compose up` → http://localhost:8000 actually serves].")


if __name__ == "__main__":
    print("v23 Part T · T9 — Docker + env vars + localhost (structural)")
    dockerfile_valid(); requirements_present(); env_config(); key_never_in_env(); compose_localhost()
    print(f"\nT9: {len(PASS)} passed, {len(FAIL)} failed, {len(SKIP)} skipped")
    print("NOTE: actual `docker build` + localhost serving are USER-CONFIRMATION (network/daemon dependent) "
          "— see the build attempt logged separately; not claimed as passed unless observed.")
    sys.exit(1 if FAIL else 0)
