"""
HARAN v22 Part S — agentic coding pipeline (write → verify → fix → optimize).
=============================================================================
The product: Claude *writes* code; HARAN *verifies* it against its spec and, when wrong, hands back a
concrete counterexample to fix. This module grows stage by stage:

  • S2  write_verify(request, key)        — Claude proposes → HARAN verifies (VERIFIED / counterexample)
  • S3  write_verify_fix(...)             — the loop: feed the counterexample back until proven   (the heart)
  • S4  + fold optimization              — closed-form speedup on the proven result
  • S5  modes (normal / extended)        — reuse v21
  • S6  Type A (spec-embedded)           — verify against the embedded spec
  • S7  agentic_code(request, mode, key) — the integrated entry point + honest measurement

HONESTY (v22 bar):
  ★ Verification is **against the spec** (`ensures`), NOT against intent. "VERIFIED" means the code meets
    the stated spec — it does NOT mean Claude guessed what you meant. Intent gaps are reported, not hidden.
  ★ The model is swappable; the key is level-1 (entered per call, stored nowhere — see claude_agent).
  ★ Mock provenance is always labeled (source='mock-sim'); a mock is never reported as 'live'.
"""
from __future__ import annotations

import time
from dataclasses import dataclass, field
from typing import Callable, List, Optional, Tuple

import ai_loop
import claude_agent as CA
import closure_classifier as CC
import fusion
import prove_exact as PE
from haran_parser import parse


@dataclass
class WriteVerifyResult:
    request: str
    code: str
    source: str               # "mock-sim" | "claude-live"  (honest provenance)
    live: bool
    status: str               # VERIFIED | FAILED | UNKNOWN | PARSE_ERROR
    ok: bool                  # True iff VERIFIED against the spec
    counterexample: Optional[dict]
    feedback: str             # minimal, targeted feedback (concrete failing input + mismatch)


def write_verify(request: str, api_key: Optional[str] = None, *,
                 model: str = CA.DEFAULT_MODEL, system: Optional[str] = None,
                 mock_response: Optional[str] = None) -> WriteVerifyResult:
    """S2: Claude proposes code for `request`; HARAN verifies it against its `ensures` spec.

    Returns VERIFIED, or a concrete counterexample (the smallest failing input + the impl/spec
    mismatch) — which S3 will feed back to drive a fix. With no `api_key` this runs the labeled mock
    (deterministic), so the whole write→verify path is testable with zero network/secrets."""
    gen = CA.claude_generate(request, api_key, model=model, system=system, mock_response=mock_response)
    v = ai_loop.verify_haran(gen.text)
    return WriteVerifyResult(
        request=request, code=gen.text, source=gen.source, live=gen.live,
        status=v.status, ok=v.ok, counterexample=v.counterexample,
        feedback=ai_loop.minimal_feedback(v) if not v.ok else "verified against spec (명세 대비)",
    )


# ---------------------------------------------------------------------------------------------------
# S3 — write → verify → FIX (the heart). Claude writes; HARAN rejects with a concrete counterexample;
# that counterexample is fed back into the next prompt; repeat until PROVEN or the budget runs out.
# The loop mechanics (fix-prompt + minimal counterexample) are reused from v7 (ai_loop.write_verify_fix);
# v22 wires Claude (level-1 key) as the model and adds honest provenance.
#
# ★ The loop and HARAN's counterexamples are REAL. In mock mode the *model text* is a scripted
#   SIMULATION (wrong→fixed) — never a fake 'live'. A weak model just loops more; HARAN never
#   rubber-stamps, so there is NO false convergence. ★
# ---------------------------------------------------------------------------------------------------

@dataclass
class WVFResult:
    converged: bool
    iters: int
    source: str               # "mock-sim" | "claude-live"
    final_code: str
    final_status: str
    trace: List[ai_loop.LoopStep] = field(default_factory=list)


def _claude_model_fn(api_key: Optional[str], model: str,
                     mock_sequence: Optional[List[str]]) -> Callable[[str], str]:
    """A `prompt -> code` callable backed by Claude. Live: each call is a real Claude turn (so the fix
    prompt, which carries the counterexample, actually drives a fix). Mock: a deterministic scripted
    sequence (wrong → fixed) advancing one step per call — an honest SIMULATION of the model's turns."""
    state = {"i": 0}
    seq = mock_sequence or [CA._MOCK_HARAN]

    def model_fn(prompt: str) -> str:
        if api_key:
            return CA.claude_generate(prompt, api_key, model=model).text
        out = seq[min(state["i"], len(seq) - 1)]
        state["i"] += 1
        return out

    return model_fn


def write_verify_fix(request: str, api_key: Optional[str] = None, *,
                     model: str = CA.DEFAULT_MODEL, mock_sequence: Optional[List[str]] = None,
                     max_iters: int = 3, verbose: bool = False) -> WVFResult:
    """S3: drive Claude→HARAN→fix until the code is PROVEN against its spec (or budget exhausted).

    Returns convergence + the full trace (each iteration's code, verdict, and the counterexample that
    was fed back). With no key, `mock_sequence` scripts the model's turns (e.g. [WRONG, GOOD]); the
    loop and counterexamples are real regardless."""
    fn = _claude_model_fn(api_key, model, mock_sequence)
    loop = ai_loop.write_verify_fix(request, fn, fn, max_iters=max_iters, verbose=verbose)
    last = loop.trace[-1] if loop.trace else None
    return WVFResult(
        converged=loop.converged, iters=loop.iters,
        source="claude-live" if api_key else "mock-sim",
        final_code=last.code if last else "", final_status=last.verdict.status if last else "NONE",
        trace=loop.trace,
    )


# ---------------------------------------------------------------------------------------------------
# S4 — fold optimization. Once code is PROVEN, HARAN tries to collapse it to a closed form (Faulhaber /
# C-finite / hypergeometric → O(1)). This is the "mathematically speed it up" half of the product.
# Reuses closure_classifier.classify_fn (kind ∈ CLOSED/UNKNOWN/NO_STRUCTURE/ABSENT).
#
# HONESTY: only PROVEN code is optimized (never optimize something unverified). `speedup` here is an
# **asymptotic class backed by a closed-form existence proof** (structural), NOT a wall-clock number —
# concrete ×N is measured separately (S7) or left as [TBD: measured]. Code with no exploitable
# structure is honestly reported as NOT collapsed (constant-factor only / Ω(N) floor), never faked.
# ---------------------------------------------------------------------------------------------------

@dataclass
class OptimizeResult:
    optimized: bool           # True iff a closed form was found (kind == CLOSED)
    kind: str                 # CLOSED | UNKNOWN | NO_STRUCTURE | ABSENT | PARSE_ERROR | NONE
    method: str               # e.g. "faulhaber"
    closed_form: str          # the closed form, or "—"
    speedup: str              # asymptotic class (proven structural), e.g. "O(1)", or "none"
    proof: str                # short justification from the classifier


def optimize(code: str) -> OptimizeResult:
    """S4: classify a (proven) HARAN function and, if it has closed-form structure, return the closed
    form + asymptotic class. No structure → honestly NOT optimized (no fabricated speedup)."""
    prog = parse(code)
    if prog.errors:
        return OptimizeResult(False, "PARSE_ERROR", "-", "—", "none", str(prog.errors[0]))
    fns = prog.fns()
    if not fns:
        return OptimizeResult(False, "NONE", "-", "—", "none", "no function found")
    v = CC.classify_fn(fns[0])
    return OptimizeResult(
        optimized=(v.kind == "CLOSED"), kind=v.kind, method=v.method,
        closed_form=v.closed_form, speedup=v.speedup if v.kind == "CLOSED" else "none",
        proof=str(v.proof),
    )


# ---------------------------------------------------------------------------------------------------
# S5 — two modes (normal / extended), reusing v21's philosophy & vocabulary.
#   • NORMAL   = speed: a SHALLOW fix budget. Solves the common case fast; on a hard request it stops
#                early with UNRESOLVED-shallow (honest — it didn't look deeper; NOT a wrong answer).
#   • EXTENDED = quality: a DEEPER fix budget → solves MORE (slightly slower). Worst case = honest
#                UNRESOLVED (budget hit), never a false PROVEN.
#
# The depth knob for the *agentic loop* is the write→verify→fix budget (mr_haran.verify_program has no
# tier param). v21's verification-tier modes (modes.py, corpus-based) remain at the verify layer; here
# we apply the SAME normal/extended contract to the loop.
#
# ★ INVARIANT (both modes): ZERO wrong answers. normal is shallow (may miss), never false; extended
#   solves more but is still not "everything". A 'wrong' would require HARAN to falsely VERIFY — it
#   never does, so `wrong` is structurally always False. ★
# ---------------------------------------------------------------------------------------------------

MODE_BUDGET = {"normal": 2, "extended": 5}   # fix-iteration budget (loop depth)


@dataclass
class ModeRun:
    mode: str
    converged: bool
    iters: int
    status: str               # VERIFIED | UNRESOLVED-shallow (normal) | UNRESOLVED (extended)
    wrong: bool               # claimed VERIFIED but actually not — must ALWAYS be False


def agentic_in_mode(request: str, mode: str = "normal", api_key: Optional[str] = None, *,
                    model: str = CA.DEFAULT_MODEL, mock_sequence: Optional[List[str]] = None) -> ModeRun:
    """Run the agentic loop under a mode's fix budget. normal = shallow/fast, extended = deeper/more."""
    budget = MODE_BUDGET.get(mode, 2)
    r = write_verify_fix(request, api_key, model=model, mock_sequence=mock_sequence, max_iters=budget)
    if r.converged:
        status = "VERIFIED"
    elif mode == "normal":
        status = "UNRESOLVED-shallow"        # honest: stopped early — not wrong
    else:
        status = "UNRESOLVED"                # extended budget exhausted — honest, not wrong
    wrong = r.converged and r.final_status != "VERIFIED"   # structurally False (HARAN never false-VERIFIES)
    return ModeRun(mode, r.converged, r.iters, status, wrong)


def compare_modes(request: str, api_key: Optional[str] = None, *,
                  model: str = CA.DEFAULT_MODEL, mock_sequence: Optional[List[str]] = None) -> dict:
    """Run both modes on the same request; returns {'normal': ModeRun, 'extended': ModeRun}."""
    return {m: agentic_in_mode(request, m, api_key, model=model, mock_sequence=mock_sequence)
            for m in ("normal", "extended")}


# ---------------------------------------------------------------------------------------------------
# S6 — Type A integration: spec-EMBEDDED verification with a PROOF TIER.
# The HARAN code carries its spec in the `ensures` clause (Type A). Instead of a boolean, we discharge
# it with the exact engine (prove_exact: jeff/Z3 closed-form ∀), yielding a graded tier:
#   PROVEN          — exact ∀ proof (unbounded; the strong result)
#   PROVEN-BOUNDED  — proven on a bounded domain
#   TESTED          — bounded fuzz found no counterexample (honestly weaker — NOT an ∀ claim)
#   FAILED          — counterexample found
#   UNKNOWN         — couldn't decide
# (Type B = general-language code via fusion/HIR; that path is for Python/C/etc., not HARAN folds.)
#
# HONESTY: the tiers are kept DISTINCT and never inflated — TESTED is never reported as PROVEN. The
# embedded spec is surfaced verbatim (no intent guessing).
# ---------------------------------------------------------------------------------------------------

@dataclass
class TypeAResult:
    tier: str                 # PROVEN | PROVEN-BOUNDED | TESTED | FAILED | UNKNOWN | PARSE_ERROR | NONE
    proven_forall: bool       # True iff tier == PROVEN (the only unbounded-∀ tier)
    spec: str                 # the embedded `ensures` spec (verbatim)
    detail: str               # the proof / failure detail
    counterexample: Optional[dict]


def verify_typeA(code: str) -> TypeAResult:
    """S6: discharge the EMBEDDED `ensures` spec with the exact proof engine, returning a graded tier
    (PROVEN ∀ / PROVEN-BOUNDED / TESTED / FAILED / UNKNOWN). Spec-embedded = Type A."""
    prog = parse(code)
    if prog.errors:
        return TypeAResult("PARSE_ERROR", False, "", str(prog.errors[0]), None)
    fns = prog.fns()
    if not fns:
        return TypeAResult("NONE", False, "", "no function found", None)
    ftab = {f.name: f for f in fns}
    v = PE.prove_correctness(fns[0], ftab)
    spec = fusion.extract_spec(code) or ""
    return TypeAResult(tier=v.tier, proven_forall=(v.tier == "PROVEN"), spec=spec,
                       detail=str(v.detail), counterexample=v.counterexample)


# ---------------------------------------------------------------------------------------------------
# S7 — the integrated entry point: agentic_code(request, mode, key, history?) + honest measurement.
# Pipeline: [history+request] → write→verify→FIX (mode budget) → on PROVEN: fold-optimize (S4) +
# Type A proof tier (S6). Everything below is wired from S1–S6. Wall-clock is REAL (perf_counter).
#
# HONESTY: ms is a genuine measurement (mock = no network; live = includes API latency, labeled). We
# do NOT fabricate cross-model "×N vs other AI" numbers — there is no such measurement here → that
# stays [TBD: measured]. Marketing copy ("압도적인…") lives in the UI as `// marketing copy`, never mixed
# with these measured values.
# ---------------------------------------------------------------------------------------------------

HistoryTurn = Tuple[str, str]   # (prior_request, prior_code)


@dataclass
class AgenticResult:
    request: str
    mode: str
    source: str               # "mock-sim" | "claude-live"
    converged: bool
    iters: int
    status: str               # VERIFIED | UNRESOLVED-shallow | UNRESOLVED | FAILED | NONE
    final_code: str
    proof_tier: str           # PROVEN | TESTED | ... | "(not proven)"
    optimization: Optional[OptimizeResult]
    ms: float                 # measured wall-clock of the whole pipeline
    history_len: int          # how many prior turns were threaded into context
    trace: List[ai_loop.LoopStep] = field(default_factory=list)


def _with_history(request: str, history: Optional[List[HistoryTurn]]) -> str:
    """Thread prior turns into the task so follow-up instructions accumulate (used live; recorded for
    mock). Conversation context = earlier (request → code) turns, then the new request."""
    if not history:
        return request
    ctx = "\n".join(f"# earlier: {req}\n{code}" for req, code in history)
    return f"{ctx}\n# now: {request}"


def agentic_code(request: str, mode: str = "normal", api_key: Optional[str] = None, *,
                 history: Optional[List[HistoryTurn]] = None,
                 model: str = CA.DEFAULT_MODEL,
                 mock_sequence: Optional[List[str]] = None) -> AgenticResult:
    """THE entry point. Claude writes code for `request` (+ conversation `history`); HARAN verifies &
    fixes under `mode`'s budget; if PROVEN, HARAN optimizes (closed form) and reports the Type A proof
    tier. Returns everything + a real measured wall-clock. `api_key` is level-1 (per-call, unstored)."""
    t0 = time.perf_counter()
    task = _with_history(request, history)
    budget = MODE_BUDGET.get(mode, 2)
    wvf = write_verify_fix(task, api_key, model=model, mock_sequence=mock_sequence, max_iters=budget)

    if wvf.converged:
        status = "VERIFIED"
    elif mode == "normal":
        status = "UNRESOLVED-shallow"
    else:
        status = "UNRESOLVED" if wvf.final_status != "FAILED" else "FAILED"

    # only optimize / prove PROVEN code (S4/S6 discipline)
    if wvf.converged:
        opt = optimize(wvf.final_code)
        tier = verify_typeA(wvf.final_code).tier
    else:
        opt, tier = None, "(not proven)"

    ms = (time.perf_counter() - t0) * 1000
    return AgenticResult(
        request=request, mode=mode, source=wvf.source, converged=wvf.converged, iters=wvf.iters,
        status=status, final_code=wvf.final_code, proof_tier=tier, optimization=opt, ms=ms,
        history_len=len(history or []), trace=wvf.trace,
    )


# ---------------------------------------------------------------------------------------------------
# U5 — streaming pipeline: same loop as write_verify_fix + optimize, but it YIELDS a stage event before
# each REAL step so the UI can show honest progress (generate → verify → fix → optimize). Each stage is
# emitted only when that work actually runs (no fake progress). Live: 'generate' is a real Claude call
# (the long wait); mock: instant (and the final result is labeled mock-sim).
# ---------------------------------------------------------------------------------------------------

def agentic_stream(request: str, mode: str = "normal", api_key: Optional[str] = None, *,
                   history: Optional[List[HistoryTurn]] = None, model: str = CA.DEFAULT_MODEL,
                   mock_sequence: Optional[List[str]] = None):
    """Yields stage dicts: {'stage': 'generate'|'fix'|'code_done'|'verify'|'refuted'|'optimize'|'done'}.
    The final {'stage':'done','result': AgenticResult} carries the full result (serialize as usual)."""
    t0 = time.perf_counter()
    task = _with_history(request, history)
    budget = MODE_BUDGET.get(mode, 2)
    fn = _claude_model_fn(api_key, model, mock_sequence)
    prompt, trace = task, []
    converged, final_code, final_status = False, "", "NONE"
    for i in range(budget):
        yield {"stage": "generate" if i == 0 else "fix", "iter": i + 1}   # Claude 호출중 / 반례 수정중
        code = fn(prompt).strip()
        yield {"stage": "code_done", "code": code, "iter": i + 1}
        yield {"stage": "verify", "iter": i + 1}                          # 검증중 (HARAN, real)
        v = ai_loop.verify_haran(code)
        trace.append(ai_loop.LoopStep(i + 1, "write" if i == 0 else "fix", prompt, code, v))
        final_code, final_status = code, v.status
        if v.ok:
            converged = True
            break
        yield {"stage": "refuted", "iter": i + 1, "counterexample": v.counterexample}
        prompt = ai_loop._fix_prompt(task, code, v)

    opt, tier = None, "(not proven)"
    if converged:
        yield {"stage": "optimize"}                                       # 최적화중 (fold, real)
        opt = optimize(final_code)
        tier = verify_typeA(final_code).tier
    ms = (time.perf_counter() - t0) * 1000
    if converged:
        status = "VERIFIED"
    elif mode == "normal":
        status = "UNRESOLVED-shallow"
    else:
        status = "UNRESOLVED" if final_status != "FAILED" else "FAILED"
    res = AgenticResult(request=request, mode=mode, source="claude-live" if api_key else "mock-sim",
                        converged=converged, iters=len(trace), status=status, final_code=final_code,
                        proof_tier=tier, optimization=opt, ms=ms, history_len=len(history or []), trace=trace)
    yield {"stage": "done", "result": res}


@dataclass
class AgenticMeasurement:
    mode: str
    n: int
    solved: int               # converged (VERIFIED) count
    proven_forall: int        # of solved, how many reach Type A PROVEN (∀)
    optimized: int            # of solved, how many collapse to a closed form
    total_ms: float           # measured wall-clock (mock = no network)
    wrong: int                # must be 0 (HARAN never false-VERIFIES)


def measure_agentic(corpus: Optional[List[Tuple[str, List[str]]]] = None,
                    mode: str = "normal") -> AgenticMeasurement:
    """Honest measurement of the agentic pipeline over a corpus of (request, mock_sequence) tasks.
    Reports REAL wall-clock + actual solved/proven/optimized counts (mock, no network). No fabricated
    cross-model speedups (those are [TBD: measured])."""
    corpus = corpus or _DEFAULT_CORPUS
    solved = proven = optimized = wrong = 0
    t0 = time.perf_counter()
    for request, seq in corpus:
        r = agentic_code(request, mode, mock_sequence=seq)
        if r.converged:
            solved += 1
            if r.proof_tier == "PROVEN":
                proven += 1
            if r.optimization and r.optimization.optimized:
                optimized += 1
        if r.converged and r.status != "VERIFIED":
            wrong += 1
    total_ms = (time.perf_counter() - t0) * 1000
    return AgenticMeasurement(mode, len(corpus), solved, proven, optimized, total_ms, wrong)


_GOOD = "fn triangular(n: Nat) -> Nat\n  ensures result = n*(n+1)/2\n{ fold k in 1..n { k } }"
_WRONG = "fn triangular(n: Nat) -> Nat\n  ensures result = n*(n+1)/2\n{ fold k in 1..n { k+1 } }"
_SQ = "fn sq(n: Nat) -> Nat\n  ensures result = n*n\n{ fold k in 1..n { 2*k - 1 } }"
_DEFAULT_CORPUS: List[Tuple[str, List[str]]] = [
    ("sum 1..n", [_GOOD]),                 # easy: 1 iter
    ("sum 1..n (fix me)", [_WRONG, _GOOD]),  # needs the counterexample fed back: 2 iters
    ("sum of odds = n^2", [_SQ]),          # different closed form, PROVEN ∀
    ("hard: 3 tries", [_WRONG, _WRONG, _GOOD]),  # normal misses (budget 2), extended solves
]
