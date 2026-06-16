"""v12 tests (L1-L3) — FnCall + recursion codegen. Run: python3 test_v12.py

L1 FnCall: a function calling other functions → whole call graph emitted + native == interpreter.
L2 Recur:  linear recurrence (Fibonacci) → companion-matrix O(log n) [orders of magnitude];
           tail recursion → loop [C-grade]; general recursion → honest C recursion [C-grade].
L3 Integrate: one dispatcher grades each function honestly (companion/fold = orders-of-magnitude;
           tail/general/straight-line = C-grade) and all compile + match the interpreter.
"""
import sys

from haran_parser import parse
import haran_recur as R
import haran_eval

PASS, FAIL, SKIP = [], [], []


def check(n, c, d=""):
    (PASS if c else FAIL).append(n)
    print(f"  [{'PASS' if c else 'FAIL'}] {n}" + (f" — {d}" if d and not c else ""))


def skip(n, w):
    SKIP.append(n)
    print(f"  [SKIP] {n} — {w}")


def _fn(src):
    return parse(src).items[0]


def _tab(src):
    return {it.name: it for it in parse(src).items}


def _interp(fn, *args, tab=None):
    return haran_eval.Interp(tab or {fn.name: fn}, max_steps=2_000_000).call_fn(fn, list(args))


FIB = "fn fib(n: Nat) -> Nat effects pure { match n { 0 => 0  1 => 1  _ => fib(n-1) + fib(n-2) } }"
SUMTO = "fn sumto(n: Nat, acc: Nat) -> Nat effects pure { match n { 0 => acc  _ => sumto(n-1, acc+n) } }"
FACT = "fn fact(n: Nat) -> Nat effects pure { match n { 0 => 1  _ => n * fact(n-1) } }"
MULTI = ("fn sq(x: Nat) -> Nat effects pure { x*x }\n"
         "fn f(n: Nat) -> Nat effects pure { sq(n) + sq(n+1) }")
PIPE = ("fn dbl(x: Nat) -> Nat effects pure { 2*x }\n"
        "fn inc(x: Nat) -> Nat effects pure { x+1 }\n"
        "fn g(n: Nat) -> Nat effects pure { dbl(inc(n)) + inc(dbl(n)) }")


# ---- L1: FnCall ----
def codegen_multi_fn():
    if not R.cc_available():
        skip("codegen_multi_fn", "no C compiler"); return
    tab = _tab(MULTI)
    p = R.compile_program("f", tab)
    ok = p.ok and all(R.run_native(p.binary, n) == _interp(tab["f"], n, tab=tab) for n in (0, 3, 7, 50))
    check("codegen_multi_fn", ok, p.detail)
    print(f"      → f calls sq twice; whole call graph emitted; f(7)={R.run_native(p.binary,7)} (=49+64=113)")


def codegen_call_chain():
    if not R.cc_available():
        skip("codegen_call_chain", "no C compiler"); return
    tab = _tab(PIPE)
    p = R.compile_program("g", tab)
    # g reaches dbl + inc; both must be emitted (prototypes + defs)
    reached = "dbl" in p.c_src and "inc" in p.c_src
    ok = p.ok and reached and all(R.run_native(p.binary, n) == _interp(tab["g"], n, tab=tab) for n in (0, 5, 9))
    check("codegen_call_chain", ok, p.detail)
    print(f"      → call graph g→{{dbl,inc}} emitted together; g(5)={R.run_native(p.binary,5)} native==interp")


# ---- L2: linear recurrence → companion (orders of magnitude) ----
def companion_exact_matches_interp():
    if not R.cc_available():
        skip("companion_exact_matches_interp", "no C compiler"); return
    fib = _fn(FIB)
    comp = R.detect_companion(fib)
    src = R.emit_companion_program(fib, comp)
    import tempfile, os, subprocess
    d = tempfile.mkdtemp(); cp, bp = os.path.join(d, "f.c"), os.path.join(d, "f")
    open(cp, "w").write(src)
    rc = subprocess.run(["cc", "-O2", cp, "-o", bp], capture_output=True, text=True).returncode
    small = all(R.run_native(bp, n) == _interp(fib, n) for n in range(0, 24))   # exponential interp ok to n~23
    known = R.run_native(bp, 90) == 2880067194370816120                          # exact F(90), fits i64
    ok = comp.c == [1, 1] and comp.init == [0, 1] and rc == 0 and small and known
    check("companion_exact_matches_interp", ok, f"c={comp.c} init={comp.init}")
    print(f"      → Fibonacci recurrence → companion matrix; exact native fib(90)={R.run_native(bp,90)} (correct)")


def companion_collapse_certified():
    if not R.cc_available():
        skip("companion_collapse_certified", "no C compiler"); return
    fib = _fn(FIB)
    comp = R.detect_companion(fib)
    cert = R.compile_and_run_cert(fib, comp, 50_000_000)   # naive O(n) still finishes at 5e7
    # collapse cert: companion (O(log n)) ≡ naive (O(n)) in F_q at large N
    matched = cert.get("ok") and cert.get("match")
    # O(log n): the 1e18-th term costs ~same as the 1e3-th (flat in n), not 1e15× more
    flat = cert.get("comp_1e18_ns", 9e9) < cert.get("comp_1e3_ns", 0) * 50 + 5000
    check("companion_collapse_certified", matched and flat, str(cert.get("raw")))
    print(f"      → companion ≡ naive mod q at N=5e7 (cert); the 10^18-th term in "
          f"{cert.get('comp_1e18_ns',0):.0f}ns (O(log n) — naive O(N) is impossible).")
    print("        THIS is 'recursion, but orders of magnitude' — the layer-1 collapse (cfinite).")


# ---- L2: tail recursion → loop (C-grade) ----
def tail_recursion_to_loop():
    if not R.cc_available():
        skip("tail_recursion_to_loop", "no C compiler"); return
    sumto = _fn(SUMTO)
    csrc = R.emit_tail_loop_c(sumto)
    body_after_sig = csrc.split("{", 1)[1]
    is_loop = "while (1)" in csrc and "sumto(" not in body_after_sig   # rewritten to a loop, no self-call
    tab = {"sumto": sumto}
    p = R.compile_program("sumto", tab)
    matches = p.ok and all(R.run_native(p.binary, n, 0) == _interp(sumto, n, 0, tab=tab) for n in (0, 5, 50, 100))
    big = R.run_native(p.binary, 1_000_000, 0) == 500000500000   # constant stack: 1e6 deep is fine natively
    ok = is_loop and matches and big and p.grades["sumto"] == "C-grade"
    check("tail_recursion_to_loop", ok, f"loop={is_loop} grades={p.grades}")
    print(f"      → tail recursion → C while-loop (constant stack); sumto(1e6,0)={R.run_native(p.binary,1000000,0)} "
          f"[C-grade Ω(N), not orders of magnitude]")


def general_recursion_honest():
    if not R.cc_available():
        skip("general_recursion_honest", "no C compiler"); return
    fact = _fn(FACT)   # n*fact(n-1): poly coefficient ⇒ NOT C-finite, NOT tail → honest C recursion
    g = R.classify_grade(fact)
    tab = {"fact": fact}
    p = R.compile_program("fact", tab)
    matches = p.ok and all(R.run_native(p.binary, n) == _interp(fact, n, tab=tab) for n in (0, 1, 5, 10, 20))
    ok = R.detect_companion(fact) is None and g.grade == "C-grade" and matches
    check("general_recursion_honest", ok, f"{g.kind}/{g.grade}")
    print(f"      → factorial is poly-coeff (not C-finite) → honest C recursion [{g.grade}]; "
          f"fact(20)={R.run_native(p.binary,20)} native==interp. No fake collapse.")


# ---- L3: integrated dispatch + honest grades ----
def integrated_grades():
    cases = {
        "fib (linear rec)": (FIB, "companion", "orders-of-magnitude"),
        "Σk² (collapsing fold)": ("fn s(n: Nat) -> Nat effects pure { fold k in 1..n { k*k } }",
                                   "fold-closed", "orders-of-magnitude"),
        "k%7 (non-closing fold)": ("fn s(n: Nat) -> Nat effects pure { fold k in 1..n { k%7 } }",
                                    "straight-line", "C-grade"),
        "sumto (tail rec)": (SUMTO, "tail-loop", "C-grade"),
        "fact (general rec)": (FACT, "general-recursion", "C-grade"),
    }
    allok = True
    print("      grade table (v9 fold-collapse + v12 recursion, integrated):")
    for label, (src, exp_kind, exp_grade) in cases.items():
        g = R.classify_grade(_fn(src))
        ok = g.kind == exp_kind and g.grade == exp_grade
        allok = allok and ok
        print(f"        {label:26} → {g.kind:18} [{g.grade}]" + ("" if ok else f"  EXPECTED {exp_kind}/{exp_grade}"))
    check("integrated_grades", allok, "kind/grade per function")


def integrated_all_native_match():
    if not R.cc_available():
        skip("integrated_all_native_match", "no C compiler"); return
    # every compilable function in the integrated set runs natively and equals the interpreter (small n)
    singles = {"fib": (FIB, [0, 5, 10, 20]), "fact": (FACT, [0, 5, 10]),
               "loop": ("fn s(n: Nat) -> Nat effects pure { fold k in 1..n { k*k } }", [1, 10, 100])}
    allok = True
    for nm, (src, ns) in singles.items():
        fn = _fn(src); tab = {fn.name: fn}
        p = R.compile_program(fn.name, tab)
        if not p.ok:
            allok = False; continue
        for n in ns:
            if R.run_native(p.binary, n) != _interp(fn, n, tab=tab):
                allok = False
    # multi-fn too
    tab = _tab(MULTI); p = R.compile_program("f", tab)
    allok = allok and p.ok and R.run_native(p.binary, 9) == _interp(tab["f"], 9, tab=tab)
    check("integrated_all_native_match", allok, "all dispatched paths native == interpreter")


if __name__ == "__main__":
    print("v12 — FnCall + recursion codegen")
    print("[L1 FnCall]"); codegen_multi_fn(); codegen_call_chain()
    print("[L2 Recur — linear→companion (orders of magnitude)]")
    companion_exact_matches_interp(); companion_collapse_certified()
    print("[L2 Recur — tail→loop / general (C-grade)]")
    tail_recursion_to_loop(); general_recursion_honest()
    print("[L3 integrate]"); integrated_grades(); integrated_all_native_match()
    print(f"\nv12: {len(PASS)} passed, {len(FAIL)} failed, {len(SKIP)} skipped")
    sys.exit(1 if FAIL else 0)
