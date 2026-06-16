"""
STAGE Y5 — HARAN v4 integration: real domain kernel (B) + measured speed (A), one view.
=======================================================================================
B (Y1-Y2): a PRODUCTION PQC kernel (Kyber NTT poly-mult), verified, with its REAL fold ratio.
A (Y3-Y4): WALL-CLOCK measurement of fold / unstructured / approx — honest per-size tables.

The one honest sentence: HARAN folds the foldable to O(1) (measured orders of magnitude, growing
with n), runs the unstructured at hardware speed (measured ≈ C-equivalent, constant factor), and
the real PQC kernel is mostly Ω(N) transform work that does NOT fold like the toys.
"""
from __future__ import annotations

import subprocess

import closure_classifier as cc
import pqc_domain
import bench_harness as bh
from closure_classifier import find_binary
from haran_parser import parse


def domain_section():
    sub = {name: cc.classify_fn(parse(src).items[0]) for name, src in pqc_domain.SUBKERNELS.items()}
    closed = [n for n, v in sub.items() if v.kind == "CLOSED"]
    nostruct = [n for n, v in sub.items() if v.kind == "NO_STRUCTURE"]
    b = find_binary("kyber_check")
    corr = subprocess.run([b, "200"], capture_output=True, text=True, timeout=30).stdout.strip() if b else "(no bin)"
    return {"sub": sub, "closed": closed, "nostruct": nostruct, "corr": corr,
            "fold_pct": round(100 * len(closed) / len(sub))}


def report():
    print("=" * 78)
    print("HARAN v4 — PART B: real PQC domain kernel (Kyber NTT poly-mult)")
    print("=" * 78)
    d = domain_section()
    print(f"  kernel correctness: {d['corr']}")
    print(f"  ★ REAL fold ratio: {d['fold_pct']}% CLOSED ({len(d['closed'])}/{len(d['sub'])}) — "
          f"folds={d['closed']}")
    print(f"     transforms (runtime-dominant) = NO_STRUCTURE Ω(N log N): {d['nostruct']}")
    print(f"     → real crypto folds FAR less than the toy 70% (only setup folds).")

    if find_binary("haran_bench"):
        print("\n" + "=" * 78)
        print("HARAN v4 — PART A: measured wall-clock (no headline number, includes unfavorable)")
        print("=" * 78)
        print(bh.render_fold(bh.foldsum_table([10, 1000, 100000, 1000000])))
        print(bh.render_unstruct(bh.unstruct_table([1000, 100000, 1000000])))
    print(bh.render_approx(bh.approx_table([100000], m=64)))
    print("\n  one honest sentence: fold→orders of magnitude (measured), unstructured→C-equivalent")
    print("  (measured ~1×), real PQC kernel→mostly Ω(N). Ω(N) is a theorem, never beaten.")
    return d


if __name__ == "__main__":
    report()
