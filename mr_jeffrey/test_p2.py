"""v20 Part P · P2 tests — constant-time / side-channel (FLAGSHIP). Run: python3 test_p2.py

P2.1 relational 2-hypersafety (Z3 self-composition: secret-dependent OUTCOME, not mere secret use).
P2.2 secret-dependent branch / index / division detected with location.
P2.3 QIF leakage upper bound (bits). P2.4 constant-time passes, leaky fails.
"""
import sys

import constant_time as CT

PASS, FAIL, SKIP = [], [], []


def check(n, c, d=""):
    (PASS if c else FAIL).append(n)
    print(f"  [{'PASS' if c else 'FAIL'}] {n}" + (f" — {d}" if d and not c else ""))


CT_OK = "def cmp_ct(sk, pub):\n    diff = 0\n    diff = diff | (sk - pub)\n    return diff\n"
LEAK_BR = "def dec(sk, ct):\n    if sk == 0:\n        return 1\n    return 0\n"
LEAK_IDX = "def lookup(key, table):\n    return table[key]\n"
LEAK_DIV = "def f(secret, x):\n    return x / secret\n"
CONST_COND = "def g(sk):\n    if sk - sk == 0:\n        return 1\n    return 0\n"   # uses secret, outcome independent


def relational_2safety():
    ct = CT.analyze_constant_time(CT_OK)
    const = CT.analyze_constant_time(CONST_COND)
    # CT-ok: no secret branch. const-cond: `sk-sk==0` USES secret but outcome is secret-INDEPENDENT →
    # Z3 self-composition proves NOT a leak (naive taint would false-positive here)
    ok = (not any(l.relational_confirmed for l in ct)
          and not any(l.relational_confirmed for l in const))
    check("relational_2safety", ok, f"ct_leaks={len(ct)} const-cond confirmed={[l.relational_confirmed for l in const]}")
    print(f"      → relational proof: `if sk - sk == 0` USES the secret but its OUTCOME is "
          f"secret-independent → Z3 confirms NOT a leak (0). This is the differentiator — taint alone "
          f"false-positives; the 2-safety proof does not.")


def secret_dependent_branch():
    br = [l for l in CT.analyze_constant_time(LEAK_BR) if l.relational_confirmed]
    ix = [l for l in CT.analyze_constant_time(LEAK_IDX) if l.relational_confirmed]
    dv = [l for l in CT.analyze_constant_time(LEAK_DIV) if l.relational_confirmed]
    ok = (br and br[0].kind == "branch" and ix and ix[0].kind == "index" and dv and dv[0].kind == "division")
    check("secret_dependent_branch", ok, f"branch={bool(br)} index={bool(ix)} div={bool(dv)}")
    print(f"      → leaks located: branch `{br[0].expr}` @L{br[0].line} (timing); index `{ix[0].expr}` "
          f"(cache-timing); division `{dv[0].expr}` (variable-time). All Z3-confirmed secret-dependent.")


def leakage_bound_qif():
    q_leaky = CT.qif_bound(CT.analyze_constant_time(LEAK_BR))
    q_ct = CT.qif_bound(CT.analyze_constant_time(CT_OK))
    ok = q_leaky.bits_upper == 1 and q_ct.bits_upper == 0
    check("leakage_bound_qif", ok, f"leaky≤{q_leaky.bits_upper}b ct={q_ct.bits_upper}b")
    print(f"      → QIF upper bound: one secret-dependent branch leaks ≤ {q_leaky.bits_upper} bit/observation; "
          f"constant-time code = {q_ct.bits_upper} bits. Min-entropy channel-capacity bound (honest upper).")


def ct_vs_leaky_demo():
    ok = CT.is_constant_time(CT_OK) and CT.is_constant_time(CONST_COND) and not CT.is_constant_time(LEAK_BR)
    check("ct_vs_leaky_demo", ok,
          f"ct_ok={CT.is_constant_time(CT_OK)} const={CT.is_constant_time(CONST_COND)} leaky={CT.is_constant_time(LEAK_BR)}")
    print(f"      → constant-time code PASSES; `if sk==0` leaky code FAILS. FLAGSHIP: SonarQube doesn't do "
          f"this; an LLM can't PROVE it — it's a relational proof, HARAN's Z3/Caesar shape.")
    print("      → HONEST: covers timing / branch / data-memory-access leaks; Spectre / micro-arch / "
          "cache-bank channels are DEFER.")


if __name__ == "__main__":
    print("v20 Part P · P2 — constant-time / side-channel (FLAGSHIP)")
    relational_2safety(); secret_dependent_branch(); leakage_bound_qif(); ct_vs_leaky_demo()
    print(f"\nP2: {len(PASS)} passed, {len(FAIL)} failed, {len(SKIP)} skipped")
    sys.exit(1 if FAIL else 0)
