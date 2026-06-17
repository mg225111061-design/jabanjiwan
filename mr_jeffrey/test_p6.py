"""v20 Part P · P6 tests — integration + per-class verdict + honesty labels. Run: python3 test_p6.py

P6.1 routing (Python→injection/constant-time, C→UAF, trace→race).
P6.2 per-class verdict (class + label). P6.3 honesty-label table. P6.4 multi-class demo, labels not mixed.
"""
import sys

import security_suite as S
import vector_clock as VCK

PASS, FAIL, SKIP = [], [], []
E = VCK.Event


def check(n, c, d=""):
    (PASS if c else FAIL).append(n)
    print(f"  [{'PASS' if c else 'FAIL'}] {n}" + (f" — {d}" if d and not c else ""))


PY = ("def handler(sk, u):\n    if sk == 0:\n        return 1\n    q = \"SELECT \" + u\n"
      "    execute(q)\n    return 0\n")
C = "int f(){\n  int* p = malloc(8);\n  free(p);\n  int v = *p;\n  return v;\n}\n"
TRACE = [E(0, "write", "x"), E(1, "write", "x")]


def integrated_routing():
    res = S.analyze_v20(python=PY, c=C, trace=TRACE, secret_params={"sk"})
    classes = {f.bug_class for f in res}
    ok = {"injection", "constant-time", "use-after-free", "race"} <= classes
    check("integrated_routing", ok, f"classes={sorted(classes)}")
    print(f"      → one codebase routed: Python → injection + constant-time; C → use-after-free; "
          f"trace → race. Classes found: {sorted(classes)}.")


def per_class_verdict():
    res = S.analyze_v20(python=PY, c=C, trace=TRACE, secret_params={"sk"})
    by = {f.bug_class: f for f in res}
    ok = (by["injection"].label == "SOUND" and by["constant-time"].label == "SOUND"
          and by["use-after-free"].label == "UNDER-APPROX" and by["race"].label == "SOUND-FOR-TRACE")
    check("per_class_verdict", ok, f"labels={ {k: v.label for k, v in by.items()} }")
    for f in res:
        print(f"      → [{f.label:15}] {f.bug_class:14} @{f.location}  ({f.technique})")


def honesty_label_table():
    t = S.honesty_table()
    cov, nc = t["covered"], t["not_covered"]
    labels = {v["label"] for v in cov.values()}
    ok = (len(cov) == 7 and len(nc) >= 5 and {"SOUND", "UNDER-APPROX", "SOUND-FOR-TRACE"} <= labels
          and "access-control" in nc and "spectre / cache" in nc)
    check("honesty_label_table", ok, f"covered={len(cov)} not_covered={len(nc)}")
    print(f"      → COVERED ({len(cov)}): " + ", ".join(f"{k}={v['label']}" for k, v in cov.items()))
    print(f"      → NOT COVERED ({len(nc)}): " + ", ".join(nc) + " — needs-spec(Rice)/micro-arch/"
          f"predictive/DB, stated not pretended.")


def multi_class_demo():
    res = S.analyze_v20(python=PY, c=C, trace=TRACE, secret_params={"sk"})
    # SOUND label must not be mixed onto a HEURISTIC/under-approx class and vice-versa
    not_mixed = not S.mixed(res)
    ok = len(res) >= 4 and not_mixed
    check("multi_class_demo", ok, f"findings={len(res)} mixed={S.mixed(res)}")
    print(f"      → {len(res)} bugs across 4 classes, each with its own honest label; mixed()={S.mixed(res)} "
          f"(confidences NOT mixed — proof vs FP=0 vs dynamic kept distinct).")


if __name__ == "__main__":
    print("v20 Part P · P6 — integration + per-class verdict + honesty labels")
    integrated_routing(); per_class_verdict(); honesty_label_table(); multi_class_demo()
    print(f"\nP6: {len(PASS)} passed, {len(FAIL)} failed, {len(SKIP)} skipped")
    sys.exit(1 if FAIL else 0)
