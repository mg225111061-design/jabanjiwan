"""v20 Part P · P3 tests — vector clocks (data-race detection). Run: python3 test_p3.py

P3.1 vector clocks built. P3.2 happens-before / concurrent. P3.3 race detected (concurrent + ≥1 write).
P3.4 synchronized (lock/message) vs unsynchronized demo.
"""
import sys

import vector_clock as VC

PASS, FAIL, SKIP = [], [], []
E = VC.Event


def check(n, c, d=""):
    (PASS if c else FAIL).append(n)
    print(f"  [{'PASS' if c else 'FAIL'}] {n}" + (f" — {d}" if d and not c else ""))


UNSYNC = [E(0, "write", "x"), E(1, "write", "x")]
SYNC = [E(0, "acquire", "L"), E(0, "write", "x"), E(0, "release", "L"),
        E(1, "acquire", "L"), E(1, "write", "x"), E(1, "release", "L")]
MSG = [E(0, "write", "x"), E(0, "send", "ch"), E(1, "recv", "ch"), E(1, "write", "x")]   # ordered by msg
RR = [E(0, "read", "x"), E(1, "read", "x")]


def vector_clock_built():
    _, acc = VC.analyze_trace(UNSYNC, 2)
    # two independent writes → clocks [1,0] and [0,1]
    ok = len(acc) == 2 and acc[0].clock == [1, 0] and acc[1].clock == [0, 1]
    check("vector_clock_built", ok, f"clocks={[a.clock for a in acc]}")
    print(f"      → per-thread vector clocks: T0 write→[1,0], T1 write→[0,1]; join = componentwise max.")


def happens_before_test():
    ok = (VC.happens_before([1, 0], [1, 1]) and not VC.happens_before([1, 0], [0, 1])
          and VC.concurrent([1, 0], [0, 1]) and not VC.concurrent([1, 0], [1, 1]))
    check("happens_before_test", ok)
    print(f"      → V(A)≤V(B) ⇒ A before B; neither ≤ the other ⇒ concurrent. [1,0] ∥ [0,1] concurrent.")


def race_detected():
    races, _ = VC.analyze_trace(UNSYNC, 2)
    rr, _ = VC.analyze_trace(RR, 2)
    ok = len(races) == 1 and races[0].var == "x" and len(rr) == 0
    check("race_detected", ok, f"unsync races={len(races)} read-read={len(rr)}")
    print(f"      → unsynchronized write/write on 'x' → 1 race ({races[0].note}); read/read → 0 "
          f"(a race needs ≥1 write). A race is an ORDERING fault, not a value fault.")


def sync_vs_race_demo():
    sync_races, _ = VC.analyze_trace(SYNC, 2)
    msg_races, _ = VC.analyze_trace(MSG, 2)
    unsync_races, _ = VC.analyze_trace(UNSYNC, 2)
    ok = len(sync_races) == 0 and len(msg_races) == 0 and len(unsync_races) == 1
    check("sync_vs_race_demo", ok, f"lock={len(sync_races)} msg={len(msg_races)} unsync={len(unsync_races)}")
    print(f"      → lock L orders the writes → 0 races; message send/recv orders them → 0 races; "
          f"no synchronization → 1 race.")
    print("      → HONEST: DYNAMIC — the race in the run trace is REAL; races not exercised by the trace "
          "are unseen (predictive WCP/M2 = DEFER).")


if __name__ == "__main__":
    print("v20 Part P · P3 — vector clocks (data-race detection)")
    vector_clock_built(); happens_before_test(); race_detected(); sync_vs_race_demo()
    print(f"\nP3: {len(PASS)} passed, {len(FAIL)} failed, {len(SKIP)} skipped")
    sys.exit(1 if FAIL else 0)
