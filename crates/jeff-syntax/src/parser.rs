//! Recursive-descent parser. Authority: CLAUDE.md APPENDIX A (grammar).
//!
//! Produces [`ast::Program`] with spans on every node (R37). Diagnostics are
//! collected with recovery (R20); never panics on user input (R38).

use crate::ast::*;
use crate::lexer::{keyword_str, Keyword as Kw, Sym, TokKind, Token};
use jeff_span::{DiagCode, Diagnostic, Span};

pub struct Parser {
    toks: Vec<Token>,
    pos: usize,
    diags: Vec<Diagnostic>,
}

impl Parser {
    /// `_file` is accepted for API symmetry with the lexer; spans already carry
    /// the file index from the tokens.
    pub fn new(toks: Vec<Token>, _file: u32) -> Self {
        Parser {
            toks,
            pos: 0,
            diags: Vec::new(),
        }
    }

    // ---- token helpers ----

    fn cur(&self) -> &Token {
        // EOF token is always present at the end (lexer guarantees it)
        &self.toks[self.pos.min(self.toks.len() - 1)]
    }
    fn kind(&self) -> &TokKind {
        &self.cur().kind
    }
    fn span(&self) -> Span {
        self.cur().span
    }
    fn at_eof(&self) -> bool {
        matches!(self.kind(), TokKind::Eof)
    }
    fn advance(&mut self) -> Token {
        let t = self.cur().clone();
        if self.pos < self.toks.len() - 1 {
            self.pos += 1;
        }
        t
    }
    fn is_sym(&self, s: Sym) -> bool {
        matches!(self.kind(), TokKind::Sym(x) if *x == s)
    }
    fn is_kw(&self, k: Kw) -> bool {
        matches!(self.kind(), TokKind::Kw(x) if *x == k)
    }
    fn eat_sym(&mut self, s: Sym) -> bool {
        if self.is_sym(s) {
            self.advance();
            true
        } else {
            false
        }
    }
    fn eat_kw(&mut self, k: Kw) -> bool {
        if self.is_kw(k) {
            self.advance();
            true
        } else {
            false
        }
    }
    fn eat_newline(&mut self) {
        while matches!(self.kind(), TokKind::Newline) {
            self.advance();
        }
    }

    fn error(&mut self, msg: impl Into<String>) {
        self.diags
            .push(Diagnostic::error(DiagCode::Parse, self.span(), msg));
    }

    fn expect_sym(&mut self, s: Sym, ctx: &str) {
        if !self.eat_sym(s) {
            self.error(format!("expected {s:?} {ctx}, found {:?}", self.kind()));
        }
    }

    fn expect_ident(&mut self, ctx: &str) -> Ident {
        if let TokKind::Ident(name) = self.kind().clone() {
            self.advance();
            name
        } else {
            self.error(format!("expected identifier {ctx}, found {:?}", self.kind()));
            "<error>".to_string()
        }
    }

    /// Recover to the next NEWLINE / DEDENT / EOF (R20).
    fn recover_line(&mut self) {
        while !matches!(
            self.kind(),
            TokKind::Newline | TokKind::Dedent | TokKind::Eof
        ) {
            self.advance();
        }
        self.eat_newline();
    }

    // ---- program ----

    pub fn parse_program(mut self) -> Result<Program, Vec<Diagnostic>> {
        let start = self.span();
        self.eat_newline();
        let module = if self.is_kw(Kw::Module) {
            self.advance();
            let dn = self.parse_dotted_name();
            self.eat_newline();
            Some(dn)
        } else {
            None
        };
        let mut imports = Vec::new();
        while self.is_kw(Kw::Import) {
            imports.push(self.parse_import());
            self.eat_newline();
        }
        let mut items = Vec::new();
        while !self.at_eof() {
            self.eat_newline();
            if self.at_eof() {
                break;
            }
            match self.parse_item() {
                Some(it) => items.push(it),
                None => self.recover_line(),
            }
        }
        let program = Program {
            module,
            imports,
            items,
            span: start.merge(self.span()),
        };
        if self.diags.iter().any(Diagnostic::is_error) {
            Err(self.diags)
        } else {
            Ok(program)
        }
    }

    fn parse_dotted_name(&mut self) -> DottedName {
        let start = self.span();
        let mut parts = vec![self.expect_path_segment("in dotted name")];
        while self.eat_sym(Sym::Dot) {
            parts.push(self.expect_path_segment("after '.'"));
        }
        DottedName {
            parts,
            span: start.merge(self.span()),
        }
    }

    /// A path segment may be an identifier or a contextual keyword (e.g. the
    /// stdlib module `core.fold`).
    fn expect_path_segment(&mut self, ctx: &str) -> Ident {
        match self.kind().clone() {
            TokKind::Ident(name) => {
                self.advance();
                name
            }
            TokKind::Kw(k) => {
                self.advance();
                keyword_str(k).to_string()
            }
            _ => {
                self.error(format!("expected a name {ctx}, found {:?}", self.kind()));
                "<error>".to_string()
            }
        }
    }

    fn parse_import(&mut self) -> Import {
        let start = self.span();
        self.advance(); // import
        let path = self.parse_dotted_name();
        let mut names = Vec::new();
        if self.eat_sym(Sym::LParen) {
            while !self.is_sym(Sym::RParen) && !self.at_eof() {
                names.push(self.expect_ident("in import list"));
                if !self.eat_sym(Sym::Comma) {
                    break;
                }
            }
            self.expect_sym(Sym::RParen, "to close import list");
        }
        Import {
            path,
            names,
            span: start.merge(self.span()),
        }
    }

    // ---- items ----

    fn parse_item(&mut self) -> Option<Item> {
        // attributes (only meaningful for fn)
        let mut attrs = Vec::new();
        while self.is_sym(Sym::At) {
            attrs.push(self.parse_attr());
        }
        if self.is_kw(Kw::Total) || self.is_kw(Kw::Fn) {
            return Some(Item::Fn(self.parse_fn(attrs)));
        }
        if !attrs.is_empty() {
            self.error("attributes can only precede a function");
        }
        if self.is_kw(Kw::Data) {
            Some(Item::Data(self.parse_data()))
        } else if self.is_kw(Kw::Codata) {
            Some(Item::Codata(self.parse_codata()))
        } else if self.is_kw(Kw::Type) {
            Some(Item::TypeAlias(self.parse_type_alias()))
        } else if self.is_kw(Kw::Const) {
            Some(Item::Const(self.parse_const()))
        } else {
            self.error(format!("expected a top-level item, found {:?}", self.kind()));
            None
        }
    }

    fn parse_attr(&mut self) -> Attr {
        let start = self.span();
        self.advance(); // @
        let name = self.expect_ident("in attribute");
        let mut args = Vec::new();
        if self.eat_sym(Sym::LParen) {
            while !self.is_sym(Sym::RParen) && !self.at_eof() {
                let key = self.expect_ident("in attribute argument");
                if self.eat_sym(Sym::Eq) {
                    let val = self.expect_ident("after '=' in attribute");
                    args.push(AttrArg::KeyVal(key, val));
                } else {
                    args.push(AttrArg::Flag(key));
                }
                if !self.eat_sym(Sym::Comma) {
                    break;
                }
            }
            self.expect_sym(Sym::RParen, "to close attribute");
        }
        Attr {
            name,
            args,
            span: start.merge(self.span()),
        }
    }

    fn parse_fn(&mut self, attrs: Vec<Attr>) -> FnDecl {
        let start = self.span();
        let mode = if self.eat_kw(Kw::Total) {
            Mode::Total
        } else {
            Mode::General
        };
        self.expect_kw(Kw::Fn, "to begin a function");
        let name = self.expect_ident("after 'fn'");
        let generics = self.parse_generics();
        self.expect_sym(Sym::LParen, "to begin parameter list");
        let mut params = Vec::new();
        while !self.is_sym(Sym::RParen) && !self.at_eof() {
            let pstart = self.span();
            let pname = self.expect_ident("in parameter");
            self.expect_sym(Sym::Colon, "after parameter name");
            let ty = self.parse_type();
            params.push(Param {
                name: pname,
                ty,
                span: pstart.merge(self.span()),
            });
            if !self.eat_sym(Sym::Comma) {
                break;
            }
        }
        self.expect_sym(Sym::RParen, "to close parameter list");
        let ret = if self.eat_sym(Sym::Arrow) {
            Some(self.parse_type())
        } else {
            None
        };
        let effect = self.parse_effect_row();
        self.expect_sym(Sym::Colon, "before function body");
        let body = self.parse_block();
        FnDecl {
            attrs,
            mode,
            name,
            generics,
            params,
            ret,
            effect,
            body,
            span: start.merge(self.span()),
        }
    }

    fn expect_kw(&mut self, k: Kw, ctx: &str) {
        if !self.eat_kw(k) {
            self.error(format!("expected '{k:?}' {ctx}"));
        }
    }

    fn parse_generics(&mut self) -> Vec<GenericParam> {
        let mut out = Vec::new();
        if self.eat_sym(Sym::LBracket) {
            while !self.is_sym(Sym::RBracket) && !self.at_eof() {
                let gstart = self.span();
                let name = self.expect_ident("in generic parameter");
                let kind = if self.eat_sym(Sym::Colon) {
                    // kind ::= "type" | "nat"
                    match self.kind().clone() {
                        TokKind::Kw(Kw::Type) => {
                            self.advance();
                            Kind::Type
                        }
                        TokKind::Ident(s) if s == "nat" => {
                            self.advance();
                            Kind::Nat
                        }
                        TokKind::Ident(s) if s == "type" => {
                            self.advance();
                            Kind::Type
                        }
                        _ => {
                            self.error("generic kind must be 'type' or 'nat'");
                            Kind::Type
                        }
                    }
                } else {
                    Kind::Type
                };
                out.push(GenericParam {
                    name,
                    kind,
                    span: gstart.merge(self.span()),
                });
                if !self.eat_sym(Sym::Comma) {
                    break;
                }
            }
            self.expect_sym(Sym::RBracket, "to close generics");
        }
        out
    }

    fn parse_effect_row(&mut self) -> EffectRow {
        if !self.is_sym(Sym::Slash) {
            return EffectRow::default();
        }
        let start = self.span();
        self.advance(); // /
        self.expect_sym(Sym::LBrace, "to begin effect row");
        let mut labels = Vec::new();
        while !self.is_sym(Sym::RBrace) && !self.at_eof() {
            let eff = match self.kind().clone() {
                TokKind::Kw(Kw::IO) => Effect::IO,
                TokKind::Kw(Kw::Div) => Effect::Div,
                TokKind::Kw(Kw::Alloc) => Effect::Alloc,
                TokKind::Kw(Kw::Rand) => Effect::Rand,
                TokKind::Kw(Kw::Unsafe) => Effect::Unsafe,
                other => {
                    self.error(format!("unknown effect label {other:?}"));
                    self.advance();
                    continue;
                }
            };
            self.advance();
            labels.push(eff);
            if !self.eat_sym(Sym::Comma) {
                break;
            }
        }
        self.expect_sym(Sym::RBrace, "to close effect row");
        EffectRow {
            labels,
            span: Some(start.merge(self.span())),
        }
    }

    fn parse_data(&mut self) -> DataDecl {
        let start = self.span();
        self.advance(); // data
        let name = self.expect_ident("after 'data'");
        let generics = self.parse_generics();
        self.expect_sym(Sym::Colon, "after data name");
        let mut ctors = Vec::new();
        if matches!(self.kind(), TokKind::Newline) {
            self.eat_newline();
            self.expect_indent("data body");
            while !matches!(self.kind(), TokKind::Dedent | TokKind::Eof) {
                if matches!(self.kind(), TokKind::Newline) {
                    self.advance();
                    continue;
                }
                let cstart = self.span();
                let cname = self.expect_ident("in constructor");
                let mut fields = Vec::new();
                if self.eat_sym(Sym::LParen) {
                    while !self.is_sym(Sym::RParen) && !self.at_eof() {
                        fields.push(self.parse_type());
                        if !self.eat_sym(Sym::Comma) {
                            break;
                        }
                    }
                    self.expect_sym(Sym::RParen, "to close constructor fields");
                }
                ctors.push(Constructor {
                    name: cname,
                    fields,
                    span: cstart.merge(self.span()),
                });
                self.eat_newline();
            }
            self.eat_dedent();
        }
        DataDecl {
            name,
            generics,
            ctors,
            span: start.merge(self.span()),
        }
    }

    fn parse_codata(&mut self) -> CodataDecl {
        let start = self.span();
        self.advance(); // codata
        let name = self.expect_ident("after 'codata'");
        let generics = self.parse_generics();
        self.expect_sym(Sym::Colon, "after codata name");
        let mut destructors = Vec::new();
        if matches!(self.kind(), TokKind::Newline) {
            self.eat_newline();
            self.expect_indent("codata body");
            while !matches!(self.kind(), TokKind::Dedent | TokKind::Eof) {
                if matches!(self.kind(), TokKind::Newline) {
                    self.advance();
                    continue;
                }
                let dstart = self.span();
                let dname = self.expect_ident("in destructor");
                self.expect_sym(Sym::Colon, "after destructor name");
                let ty = self.parse_type();
                destructors.push(Destructor {
                    name: dname,
                    ty,
                    span: dstart.merge(self.span()),
                });
                self.eat_newline();
            }
            self.eat_dedent();
        }
        CodataDecl {
            name,
            generics,
            destructors,
            span: start.merge(self.span()),
        }
    }

    fn parse_type_alias(&mut self) -> TypeAlias {
        let start = self.span();
        self.advance(); // type
        let name = self.expect_ident("after 'type'");
        let generics = self.parse_generics();
        self.expect_sym(Sym::Eq, "in type alias");
        let target = self.parse_type();
        self.eat_newline();
        TypeAlias {
            name,
            generics,
            target,
            span: start.merge(self.span()),
        }
    }

    fn parse_const(&mut self) -> ConstDecl {
        let start = self.span();
        self.advance(); // const
        let name = self.expect_ident("after 'const'");
        self.expect_sym(Sym::Colon, "after const name");
        let ty = self.parse_type();
        self.expect_sym(Sym::Eq, "in const declaration");
        let value = self.parse_expr();
        self.eat_newline();
        ConstDecl {
            name,
            ty,
            value,
            span: start.merge(self.span()),
        }
    }

    fn expect_indent(&mut self, ctx: &str) {
        if !matches!(self.kind(), TokKind::Indent) {
            self.error(format!("expected an indented block for {ctx}"));
        } else {
            self.advance();
        }
    }
    fn eat_dedent(&mut self) {
        if matches!(self.kind(), TokKind::Dedent) {
            self.advance();
        }
    }

    // ---- types (A.6) ----

    fn parse_type(&mut self) -> Type {
        let start = self.span();
        // ref types
        if self.eat_sym(Sym::Amp) {
            let mutable = self.eat_kw_word("mut");
            let inner = Box::new(self.parse_type());
            return Type {
                kind: TypeKind::Ref { mutable, inner },
                span: start.merge(self.span()),
            };
        }
        if self.eat_kw(Kw::Own) {
            let inner = Box::new(self.parse_type());
            return Type {
                kind: TypeKind::Own(inner),
                span: start.merge(self.span()),
            };
        }
        if self.eat_kw(Kw::Secret) {
            self.expect_sym(Sym::LBracket, "after 'secret'");
            let inner = Box::new(self.parse_type());
            self.expect_sym(Sym::RBracket, "to close secret[...]");
            return Type {
                kind: TypeKind::Secret(inner),
                span: start.merge(self.span()),
            };
        }
        if self.is_sym(Sym::LBrace) {
            return self.parse_refinement(start);
        }
        // app_type / base_type
        let name = self.expect_ident("in type");
        // mod[q] / bv[n] base types
        if name == "mod" || name == "bv" {
            self.expect_sym(Sym::LBracket, &format!("after '{name}'"));
            let e = self.parse_expr();
            self.expect_sym(Sym::RBracket, "to close type index");
            let base = if name == "mod" {
                BaseType::Mod(Box::new(e))
            } else {
                BaseType::Bv(Box::new(e))
            };
            return Type {
                kind: TypeKind::Base(base),
                span: start.merge(self.span()),
            };
        }
        // type args?
        if self.is_sym(Sym::LBracket) {
            self.advance();
            let mut args = Vec::new();
            while !self.is_sym(Sym::RBracket) && !self.at_eof() {
                args.push(self.parse_type_arg());
                if !self.eat_sym(Sym::Comma) {
                    break;
                }
            }
            self.expect_sym(Sym::RBracket, "to close type arguments");
            return Type {
                kind: TypeKind::App(name, args),
                span: start.merge(self.span()),
            };
        }
        // bare base type or nullary user type
        let kind = match base_type_of(&name) {
            Some(b) => TypeKind::Base(b),
            None => TypeKind::App(name, Vec::new()),
        };
        Type {
            kind,
            span: start.merge(self.span()),
        }
    }

    fn eat_kw_word(&mut self, word: &str) -> bool {
        if let TokKind::Ident(s) = self.kind() {
            if s == word {
                self.advance();
                return true;
            }
        }
        false
    }

    fn parse_refinement(&mut self, start: Span) -> Type {
        self.advance(); // {
        let var = self.expect_ident("in refinement");
        self.expect_sym(Sym::Colon, "in refinement");
        let base_name = self.expect_ident("base type in refinement");
        let base = base_type_of(&base_name).unwrap_or(BaseType::Int);
        self.expect_sym(Sym::Pipe, "in refinement (| predicate)");
        let pred = Box::new(self.parse_expr());
        self.expect_sym(Sym::RBrace, "to close refinement");
        Type {
            kind: TypeKind::Refine { var, base, pred },
            span: start.merge(self.span()),
        }
    }

    /// Disambiguate type-vs-term args (DR6 heuristic): integer literals and
    /// lowercase non-base identifiers are type-level *terms* (nats); base names,
    /// uppercase names, `&`, `secret`, `{` start *types*.
    fn parse_type_arg(&mut self) -> TypeArg {
        match self.kind().clone() {
            TokKind::Int(..) | TokKind::Sym(Sym::Minus) => TypeArg::Term(self.parse_expr()),
            TokKind::Ident(name) => {
                let is_base = base_type_of(&name).is_some() || name == "mod" || name == "bv";
                let uppercase = name.chars().next().is_some_and(|c| c.is_uppercase());
                if is_base || uppercase {
                    TypeArg::Ty(self.parse_type())
                } else {
                    TypeArg::Term(self.parse_expr())
                }
            }
            _ => TypeArg::Ty(self.parse_type()),
        }
    }

    // ---- statements ----

    fn parse_block(&mut self) -> Block {
        let start = self.span();
        if matches!(self.kind(), TokKind::Newline) {
            self.eat_newline();
            self.expect_indent("block");
            let mut stmts = Vec::new();
            while !matches!(self.kind(), TokKind::Dedent | TokKind::Eof) {
                if matches!(self.kind(), TokKind::Newline) {
                    self.advance();
                    continue;
                }
                stmts.push(self.parse_stmt());
            }
            self.eat_dedent();
            Block {
                stmts,
                span: start.merge(self.span()),
            }
        } else {
            // single-expression body
            let e = self.parse_expr();
            let span = e.span;
            self.eat_newline();
            Block {
                stmts: vec![Stmt {
                    kind: StmtKind::Expr(e),
                    span,
                }],
                span: start.merge(self.span()),
            }
        }
    }

    fn parse_stmt(&mut self) -> Stmt {
        let start = self.span();
        let kind = match self.kind().clone() {
            TokKind::Kw(Kw::Let) => {
                self.advance();
                let pat = self.parse_pattern();
                let ty = if self.eat_sym(Sym::Colon) {
                    Some(self.parse_type())
                } else {
                    None
                };
                self.expect_sym(Sym::Eq, "in let binding");
                let init = self.parse_expr();
                StmtKind::Let { pat, ty, init }
            }
            TokKind::Kw(Kw::Var) => {
                self.advance();
                let name = self.expect_ident("after 'var'");
                self.expect_sym(Sym::Colon, "after var name");
                let ty = self.parse_type();
                self.expect_sym(Sym::Eq, "in var binding");
                let init = self.parse_expr();
                StmtKind::Var { name, ty, init }
            }
            TokKind::Kw(Kw::For) => {
                self.advance();
                let pat = self.parse_pattern();
                self.expect_kw(Kw::In, "in for loop");
                let iter = self.parse_expr();
                self.expect_sym(Sym::Colon, "before for body");
                let body = self.parse_block();
                StmtKind::For { pat, iter, body }
            }
            TokKind::Kw(Kw::While) => {
                self.advance();
                let cond = self.parse_expr();
                self.expect_sym(Sym::Colon, "before while body");
                let body = self.parse_block();
                StmtKind::While { cond, body }
            }
            TokKind::Kw(Kw::Return) => {
                self.advance();
                if matches!(self.kind(), TokKind::Newline | TokKind::Dedent | TokKind::Eof) {
                    StmtKind::Return(None)
                } else {
                    StmtKind::Return(Some(self.parse_expr()))
                }
            }
            _ => {
                let e = self.parse_expr();
                if let Some(op) = self.assign_op() {
                    self.advance();
                    let rhs = self.parse_expr();
                    match expr_to_lvalue(&e) {
                        Some(lhs) => StmtKind::Assign { lhs, op, rhs },
                        None => {
                            self.error("left-hand side of assignment is not an l-value");
                            StmtKind::Expr(e)
                        }
                    }
                } else {
                    StmtKind::Expr(e)
                }
            }
        };
        self.eat_newline();
        Stmt {
            kind,
            span: start.merge(self.span()),
        }
    }

    fn assign_op(&self) -> Option<AssignOp> {
        let TokKind::Sym(s) = self.kind() else {
            return None;
        };
        Some(match s {
            Sym::Eq => AssignOp::Eq,
            Sym::PlusEq => AssignOp::Add,
            Sym::MinusEq => AssignOp::Sub,
            Sym::StarEq => AssignOp::Mul,
            Sym::SlashEq => AssignOp::Div,
            Sym::PercentEq => AssignOp::Rem,
            Sym::CaretEq => AssignOp::Xor,
            Sym::AmpEq => AssignOp::And,
            Sym::PipeEq => AssignOp::Or,
            _ => return None,
        })
    }

    // ---- expressions (A.8) ----

    pub fn parse_expr(&mut self) -> Expr {
        let e = self.parse_or();
        // range tail: a..b | a..=b
        if self.is_sym(Sym::DotDot) || self.is_sym(Sym::DotDotEq) {
            let inclusive = self.is_sym(Sym::DotDotEq);
            self.advance();
            let hi = self.parse_or();
            let span = e.span.merge(hi.span);
            return Expr {
                kind: ExprKind::Range {
                    lo: Box::new(e),
                    hi: Box::new(hi),
                    inclusive,
                },
                span,
            };
        }
        e
    }

    fn parse_or(&mut self) -> Expr {
        let mut lhs = self.parse_and();
        while self.is_kw(Kw::Or) {
            self.advance();
            let rhs = self.parse_and();
            lhs = bin(BinOp::Or, lhs, rhs);
        }
        lhs
    }
    fn parse_and(&mut self) -> Expr {
        let mut lhs = self.parse_not();
        while self.is_kw(Kw::And) {
            self.advance();
            let rhs = self.parse_not();
            lhs = bin(BinOp::And, lhs, rhs);
        }
        lhs
    }
    fn parse_not(&mut self) -> Expr {
        if self.is_kw(Kw::Not) {
            let start = self.span();
            self.advance();
            let inner = self.parse_not();
            let span = start.merge(inner.span);
            return Expr {
                kind: ExprKind::Un(UnOp::Not, Box::new(inner)),
                span,
            };
        }
        self.parse_cmp()
    }
    fn parse_cmp(&mut self) -> Expr {
        let mut lhs = self.parse_bitor();
        while let Some(op) = self.cmp_binop() {
            self.advance();
            let rhs = self.parse_bitor();
            lhs = bin(op, lhs, rhs);
        }
        lhs
    }
    fn cmp_binop(&self) -> Option<BinOp> {
        let TokKind::Sym(s) = self.kind() else {
            return None;
        };
        Some(match s {
            Sym::EqEq => BinOp::Eq,
            Sym::Ne => BinOp::Ne,
            Sym::Lt => BinOp::Lt,
            Sym::Le => BinOp::Le,
            Sym::Gt => BinOp::Gt,
            Sym::Ge => BinOp::Ge,
            _ => return None,
        })
    }
    fn parse_bitor(&mut self) -> Expr {
        let mut lhs = self.parse_bitxor();
        while self.is_sym(Sym::Pipe) {
            self.advance();
            let rhs = self.parse_bitxor();
            lhs = bin(BinOp::BitOr, lhs, rhs);
        }
        lhs
    }
    fn parse_bitxor(&mut self) -> Expr {
        let mut lhs = self.parse_bitand();
        while self.is_sym(Sym::Caret) {
            self.advance();
            let rhs = self.parse_bitand();
            lhs = bin(BinOp::BitXor, lhs, rhs);
        }
        lhs
    }
    fn parse_bitand(&mut self) -> Expr {
        let mut lhs = self.parse_shift();
        while self.is_sym(Sym::Amp) {
            self.advance();
            let rhs = self.parse_shift();
            lhs = bin(BinOp::BitAnd, lhs, rhs);
        }
        lhs
    }
    fn parse_shift(&mut self) -> Expr {
        let mut lhs = self.parse_add();
        loop {
            let op = if self.is_sym(Sym::Shl) {
                BinOp::Shl
            } else if self.is_sym(Sym::Shr) {
                BinOp::Shr
            } else {
                break;
            };
            self.advance();
            let rhs = self.parse_add();
            lhs = bin(op, lhs, rhs);
        }
        lhs
    }
    fn parse_add(&mut self) -> Expr {
        let mut lhs = self.parse_mul();
        loop {
            let op = if self.is_sym(Sym::Plus) {
                BinOp::Add
            } else if self.is_sym(Sym::Minus) {
                BinOp::Sub
            } else {
                break;
            };
            self.advance();
            let rhs = self.parse_mul();
            lhs = bin(op, lhs, rhs);
        }
        lhs
    }
    fn parse_mul(&mut self) -> Expr {
        let mut lhs = self.parse_unary();
        loop {
            let op = if self.is_sym(Sym::Star) {
                BinOp::Mul
            } else if self.is_sym(Sym::Slash) {
                BinOp::Div
            } else if self.is_sym(Sym::SlashSlash) {
                BinOp::FloorDiv
            } else if self.is_sym(Sym::Percent) {
                BinOp::Rem
            } else {
                break;
            };
            self.advance();
            let rhs = self.parse_unary();
            lhs = bin(op, lhs, rhs);
        }
        lhs
    }
    fn parse_unary(&mut self) -> Expr {
        let start = self.span();
        if self.is_sym(Sym::Minus) {
            self.advance();
            let inner = self.parse_unary();
            let span = start.merge(inner.span);
            return Expr {
                kind: ExprKind::Un(UnOp::Neg, Box::new(inner)),
                span,
            };
        }
        if self.is_sym(Sym::Tilde) {
            self.advance();
            let inner = self.parse_unary();
            let span = start.merge(inner.span);
            return Expr {
                kind: ExprKind::Un(UnOp::BitNot, Box::new(inner)),
                span,
            };
        }
        self.parse_pow()
    }
    fn parse_pow(&mut self) -> Expr {
        let base = self.parse_postfix();
        if self.is_sym(Sym::StarStar) {
            self.advance();
            let exp = self.parse_unary(); // right-assoc
            let span = base.span.merge(exp.span);
            return Expr {
                kind: ExprKind::Bin(BinOp::Pow, Box::new(base), Box::new(exp)),
                span,
            };
        }
        base
    }
    fn parse_postfix(&mut self) -> Expr {
        let mut e = self.parse_primary();
        loop {
            if self.is_sym(Sym::LParen) {
                self.advance();
                let mut args = Vec::new();
                while !self.is_sym(Sym::RParen) && !self.at_eof() {
                    args.push(self.parse_expr());
                    if !self.eat_sym(Sym::Comma) {
                        break;
                    }
                }
                self.expect_sym(Sym::RParen, "to close call");
                let span = e.span.merge(self.span());
                e = Expr {
                    kind: ExprKind::Call(Box::new(e), args),
                    span,
                };
            } else if self.is_sym(Sym::LBracket) {
                self.advance();
                let idx = self.parse_expr();
                self.expect_sym(Sym::RBracket, "to close index");
                let span = e.span.merge(self.span());
                e = Expr {
                    kind: ExprKind::Index(Box::new(e), Box::new(idx)),
                    span,
                };
            } else if self.is_sym(Sym::Dot) {
                self.advance();
                let field = self.expect_ident("after '.'");
                let span = e.span.merge(self.span());
                e = Expr {
                    kind: ExprKind::Field(Box::new(e), field),
                    span,
                };
            } else {
                break;
            }
        }
        e
    }

    fn parse_primary(&mut self) -> Expr {
        let start = self.span();
        match self.kind().clone() {
            TokKind::Kw(Kw::Sum) => self.parse_reduction(RedKind::Sum, start),
            TokKind::Kw(Kw::Prod) => self.parse_reduction(RedKind::Prod, start),
            TokKind::Kw(Kw::Count) => self.parse_reduction(RedKind::Count, start),
            TokKind::Kw(Kw::Fold) => self.parse_reduction(RedKind::Fold, start),
            TokKind::Kw(Kw::Match) => self.parse_match(start),
            TokKind::Kw(Kw::Own) => {
                self.advance();
                let inner = self.parse_unary();
                let span = start.merge(inner.span);
                Expr {
                    kind: ExprKind::Own(Box::new(inner)),
                    span,
                }
            }
            TokKind::Kw(Kw::Move) => {
                self.advance();
                self.expect_sym(Sym::LParen, "after 'move'");
                let inner = self.parse_expr();
                self.expect_sym(Sym::RParen, "to close move(...)");
                let span = start.merge(self.span());
                Expr {
                    kind: ExprKind::Move(Box::new(inner)),
                    span,
                }
            }
            TokKind::Sym(Sym::Amp) => {
                self.advance();
                let mutable = self.eat_kw_word("mut");
                let inner = self.parse_unary();
                let span = start.merge(inner.span);
                Expr {
                    kind: ExprKind::Borrow {
                        mutable,
                        inner: Box::new(inner),
                    },
                    span,
                }
            }
            TokKind::Kw(Kw::True) => {
                self.advance();
                lit(Lit::Bool(true), start)
            }
            TokKind::Kw(Kw::False) => {
                self.advance();
                lit(Lit::Bool(false), start)
            }
            TokKind::Int(s, suf) => {
                self.advance();
                if let Some(stripped) = s.strip_prefix("0b") {
                    lit(Lit::Bv(stripped.to_string()), start)
                } else {
                    lit(Lit::Int(s, suf), start)
                }
            }
            TokKind::Float(s) => {
                self.advance();
                lit(Lit::Float(s), start)
            }
            TokKind::Str(s) => {
                self.advance();
                lit(Lit::Str(s), start)
            }
            TokKind::Char(c) => {
                self.advance();
                lit(Lit::Char(c), start)
            }
            TokKind::Ident(name) => {
                self.advance();
                Expr {
                    kind: ExprKind::Var(name),
                    span: start,
                }
            }
            TokKind::Sym(Sym::LParen) => {
                self.advance();
                let inner = self.parse_expr();
                self.expect_sym(Sym::RParen, "to close parenthesised expression");
                let span = start.merge(self.span());
                Expr {
                    kind: ExprKind::Paren(Box::new(inner)),
                    span,
                }
            }
            other => {
                self.error(format!("expected an expression, found {other:?}"));
                self.advance();
                lit(Lit::Int("0".to_string(), None), start)
            }
        }
    }

    fn parse_reduction(&mut self, kind: RedKind, start: Span) -> Expr {
        self.advance(); // sum/prod/count/fold
        let mut binder = Vec::new();
        if self.eat_sym(Sym::LParen) {
            while !self.is_sym(Sym::RParen) && !self.at_eof() {
                binder.push(self.parse_pattern());
                if !self.eat_sym(Sym::Comma) {
                    break;
                }
            }
            self.expect_sym(Sym::RParen, "to close binder tuple");
        } else {
            binder.push(self.parse_pattern());
        }
        self.expect_kw(Kw::In, "in reduction");
        let domain = self.parse_domain();
        self.expect_sym(Sym::Colon, "before reduction body");
        let body = self.parse_expr();
        let span = start.merge(body.span);
        Expr {
            kind: ExprKind::Reduction {
                kind,
                binder,
                domain,
                body: Box::new(body),
            },
            span,
        }
    }

    fn parse_domain(&mut self) -> Domain {
        if self.is_sym(Sym::LBrace) {
            self.advance();
            let mut constraints = Vec::new();
            while !self.is_sym(Sym::RBrace) && !self.at_eof() {
                constraints.push(self.parse_constraint());
                if !self.eat_sym(Sym::Comma) {
                    break;
                }
            }
            self.expect_sym(Sym::RBrace, "to close set domain");
            Domain::Set(constraints)
        } else {
            let e = self.parse_expr();
            if matches!(e.kind, ExprKind::Range { .. }) {
                Domain::Range(Box::new(e))
            } else {
                Domain::Expr(Box::new(e))
            }
        }
    }

    fn parse_constraint(&mut self) -> Constraint {
        let start = self.span();
        let head = self.parse_add();
        let mut parts = Vec::new();
        while let Some(op) = self.cmp_op() {
            self.advance();
            let rhs = self.parse_add();
            parts.push((op, rhs));
        }
        Constraint {
            head,
            parts,
            span: start.merge(self.span()),
        }
    }

    fn cmp_op(&self) -> Option<CmpOp> {
        let TokKind::Sym(s) = self.kind() else {
            return None;
        };
        Some(match s {
            Sym::EqEq => CmpOp::Eq,
            Sym::Ne => CmpOp::Ne,
            Sym::Lt => CmpOp::Lt,
            Sym::Le => CmpOp::Le,
            Sym::Gt => CmpOp::Gt,
            Sym::Ge => CmpOp::Ge,
            _ => return None,
        })
    }

    fn parse_match(&mut self, start: Span) -> Expr {
        self.advance(); // match
        let scrut = self.parse_expr();
        self.expect_sym(Sym::Colon, "after match scrutinee");
        let mut arms = Vec::new();
        if matches!(self.kind(), TokKind::Newline) {
            self.eat_newline();
            self.expect_indent("match body");
            while !matches!(self.kind(), TokKind::Dedent | TokKind::Eof) {
                if matches!(self.kind(), TokKind::Newline) {
                    self.advance();
                    continue;
                }
                let astart = self.span();
                let pat = self.parse_pattern();
                self.expect_sym(Sym::FatArrow, "in match arm");
                let body = self.parse_block();
                arms.push(Arm {
                    pat,
                    body,
                    span: astart.merge(self.span()),
                });
            }
            self.eat_dedent();
        }
        let span = start.merge(self.span());
        Expr {
            kind: ExprKind::Match {
                scrut: Box::new(scrut),
                arms,
            },
            span,
        }
    }

    fn parse_pattern(&mut self) -> Pattern {
        let start = self.span();
        match self.kind().clone() {
            TokKind::Ident(name) if name == "_" => {
                self.advance();
                Pattern {
                    kind: PatternKind::Wild,
                    span: start,
                }
            }
            TokKind::Ident(name) => {
                self.advance();
                // ctor with args?
                if self.is_sym(Sym::LParen) {
                    self.advance();
                    let mut args = Vec::new();
                    while !self.is_sym(Sym::RParen) && !self.at_eof() {
                        args.push(self.parse_pattern());
                        if !self.eat_sym(Sym::Comma) {
                            break;
                        }
                    }
                    self.expect_sym(Sym::RParen, "to close constructor pattern");
                    return Pattern {
                        kind: PatternKind::Ctor(name, args),
                        span: start.merge(self.span()),
                    };
                }
                // nat_succ: ident '+' int
                if self.is_sym(Sym::Plus) {
                    if let TokKind::Int(s, _) = &self.toks[self.pos + 1].kind {
                        if let Ok(k) = s.parse::<u64>() {
                            self.advance(); // +
                            self.advance(); // int
                            return Pattern {
                                kind: PatternKind::NatSucc(name, k),
                                span: start.merge(self.span()),
                            };
                        }
                    }
                }
                let upper = name.chars().next().is_some_and(|c| c.is_uppercase());
                let kind = if upper {
                    PatternKind::Ctor(name, Vec::new())
                } else {
                    PatternKind::Var(name)
                };
                Pattern { kind, span: start }
            }
            TokKind::Int(s, suf) => {
                self.advance();
                Pattern {
                    kind: PatternKind::Lit(Lit::Int(s, suf)),
                    span: start,
                }
            }
            TokKind::Kw(Kw::True) => {
                self.advance();
                Pattern {
                    kind: PatternKind::Lit(Lit::Bool(true)),
                    span: start,
                }
            }
            TokKind::Kw(Kw::False) => {
                self.advance();
                Pattern {
                    kind: PatternKind::Lit(Lit::Bool(false)),
                    span: start,
                }
            }
            other => {
                self.error(format!("expected a pattern, found {other:?}"));
                self.advance();
                Pattern {
                    kind: PatternKind::Wild,
                    span: start,
                }
            }
        }
    }
}

fn bin(op: BinOp, l: Expr, r: Expr) -> Expr {
    let span = l.span.merge(r.span);
    Expr {
        kind: ExprKind::Bin(op, Box::new(l), Box::new(r)),
        span,
    }
}
fn lit(l: Lit, span: Span) -> Expr {
    Expr {
        kind: ExprKind::Lit(l),
        span,
    }
}

fn base_type_of(name: &str) -> Option<BaseType> {
    Some(match name {
        "i8" => BaseType::I(8),
        "i16" => BaseType::I(16),
        "i32" => BaseType::I(32),
        "i64" => BaseType::I(64),
        "u8" => BaseType::U(8),
        "u16" => BaseType::U(16),
        "u32" => BaseType::U(32),
        "u64" => BaseType::U(64),
        "int" => BaseType::Int,
        "nat" => BaseType::Nat,
        "rat" => BaseType::Rat,
        "bool" => BaseType::Bool,
        "char" => BaseType::Char,
        "str" => BaseType::Str,
        "f32" => BaseType::F32,
        "f64" => BaseType::F64,
        "Vec" => BaseType::Vec,
        "Fin" => BaseType::Fin,
        "Mat" => BaseType::Mat,
        _ => return None,
    })
}

fn expr_to_lvalue(e: &Expr) -> Option<LValue> {
    match &e.kind {
        ExprKind::Var(n) => Some(LValue::Var(n.clone())),
        ExprKind::Index(base, idx) => Some(LValue::Index(
            Box::new(expr_to_lvalue(base)?),
            idx.clone(),
        )),
        ExprKind::Field(base, f) => Some(LValue::Field(Box::new(expr_to_lvalue(base)?), f.clone())),
        ExprKind::Paren(inner) => expr_to_lvalue(inner),
        _ => None,
    }
}
