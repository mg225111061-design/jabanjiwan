"""STAGE 4 tests — productization (CLI, web demo, showcase). Run: python3 test_stage4.py"""
import json
import os
import subprocess
import sys
import threading
import time
import urllib.request

HERE = os.path.dirname(os.path.abspath(__file__))
PY = sys.executable

PASS, FAIL = [], []
def check(name, cond, detail=""):
    (PASS if cond else FAIL).append(name)
    print(f"  [{'PASS' if cond else 'FAIL'}] {name}" + (f" — {detail}" if detail and not cond else ""))


def _run_cli(*args, timeout=60):
    p = subprocess.run([PY, "mr.py", *args], cwd=HERE, capture_output=True, text=True, timeout=timeout)
    return p.returncode, p.stdout + p.stderr


def cli_runs():
    # `mr prove` on a true identity → exit 0, PROVEN in output.
    rc, out = _run_cli("prove", "(x+1)**2", "x**2 + 2*x + 1", "--vars", "x")
    check("cli_prove_true", rc == 0 and "PROVEN" in out, f"rc={rc} out={out!r}")
    # `mr prove` on a false identity → exit 1, REFUTED.
    rc2, out2 = _run_cli("prove", "n*n/2", "n*(n+1)/2", "--vars", "n")
    check("cli_prove_false", rc2 == 1 and "REFUTED" in out2, f"rc={rc2} out={out2!r}")
    # `mr verify` on a buggy function loaded from a temp file → exit 1, REFUTED.
    src = os.path.join(HERE, "_tmp_cli_target.py")
    with open(src, "w") as f:
        f.write("def find_max(xs):\n    m = 0\n    for x in xs:\n        if x > m: m = x\n    return m\n")
    spec = os.path.join(HERE, "_tmp_cli_spec.py")
    with open(spec, "w") as f:
        f.write("def reference(xs):\n    return max(xs) if xs else 0\narg_kinds = ['list_int']\n")
    try:
        rc3, out3 = _run_cli("verify", src, "--func", "find_max", "--spec", spec)
        check("cli_verify_refutes_bug", rc3 == 1 and "REFUTED" in out3, f"rc={rc3} out={out3!r}")
    finally:
        for p in (src, spec):
            if os.path.exists(p):
                os.remove(p)


def web_demo_serves():
    import mr_web
    srv = mr_web.make_server("127.0.0.1", 0)
    host, port = srv.server_address
    t = threading.Thread(target=srv.serve_forever, daemon=True)
    t.start()
    try:
        time.sleep(0.1)
        base = f"http://127.0.0.1:{port}"
        # health
        with urllib.request.urlopen(base + "/health", timeout=5) as r:
            health = r.read().decode()
        check("web_health", health.strip() == "ok", repr(health))
        # index page
        with urllib.request.urlopen(base + "/", timeout=5) as r:
            page = r.read().decode()
        check("web_index_served", r.status == 200 and "Mr. Jeffrey" in page, f"status={r.status}")
        # prove API: a true identity → PROVEN
        body = json.dumps({"cand": "(x+1)**2", "ref": "x**2 + 2*x + 1", "vars": ["x"]}).encode()
        req = urllib.request.Request(base + "/api/prove", data=body,
                                     headers={"content-type": "application/json"}, method="POST")
        with urllib.request.urlopen(req, timeout=15) as r:
            j = json.loads(r.read().decode())
        check("web_api_prove", j.get("verdict") == "PROVEN", str(j))
        # verify API: a buggy function → REFUTED
        vb = json.dumps({"code": "def f(xs):\n    return sum(xs)/len(xs)\n",
                         "func": "f", "kinds": ["list_float"],
                         "reference": "def f(xs):\n    return sum(xs)/len(xs) if xs else 0.0\n"}).encode()
        req2 = urllib.request.Request(base + "/api/verify", data=vb,
                                      headers={"content-type": "application/json"}, method="POST")
        with urllib.request.urlopen(req2, timeout=15) as r:
            j2 = json.loads(r.read().decode())
        check("web_api_verify", j2.get("verdict") == "REFUTED", str(j2))
    finally:
        srv.shutdown()
        srv.server_close()


def examples_all_caught():
    import examples_demo
    results = examples_demo.run_all(verbose=False)
    bad = [r.id for r in results if not r.ok]
    check("examples_all_caught", len(results) == 10 and not bad,
          f"{len(results)} examples, mismatches={bad}")
    # honesty: at least one PROVEN (exact tier) and several REFUTED (real bugs)
    proven = sum(1 for r in results if r.actual == "PROVEN")
    refuted = sum(1 for r in results if r.actual == "REFUTED")
    check("examples_cover_all_tiers", proven >= 1 and refuted >= 7,
          f"proven={proven} refuted={refuted}")


if __name__ == "__main__":
    print("STAGE 4 — productization (CLI / web / showcase)")
    cli_runs()
    web_demo_serves()
    examples_all_caught()
    print(f"\nStage 4: {len(PASS)} passed, {len(FAIL)} failed")
    sys.exit(1 if FAIL else 0)
