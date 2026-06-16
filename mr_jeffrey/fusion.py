"""
HARAN v17 Part D · STAGE D1 — Type A+B fusion: B-found loop → A's fold engine.
=============================================================================
B localizes a slow / suspect loop in ordinary code; D1 lowers that loop into HARAN's fold engine
(Faulhaber / c-finite / hypergeometric) to see if it CLOSES to a formula with a certificate. If it does
→ closed form + proof + O(1). If not → NO_STRUCTURE, honestly (the 4-bucket vocabulary).

★ DISCIPLINE (v17 rule 6) ★ fold closes only for fold-ABLE loops (a sum of a closed summand). An
accumulator-dependent recurrence or a data-dependent loop does NOT close — and being in a "general"
language does not change that (the engine is the same). We also DIFFERENTIALLY verify: the closed form
is evaluated against the REAL loop on sample inputs, so a wrong range-mapping can never be reported as
CLOSED.
"""
from __future__ import annotations

import ast
import re
from dataclasses import dataclass
from typing import Optional, Tuple

import hir
import properties as PR
import property_test as PT
import closure_classifier as CC
import prove_exact
from haran_parser import parse as haran_parse


@dataclass
class FusionResult:
    kind: str                 # CLOSED | NO_STRUCTURE | UNKNOWN | NOT_A_SUM
    closed_form: str
    proof: str
    verified: bool            # closed form matches the REAL loop on samples
    detail: str


# ----------------------------------------------------------------- detect Σ accumulation (Python)
def _range_to_haran(call: ast.Call) -> Optional[Tuple[str, str]]:
    """range(...) → (lo, hi_inclusive) as HARAN strings, only for clean lo..VAR forms."""
    args = call.args
    if len(args) == 1:
        lo, hi = ast.Constant(0), args[0]
    elif len(args) == 2:
        lo, hi = args[0], args[1]
    else:
        return None
    lo_s = ast.unparse(lo)
    # inclusive upper: range stops before hi, so inclusive = hi-1; we accept hi == VAR+1  → VAR
    if isinstance(hi, ast.BinOp) and isinstance(hi.op, ast.Add) and isinstance(hi.right, ast.Constant) \
            and hi.right.value == 1 and isinstance(hi.left, ast.Name):
        return lo_s, hi.left.id                  # range(lo, n+1) → lo..n
    if isinstance(hi, ast.Name):
        return lo_s, None                        # range(lo, n) → 0..n-1 : not a clean lo..VAR → DEFER
    return None


def extract_sum_python(hfn: hir.HFunction) -> Optional[Tuple[str, str, str, str]]:
    """Return (binder, lo, hi_var, summand_haran) for `for i in range(...): acc += f(i)` where f(i)
    does NOT reference the accumulator or index data (a genuine sum). Else None."""
    tree = ast.parse(hfn.source)
    fn = next((n for n in ast.walk(tree) if isinstance(n, ast.FunctionDef)), None)
    if fn is None:
        return None
    for node in ast.walk(fn):
        if not (isinstance(node, ast.For) and isinstance(node.iter, ast.Call)
                and isinstance(node.iter.func, ast.Name) and node.iter.func.id == "range"
                and isinstance(node.target, ast.Name)):
            continue
        binder = node.target.id
        # find an assignment to some accumulator in the loop body
        acc, summand, self_ref = None, None, False
        for st in node.body:
            tgt = None
            if isinstance(st, ast.AugAssign) and isinstance(st.target, ast.Name):
                tgt, val = st.target.id, st.value
                if isinstance(st.op, ast.Add) and tgt not in {n.id for n in ast.walk(val) if isinstance(n, ast.Name)}:
                    acc, summand = tgt, val
                else:
                    acc, self_ref = tgt, True            # e.g. acc *= ..  or acc += acc*..
            elif isinstance(st, ast.Assign) and len(st.targets) == 1 and isinstance(st.targets[0], ast.Name):
                tgt, val = st.targets[0].id, st.value
                refs = {n.id for n in ast.walk(val) if isinstance(n, ast.Name)}
                if isinstance(val, ast.BinOp) and isinstance(val.op, ast.Add) and \
                        ((isinstance(val.left, ast.Name) and val.left.id == tgt and tgt not in
                          {n.id for n in ast.walk(val.right) if isinstance(n, ast.Name)})):
                    acc, summand = tgt, val.right        # acc = acc + summand (clean)
                elif tgt in refs:
                    acc, self_ref = tgt, True            # acc = f(acc, ..)  → recurrence
        if summand is not None:
            if any(isinstance(n, ast.Subscript) for n in ast.walk(summand)):
                return ("__DATA__", "", "", ast.unparse(summand))      # data-dependent → Ω(N)
            rng = _range_to_haran(node.iter)
            if not rng or rng[1] is None:
                return ("__RANGE__", "", "", "loop bound not in supported lo..VAR form")
            return (binder, rng[0], rng[1], ast.unparse(summand))
        if self_ref:
            return ("__RECUR__", "", "", "accumulator updated non-additively (recurrence)")
    return None


# ----------------------------------------------------------------- fold injection
def _verify_closed(hfn: hir.HFunction, closed_form: str, hi_var: str) -> bool:
    """Differential check: closed_form(n) must equal the REAL loop on sample n."""
    try:
        import sympy as sp
        n = sp.Symbol(hi_var)
        expr = sp.sympify(closed_form.replace("^", "**"))
        fn = PR.compile_callable(hfn)
        for val in (3, 5, 8, 11):
            got = fn(val)
            want = int(expr.subs(n, val))
            if int(got) != want:
                return False
        return True
    except Exception:
        return False


def extract_sum_c(hfn: hir.HFunction):
    """C `for(i=lo;i<=hi;i++){ acc = acc + summand; }` → (binder, lo, hi_var, summand_haran). Supports the
    inclusive `i<=n` form (clean lo..VAR); other forms rely on the differential check or DEFER."""
    try:
        from pycparser import c_parser, c_ast, c_generator
    except Exception:
        return None
    try:
        ast_c = c_parser.CParser().parse(hfn.source)
    except Exception:
        return None
    gen = c_generator.CGenerator()
    for ext in ast_c.ext:
        if not isinstance(ext, c_ast.FuncDef):
            continue
        for node in (n for n in _c_walk(ext)) if False else _c_iter(ext):
            if isinstance(node, c_ast.For) and isinstance(node.cond, c_ast.BinaryOp):
                cond = node.cond
                if cond.op not in ("<=",) or not isinstance(cond.left, c_ast.ID):
                    continue
                binder = cond.left.name
                hi = cond.right
                if not isinstance(hi, c_ast.ID):
                    continue                 # need a clean lo..VAR
                lo = _c_init_value(node.init, binder)
                if lo is None:
                    continue
                summand = _c_accum_summand(node.stmt, binder)
                if summand is None:
                    continue
                if isinstance(summand, str):
                    return (summand, "", "", "")    # marker (__DATA__/__RECUR__)
                return (binder, str(lo), hi.name, gen.visit(summand))
    return None


def _c_iter(node):
    from pycparser import c_ast
    stack = [node]
    while stack:
        x = stack.pop()
        yield x
        stack.extend(c for _, c in x.children())


def _c_walk(node):
    return _c_iter(node)


def _c_init_value(init, binder):
    from pycparser import c_ast
    if init is None:
        return None
    decls = init.decls if isinstance(init, c_ast.DeclList) else [init]
    for d in decls:
        if isinstance(d, c_ast.Decl) and d.name == binder and isinstance(d.init, c_ast.Constant):
            return d.init.value
        if isinstance(d, c_ast.Assignment) and isinstance(d.lvalue, c_ast.ID) and d.lvalue.name == binder \
                and isinstance(d.rvalue, c_ast.Constant):
            return d.rvalue.value
    return None


def _c_accum_summand(stmt, binder):
    """Find acc = acc + summand (or acc += summand) in the loop body; return the summand node, or a
    marker string for data/recurrence."""
    from pycparser import c_ast
    body = stmt.block_items if isinstance(stmt, c_ast.Compound) else [stmt]
    for st in (body or []):
        if isinstance(st, c_ast.Assignment) and isinstance(st.lvalue, c_ast.ID):
            acc = st.lvalue.name
            if st.op == "+=":
                val = st.rvalue
            elif st.op == "=" and isinstance(st.rvalue, c_ast.BinaryOp) and st.rvalue.op == "+":
                L, R = st.rvalue.left, st.rvalue.right
                val = R if (isinstance(L, c_ast.ID) and L.name == acc) else \
                    (L if (isinstance(R, c_ast.ID) and R.name == acc) else None)
            else:
                val = None
            if val is None:
                continue
            ids = [n.name for n in _c_iter(val) if isinstance(n, c_ast.ID)]
            if acc in ids:
                return "__RECUR__"
            if any(isinstance(n, c_ast.ArrayRef) for n in _c_iter(val)):
                return "__DATA__"
            return val
    return None


def fold_inject(hfn: hir.HFunction) -> FusionResult:
    lang = getattr(hfn, "lang", "python")
    if lang == "c":
        ex = extract_sum_c(hfn)
    elif lang == "python":
        ex = extract_sum_python(hfn)
    else:
        return FusionResult("UNKNOWN", "—", f"fold extraction not yet wired for {lang} (engine shared; DEFER)",
                            False, "loop→sum extractor per language is the only missing piece")
    if lang == "c" and ex and ex[0] in ("__RECUR__", "__DATA__"):
        why = "accumulator recurrence" if ex[0] == "__RECUR__" else "data-dependent summand (Ω(N))"
        return FusionResult("NO_STRUCTURE", "—", why, False, why)
    if ex is None:
        return FusionResult("NOT_A_SUM", "—", "no Σ-accumulation loop found", False, "not a fold-shaped loop")
    binder, lo, hi_var, summand = ex
    if binder == "__RECUR__":
        return FusionResult("NO_STRUCTURE", "—", "accumulator-dependent recurrence — not a simple sum",
                            False, f"summand '{summand}' references the accumulator (recurrence, not fold)")
    if binder == "__DATA__":
        return FusionResult("NO_STRUCTURE", "—", "data-dependent summand (Ω(N) floor)", False,
                            f"summand '{summand}' indexes data — outside every closure class")
    if binder == "__RANGE__":
        return FusionResult("UNKNOWN", "—", "loop bound not in supported lo..VAR form (DEFER)", False, summand)
    src = f"fn g(n: Nat) -> Nat {{ fold {binder} in {lo}..{hi_var} {{ {summand} }} }}"
    try:
        prog = haran_parse(src)
        fold_fn = prog.items[0]
    except Exception as e:
        return FusionResult("UNKNOWN", "—", f"HARAN parse failed ({e})", False, "summand not expressible")
    v = CC.classify_fn(fold_fn)
    if v.kind == "CLOSED":
        ok = _verify_closed(hfn, v.closed_form, hi_var)
        return FusionResult("CLOSED" if ok else "UNKNOWN", v.closed_form, v.proof, ok,
                            "closed form matches the real loop on samples" if ok
                            else "closed form did NOT match the loop — not claimed (honest)")
    return FusionResult(v.kind, v.closed_form, v.proof, False, v.proof)


# ===================================================================================================
# D2 — HIR → Z3 correctness injection (formal verification into general-language code).
# ===================================================================================================
@dataclass
class Z3Verdict:
    tier: str                 # PROVEN | FAILED | PROPERTY-ONLY | UNKNOWN
    detail: str
    spec: Optional[str]
    method: str
    counterexample: Optional[dict] = None


def extract_spec(source: str) -> Optional[str]:
    """User spec from a comment/decorator, e.g. `# ensures result == n*(n+1)/2`."""
    m = re.search(r"ensures\s+result\s*(?:==|=)\s*(.+)", source)
    return m.group(1).strip().rstrip(";") if m else None


def _fold_parts(hfn: hir.HFunction):
    lang = getattr(hfn, "lang", "python")
    if lang == "c":
        return extract_sum_c(hfn)
    if lang == "python":
        return extract_sum_python(hfn)
    return None


def z3_inject(hfn: hir.HFunction, source: Optional[str] = None) -> Z3Verdict:
    """If the code carries a spec and the loop is fold-expressible → prove (or refute) it ∀ with the
    HARAN/Z3 pipeline. If there is NO spec → fall back to properties as the spec proxy (honest: a
    spec-less check is only as strong as the properties). `source` lets the caller pass the full file
    text (a spec comment may sit ABOVE the function, outside hfn.source)."""
    spec = extract_spec(source or hfn.source)
    parts = _fold_parts(hfn)
    foldable = parts is not None and parts[0] not in ("__RECUR__", "__DATA__", "__RANGE__")
    if spec and foldable:
        binder, lo, hi, summand = parts
        src = (f"fn g(n: Nat) -> Nat\n  ensures result = {spec}\n"
               f"{{ fold {binder} in {lo}..{hi} {{ {summand} }} }}")
        try:
            fn = haran_parse(src).items[0]
            v = prove_exact.prove_correctness(fn, {"g": fn})
        except Exception as e:
            return Z3Verdict("UNKNOWN", f"spec synthesis/parse failed ({e})", spec, "-")
        tier = "PROVEN" if v.proven() else ("FAILED" if v.tier == "FAILED" else "UNKNOWN")
        return Z3Verdict(tier, v.detail, spec, "Z3/JEFF exact ∀ over the fold closed form", v.counterexample)
    if spec and not foldable:
        return Z3Verdict("UNKNOWN", "spec present but loop not fold-expressible → only bounded checks apply",
                         spec, "bounded (no symbolic closed form)")
    # no spec → properties proxy (B2/B3)
    try:
        fn = PR.compile_callable(hfn)
        props = PR.extract_properties(hfn)
        rep = PT.test_properties(fn, props, PT.gen_int_lists(120))
        viol = rep.violated_properties()
    except Exception as e:
        return Z3Verdict("UNKNOWN", f"no spec; property proxy failed ({e})", None, "properties (no spec)")
    if viol:
        return Z3Verdict("FAILED", f"no user spec; properties used as spec → violated {viol}",
                         None, "properties as spec proxy")
    return Z3Verdict("PROPERTY-ONLY", f"no user spec; {len(props)} properties hold (weak without a spec)",
                     None, "properties as spec proxy")


# ===================================================================================================
# D3 — HIR → Coq unbounded-∀ injection (route all-lengths goals to Coq, beyond Z3's bounded reach).
# ===================================================================================================
import haran_coq  # noqa: E402


@dataclass
class CoqVerdict:
    available: bool
    attempted: list
    proven: list
    mode: dict                # theorem -> auto|manual
    detail: str


def coq_inject(hfn: hir.HFunction, n: int = 60) -> CoqVerdict:
    """If B flags a sort-shaped function that PASSES bounded checks, the open question is whether it holds
    for ALL lengths — which Z3 cannot decide (no induction). Route that unbounded-∀ goal to Coq (v16 A3).
    Honest: Coq proves the canonical sortedness/permutation theorems for all lengths (vs Z3 length≤4);
    translating an ARBITRARY user algorithm into Coq is semi-automatic → DEFER."""
    try:
        fn = PR.compile_callable(hfn)
        props = PR.extract_properties(hfn)
        rep = PT.test_properties(fn, props, PT.gen_int_lists(n))
    except Exception as e:
        return CoqVerdict(False, [], [], {}, f"could not run bounded checks ({e})")
    names = {p.name for p in props}
    if "ordered_output" not in names:
        return CoqVerdict(haran_coq.coq_available(), [], [], {},
                          "not a sort-shaped unbounded-∀ goal (Coq routing applies to recognized shapes)")
    if "ordered_output" in rep.violated_properties():
        return CoqVerdict(haran_coq.coq_available(), [], [], {},
                          "sortedness FAILS on samples — this is a bug; fix before any unbounded proof")
    if not haran_coq.coq_available():
        return CoqVerdict(False, [], [], {}, "Coq BLOCKED → Z3 bounded (length ≤ 4) is the only fallback")
    attempt = ["isort_sorted", "isort_perm"]
    results = [haran_coq.prove_property(nm) for nm in attempt]
    proven = [r.name for r in results if r.proven]
    mode = {r.name: r.mode for r in results}
    return CoqVerdict(True, attempt, proven, mode,
                      "Coq proves sortedness + permutation for ALL lengths (vs Z3 length ≤ 4); translating "
                      "THIS specific algorithm to Coq is semi-automatic → DEFER")
