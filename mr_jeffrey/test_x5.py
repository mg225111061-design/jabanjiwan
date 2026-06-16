"""
STAGE X5 tests — HARAN v3 integration + whole-system measurement.  Run: python3 test_x5.py
"""
import haran_v3
from approx_lib import approx_quantile_conquest, approx_distinct_conquest, route
from prove_exact import prove_correctness
from ai_loop import write_verify_fix
from llm_adapters import get_writer_verifier
from haran_parser import parse
from z3_adapter import z3_available

PASS, FAIL, SKIP = [], [], []
def check(name, cond, detail=""):
    (PASS if cond else FAIL).append(name)
    print(f"  [{'PASS' if cond else 'FAIL'}] {name}" + (f" — {detail}" if detail and not cond else ""))
def skip(name, why):
    SKIP.append(name); print(f"  [SKIP] {name} — {why}")


def demo_unstructured_conquered():
    q = approx_quantile_conquest()
    d = approx_distinct_conquest()
    if not z3_available():
        skip("demo_unstructured_conquered", "Z3 absent → quantile would be TESTED-BOUND")
        return
    # conquest = approx + PROVEN error bound; distinct stays TESTED (not conquered); kept distinct
    ok = q.conquered() and q.kind == "PROVEN-BOUND" and (not d.conquered()) and d.kind == "TESTED-BOUND"
    check("demo_unstructured_conquered", ok, f"quantile={q.kind}; distinct={d.kind}")


def demo_exact_proven():
    ftab = {f.name: f for f in parse(haran_v3.SORT).fns()}
    s = prove_correctness(parse(haran_v3.SORT).get("sort"), ftab)
    # payment exact → approximation refused (not conquered)
    p = route("payment", approx_ok=False, exact_required=True, conquest_fn=approx_quantile_conquest)
    ok = s.proven() and p.kind == "REJECTED-EXACT"
    check("demo_exact_proven", ok, f"sort={s.tier}; payment={p.kind}")
    print(f"      → sort: {s.tier}; payment(exact): {p.kind} (approx refused — exactness preserved)")


def demo_ai_loop():
    w, v, mode = get_writer_verifier(prefer="qwen3", verbose=False,
                                     scripted_writer=[haran_v3._WRONG], scripted_verifier=[haran_v3._RIGHT])
    res = write_verify_fix(haran_v3.TASK, w, v, verbose=False)
    ok = res.converged and res.iters >= 2
    check("demo_ai_loop", ok, f"mode={mode} converged={res.converged} iters={res.iters}")
    print(f"      → AI loop ({mode}): converged in {res.iters} iters")


def whole_system_measure():
    m = haran_v3.measure(verbose=True)
    ok = ("conquest" in m and m["exact_proven_pct"] >= 60 and m["ai_converged"])
    check("whole_system_measure", ok, str({k: m.get(k) for k in ("exact_proven_pct", "ai_iters", "ai_mode")}))


if __name__ == "__main__":
    print("STAGE X5 — HARAN v3 integration")
    demo_unstructured_conquered()
    demo_exact_proven()
    demo_ai_loop()
    whole_system_measure()
    print(f"\nStage X5: {len(PASS)} passed, {len(FAIL)} failed, {len(SKIP)} skipped")
    import sys
    sys.exit(1 if FAIL else 0)
