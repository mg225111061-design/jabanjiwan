"""
STAGE H6 — Mr.Jeffrey × HARAN integrated verifier (the §2.2 three-verdict pipeline).
====================================================================================
End-to-end: parse (H1) → generate obligations (H2) → discharge each with the right engine
  · correctness : fold collapse + JEFF coeff-zero (H4) / JEFF-or-sympy exact (H2) / bounded fuzz (eval)
  · termination : measure synthesis + ordinal engine (H3)
  · productivity : cofix guardedness (H5)
  · exhaustiveness: simple list/wildcard coverage
→ aggregate into ONE verdict per function in the design's §2.2 format:
  ✅ VERIFIED (명세 대비, with method+strength)  ·  ❌ FAILED (어디/왜/무엇을 + 반례)
  ·  ⚠️ UNKNOWN (이유 + 선택지 3개).

Boundary 1 (§4): every ✅ is *명세 대비* and carries HOW it was shown (exact ∀-proof vs bounded test).
"""
from __future__ import annotations

from dataclasses import dataclass, field
from typing import List, Optional

import haran_ast as A
from haran_parser import parse
from haran_to_obligations import discharge_correctness, _show
from measure_synth import synthesize
from fold_collapse import collapse_fn_fold
from productivity import check_proc
import haran_eval


@dataclass
class ObVerdict:
    kind: str
    status: str               # PASS | FAIL | UNKNOWN
    method: str = ""          # for PASS: how it was shown
    why: str = ""             # for FAIL/UNKNOWN
    what: str = ""            # for FAIL: 무엇을
    counterexample: Optional[dict] = None
    options: List[str] = field(default_factory=list)   # for UNKNOWN: 선택지


@dataclass
class FnReport:
    name: str
    verdict: str              # VERIFIED | FAILED | UNKNOWN
    obligations: List[ObVerdict]


SYM = {"VERIFIED": "✅", "FAILED": "❌", "UNKNOWN": "⚠️"}


# ----------------------------------------------------------------- per-obligation discharge
def _correctness(fn: A.FnDecl, ftab: dict) -> ObVerdict:
    ens = fn.ensures
    # (1) fold + closed-form spec → H4 collapse certificate (exact, ∀n, O(n)→O(1))
    fc = collapse_fn_fold(fn)
    if fc.verdict == "COLLAPSED" and fc.matches_ensures is True:
        return ObVerdict("correctness", "PASS",
                         method=f"exact: fold collapse {fc.cert.closed_form} ≡ ensures, "
                                f"JEFF coeff-zero ∀n; {fc.cert.speedup}")
    if fc.verdict == "COLLAPSED" and fc.matches_ensures is False:
        d = discharge_correctness(fn)
        return ObVerdict("correctness", "FAIL",
                         why=f"implementation collapses to {fc.cert.closed_form}, which differs from ensures",
                         what="fix the closed form in `ensures`, or the implementation",
                         counterexample=d.counterexample)
    # (2) arithmetic closed-form spec → H2 exact (JEFF / sympy)
    d = discharge_correctness(fn)
    if d.verdict == "PROVEN":
        return ObVerdict("correctness", "PASS", method=f"exact ({d.backend}): {d.detail}")
    if d.verdict == "REFUTED":
        return ObVerdict("correctness", "FAIL", why=d.detail,
                         what="fix the spec or the implementation", counterexample=d.counterexample)
    # (3) general proposition → bounded fuzz over the evaluator
    status, detail, cx = haran_eval.bounded_fuzz(fn, ftab, ens)
    if status == "PASS":
        return ObVerdict("correctness", "PASS", method=f"bounded: {detail}")
    if status == "FAIL":
        return ObVerdict("correctness", "FAIL", why="ensures is false on a concrete input",
                         what="fix the implementation or the spec", counterexample=cx)
    return ObVerdict("correctness", "UNKNOWN", why=detail,
                     options=["provide a closed-form `ensures result = <expr>` (→ exact JEFF proof)",
                              "ensure the body uses only evaluable builtins (filter/map/length/…)",
                              "supply concrete test inputs to fuzz"])


def _termination(fn: A.FnDecl) -> Optional[ObVerdict]:
    if fn.kind != "fn":
        return None
    s = synthesize(fn)
    if s.verdict == "PROVEN":
        oc = f"; ordinal-engine: {s.ordinal_cert}" if s.ordinal_cert else ""
        return ObVerdict("termination", "PASS", method=f"measure {s.measure} ({s.kind}, layer {s.layer}){oc}")
    if s.verdict == "ASSUMED":
        return ObVerdict("termination", "PASS", method=f"ASSUMED ({s.measure}) — termination consciously assumed, out of scope")
    return ObVerdict("termination", "UNKNOWN", why=s.detail,
                     options=["add `decreases <measure>` if you know a descending quantity",
                              "move to a `proc` (coinductive) if it is meant to stream forever",
                              "restructure so a parameter provably descends (list tail / n-1)"])


def _productivity(fn: A.FnDecl, proc_names) -> Optional[ObVerdict]:
    if fn.kind != "proc":
        return None
    r = check_proc(fn, proc_names)
    if r.verdict == "PROVEN":
        return ObVerdict("productivity", "PASS", method=r.detail)
    if r.verdict == "REFUTED":
        return ObVerdict("productivity", "FAIL", why=r.detail,
                         what="yield (produce) before each corecursive call",
                         counterexample={"unguarded_at": [str(s) for s in r.unguarded]})
    return ObVerdict("productivity", "UNKNOWN", why=r.detail,
                     options=["restructure so every path yields before recursing",
                              "avoid nested cofix / mutual recursion (out of scope)",
                              "if conditional, make each branch yield before its recursion"])


def _exhaustiveness(fn: A.FnDecl) -> Optional[ObVerdict]:
    matches = [x for x in _walk(fn.body) if isinstance(x, A.Match)] if fn.body else []
    if not matches:
        return None
    for m in matches:
        pats = [a.pattern for a in m.arms]
        has_wild = any(isinstance(p, (A.PWild, A.PVar)) for p in pats)
        has_empty = any(isinstance(p, A.PListEmpty) for p in pats)
        has_cons = any(isinstance(p, A.PCons) for p in pats)
        if has_wild or (has_empty and has_cons):
            continue
        return ObVerdict("exhaustiveness", "UNKNOWN",
                         why="cannot confirm all constructors are covered (no data decl / no wildcard)",
                         options=["add a wildcard `_` arm", "cover every constructor explicitly",
                                  "provide the `data` declaration so coverage can be checked"])
    return ObVerdict("exhaustiveness", "PASS", method="all match arms cover their cases (list/wildcard)")


def _walk(node):
    import dataclasses
    yield node
    if dataclasses.is_dataclass(node) and not isinstance(node, A.Span):
        for f in dataclasses.fields(node):
            v = getattr(node, f.name)
            if isinstance(v, list):
                for x in v:
                    if dataclasses.is_dataclass(x):
                        yield from _walk(x)
            elif dataclasses.is_dataclass(v):
                yield from _walk(v)


# ----------------------------------------------------------------- aggregate
def verify_fn(fn: A.FnDecl, ftab: dict, proc_names) -> FnReport:
    obs: List[ObVerdict] = []
    if fn.ensures is not None:
        obs.append(_correctness(fn, ftab))
    for ob in (_termination(fn), _productivity(fn, proc_names), _exhaustiveness(fn)):
        if ob is not None:
            obs.append(ob)
    if any(o.status == "FAIL" for o in obs):
        verdict = "FAILED"
    elif any(o.status == "UNKNOWN" for o in obs):
        verdict = "UNKNOWN"
    else:
        verdict = "VERIFIED"
    return FnReport(fn.name, verdict, obs)


def verify_program(src: str) -> List[FnReport]:
    prog = parse(src)
    if prog.errors:
        raise ValueError("; ".join(str(e) for e in prog.errors))
    fns = [it for it in prog.items if isinstance(it, A.FnDecl)]
    ftab = {f.name: f for f in fns}
    proc_names = {f.name for f in fns if f.kind == "proc"}
    return [verify_fn(f, ftab, proc_names) for f in fns]


# ----------------------------------------------------------------- §2.2 renderer
def render(report: FnReport) -> str:
    lines = [f"{SYM[report.verdict]} {report.name} — {report.verdict}"
             + (" (명세 대비)" if report.verdict == "VERIFIED" else "")]
    for o in report.obligations:
        if o.status == "PASS":
            lines.append(f"   · {o.kind:14s}: ✓ {o.method}")
        elif o.status == "FAIL":
            lines.append(f"   · {o.kind:14s}: ✗ FAILED")
            lines.append(f"        어디(where): {o.kind} obligation")
            lines.append(f"        왜(why)    : {o.why}")
            if o.counterexample:
                lines.append(f"        반례(c-ex) : {o.counterexample}")
            lines.append(f"        무엇을(do) : {o.what}")
        else:
            lines.append(f"   · {o.kind:14s}: ⚠ UNKNOWN — {o.why}")
            for i, opt in enumerate(o.options, 1):
                lines.append(f"        선택지 {i}: {opt}")
    return "\n".join(lines)


if __name__ == "__main__":
    import sys
    src = open(sys.argv[1]).read() if len(sys.argv) > 1 else ""
    for r in verify_program(src):
        print(render(r))
        print()
