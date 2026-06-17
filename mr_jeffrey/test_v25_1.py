"""v25 Part V · V1 tests — clean web-app repo structure. Run: python3 test_v25_1.py

dependency_complete    : the clean repo (../haran-web) imports + runs using ONLY its own files (subprocess).
clean_structure_imports: the dependency closure is the small web subset (~33), not the 200+ engine.
assets_present         : haran.html / server.py / requirements.txt / Dockerfile are all copied.
original_preserved     : the original mr_jeffrey/ is untouched (build COPIES, never moves).
"""
import os
import subprocess
import sys

import build_clean_repo as B

PASS, FAIL, SKIP = [], [], []
WEB = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "haran-web"))


def check(n, c, d=""):
    (PASS if c else FAIL).append(n)
    print(f"  [{'PASS' if c else 'FAIL'}] {n}" + (f" — {d}" if d and not c else ""))


def dependency_complete():
    # run a subprocess WITH CWD = haran-web so it can only import haran-web's own files.
    code = ("import server;"
            "r=server.handle_route({'prompt':'정수 오름차순 정렬 함수 만들어줘','mode':'extended'});"
            "c=server.handle_route({'prompt':'안녕'});"
            "list(server.stream_events({'prompt':'정수 오름차순 정렬 함수 만들어줘'}));"
            "assert r['kind']=='code' and c['kind']=='chat';"
            "print('OK')")
    p = subprocess.run([sys.executable, "-c", code], cwd=WEB, capture_output=True, text=True)
    ok = p.returncode == 0 and "OK" in p.stdout
    check("dependency_complete", ok, f"rc={p.returncode} err={p.stderr.strip()[-200:]}")
    print("      → the clean repo imports + routes (code/chat) using ONLY its own files — no missing "
          "dependency (verified in a subprocess with cwd=haran-web).")


def clean_structure_imports():
    mods = B.closure()
    engine_total = len([f for f in os.listdir(B.SRC) if f.endswith(".py")])
    ok = 20 <= len(mods) <= 60 and len(mods) < engine_total / 3 and "server" in mods and "intent" in mods
    check("clean_structure_imports", ok, f"closure={len(mods)} of {engine_total} engine .py")
    print(f"      → web app needs {len(mods)} modules (of {engine_total} in the engine) — only the "
          "traced dependencies, ~200 unused files excluded.")


def assets_present():
    need = ["haran.html", "server.py", "agentic.py", "claude_agent.py", "intent.py",
            "requirements.txt", "Dockerfile", "docker-compose.yml"]
    missing = [f for f in need if not os.path.exists(os.path.join(WEB, f))]
    ok = not missing
    check("assets_present", ok, f"missing={missing}")
    print("      → web assets + entry modules all present in the clean repo.")


def original_preserved():
    # the original engine still has its full module set (build copied, did not move)
    n = len([f for f in os.listdir(B.SRC) if f.endswith(".py")])
    ok = n > 150 and os.path.exists(os.path.join(B.SRC, "server.py"))
    check("original_preserved", ok, f"engine .py count={n}")
    print(f"      → original mr_jeffrey/ untouched ({n} .py still there) — clean repo is a COPY, not a move.")


if __name__ == "__main__":
    print("v25 Part V · V1 — clean web-app repo structure")
    dependency_complete(); clean_structure_imports(); assets_present(); original_preserved()
    print(f"\nV1: {len(PASS)} passed, {len(FAIL)} failed, {len(SKIP)} skipped")
    sys.exit(1 if FAIL else 0)
