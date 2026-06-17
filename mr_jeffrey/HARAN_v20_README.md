# HARAN v20 — security + concurrency + memory + termination (the missed bug classes)

Type B (v16–17) caught correctness/crash/performance bugs. v20 fills the classes it **could not** see —
injection, side-channels, races, memory errors, multi-bug, non-termination — each with a research-proven
technique and an **honest label** (SOUND / UNDER-APPROX / SOUND-FOR-TRACE / HEURISTIC). "All bugs zero"
is impossible (Rice); the goal is **cover every catchable class + do each maximally**, and state the rest.

## Bug-class coverage
| class | technique | label | scope / honesty |
|---|---|---|---|
| **injection** (SQL/cmd/path) | taint/IFDS over PDG + Z3 path refine | **SOUND** | modulo aliasing / call-graph |
| **constant-time** (side-channel) ★flagship | relational 2-hypersafety + Z3 self-composition + QIF | **SOUND** | timing/branch/data-access; Spectre/cache DEFER |
| **data race** | vector clocks (happens-before) | **SOUND-FOR-TRACE** | dynamic — the run's race is real; predictive WCP DEFER |
| **use-after-free** | Incorrectness Separation Logic | **UNDER-APPROX** | false-positives 0; incomplete |
| **double-free** | Incorrectness Separation Logic | **UNDER-APPROX** | false-positives 0; incomplete |
| **multi-bug** | MaxSAT minimal diagnosis (Z3) | **SOUND** | min #faults + suspects (SBFL assumes one) |
| **termination** | ranking-function synthesis (Z3 ∃coef.∀state) | **SOUND / UNKNOWN** | linear; non-linear/lexicographic DEFER |

## Highlights
- **P1 taint**: param/source → sink without a sanitizer is an injection; Z3 prunes dead-guard paths
  (`if False:` → infeasible). SQL/command injection located with the source→sink path.
- **P2 constant-time (flagship)**: a relational proof — `if sk - sk == 0` *uses* the secret but its
  *outcome* is secret-independent, so Z3 confirms **not** a leak (naive taint false-positives; the
  2-safety proof does not). Detects secret branch (timing), secret index (cache), secret divisor; QIF
  upper-bounds the leak in bits. **SonarQube can't do it; an LLM can't *prove* it** — it's HARAN's Z3 shape.
- **P3 vector clocks**: locks / messages create happens-before; concurrent same-var accesses with ≥1 write
  = race. Synchronized → 0, unsynchronized → race.
- **P4 ISL**: under-approximate heap tracking → UAF / double-free with **false-positives 0** (every report
  is real). HARAN own/& (RAII) make owned values UAF-free by construction; this targets C manual memory.
- **P5 MaxSAT + ranking**: minimal multi-bug diagnosis (two independent wrong outputs → 2 bugs); linear
  ranking functions prove loop termination, else honest UNKNOWN.
- **P6 integration**: `analyze_v20` routes by language/trace and tags every finding with its class +
  honest label; confidence kinds are never mixed (`mixed()==False`).

## NOT covered (stated honestly — the point of v20's discipline)
- **access-control / business-logic**: need a policy/spec — Rice-undecidable, HEURISTIC at best → not done.
- **Spectre / cache / micro-architectural**: out of the timing/branch/access model → DEFER.
- **predictive races** (WCP/M2, unobserved interleavings): dynamic-only here → DEFER.
- **CVE matching**: a database lookup, not mathematics → not done.
- **aliasing precision, non-Python/C frontends for taint/PDG**: limits, stated.

## Discipline
- **No mirage**: zero homology/TDA/fluid/quantum/relativity — only measured techniques (taint/IFDS,
  relational 2-safety/QIF, vector clocks, ISL, MaxSAT, ranking functions).
- **Honest labels**: SOUND (proof) vs UNDER-APPROX (FP=0, may miss) vs SOUND-FOR-TRACE (dynamic) vs
  HEURISTIC/NOT-DONE — never conflated.
- Existing suite stays green; per-class confidences un-mixed.

## One line
**v20 covers the bug classes Type B missed — injection, constant-time (flagship, a relational proof),
races, UAF/double-free (FP=0), multi-bug, termination — each with a real technique and an honest label,
and it states plainly the classes that need a spec or are out of model.**
