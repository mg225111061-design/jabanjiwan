"""
HARAN v17 Part E · STAGE E1 — differential specs (catch property-INVISIBLE bugs).
=================================================================================
Some bugs break no metamorphic property (v16's scale_bug: `x-1` instead of `x*2` is still deterministic
and shape-preserving). A differential oracle catches them by comparing against a reference:
  • git differential — the last passing commit IS the spec: run old vs new, report inputs where they
    diverge (a regression);
  • N-version differential — several implementations vote; an implementation that disagrees with the
    majority is the suspect.

★ DISCIPLINE: differential needs a reference (a previous passing version, or other implementations).
With none, it CANNOT run — stated honestly. Property + differential together still do not catch every
bug (no oracle does).
"""
from __future__ import annotations

import ast
import os
import subprocess
from dataclasses import dataclass, field
from typing import Callable, List, Optional


def extract_func_src(full_src: str, name: str) -> Optional[str]:
    try:
        tree = ast.parse(full_src)
    except SyntaxError:
        return None
    lines = full_src.splitlines()
    for node in ast.walk(tree):
        if isinstance(node, ast.FunctionDef) and node.name == name:
            return "\n".join(lines[node.lineno - 1:getattr(node, "end_lineno", node.lineno)])
    return None


def _callable(src: str, name: str):
    ns: dict = {}
    exec(src, ns)
    return ns[name]


# ----------------------------------------------------------------- core differential
@dataclass
class Divergence:
    inp: object
    ref_out: object
    new_out: object


@dataclass
class DifferentialResult:
    diverged: bool
    divergences: List[Divergence]
    tested: int
    detail: str


def differential(ref_src: str, new_src: str, name: str, inputs: List) -> DifferentialResult:
    try:
        ref, new = _callable(ref_src, name), _callable(new_src, name)
    except Exception as e:
        return DifferentialResult(False, [], 0, f"could not build callables ({e})")
    divs: List[Divergence] = []
    for x in inputs:
        ax = list(x) if isinstance(x, list) else x
        bx = list(x) if isinstance(x, list) else x
        try:
            ro, no = ref(ax), new(bx)
        except Exception as e:
            divs.append(Divergence(x, "ok", f"crash:{e}"))
            continue
        if ro != no:
            divs.append(Divergence(x, ro, no))
    return DifferentialResult(bool(divs), divs, len(inputs),
                              f"{len(divs)}/{len(inputs)} inputs diverge from the reference" if divs
                              else "no divergence from the reference")


# ----------------------------------------------------------------- git differential (real)
def git_show(repo: str, ref: str, relpath: str) -> Optional[str]:
    r = subprocess.run(["git", "-C", repo, "show", f"{ref}:{relpath}"], capture_output=True, text=True)
    return r.stdout if r.returncode == 0 else None


def git_differential(repo: str, relpath: str, name: str, inputs: List,
                     ref_old: str = "HEAD~1", ref_new: str = "HEAD") -> DifferentialResult:
    old_full = git_show(repo, ref_old, relpath)
    new_full = git_show(repo, ref_new, relpath)
    if old_full is None or new_full is None:
        return DifferentialResult(False, [], 0,
                                  f"no reference available (git show {ref_old}/{ref_new} failed) — cannot diff (honest)")
    old_src = extract_func_src(old_full, name)
    new_src = extract_func_src(new_full, name)
    if not old_src or not new_src:
        return DifferentialResult(False, [], 0, f"function '{name}' not found in one of the versions")
    return differential(old_src, new_src, name, inputs)


# ----------------------------------------------------------------- N-version differential
@dataclass
class NVersionResult:
    majority_index: int
    outliers: List[int]              # implementation indices disagreeing with the majority
    detail: str


def n_version(impl_srcs: List[str], name: str, inputs: List) -> NVersionResult:
    fns = []
    for s in impl_srcs:
        try:
            fns.append(_callable(s, name))
        except Exception:
            fns.append(None)
    # signature of outputs per implementation across all inputs
    sigs = []
    for fn in fns:
        if fn is None:
            sigs.append(None); continue
        out = []
        for x in inputs:
            try:
                out.append(repr(fn(list(x) if isinstance(x, list) else x)))
            except Exception as e:
                out.append(f"crash:{e}")
        sigs.append(tuple(out))
    # majority vote over identical output-signatures
    from collections import Counter
    counts = Counter(s for s in sigs if s is not None)
    if not counts:
        return NVersionResult(-1, [], "no runnable implementations")
    majority_sig, _ = counts.most_common(1)[0]
    maj_idx = next(i for i, s in enumerate(sigs) if s == majority_sig)
    outliers = [i for i, s in enumerate(sigs) if s != majority_sig]
    return NVersionResult(maj_idx, outliers,
                          f"{len(impl_srcs) - len(outliers)}/{len(impl_srcs)} agree; outliers={outliers}")
