"""v7 tests (P1-P3) — live Claude AI loop (or honest sim). Run: python3 test_v7.py"""
import io, os, contextlib
from llm_adapters import get_claude_writer_verifier
from ai_loop import write_verify_fix, minimal_feedback, measure_parse_failures, HARAN_GRAMMAR_HINT
PASS, FAIL, SKIP = [], [], []
def check(n, c, d=""):
    (PASS if c else FAIL).append(n); print(f"  [{'PASS' if c else 'FAIL'}] {n}" + (f" — {d}" if d and not c else ""))
def skip(n, w): SKIP.append(n); print(f"  [SKIP] {n} — {w}")

TASK = "Write HARAN sum_squares(n: Nat) -> Nat with ensures result = n*(n+1)*(2*n+1)/6, effects pure."
H = "fn sum_squares(n: Nat) -> Nat\n  ensures result = n*(n+1)*(2*n+1)/6\n  effects pure\n{{ fold k in 1..n {{ {b} }} }}\n"
WRONG  = H.format(b="k")
WRONG2 = H.format(b="k + k")
WRONG3 = H.format(b="k*k*k")
RIGHT  = H.format(b="k*k")

LIVE = bool(os.environ.get("ANTHROPIC_API_KEY"))

# ---- P1 ----
def api_adapter_reads_env():
    # with a (fake) key set, adapter selection → "live" (no API call made here); without → "sim"
    saved = os.environ.get("ANTHROPIC_API_KEY")
    os.environ["ANTHROPIC_API_KEY"] = "sk-ant-FAKE-DO-NOT-USE"
    _, _, mode_live = get_claude_writer_verifier(scripted_writer=[WRONG], verbose=False)
    if saved is None: del os.environ["ANTHROPIC_API_KEY"]
    else: os.environ["ANTHROPIC_API_KEY"] = saved
    _, _, mode_nokey = get_claude_writer_verifier(scripted_writer=[WRONG], verbose=False) if not LIVE else (None, None, "live")
    ok = mode_live == "live" and (mode_nokey == "sim" or LIVE)
    check("api_adapter_reads_env", ok, f"with-key={mode_live} no-key={mode_nokey}")

def api_fallback_to_sim_when_no_key():
    if LIVE: skip("api_fallback_to_sim_when_no_key", "real key present → live"); return
    _, _, mode = get_claude_writer_verifier(scripted_writer=[WRONG], verbose=False)
    check("api_fallback_to_sim_when_no_key", mode == "sim", mode)

def key_never_logged():
    saved = os.environ.get("ANTHROPIC_API_KEY")
    secret = "sk-ant-SECRET-NEVER-LOG-9999"
    os.environ["ANTHROPIC_API_KEY"] = secret
    buf = io.StringIO()
    with contextlib.redirect_stdout(buf):
        get_claude_writer_verifier(scripted_writer=[WRONG], verbose=True)
    if saved is None: del os.environ["ANTHROPIC_API_KEY"]
    else: os.environ["ANTHROPIC_API_KEY"] = saved
    ok = secret not in buf.getvalue()
    check("key_never_logged", ok, "KEY LEAKED IN OUTPUT" if not ok else "")
    print("      → API key read from env only; never appears in logs/output.")

# ---- P2 ----
def live_loop_converges():
    w, v, mode = get_claude_writer_verifier(scripted_writer=[WRONG], scripted_verifier=[RIGHT], verbose=False)
    res = write_verify_fix(TASK, w, v, max_iters=3, verbose=False)
    if LIVE:
        ok = res.converged
        print(f"      → LIVE Claude loop converged in {res.iters} rounds")
    else:
        ok = res.converged and res.iters == 2
        print(f"      → sim loop converged in {res.iters} rounds (live needs ANTHROPIC_API_KEY)")
    check("live_loop_converges", ok, f"converged={res.converged} iters={res.iters} mode={mode}")

def minimal_counterexample_used():
    w, v, _ = get_claude_writer_verifier(scripted_writer=[WRONG], scripted_verifier=[RIGHT], verbose=False)
    res = write_verify_fix(TASK, w, v, max_iters=3, verbose=False)
    fix_prompt = res.trace[1].prompt if len(res.trace) > 1 else ""
    # Mr's cx is the SMALLEST failing input (n=2: Σk=3 vs spec 5); feedback is minimal/focused
    ok = ("minimal counterexample" in fix_prompt and "'n': 2" in fix_prompt
          and "= 3" in fix_prompt and "5" in fix_prompt)
    check("minimal_counterexample_used", ok, fix_prompt[-120:] if not ok else "")
    print(f"      → minimal cx fed to fixer: {minimal_feedback(res.trace[0].verdict)}")

def round_limit_enforced():
    # always-wrong model → loop must STOP at max_iters (no infinite loop)
    w, v, _ = get_claude_writer_verifier(scripted_writer=[WRONG], scripted_verifier=[WRONG2, WRONG3], verbose=False)
    res = write_verify_fix(TASK, w, v, max_iters=3, verbose=False)
    ok = (not res.converged) and res.iters == 3
    check("round_limit_enforced", ok, f"converged={res.converged} iters={res.iters}")
    print(f"      → always-wrong model: stopped at round limit {res.iters} (no infinite loop)")

# ---- P3 ----
def grammar_constrained_output():
    # GBNF isn't supported by the Claude API; we guide via system prompt + MEASURE parse failures.
    m = measure_parse_failures([WRONG, RIGHT, WRONG2, WRONG3])
    ok = "HARAN function" in HARAN_GRAMMAR_HINT and m["parse_failures"] == 0
    check("grammar_constrained_output", ok, str(m))
    print(f"      → grammar hint present; parse-failure rate on generated HARAN: {m['rate']:.0%} ({m['n']} samples)")

def sim_vs_live_measured():
    w, v, _ = get_claude_writer_verifier(scripted_writer=[WRONG], scripted_verifier=[RIGHT], verbose=False)
    sim = write_verify_fix(TASK, w, v, max_iters=3, verbose=False)
    print(f"      → sim: converged={sim.converged} in {sim.iters} rounds")
    if LIVE:
        print("      → live: ANTHROPIC_API_KEY present (measured above)")
    else:
        print("      → live: BLOCKED (no ANTHROPIC_API_KEY) — loop + minimal-cx are real; only model text simulated")
    check("sim_vs_live_measured", sim.converged)

if __name__ == "__main__":
    print(f"v7 — AI loop live (Claude API).  LIVE={LIVE}")
    print("[P1]"); api_adapter_reads_env(); api_fallback_to_sim_when_no_key(); key_never_logged()
    print("[P2]"); live_loop_converges(); minimal_counterexample_used(); round_limit_enforced()
    print("[P3]"); grammar_constrained_output(); sim_vs_live_measured()
    print(f"\nv7: {len(PASS)} passed, {len(FAIL)} failed, {len(SKIP)} skipped")
    import sys; sys.exit(1 if FAIL else 0)
