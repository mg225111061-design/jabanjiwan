#!/usr/bin/env python3
"""
Mr. Jeffrey — CLI.  "Fast, instant, decisive — and PROVEN (or honestly not)."
============================================================================
Subcommands:
  mr verify <file.py> --func NAME [--kinds int,list_int] [--spec spec.py]
        load a function and hunt for bugs (bounded+fuzzed, strengthened verifier).
  mr prove "<cand>" "<ref>" --vars n[,k]
        EXACT-prove an identity for ALL inputs (JEFF → sympy → defer).
  mr solve "<task>" --func NAME --kinds K [--backend auto] [--exact cand,ref,vars]
        an AI writes the function, Mr. verifies + feeds back the exact breaking input until it holds.
  mr demo
        run the 10-real-bugs showcase.

Verdicts are color-coded and HONEST:
  PROVEN  (green)  exact, all inputs        VERIFIED (yellow) bounded — strong, NOT a proof
  REFUTED (red)    counterexample shown     DEFER/FAILED (gray) couldn't decide
"""
from __future__ import annotations
import argparse
import importlib.util
import os
import sys

# color (TTY-aware)
def _supports_color():
    return sys.stdout.isatty() and os.environ.get("NO_COLOR") is None
class C:
    G = "\033[32m"; Y = "\033[33m"; R = "\033[31m"; D = "\033[90m"; B = "\033[1m"; X = "\033[0m"
    @classmethod
    def off(cls):
        cls.G = cls.Y = cls.R = cls.D = cls.B = cls.X = ""
if not _supports_color():
    C.off()

def banner(verdict: str) -> str:
    col = {"PROVEN": C.G, "VERIFIED": C.Y, "REFUTED": C.R}.get(verdict, C.D)
    return f"{col}{C.B}{verdict}{C.X}"


def _load_module(path):
    spec = importlib.util.spec_from_file_location("_mr_user", path)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def cmd_verify(args):
    from verify_strong import StrongVerifier
    mod = _load_module(args.file)
    fn = getattr(mod, args.func, None)
    if fn is None:
        print(f"{C.R}error{C.X}: function '{args.func}' not found in {args.file}")
        return 2
    kinds = args.kinds.split(",") if args.kinds else ["int"]
    reference = properties = None
    if args.spec:
        smod = _load_module(args.spec)
        reference = getattr(smod, "reference", None)
        properties = getattr(smod, "properties", None)
        if getattr(smod, "arg_kinds", None):
            kinds = smod.arg_kinds
    V = StrongVerifier(timeout=args.timeout)
    rep = V.verify(fn, kinds, reference=reference, properties=properties, func_name=args.func)
    print(f"\n  Mr. → {banner(rep.verdict)}")
    print("  " + str(rep).replace("\n", "\n  "))
    return 0 if rep.verdict == "VERIFIED" else (1 if rep.verdict == "REFUTED" else 3)


def cmd_prove(args):
    from jeff_adapter import prove_identity
    res = prove_identity(args.cand, args.ref, args.vars.split(","))
    print(f"\n  Mr. → {banner(res.verdict)}  [{res.backend}]")
    print(f"  {res}")
    return 0 if res.verdict == "PROVEN" else (1 if res.verdict == "REFUTED" else 3)


def cmd_solve(args):
    from llm_adapters import get_adapter
    from mr_jeffrey import MrJeffrey, Task
    llm = get_adapter(args.backend)
    if getattr(llm, "name", "") == "scripted":
        print(f"  {C.Y}note{C.X}: no LLM backend available "
              f"(set ANTHROPIC_API_KEY or OPENAI_API_KEY to let a model write the code).")
        print("  'solve' needs a model. Offline you can still run: "
              "'mr demo', 'mr prove ...', or 'mr verify <file> --func ...'.")
        return 3
    exact = None
    if args.exact:
        parts = args.exact.split(";")
        if len(parts) == 3:
            exact = ("equiv", parts[0], parts[1], parts[2].split(","))
    task = Task(name=args.func, prompt=args.task, func_name=args.func,
                arg_kinds=args.kinds.split(","), exact_claim=exact)
    res = MrJeffrey(llm).solve(task)
    print(f"\n  Mr. → {banner(res.status if res.status != 'FAILED' else 'REFUTED')}")
    print("  " + res.summary().replace("\n", "\n  "))
    if res.final_code:
        print(f"\n  {C.B}final code:{C.X}\n" + "\n".join("    " + l for l in res.final_code.splitlines()))
    return 0 if res.status in ("PROVEN", "VERIFIED") else 1


def cmd_demo(args):
    import examples_demo
    return examples_demo.main()


def build_parser():
    p = argparse.ArgumentParser(prog="mr", description="Mr. Jeffrey — verify & prove AI code.")
    sub = p.add_subparsers(dest="cmd", required=True)

    v = sub.add_parser("verify", help="hunt for bugs in a function (bounded+fuzzed)")
    v.add_argument("file"); v.add_argument("--func", required=True)
    v.add_argument("--kinds", default="int", help="comma list: int,nonneg_int,float,str,list_int,...")
    v.add_argument("--spec", help="python file defining reference(...) / properties / arg_kinds")
    v.add_argument("--timeout", type=float, default=2.0)
    v.set_defaults(run=cmd_verify)

    pr = sub.add_parser("prove", help="EXACT-prove an identity for all inputs")
    pr.add_argument("cand"); pr.add_argument("ref"); pr.add_argument("--vars", default="n")
    pr.set_defaults(run=cmd_prove)

    s = sub.add_parser("solve", help="AI writes the function, Mr. verifies + self-corrects")
    s.add_argument("task"); s.add_argument("--func", required=True)
    s.add_argument("--kinds", default="int"); s.add_argument("--backend", default="auto")
    s.add_argument("--exact", help="exact claim 'cand;ref;vars' e.g. 'n*(n+1)/2;Sum;n'")
    s.set_defaults(run=cmd_solve)

    d = sub.add_parser("demo", help="run the 10-real-bugs showcase")
    d.set_defaults(run=cmd_demo)
    return p


def main(argv=None):
    args = build_parser().parse_args(argv)
    return args.run(args)


if __name__ == "__main__":
    sys.exit(main())
