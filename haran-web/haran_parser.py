"""
HARAN parser (STAGE H1) — source → AST, with located diagnostics.
=================================================================
Hand-written lexer + recursive-descent parser for the HARAN surface in haran-design.md
(§1.1 fn 4중 구조, §1.2 refinement/dependent types, §1.3 proc/cofix, §1.4 own/&, §1.5 fold).

Discipline (H1):
  - errors carry an EXACT line:col and parsing does NOT stop at the first one (item-level recovery);
  - NO fake success: a malformed program returns a Program whose `.errors` is non-empty;
  - the grammar is delimiter-driven (keywords / brackets / explicit operators), so newlines are
    insignificant — there is no juxtaposition application, so adjacent statements never merge.

Public API:  parse(src) -> Program     (Program.items, Program.errors, Program.ok)
"""
from __future__ import annotations

from typing import List, Optional

import haran_ast as A
from haran_ast import Span


# ----------------------------------------------------------------------------- lexer
KEYWORDS = {
    "fn", "proc", "type", "data", "requires", "ensures", "decreases", "effects", "produces",
    "match", "let", "fold", "cofix", "yield", "in", "own", "mut", "true", "false",
}
# two-char operators (checked before single-char); '..' before '.', '->' before '-', etc.
OPS2 = ["**", "->", "=>", "++", "..", "==", "!=", ">=", "<=", "&&", "||"]
OPS1 = set("(){}[],:;|&<>=+-*/%.!~")
UNI_OPS = set("∧∨¬≤≥≠≡⊂⊆∈→λ∀∃")


class Token:
    __slots__ = ("kind", "text", "line", "col")
    def __init__(self, kind, text, line, col):
        self.kind = kind      # "num" | "ident" | "kw" | "op" | "eof"
        self.text = text
        self.line = line
        self.col = col
    def __repr__(self):
        return f"<{self.kind}:{self.text!r}@{self.line}:{self.col}>"


def tokenize(src: str):
    toks: List[Token] = []
    i, n = 0, len(src)
    line, col = 1, 1
    def adv(k=1):
        nonlocal i, col
        i += k
        col += k
    while i < n:
        ch = src[i]
        if ch == "\n":
            i += 1; line += 1; col = 1; continue
        if ch in " \t\r":
            adv(); continue
        if ch == "/" and i + 1 < n and src[i + 1] == "/":   # // comment
            while i < n and src[i] != "\n":
                i += 1; col += 1
            continue
        # number
        if ch.isdigit():
            start_col = col
            j = i
            while j < n and src[j].isdigit():
                j += 1
            is_float = False
            if j + 1 < n and src[j] == "." and src[j + 1].isdigit():   # float, NOT range '..'
                is_float = True
                j += 1
                while j < n and src[j].isdigit():
                    j += 1
            text = src[i:j]
            toks.append(Token("num", text, line, start_col))
            col += (j - i); i = j
            continue
        # two-char operators
        two = src[i:i + 2]
        if two in OPS2:
            toks.append(Token("op", two, line, col)); adv(2); continue
        # single-char ascii op / unicode op
        if ch in OPS1 or ch in UNI_OPS:
            toks.append(Token("op", ch, line, col)); adv(); continue
        # identifier / keyword  (note: λ ∀ ∃ are handled above as ops, before isalpha)
        if ch == "_" or ch.isalpha():
            start_col = col
            j = i
            while j < n and (src[j] == "_" or src[j].isalnum()):
                j += 1
            text = src[i:j]
            kind = "kw" if text in KEYWORDS else "ident"
            toks.append(Token(kind, text, line, start_col))
            col += (j - i); i = j
            continue
        # unknown character — emit as an op token so the parser reports it at this line:col
        toks.append(Token("op", ch, line, col)); adv()
    toks.append(Token("eof", "<eof>", line, col))
    return toks


# ----------------------------------------------------------------------------- parser
class _ParseError(Exception):
    pass


CMP_OPS = {"=", "==", "≠", "!=", "<", "≤", "<=", ">", "≥", ">=", "≡"}
AND_OPS = {"∧", "&&"}
OR_OPS = {"∨", "||"}
ADD_OPS = {"+", "-"}
MUL_OPS = {"*", "/", "%"}
UN_OPS = {"-", "¬", "!"}
CLAUSE_KW = {"requires", "ensures", "decreases", "effects", "produces"}


class Parser:
    def __init__(self, tokens: List[Token]):
        self.toks = tokens
        self.i = 0
        self.errors: List[A.Diagnostic] = []

    # --- token cursor ---
    def peek(self, k=0) -> Token:
        j = self.i + k
        return self.toks[j] if j < len(self.toks) else self.toks[-1]
    def at(self, *texts) -> bool:
        t = self.peek()
        return t.kind in ("op", "kw") and t.text in texts
    def at_kw(self, *texts) -> bool:
        t = self.peek()
        return t.kind == "kw" and t.text in texts
    def at_kind(self, kind) -> bool:
        return self.peek().kind == kind
    def eof(self) -> bool:
        return self.peek().kind == "eof"
    def next(self) -> Token:
        t = self.peek()
        if t.kind != "eof":
            self.i += 1
        return t
    def span(self, t: Token) -> Span:
        return Span(t.line, t.col)

    # --- error / recovery ---
    def fail(self, msg: str, tok: Optional[Token] = None):
        tok = tok or self.peek()
        self.errors.append(A.Diagnostic(tok.line, tok.col, msg))
        raise _ParseError(msg)
    def expect(self, text: str, what: str = None) -> Token:
        if self.at(text):
            return self.next()
        self.fail(f"expected '{text}'" + (f" ({what})" if what else "")
                  + f", found '{self.peek().text}'")
    def expect_ident(self, what: str = "identifier") -> Token:
        if self.at_kind("ident"):
            return self.next()
        self.fail(f"expected {what}, found '{self.peek().text}'")
    def recover_to_item(self):
        # skip to the next top-level item start, so one bad item doesn't poison the rest
        while not self.eof() and not self.at_kw("fn", "proc", "type", "data"):
            self.next()

    # ============================= top level =============================
    def parse_program(self) -> A.Program:
        items = []
        while not self.eof():
            if self.at_kw("fn", "proc", "type", "data"):
                try:
                    items.append(self.parse_item())
                except _ParseError:
                    self.recover_to_item()
            else:
                # stray token at top level
                self.errors.append(A.Diagnostic(self.peek().line, self.peek().col,
                                                f"expected 'fn', 'proc', 'type' or 'data', found '{self.peek().text}'"))
                self.recover_to_item()
        return A.Program(items, self.errors)

    def parse_item(self):
        if self.at_kw("type"):
            return self.parse_type_alias()
        if self.at_kw("data"):
            return self.parse_data()
        return self.parse_fn()

    def parse_data(self) -> A.DataDecl:
        # brace form (consistent with this parser's newline-insensitive style): data Name<g> { Ctor(T,..) ... }
        start = self.next()                      # 'data'
        name = self.expect_ident("data type name").text
        generics = self.parse_generic_params() if self.at("<") else []
        self.expect("{", "constructor list")
        ctors = []
        while not self.at("}") and not self.eof():
            t = self.expect_ident("constructor name")
            arg_types = []
            if self.at("("):
                self.next()
                arg_types.append(self.parse_type())
                while self.at(","):
                    self.next()
                    arg_types.append(self.parse_type())
                self.expect(")")
            ctors.append(A.Ctor(t.text, arg_types, self.span(t)))
        self.expect("}", "close data declaration")
        return A.DataDecl(name, generics, ctors, self.span(start))

    # ============================= fn / proc =============================
    def parse_fn(self) -> A.FnDecl:
        start = self.next()                      # 'fn' | 'proc'
        kind = start.text
        name = self.expect_ident("function name").text
        generics = self.parse_generic_params() if self.at("<") else []
        self.expect("(", "parameter list")
        params = self.parse_params()
        self.expect(")")
        ret = None
        if self.at("->", "→"):
            self.next()
            ret = self.parse_type()
        requires = ensures = decreases = produces = None
        effects: List[str] = []
        while self.at_kw(*CLAUSE_KW):
            kw = self.next().text
            if kw == "effects":
                effects = self.parse_effects()
            else:
                e = self.parse_expr()
                if kw == "requires": requires = e
                elif kw == "ensures": ensures = e
                elif kw == "decreases": decreases = e
                elif kw == "produces": produces = e
        body = self.parse_block() if self.at("{") else self._fail_body()
        return A.FnDecl(kind, name, generics, params, ret, requires, ensures,
                        decreases, produces, effects, body, self.span(start))

    def _fail_body(self):
        self.fail(f"expected '{{' (function body), found '{self.peek().text}'")

    def parse_effects(self) -> List[str]:
        names = [self.expect_ident("effect kind (pure|io|state|...)").text]
        while self.at(","):
            self.next()
            names.append(self.expect_ident("effect kind").text)
        return names

    def parse_params(self) -> List[A.Param]:
        params = []
        if self.at(")"):
            return params
        while True:
            t = self.expect_ident("parameter name")
            self.expect(":", "parameter type")
            ty = self.parse_type()
            params.append(A.Param(t.text, ty, self.span(t)))
            if self.at(","):
                self.next(); continue
            break
        return params

    # ============================= types (§1.2) =============================
    def parse_type_alias(self) -> A.TypeAlias:
        start = self.next()                      # 'type'
        name = self.expect_ident("type name").text
        generics = self.parse_generic_params() if self.at("<") else []
        self.expect("=", "type alias body")
        body = self.parse_type()
        return A.TypeAlias(name, generics, body, self.span(start))

    def parse_generic_params(self) -> List[A.GenericParam]:
        self.expect("<")
        ps = []
        while not self.at(">"):
            t = self.expect_ident("generic parameter")
            kind = None
            if self.at(":"):                     # value param: n: Nat
                self.next()
                kind = self.parse_type()
            ps.append(A.GenericParam(t.text, kind, self.span(t)))
            if self.at(","):
                self.next(); continue
            break
        self.expect(">", "close generic parameters")
        return ps

    def parse_type(self):
        t = self.peek()
        if self.at_kw("own"):
            self.next()
            return A.TyOwn(self.parse_type(), self.span(t))
        if self.at("&"):
            self.next()
            mutable = False
            if self.at_kw("mut"):
                self.next(); mutable = True
            return A.TyRef(self.parse_type(), mutable, self.span(t))
        if self.at("{"):
            return self.parse_refinement()
        name = self.expect_ident("type name").text
        args = self.parse_type_args() if self.at("<") else []
        return A.TyName(name, args, self.span(t))

    def parse_refinement(self):
        start = self.expect("{")
        var = self.expect_ident("refinement variable").text
        self.expect(":", "refinement variable type")
        base = self.parse_type()
        self.expect("|", "refinement predicate (use '{ x: T | pred }')")
        pred = self.parse_expr()
        self.expect("}")
        return A.TyRefine(var, base, pred, self.span(start))

    def parse_type_args(self) -> List[A.TypeArg]:
        self.expect("<")
        args = []
        while not self.at(">"):
            t = self.peek()
            if self.at_kind("num"):              # value/term arg (e.g. Vec<Int, 3>, mod<3329>)
                # additive precedence: '<'/'>' delimit the generic — NOT comparisons here
                args.append(A.TypeArg("term", self.parse_add(), self.span(t)))
            else:
                args.append(A.TypeArg("type", self.parse_type(), self.span(t)))
            if self.at(","):
                self.next(); continue
            break
        self.expect(">", "close type arguments")
        return args

    # ============================= blocks / statements =============================
    def parse_block(self) -> A.Block:
        start = self.expect("{")
        stmts = []
        while not self.at("}") and not self.eof():
            if self.at_kw("let"):
                stmts.append(self.parse_let())
            elif self.at_kw("yield"):
                yt = self.next()
                stmts.append(A.Yield(self.parse_expr(), self.span(yt)))
            else:
                e = self.parse_expr()
                stmts.append(A.ExprStmt(e, e.span if hasattr(e, "span") else self.span(start)))
        self.expect("}", "close block")
        return A.Block(stmts, self.span(start))

    def parse_let(self) -> A.Let:
        start = self.next()                      # 'let'
        name = self.expect_ident("binding name").text
        ty = None
        if self.at(":"):
            self.next()
            ty = self.parse_type()
        self.expect("=", "binding value")
        value = self.parse_expr()
        return A.Let(name, ty, value, self.span(start))

    # ============================= expressions =============================
    def parse_expr(self):
        if self.at("λ"):
            return self.parse_lambda()
        if self.at("∀", "∃"):
            return self.parse_quant()
        return self.parse_range()

    def parse_lambda(self):
        start = self.next()                      # λ
        params = [self.expect_ident("lambda parameter").text]
        while self.at(","):
            self.next()
            params.append(self.expect_ident("lambda parameter").text)
        self.expect(".", "lambda body (use 'λx. expr')")
        body = self.parse_expr()
        return A.Lambda(params, body, self.span(start))

    def parse_quant(self):
        start = self.next()                      # ∀ | ∃
        vars_ = [self.expect_ident("quantified variable").text]
        while self.at(","):
            self.next()
            vars_.append(self.expect_ident("quantified variable").text)
        self.expect(".", "quantifier body (use '∀x. expr')")
        body = self.parse_expr()
        return A.Quant(start.text, vars_, body, self.span(start))

    def parse_range(self):
        lo = self.parse_or()
        if self.at(".."):
            t = self.next()
            hi = self.parse_or()
            return A.Range(lo, hi, self.span(t))
        return lo

    def _binary_level(self, sub, ops):
        lhs = sub()
        while self.at(*ops):
            t = self.next()
            rhs = sub()
            lhs = A.Bin(t.text, lhs, rhs, self.span(t))
        return lhs

    def parse_or(self):
        return self._binary_level(self.parse_and, OR_OPS)
    def parse_and(self):
        return self._binary_level(self.parse_cmp, AND_OPS)
    def parse_cmp(self):
        return self._binary_level(self.parse_concat, CMP_OPS)
    def parse_concat(self):
        return self._binary_level(self.parse_add, {"++"})
    def parse_add(self):
        return self._binary_level(self.parse_mul, ADD_OPS)
    def parse_mul(self):
        return self._binary_level(self.parse_unary, MUL_OPS)

    def parse_unary(self):
        if self.at(*UN_OPS):
            t = self.next()
            return A.Un(t.text, self.parse_unary(), self.span(t))
        return self.parse_pow()

    def parse_pow(self):
        base = self.parse_postfix()
        if self.at("**"):                        # right-assoc: postfix ** unary  (grammar A.8)
            t = self.next()
            return A.Bin("**", base, self.parse_unary(), self.span(t))
        return base

    def parse_postfix(self):
        e = self.parse_primary()
        while self.at("("):                      # call f(args) — HARAN has no juxtaposition/index
            t = self.next()
            args = []
            if not self.at(")"):
                args.append(self.parse_expr())
                while self.at(","):
                    self.next()
                    args.append(self.parse_expr())
            self.expect(")")
            e = A.Call(e, args, self.span(t))
        return e

    def parse_primary(self):
        t = self.peek()
        if self.at_kind("num"):
            self.next()
            return A.Num(t.text, "." in t.text, self.span(t))
        if self.at_kw("true", "false"):
            self.next()
            return A.BoolLit(t.text == "true", self.span(t))
        if self.at_kw("match"):
            return self.parse_match()
        if self.at_kw("fold"):
            return self.parse_fold()
        if self.at_kw("cofix"):
            return self.parse_cofix()
        if self.at("("):
            self.next()
            e = self.parse_expr()
            self.expect(")")
            return e
        if self.at("["):
            return self.parse_list_expr()
        if self.at("{"):
            return self.parse_block()
        if self.at_kind("ident"):
            self.next()
            return A.Var(t.text, self.span(t))
        self.fail(f"expected an expression, found '{t.text}'")

    def parse_list_expr(self):
        start = self.expect("[")
        elems = []
        if not self.at("]"):
            elems.append(self.parse_expr())
            while self.at(","):
                self.next()
                elems.append(self.parse_expr())
        self.expect("]", "close list literal")
        return A.ListLit(elems, self.span(start))

    def parse_match(self):
        start = self.next()                      # 'match'
        scrut = self.parse_expr()
        self.expect("{", "match arms")
        arms = []
        while not self.at("}") and not self.eof():
            pt = self.peek()
            pat = self.parse_pattern()
            self.expect("=>", "match arm body")
            body = self.parse_expr()
            arms.append(A.MatchArm(pat, body, self.span(pt)))
        self.expect("}", "close match")
        return A.Match(scrut, arms, self.span(start))

    def parse_fold(self):
        start = self.next()                      # 'fold'
        binder = self.expect_ident("fold binder").text
        self.expect("in", "fold domain")
        domain = self.parse_expr()
        body = self.parse_block()
        return A.Fold(binder, domain, body, self.span(start))

    def parse_cofix(self):
        start = self.next()                      # 'cofix'
        name = self.expect_ident("cofix label").text
        body = self.parse_block()
        return A.Cofix(name, body, self.span(start))

    # ============================= patterns =============================
    def parse_pattern(self):
        t = self.peek()
        if self.at("_"):
            self.next()
            return A.PWild(self.span(t))
        if self.at("["):
            self.next()
            if self.at("]"):
                self.next()
                return A.PListEmpty(self.span(t))
            first = self.parse_pattern()
            if self.at("|"):
                self.next()
                tail = self.parse_pattern()
                self.expect("]", "close cons pattern '[h|t]'")
                return A.PCons(first, tail, self.span(t))
            elems = [first]
            while self.at(","):
                self.next()
                elems.append(self.parse_pattern())
            self.expect("]", "close list pattern")
            return A.PList(elems, self.span(t))
        if self.at_kind("num"):
            self.next()
            return A.PNum(t.text, self.span(t))
        if self.at_kw("true", "false"):
            self.next()
            return A.PBool(t.text == "true", self.span(t))
        if self.at_kind("ident"):
            self.next()
            if self.at("("):                     # constructor pattern Ctor(p, ...)
                self.next()
                args = [self.parse_pattern()]
                while self.at(","):
                    self.next()
                    args.append(self.parse_pattern())
                self.expect(")", "close constructor pattern")
                return A.PCtor(t.text, args, self.span(t))
            return A.PVar(t.text, self.span(t))
        self.fail(f"expected a pattern, found '{t.text}'")


def parse(src: str) -> A.Program:
    """Parse HARAN source into a Program (with located errors). Never raises on user input."""
    return Parser(tokenize(src)).parse_program()
