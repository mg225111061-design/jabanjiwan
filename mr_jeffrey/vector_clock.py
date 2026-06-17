"""
HARAN v20 Part P · STAGE P3 — vector clocks (data-race detection).
=================================================================
A data race = two accesses to the same variable that are CONCURRENT (no happens-before order) with at
least one write. We compute happens-before with vector clocks over an execution TRACE: each thread keeps
a per-thread counter vector; an event bumps the thread's own slot; a lock release publishes its clock and
a later acquire joins it (componentwise max) — that is what creates ordering. Two same-variable accesses
whose clocks are incomparable, on different threads, with ≥1 write, are a race.

Honest: this is DYNAMIC — the race in the trace we ran is REAL (sound for that execution); races not
exercised by the trace are not seen (predictive WCP/M2 are DEFER). A race is an ORDERING fault, not a
value fault — exactly the class properties/diffusion never caught.
"""
from __future__ import annotations

from dataclasses import dataclass, field
from typing import Dict, List, Optional, Tuple


@dataclass
class Event:
    thread: int
    kind: str        # read | write | acquire | release | send | recv (chan in `var`)
    var: str


def _zero(n):
    return [0] * n


def _leq(a, b):
    return all(x <= y for x, y in zip(a, b))


def _join(a, b):
    return [max(x, y) for x, y in zip(a, b)]


def happens_before(va, vb) -> bool:
    return _leq(va, vb) and va != vb


def concurrent(va, vb) -> bool:
    return not _leq(va, vb) and not _leq(vb, va)


@dataclass
class Access:
    idx: int
    thread: int
    kind: str
    var: str
    clock: List[int]


@dataclass
class Race:
    var: str
    a: Access
    b: Access
    note: str


def analyze_trace(events: List[Event], n_threads: int) -> Tuple[List[Race], List[Access]]:
    V = [_zero(n_threads) for _ in range(n_threads)]   # per-thread vector clocks
    lock_clock: Dict[str, List[int]] = {}              # last release clock per lock
    chan_clock: Dict[str, List[int]] = {}              # last send clock per channel
    accesses: List[Access] = []
    for i, e in enumerate(events):
        t = e.thread
        if e.kind == "acquire":
            V[t] = _join(V[t], lock_clock.get(e.var, _zero(n_threads)))
        elif e.kind == "recv":
            V[t] = _join(V[t], chan_clock.get(e.var, _zero(n_threads)))
        V[t][t] += 1                                   # the event itself
        if e.kind == "release":
            lock_clock[e.var] = list(V[t])
        elif e.kind == "send":
            chan_clock[e.var] = list(V[t])
        elif e.kind in ("read", "write"):
            accesses.append(Access(i, t, e.kind, e.var, list(V[t])))
    # races: same var, different threads, ≥1 write, concurrent clocks
    races: List[Race] = []
    for i in range(len(accesses)):
        for j in range(i + 1, len(accesses)):
            a, b = accesses[i], accesses[j]
            if a.var != b.var or a.thread == b.thread:
                continue
            if a.kind == "read" and b.kind == "read":
                continue
            if concurrent(a.clock, b.clock):
                races.append(Race(a.var, a, b,
                                  f"concurrent {a.kind}(T{a.thread}) / {b.kind}(T{b.thread}) on '{a.var}'"))
    return races, accesses
