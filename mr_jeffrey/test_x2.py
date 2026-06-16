"""
STAGE X2 tests — unstructured conquest (approx + PROVEN/TESTED error, kept distinct).
Run: python3 test_x2.py
"""
from approx_lib import (kmv_distinct, approx_quantile_conquest, approx_distinct_conquest,
                        bucketed_quantile, route, conquest_ratio, render_ratio)
from z3_adapter import z3_available

PASS, FAIL, SKIP = [], [], []
def check(name, cond, detail=""):
    (PASS if cond else FAIL).append(name)
    print(f"  [{'PASS' if cond else 'FAIL'}] {name}" + (f" — {detail}" if detail and not cond else ""))
def skip(name, why):
    SKIP.append(name); print(f"  [SKIP] {name} — {why}")


def hll_count_approx_structured():
    # KMV sketch structures an O(distinct)-space exact count into O(k) space, estimate close to truth
    items = list(range(5000)) + [i % 5000 for i in range(10000)]   # 5000 distinct
    est = kmv_distinct(items, k=1024)
    rel = abs(est - 5000) / 5000
    r = approx_distinct_conquest()
    ok = rel < 0.15 and r.kind == "TESTED-BOUND"
    check("hll_count_approx_structured", ok, f"est={est:.0f} rel={rel:.3f} kind={r.kind}")
    print(f"      → KMV distinct≈{est:.0f} (true 5000, rel {rel:.3f}); kind={r.kind} (probabilistic)")


def error_bound_proven_or_tested_distinct():
    q = approx_quantile_conquest()
    d = approx_distinct_conquest()
    if not z3_available():
        skip("error_bound_proven_or_tested_distinct", "Z3 absent → quantile would be TESTED-BOUND only")
        return
    # the two MUST be different kinds — proven (deterministic) vs tested (probabilistic), never relabeled
    ok = q.kind == "PROVEN-BOUND" and d.kind == "TESTED-BOUND" and q.kind != d.kind
    check("error_bound_proven_or_tested_distinct", ok, f"quantile={q.kind}; distinct={d.kind}")
    print(f"      → quantile: {q.kind} ({q.proof})")
    print(f"      → distinct: {d.kind} ({d.proof})")


def exact_required_rejects_approx():
    r = route("payment_total", approx_ok=False, exact_required=True, conquest_fn=approx_quantile_conquest)
    ok = r.kind == "REJECTED-EXACT" and not r.conquered()
    check("exact_required_rejects_approx", ok, str(r))
    print(f"      → payment (exact required): {r.kind} — {r.proof}")


def unstructured_conquest_ratio():
    r = conquest_ratio()
    print("\n" + render_ratio(r))
    proven = sum(1 for x in r.rows if x.kind == "PROVEN-BOUND")
    tested = sum(1 for x in r.rows if x.kind == "TESTED-BOUND")
    rejected = sum(1 for x in r.rows if x.kind == "REJECTED-EXACT")
    if not z3_available():
        skip("unstructured_conquest_ratio", "Z3 absent → no PROVEN-BOUND")
        return
    ok = proven >= 1 and tested >= 1 and rejected >= 1 and proven != tested or proven >= 1
    check("unstructured_conquest_ratio", proven >= 1 and tested >= 1 and rejected >= 1,
          f"proven={proven} tested={tested} rejected={rejected}")


if __name__ == "__main__":
    print("STAGE X2 — unstructured conquest")
    hll_count_approx_structured()
    error_bound_proven_or_tested_distinct()
    exact_required_rejects_approx()
    unstructured_conquest_ratio()
    print(f"\nStage X2: {len(PASS)} passed, {len(FAIL)} failed, {len(SKIP)} skipped")
    import sys
    sys.exit(1 if FAIL else 0)
