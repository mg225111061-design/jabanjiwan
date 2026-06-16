#!/usr/bin/env python3
"""
STAGE 4.2 — Mr. Jeffrey web demo (pure stdlib http.server — no Flask dependency).
================================================================================
A one-file local demo: paste a Python function (+ an optional reference) and Mr. hunts for the
breaking input; or give an algebraic identity and Mr. PROVES it for all inputs (JEFF → sympy).

  GET  /                 the human page (form + 3-tier legend)
  GET  /health           "ok"            (liveness, used by tests)
  POST /api/prove        JSON {cand, ref, vars:[...]}        -> {verdict, backend, detail}
  POST /api/verify       JSON {code, func, kinds:[...], reference?} -> {verdict, detail}

Run:   python3 mr_web.py [--host 127.0.0.1] [--port 8000]
Test:  make_server(port=0) returns an HTTPServer you can run in a thread.

⚠ HONEST LIMIT: /api/verify executes the code you send, inside Mr.'s file/network sandbox but in
this process. This is a LOCAL developer demo — do not expose it to untrusted input on a network.
Verdicts keep the discipline: PROVEN (exact) / VERIFIED (bounded, NOT proof) / REFUTED (witness).
"""
from __future__ import annotations

import argparse
import json
from http.server import BaseHTTPRequestHandler, HTTPServer

from jeff_adapter import backends_available, prove_identity
from verify_strong import StrongVerifier

INDEX_HTML = """<!doctype html><html><head><meta charset="utf-8">
<title>Mr. Jeffrey — verify & prove AI code</title>
<style>
 body{font:15px/1.5 system-ui,Segoe UI,Roboto,sans-serif;max-width:860px;margin:2rem auto;padding:0 1rem;color:#1c1c1c}
 h1{margin-bottom:.2rem} .sub{color:#666;margin-top:0}
 textarea,input{width:100%;font-family:ui-monospace,Menlo,monospace;font-size:13px;padding:.5rem;box-sizing:border-box}
 textarea{height:8.5rem} fieldset{border:1px solid #ddd;border-radius:8px;margin:1rem 0}
 button{padding:.5rem 1.1rem;font-size:14px;border:0;border-radius:6px;background:#1565c0;color:#fff;cursor:pointer}
 .legend span{display:inline-block;padding:.1rem .5rem;border-radius:4px;font-weight:600;margin-right:.4rem}
 .proven{background:#e6f4ea;color:#137333} .verified{background:#fef7e0;color:#b06000} .refuted{background:#fce8e6;color:#c5221f}
 pre{background:#0d1117;color:#d1d5da;padding:1rem;border-radius:8px;overflow:auto;white-space:pre-wrap}
 .row{display:flex;gap:1rem} .row>div{flex:1}
</style></head><body>
<h1>Mr. Jeffrey</h1>
<p class="sub">Fast, decisive, <b>honest</b> verification of AI-written code. The verification is the
product — not the model.</p>
<p class="legend">
 <span class="proven">PROVEN</span> exact, all inputs &nbsp;
 <span class="verified">VERIFIED</span> bounded — strong, NOT a proof &nbsp;
 <span class="refuted">REFUTED</span> here is the breaking input
</p>

<fieldset><legend><b>Prove an identity</b> (exact, all inputs — JEFF → sympy)</legend>
 <div class="row">
   <div><label>candidate<br><input id="pcand" value="(x+1)**2"></label></div>
   <div><label>reference<br><input id="pref" value="x**2 + 2*x + 1"></label></div>
   <div><label>vars (comma)<br><input id="pvars" value="x"></label></div>
 </div>
 <p><button onclick="prove()">Prove</button></p>
 <pre id="pout">…</pre>
</fieldset>

<fieldset><legend><b>Hunt for bugs</b> (bounded + fuzz + timeout guard)</legend>
 <label>function under test<br>
 <textarea id="vcode">def average(xs):
    return sum(xs) / len(xs)   # bug: crashes on []</textarea></label>
 <div class="row">
   <div><label>function name<br><input id="vfunc" value="average"></label></div>
   <div><label>arg kinds (comma)<br><input id="vkinds" value="list_float"></label></div>
 </div>
 <label>reference (optional — a correct version named the same)<br>
 <textarea id="vref" style="height:5rem">def average(xs):
    return sum(xs) / len(xs) if xs else 0.0</textarea></label>
 <p><button onclick="hunt()">Hunt</button></p>
 <pre id="vout">…</pre>
</fieldset>

<script>
function cls(v){return v=="PROVEN"?"proven":v=="VERIFIED"?"verified":v=="REFUTED"?"refuted":""}
async function post(u,b){let r=await fetch(u,{method:"POST",headers:{"content-type":"application/json"},body:JSON.stringify(b)});return r.json()}
async function prove(){
 let b={cand:pcand.value,ref:pref.value,vars:pvars.value.split(",").map(s=>s.trim())};
 let j=await post("/api/prove",b);
 pout.innerHTML="["+(j.backend||"?")+"] "+(j.verdict||"ERR")+"\\n"+(j.detail||j.error||"");
}
async function hunt(){
 let b={code:vcode.value,func:vfunc.value,kinds:vkinds.value.split(",").map(s=>s.trim()),reference:vref.value};
 let j=await post("/api/verify",b);
 vout.innerHTML=(j.verdict||"ERR")+"\\n"+(j.detail||j.error||"");
}
</script>
<p class="sub">⚠ Local developer demo: the bug-hunt runs your code in Mr.'s file/network sandbox, in this
process. Don't expose to untrusted input.</p>
</body></html>"""


def _load_func(code: str, func: str):
    ns: dict = {}
    exec(compile(code, "<mr_web_user>", "exec"), ns)  # noqa: S102 - local demo, sandboxed at call time
    fn = ns.get(func)
    if fn is None:
        raise ValueError(f"function '{func}' not defined")
    return fn


def api_prove(body: dict) -> dict:
    cand = body.get("cand", "")
    ref = body.get("ref", "")
    variables = body.get("vars") or ["x"]
    if not cand or not ref:
        return {"error": "need 'cand' and 'ref'"}
    res = prove_identity(cand, ref, variables)
    return {"verdict": res.verdict, "backend": res.backend, "detail": str(res)}


def api_verify(body: dict) -> dict:
    code = body.get("code", "")
    func = body.get("func", "")
    kinds = body.get("kinds") or ["int"]
    ref_code = body.get("reference") or ""
    if not code or not func:
        return {"error": "need 'code' and 'func'"}
    try:
        fn = _load_func(code, func)
    except Exception as e:  # noqa: BLE001
        return {"error": f"could not load function: {e}"}
    reference = None
    if ref_code.strip():
        try:
            reference = _load_func(ref_code, func)
        except Exception as e:  # noqa: BLE001
            return {"error": f"could not load reference: {e}"}
    V = StrongVerifier(timeout=1.0, fuzz_per_arg=20, max_tests=200)
    rep = V.verify(fn, kinds, reference=reference, func_name=func)
    return {"verdict": rep.verdict, "detail": str(rep)}


class Handler(BaseHTTPRequestHandler):
    def log_message(self, *a):  # keep the demo quiet
        pass

    def _send(self, code, body, ctype="text/html; charset=utf-8"):
        data = body.encode("utf-8") if isinstance(body, str) else body
        self.send_response(code)
        self.send_header("Content-Type", ctype)
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def do_GET(self):
        path = self.path.split("?", 1)[0]
        if path == "/":
            self._send(200, INDEX_HTML)
        elif path == "/health":
            self._send(200, "ok", "text/plain; charset=utf-8")
        elif path == "/api/backends":
            self._send(200, json.dumps(backends_available()), "application/json")
        else:
            self._send(404, "not found", "text/plain; charset=utf-8")

    def do_POST(self):
        path = self.path.split("?", 1)[0]
        n = int(self.headers.get("Content-Length", 0))
        raw = self.rfile.read(n) if n else b"{}"
        try:
            body = json.loads(raw.decode("utf-8") or "{}")
        except Exception:
            self._send(400, json.dumps({"error": "invalid JSON"}), "application/json")
            return
        if path == "/api/prove":
            out = api_prove(body)
        elif path == "/api/verify":
            out = api_verify(body)
        else:
            self._send(404, json.dumps({"error": "no such endpoint"}), "application/json")
            return
        self._send(200, json.dumps(out), "application/json")


def make_server(host="127.0.0.1", port=8000):
    """Build (but do not start) the HTTPServer. port=0 picks a free port (handy for tests)."""
    return HTTPServer((host, port), Handler)


def main(argv=None):
    ap = argparse.ArgumentParser(prog="mr_web", description="Mr. Jeffrey web demo (stdlib).")
    ap.add_argument("--host", default="127.0.0.1")
    ap.add_argument("--port", type=int, default=8000)
    args = ap.parse_args(argv)
    srv = make_server(args.host, args.port)
    host, port = srv.server_address
    b = backends_available()
    print(f"Mr. Jeffrey web demo on http://{host}:{port}   (exact backends: JEFF={b['jeff']} sympy={b['sympy']})")
    print("Ctrl-C to stop.")
    try:
        srv.serve_forever()
    except KeyboardInterrupt:
        print("\nbye.")
        srv.server_close()
    return 0


if __name__ == "__main__":
    import sys
    sys.exit(main())
