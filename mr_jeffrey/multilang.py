"""
HARAN v17 Part C · STAGE C5 — multilang dispatcher + per-language measurement.
=============================================================================
One entry point: code → detect language → frontend → HIR → the shared Type B engine. We measure the
same bug (a descending bubble sort) across every language whose toolchain is present, reporting which
operation is localized, the speed, and the parser/runtime used. Honest: BLOCKED languages are listed,
not pretended; statically-typed languages give a cleaner shape than dynamic ones.
"""
from __future__ import annotations

import time
from dataclasses import dataclass, field
from typing import Dict, List, Optional

import hir
import properties as PR
import property_test as PT
import narrow as NA


# the SAME bug (comparison flipped → descending) in each language
SORTS = {
    "python": ("def sortf(a):\n b=list(a)\n for i in range(len(b)):\n  for j in range(len(b)-1):\n"
               "   if b[j] < b[j+1]:\n    b[j],b[j+1]=b[j+1],b[j]\n return b\n", "s.py"),
    "c": ("int* sortf(int* a, int n){int i,j,t;for(i=0;i<n;i++){for(j=0;j<n-1;j++){"
          "if(a[j]<a[j+1]){t=a[j];a[j]=a[j+1];a[j+1]=t;}}}return a;}\n", "s.c"),
    "go": ("func sortf(a []int) []int {\n for i:=0;i<len(a);i++ {\n  for j:=0;j<len(a)-1;j++ {\n"
           "   if a[j] < a[j+1] {\n    a[j],a[j+1]=a[j+1],a[j]\n   }\n  }\n }\n return a\n}\n", "s.go"),
    "rust": ("fn sortf(mut a: Vec<i32>) -> Vec<i32> {\n for i in 0..a.len() {\n  for j in 0..a.len()-1 {\n"
             "   if a[j] < a[j+1] {\n    a.swap(j,j+1);\n   }\n  }\n }\n a\n}\n", "s.rs"),
    "javascript": ("function sortf(a){\n for(let i=0;i<a.length;i++){\n  for(let j=0;j<a.length-1;j++){\n"
                   "   if(a[j]<a[j+1]){\n    let t=a[j];a[j]=a[j+1];a[j+1]=t;\n   }\n  }\n }\n return a;\n}\n", "s.js"),
    "typescript": ("function sortf(a: number[]): number[] {\n for(let i=0;i<a.length;i++){\n"
                   "  for(let j=0;j<a.length-1;j++){\n   if(a[j]<a[j+1]){\n    let t=a[j];a[j]=a[j+1];a[j+1]=t;\n"
                   "   }\n  }\n }\n return a;\n}\n", "s.ts"),
    "java": ("static int[] sortf(int[] a){\n for(int i=0;i<a.length;i++){\n  for(int j=0;j<a.length-1;j++){\n"
             "   if(a[j]<a[j+1]){\n    int t=a[j];a[j]=a[j+1];a[j+1]=t;\n   }\n  }\n }\n return a;\n}\n", "S.java"),
}

TYPED = {"c", "rust", "go", "java", "typescript"}     # static types → explicit shape (stronger)
DYNAMIC = {"python", "javascript"}                    # inferred shape (weaker)


@dataclass
class LangResult:
    lang: str
    supported: bool
    out: Optional[list]
    violated: List[str]
    top1: Optional[str]
    top1_lines: List[int]
    top5: List[str]
    elapsed_s: float
    detail: str


def analyze_language(lang: str, n: int = 40) -> LangResult:
    src, fname = SORTS[lang]
    t = time.perf_counter()
    fr = hir.to_hir(src, fname)
    if not fr.supported:
        return LangResult(lang, False, None, [], None, [], [], time.perf_counter() - t, fr.detail)
    f = fr.module.functions[0]
    try:
        fn = PR.compile_callable(f)
        out = fn([5, 2, 9, 1, 7])
        props = PR.extract_properties(f)
        rep = PT.test_properties(fn, props, PT.gen_int_lists(n))
        res = NA.bayesian_narrow(f, [p for p in props if p.name in rep.violated_properties()])
        top = res.ranked[0] if res.ranked else None
        return LangResult(lang, True, out, rep.violated_properties(),
                          top.op_kind if top else None, top.lines if top else [],
                          [s.op_kind for s in res.top(5)], time.perf_counter() - t, "ok")
    except Exception as e:  # toolchain/compile error → honest BLOCKED for this language
        return LangResult(lang, False, None, [], None, [], [], time.perf_counter() - t, f"runtime error: {e}")


@dataclass
class MultilangMeasurement:
    results: List[LangResult]

    def covered(self):
        return [r.lang for r in self.results if r.supported]

    def blocked(self):
        return [(r.lang, r.detail) for r in self.results if not r.supported]

    def top1_hits(self):
        return sum(1 for r in self.results if r.supported and r.top1 == "compare")


def measure_all(n: int = 40) -> MultilangMeasurement:
    return MultilangMeasurement([analyze_language(l, n) for l in SORTS])
