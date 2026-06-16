"""STAGE 2 tests — strengthened verification. Run: python3 test_stage2.py"""
import time
from verify_strong import StrongVerifier, infer_properties, shrink

PASS, FAIL = [], []
def check(name, cond, detail=""):
    (PASS if cond else FAIL).append(name)
    print(f"  [{'PASS' if cond else 'FAIL'}] {name}" + (f" — {detail}" if detail and not cond else ""))


def float_bugs_caught():
    # average that divides without guarding empty + wrong on floats; reference is correct mean.
    def buggy_mean(xs):
        return sum(xs) / len(xs)  # crashes on []
    def ref_mean(xs):
        return sum(xs) / len(xs) if xs else 0.0
    V = StrongVerifier(timeout=1.0)
    rep = V.verify(buggy_mean, ["list_float"], reference=ref_mean)
    check("float_bugs_caught", rep.verdict == "REFUTED", str(rep))


def performance_guard_no_hang():
    # a function that infinite-loops on inputs > 5: the verifier must NOT hang — it times out.
    def hangs(n):
        while n > 5:
            pass
        return n
    V = StrongVerifier(timeout=0.3, fuzz_per_arg=2)
    t0 = time.time()
    rep = V.verify(hangs, ["nonneg_int"], reference=lambda n: n)
    elapsed = time.time() - t0
    timed_out = any("TIMEOUT" in (c.detail or "") for c in rep.counterexamples)
    check("performance_guard_no_hang", rep.verdict == "REFUTED" and timed_out and elapsed < 30,
          f"verdict={rep.verdict} elapsed={elapsed:.1f}s timed_out={timed_out}")


def auto_property_sort():
    # buggy sort (drops duplicates via set) — no reference given; inferred properties catch it.
    def buggy_sort(xs):
        return sorted(set(xs))  # loses duplicates → not a permutation
    V = StrongVerifier(timeout=1.0)
    rep = V.verify(buggy_sort, ["list_int"], func_name="sort_it")  # properties auto-inferred
    caught = rep.verdict == "REFUTED" and any(c.kind == "property_violated" for c in rep.counterexamples)
    check("auto_property_sort", caught, str(rep))
    # a correct sort passes the inferred properties.
    rep2 = V.verify(sorted, ["list_int"], func_name="sort_it")
    check("auto_property_correct_sort_passes", rep2.verdict == "VERIFIED", str(rep2))


def shrink_minimizes_counterexample():
    # f is wrong whenever the list contains a negative; shrink must reduce to a tiny failing input.
    def f(xs):
        return [x for x in xs if x >= 0]  # silently drops negatives
    def ref(xs):
        return list(xs)
    # direct shrink test: a big failing input → minimal still-failing input.
    big = ([5, 9, 3, -7, 2, 8, 1, 6, 4],)
    still = lambda inp: f(inp[0]) != ref(inp[0])
    mini = shrink(big, still)
    check("shrink_minimizes_counterexample",
          still(mini) and len(mini[0]) < len(big[0]) and len(mini[0]) <= 2,
          f"minimal = {mini[0]} (from len {len(big[0])})")


def sandbox_catches_side_effects():
    # a function that opens a file is flagged (a verified function must be pure / no I/O).
    def sneaky(n):
        open("/tmp/mr_sneaky.txt", "w")
        return n
    V = StrongVerifier(timeout=1.0, sandbox=True)
    rep = V.verify(sneaky, ["nonneg_int"], reference=lambda n: n)
    check("sandbox_catches_side_effects",
          rep.verdict == "REFUTED" and any(c.kind == "side_effect" for c in rep.counterexamples), str(rep))


def more_types_supported():
    # dict / tuple / 2d-list generators work end to end.
    V = StrongVerifier(timeout=1.0, fuzz_per_arg=10)
    r1 = V.verify(lambda d: len(d), ["dict"], reference=len)
    r2 = V.verify(lambda t: len(t), ["tuple"], reference=len)
    r3 = V.verify(lambda m: sum(len(r) for r in m), ["list2d"], reference=lambda m: sum(len(r) for r in m))
    check("more_types_supported", all(r.verdict == "VERIFIED" for r in (r1, r2, r3)),
          f"{r1.verdict},{r2.verdict},{r3.verdict}")


if __name__ == "__main__":
    print("STAGE 2 — strengthened verification")
    float_bugs_caught()
    performance_guard_no_hang()
    auto_property_sort()
    shrink_minimizes_counterexample()
    sandbox_catches_side_effects()
    more_types_supported()
    print(f"\nStage 2: {len(PASS)} passed, {len(FAIL)} failed")
    import sys
    sys.exit(1 if FAIL else 0)
