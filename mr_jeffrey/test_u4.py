"""STAGE U4 tests (v6) — approximation routing + exact-required rejection. Run: python3 test_u4.py"""
from approx_router import route, routing_ratio
PASS, FAIL = [], []
def check(n, c, d=""):
    (PASS if c else FAIL).append(n); print(f"  [{'PASS' if c else 'FAIL'}] {n}" + (f" — {d}" if d and not c else ""))

def approx_ok_routed():
    r1 = route("frequency_recovery", "spectral")
    r2 = route("distinct_users", "monitoring")
    ok = r1.decision == "APPROX" and "Prony" in r1.engine and r2.decision == "APPROX"
    check("approx_ok_routed", ok, f"{r1}; {r2}")
    print(f"      → {r1}")
    print(f"      → {r2}")

def exact_required_rejected():
    pay = route("compute_total", "payment")
    crypto = route("key_derive", "crypto")
    unknown = route("mystery", "weird_domain")
    ok = (pay.decision == "REJECTED-EXACT" and crypto.decision == "REJECTED-EXACT"
          and unknown.decision == "REJECTED-EXACT")
    check("exact_required_rejected", ok, f"pay={pay.decision} crypto={crypto.decision} unknown={unknown.decision}")
    print(f"      → payment: {pay.decision} ({pay.reason})")
    print(f"      → unknown domain: {unknown.decision} (conservative default = EXACT)")

def routing_ratio_measured():
    corpus = [("freq_recovery", "spectral"), ("sparse_recover", "sensing"), ("distinct_count", "monitoring"),
              ("quantile", "analytics"), ("payment_total", "payment"), ("key_derive", "crypto"),
              ("ledger_sum", "ledger"), ("mystery", "unknown_x")]
    rr = routing_ratio(corpus)
    print(f"\n      routing: {rr.approx_pct()}% APPROX (authorized) vs {rr.exact_pct()}% REJECTED-EXACT")
    for r in rr.rows:
        print(f"        · {r.task:16s} {r.domain:12s} {r.decision}")
    ok = rr.approx_pct() > 0 and rr.exact_pct() > 0 and rr.approx_pct() + rr.exact_pct() == 100
    check("routing_ratio_measured", ok)

if __name__ == "__main__":
    print("STAGE U4 — approximation routing + exact rejection")
    approx_ok_routed()
    exact_required_rejected()
    routing_ratio_measured()
    print(f"\nStage U4: {len(PASS)} passed, {len(FAIL)} failed")
    import sys; sys.exit(1 if FAIL else 0)
