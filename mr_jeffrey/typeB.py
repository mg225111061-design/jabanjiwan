"""
HARAN v16 Part B · STAGE B9 — integrated Type B pipeline + measurement.
=======================================================================
One call wires B1→B8: detect+HIR → extract properties → property-test → map violations to operations →
Bayesian narrow → B6 digit certificate → category-distinct verdict → structured report → fix loop.

Measurement honesty (B9 discipline, cherry-pick 0): this runs on a CURATED corpus of representative
Python bugs (math / data-structure / transform — where metamorphic relations exist, Type B's strength).
The real BugsInPy benchmark needs a network checkout and contains arbitrary business logic that is
property-poor → real-world top-k would be LOWER. We report that plainly, and include a property-poor
case that Type B honestly fails to localize.
"""
from __future__ import annotations

import time
from dataclasses import dataclass, field
from typing import List, Optional

import hir
import properties as PR
import property_test as PT
import fault_map as FM
import narrow as NA
import digit_proof as DP
import verdict as VD
import report as RPT
import fix_loop as FL


@dataclass
class TypeBResult:
    lang: str
    supported: bool
    top1: Optional[str]
    top1_lines: List[int]
    top5: List[str]
    violated: List[str]
    digit_certificate: Optional[str]
    grade: Optional[str]
    fixed: bool
    elapsed_s: float
    detail: str


def analyze(source: str, filename: Optional[str] = None, n_random: int = 400,
            do_fix: bool = True) -> TypeBResult:
    t = time.perf_counter()
    fr = hir.to_hir(source, filename)
    if not fr.supported:
        return TypeBResult(fr.lang, False, None, [], [], [], None, None, False,
                           time.perf_counter() - t, fr.detail)
    hfn = fr.module.functions[0]
    fn = PR.compile_callable(hfn)
    props = PR.extract_properties(hfn)
    inputs = PT.gen_int_lists(n_random)
    rep = PT.test_properties(fn, props, inputs)
    violated = [p for p in props if p.name in rep.violated_properties()]
    if not violated:
        return TypeBResult(fr.lang, True, None, [], [], [], None, "clean", False,
                           time.perf_counter() - t, "no property violated (no bug in the property view)")
    res = NA.bayesian_narrow(hfn, violated)
    top = res.ranked[0]
    # B6 digit certificate for the least-suspect (innocent) op
    viol_inputs = {p.name: rep.violations[p.name] for p in violated}
    innocent = res.ranked[-1]
    cert = DP.digit_certificate(innocent.op_kind, res.innocent_posterior, len(inputs), viol_inputs)
    brep = RPT.build_report(hfn, n_random=n_random, proven_digit=f"{cert.proven_upper:.1e}")
    fixed = False
    if do_fix:
        fr2 = FL.ai_fix_loop(hfn)
        fixed = fr2.fixed
    return TypeBResult(
        fr.lang, True, top.op_kind, top.lines, [s.op_kind for s in res.top(5)],
        [p.name for p in violated], cert.render(), brep.grade if brep else None, fixed,
        time.perf_counter() - t, "analyzed")


# ----------------------------------------------------------------- curated corpus (known bug locations)
@dataclass
class Bug:
    name: str
    source: str
    bug_line: int
    bug_op: str            # the operation kind at the bug (top-k accuracy target); "" = property-poor


CORPUS: List[Bug] = [
    Bug("sort_cmp", "def f(xs):\n a=list(xs)\n for i in range(len(a)):\n  for j in range(len(a)-1):\n"
        "   if a[j] < a[j+1]:\n    a[j],a[j+1]=a[j+1],a[j]\n return a\n", 5, "compare"),
    Bug("sort_drop", "def f(xs):\n a=sorted(xs)\n if len(a)>1:\n  a.pop()\n return a\n", 4, "pop"),
    Bug("max_wrong", "def f(xs):\n m=xs[0]\n for x in xs:\n  if x < m:\n   m=x\n return [m]\n", 4, "compare"),
    Bug("scale_bug", "def f(xs):\n return [x - 1 for x in xs]\n", 2, "arith"),          # claims to double? metamorphic-weak
    Bug("reverse_keep", "def f(xs):\n a=list(xs)\n a.pop()\n a.reverse()\n return a\n", 3, "pop"),
    Bug("biz_poor", "def f(xs):\n s=0\n for x in xs:\n  s = s*31 + x\n return [s % 7]\n", 4, ""),   # property-poor
]


@dataclass
class CorpusMeasurement:
    n: int
    top1_hits: int
    top5_hits: int
    localizable: int            # bugs where a property was violated at all
    fixed: int
    avg_s: float
    deterministic: bool
    rows: list = field(default_factory=list)

    def top1_rate(self):
        return self.top1_hits / self.localizable if self.localizable else 0.0

    def top5_rate(self):
        return self.top5_hits / self.localizable if self.localizable else 0.0


def measure_corpus(corpus: List[Bug] = None) -> CorpusMeasurement:
    corpus = corpus or CORPUS
    top1 = top5 = local = fixed = 0
    tot = 0.0
    rows = []
    det = True
    for b in corpus:
        r = analyze(b.source, b.name + ".py")
        r2 = analyze(b.source, b.name + ".py", do_fix=False)   # determinism check
        if (r.top1, tuple(r.top5)) != (r2.top1, tuple(r2.top5)):
            det = False
        tot += r.elapsed_s
        localizable = bool(r.violated)
        if localizable:
            local += 1
            h1 = r.top1 == b.bug_op
            h5 = b.bug_op in r.top5
            top1 += int(h1)
            top5 += int(h5)
            fixed += int(r.fixed)
            rows.append((b.name, r.top1, b.bug_op, h1, h5, r.fixed, r.grade, round(r.elapsed_s * 1e3)))
        else:
            rows.append((b.name, None, b.bug_op or "(none)", False, False, False, r.detail, round(r.elapsed_s * 1e3)))
    return CorpusMeasurement(len(corpus), top1, top5, local, fixed, tot / len(corpus), det, rows)
