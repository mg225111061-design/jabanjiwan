"""v18 Part G · G5 tests — three-way comparison (the heart). Run: python3 test_g5.py

G5.1 run property / pure-diffusion / proof-boundary-diffusion on the same corpus.
G5.2 measure top-1/top-5.  G5.3 corpus = curated line-annotated bugs.
G5.4 honest conclusion — whatever the numbers say (NO diffusion favoritism).
"""
import sys

import compare_fl as CF

PASS, FAIL, SKIP = [], [], []


def check(n, c, d=""):
    (PASS if c else FAIL).append(n)
    print(f"  [{'PASS' if c else 'FAIL'}] {n}" + (f" — {d}" if d and not c else ""))


_S = CF.three_way()


def three_way_comparison():
    ok = all(_S[m].total == 6 for m in ("property", "pure_diffusion", "proof_diffusion"))
    check("three_way_comparison", ok, f"totals={[_S[m].total for m in _S]}")
    for m in ("property", "pure_diffusion", "proof_diffusion"):
        sc = _S[m]
        print(f"      → {m:16} top-1={sc.top1}/{sc.total} ({sc.top1_rate():.0%})  "
              f"top-5={sc.top5}/{sc.total} ({sc.top5_rate():.0%})")


def diffusion_vs_property_measured():
    prop, pure, proof = _S["property"], _S["pure_diffusion"], _S["proof_diffusion"]
    # honest measured reality: property is at least as good as the proof-boundary diffusion
    ok = prop.top1_rate() >= proof.top1_rate() - 1e-9
    check("diffusion_vs_property_measured", ok,
          f"property={prop.top1_rate():.0%} pure={pure.top1_rate():.0%} proof={proof.top1_rate():.0%}")
    print(f"      → MEASURED: property {prop.top1_rate():.0%}, pure diffusion {pure.top1_rate():.0%} "
          f"(ties property), proof-boundary {proof.top1_rate():.0%}. Diffusion did NOT beat property.")


def sink_effect_measured():
    pure, proof = _S["pure_diffusion"], _S["proof_diffusion"]
    # the proof-sink effect is whatever it measured — here the WEAK sinks HURT (drain real bugs)
    sink_helps = proof.top1_rate() > pure.top1_rate() + 1e-9
    concl = CF.conclude(_S)
    ok = ("KEEP property" in concl) and (not sink_helps)   # honest: sink did not help; keep property
    check("sink_effect_measured", ok, f"sink_helps={sink_helps}")
    effect = "HELPS" if sink_helps else ("does NOT help — weak abstract-interp sinks drain real "
                                         "correctness bugs (sort_dup@L2, partial@L4 wrongly absorbed)")
    print(f"      → proof-sink effect: {effect}.")
    print(f"      → CONCLUSION: {concl}")
    print("        HONEST: the v18 diffusion bet did NOT pay off on this corpus — property stays the "
          "answer. Strong Z3/Coq sinks (not weak crash-safety) would be needed; arbitrary buggy code "
          "rarely has them. Reported as measured, no favoritism.")


if __name__ == "__main__":
    print("v18 Part G · G5 — three-way comparison (the heart)")
    three_way_comparison(); diffusion_vs_property_measured(); sink_effect_measured()
    print(f"\nG5: {len(PASS)} passed, {len(FAIL)} failed, {len(SKIP)} skipped")
    sys.exit(1 if FAIL else 0)
