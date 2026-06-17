"""
HARAN AST (STAGE H1) — the structured form of a HARAN program.
==============================================================
Per haran-design.md §1.1 every function carries FOUR parts side by side:
  fn name(args) -> Ret  requires <e>  ensures <e>  decreases <e>  effects <k>  { body }
and §1.3 adds the coinductive shell:  proc name(...) -> ...  produces <e>  effects io { cofix ... }.

The whole point (§0 공리 1, §2.1 단계 1): the SPEC (requires/ensures/decreases/produces) lives
*next to* the IMPL (body), so Mr.Jeffrey READS the intent — it never infers it. So `FnDecl` keeps the
spec clauses and the body as separate, explicit fields. Every node carries a source `Span` so
diagnostics can point at an exact line:col (H1 requirement: "파싱 에러는 정확한 line/col과 함께").
"""
from __future__ import annotations

from dataclasses import dataclass, field
from typing import List, Optional


@dataclass
class Span:
    line: int
    col: int
    end_line: int = 0
    end_col: int = 0
    def __str__(self):
        return f"{self.line}:{self.col}"


# ============================== Types (§1.2) ==============================
@dataclass
class TypeArg:
    """An argument inside `Name<...>`: a type (List<T>) or a term (Vec<Int, 3>)."""
    kind: str            # "type" | "term"
    value: object        # Ty if type, Expr if term
    span: Span


@dataclass
class TyName:
    name: str
    args: List[TypeArg]
    span: Span


@dataclass
class TyOwn:               # own Buffer  (§1.4 ownership move)
    inner: object
    span: Span


@dataclass
class TyRef:               # &Buffer / &mut Buffer  (§1.4 borrow)
    inner: object
    mutable: bool
    span: Span


@dataclass
class TyRefine:            # { x: T | pred }  (§1.2 refinement / dependent type)
    var: str
    base: object           # Ty
    pred: object           # Expr
    span: Span


@dataclass
class GenericParam:
    """A generic parameter in a `type Name<...>` header: `T` (type) or `n: Nat` (value)."""
    name: str
    kind: Optional[object]   # None => type param; Ty => value param (n: Nat)
    span: Span


# ============================== Expressions ==============================
@dataclass
class Num:
    value: str
    is_float: bool
    span: Span


@dataclass
class Var:
    name: str             # 'result' is just a Var named "result" (special at H2, not here)
    span: Span


@dataclass
class BoolLit:
    value: bool
    span: Span


@dataclass
class Bin:
    op: str               # ∧ ∨ = ≠ < ≤ > ≥ + - * / % ++ ...
    lhs: object
    rhs: object
    span: Span


@dataclass
class Un:
    op: str               # - ¬ !
    operand: object
    span: Span


@dataclass
class Call:
    func: object
    args: List[object]
    span: Span


@dataclass
class Lambda:             # λy. y ≤ p
    params: List[str]
    body: object
    span: Span


@dataclass
class Quant:             # ∀ req. ...   /   ∃ x. ...
    kind: str            # "∀" | "∃"
    vars: List[str]
    body: object
    span: Span


@dataclass
class ListLit:           # []  /  [p]  /  [a, b, c]
    elems: List[object]
    span: Span


@dataclass
class Range:             # 1..n  (fold domain)
    lo: object
    hi: object
    span: Span


@dataclass
class Block:             # { stmt* }  — also usable as an expression (last stmt is the value)
    stmts: List[object]
    span: Span


@dataclass
class Match:             # match scrut { pat => body ... }
    scrut: object
    arms: List[object]   # List[MatchArm]
    span: Span


@dataclass
class Fold:              # fold k in 1..n { body }   (§1.5)
    binder: str
    domain: object
    body: object
    span: Span


@dataclass
class Cofix:             # cofix loop { ... }   (§1.3 coinductive fixpoint)
    name: str
    body: object
    span: Span


# ============================== Statements ==============================
@dataclass
class Let:
    name: str
    ty: Optional[object]
    value: object
    span: Span


@dataclass
class Yield:             # yield resp   (productive step in a proc)
    value: object
    span: Span


@dataclass
class ExprStmt:
    value: object
    span: Span


# ============================== Patterns ==============================
@dataclass
class PWild:
    span: Span
@dataclass
class PVar:
    name: str
    span: Span
@dataclass
class PNum:
    value: str
    span: Span
@dataclass
class PBool:
    value: bool
    span: Span
@dataclass
class PListEmpty:
    span: Span
@dataclass
class PCons:            # [head | tail]
    head: object
    tail: object
    span: Span
@dataclass
class PList:            # [a, b, c]
    elems: List[object]
    span: Span
@dataclass
class PCtor:            # Ctor(p1, p2)
    name: str
    args: List[object]
    span: Span


@dataclass
class MatchArm:
    pattern: object
    body: object
    span: Span


# ============================== Items ==============================
@dataclass
class Param:
    name: str
    ty: object
    span: Span


@dataclass
class FnDecl:
    kind: str                       # "fn" (total core) | "proc" (coinductive shell)
    name: str
    generics: List[GenericParam]
    params: List[Param]
    ret: Optional[object]
    requires: Optional[object]      # spec
    ensures: Optional[object]       # spec  ← THE specification (§1.1)
    decreases: Optional[object]     # spec  (fn termination measure)
    produces: Optional[object]      # spec  (proc productivity)
    effects: List[str]              # ["pure"] | ["io"] | ...
    body: Optional[object]          # impl
    span: Span

    def spec_clauses(self) -> dict:
        """The specification half — what Mr.Jeffrey READS (no inference)."""
        return {"requires": self.requires, "ensures": self.ensures,
                "decreases": self.decreases, "produces": self.produces,
                "effects": self.effects}


@dataclass
class TypeAlias:
    name: str
    generics: List[GenericParam]
    body: object                    # Ty
    span: Span


@dataclass
class Ctor:
    name: str
    arg_types: List[object]
    span: Span


@dataclass
class DataDecl:
    name: str
    generics: List[GenericParam]
    ctors: List[Ctor]
    span: Span


@dataclass
class Diagnostic:
    line: int
    col: int
    message: str
    def __str__(self):
        return f"{self.line}:{self.col}: {self.message}"


@dataclass
class Program:
    items: List[object]
    errors: List[Diagnostic] = field(default_factory=list)

    @property
    def ok(self) -> bool:
        return not self.errors

    def fns(self) -> List[FnDecl]:
        return [it for it in self.items if isinstance(it, FnDecl)]

    def get(self, name: str) -> Optional[object]:
        for it in self.items:
            if getattr(it, "name", None) == name:
                return it
        return None
