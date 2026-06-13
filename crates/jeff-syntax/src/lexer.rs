//! Lexer with Python-style layout. Authority: CLAUDE.md APPENDIX A.2 / A.10.
//!
//! Synthesises `Indent` / `Dedent` / `Newline` tokens from indentation. Spaces
//! only (tabs are an error, A.2). Newlines inside brackets `()[]{}` are joined
//! (implicit continuation). Comments `# ...` run to end of line. Diagnostics are
//! collected; the lexer never panics on user input (R20/R38).

use jeff_span::{DiagCode, Diagnostic, Span};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Keyword {
    Module,
    Import,
    Total,
    Fn,
    Data,
    Codata,
    Type,
    Const,
    Let,
    Var,
    For,
    While,
    Return,
    Match,
    In,
    And,
    Or,
    Not,
    Own,
    Move,
    Secret,
    True,
    False,
    IO,
    Div,
    Alloc,
    Rand,
    Unsafe,
    Sum,
    Prod,
    Count,
    Fold,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sym {
    LParen,
    RParen,
    LBracket,
    RBracket,
    LBrace,
    RBrace,
    Colon,
    Comma,
    Dot,
    DotDot,
    DotDotEq,
    Arrow,
    FatArrow,
    At,
    Eq,
    EqEq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    Plus,
    Minus,
    Star,
    StarStar,
    Slash,
    SlashSlash,
    Percent,
    Caret,
    Amp,
    Pipe,
    Shl,
    Shr,
    Tilde,
    PlusEq,
    MinusEq,
    StarEq,
    SlashEq,
    PercentEq,
    CaretEq,
    AmpEq,
    PipeEq,
}

use crate::ast::NumSuffix;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TokKind {
    Ident(String),
    Int(String, Option<NumSuffix>),
    Float(String),
    Str(String),
    Char(char),
    Kw(Keyword),
    Sym(Sym),
    Indent,
    Dedent,
    Newline,
    Eof,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Token {
    pub kind: TokKind,
    pub span: Span,
}

/// Reverse map: a keyword's source text. Used where keywords double as contextual
/// identifiers (module path segments like `core.fold`, field names).
pub fn keyword_str(k: Keyword) -> &'static str {
    use Keyword::*;
    match k {
        Module => "module",
        Import => "import",
        Total => "total",
        Fn => "fn",
        Data => "data",
        Codata => "codata",
        Type => "type",
        Const => "const",
        Let => "let",
        Var => "var",
        For => "for",
        While => "while",
        Return => "return",
        Match => "match",
        In => "in",
        And => "and",
        Or => "or",
        Not => "not",
        Own => "own",
        Move => "move",
        Secret => "secret",
        True => "true",
        False => "false",
        IO => "IO",
        Div => "Div",
        Alloc => "Alloc",
        Rand => "Rand",
        Unsafe => "Unsafe",
        Sum => "sum",
        Prod => "prod",
        Count => "count",
        Fold => "fold",
    }
}

fn keyword(s: &str) -> Option<Keyword> {
    use Keyword::*;
    Some(match s {
        "module" => Module,
        "import" => Import,
        "total" => Total,
        "fn" => Fn,
        "data" => Data,
        "codata" => Codata,
        "type" => Type,
        "const" => Const,
        "let" => Let,
        "var" => Var,
        "for" => For,
        "while" => While,
        "return" => Return,
        "match" => Match,
        "in" => In,
        "and" => And,
        "or" => Or,
        "not" => Not,
        "own" => Own,
        "move" => Move,
        "secret" => Secret,
        "true" => True,
        "false" => False,
        "IO" => IO,
        "Div" => Div,
        "Alloc" => Alloc,
        "Rand" => Rand,
        "Unsafe" => Unsafe,
        "sum" => Sum,
        "prod" => Prod,
        "count" => Count,
        "fold" => Fold,
        _ => return None,
    })
}

struct Lexer<'a> {
    chars: Vec<char>,
    byte_offsets: Vec<usize>, // byte offset of chars[i]; len = chars.len()+1
    src_len: usize,
    pos: usize,
    line: u32,
    col: u32,
    file: u32,
    indent_stack: Vec<usize>,
    bracket_depth: i32,
    line_had_tokens: bool,
    out: Vec<Token>,
    diags: Vec<Diagnostic>,
    _src: &'a str,
}

impl<'a> Lexer<'a> {
    fn new(src: &'a str, file: u32) -> Self {
        let chars: Vec<char> = src.chars().collect();
        let mut byte_offsets = Vec::with_capacity(chars.len() + 1);
        let mut b = 0;
        for c in &chars {
            byte_offsets.push(b);
            b += c.len_utf8();
        }
        byte_offsets.push(b);
        Lexer {
            chars,
            byte_offsets,
            src_len: src.len(),
            pos: 0,
            line: 1,
            col: 1,
            file,
            indent_stack: vec![0],
            bracket_depth: 0,
            line_had_tokens: false,
            out: Vec::new(),
            diags: Vec::new(),
            _src: src,
        }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }
    fn peek2(&self) -> Option<char> {
        self.chars.get(self.pos + 1).copied()
    }

    fn byte(&self) -> u32 {
        self.byte_offsets[self.pos.min(self.chars.len())] as u32
    }

    fn advance(&mut self) -> Option<char> {
        let c = self.chars.get(self.pos).copied()?;
        self.pos += 1;
        if c == '\n' {
            self.line += 1;
            self.col = 1;
        } else {
            self.col += 1;
        }
        Some(c)
    }

    fn span_from(&self, lo: u32, line: u32, col: u32) -> Span {
        Span::new(self.file, lo, self.byte(), line, col)
    }

    fn push(&mut self, kind: TokKind, lo: u32, line: u32, col: u32) {
        let span = self.span_from(lo, line, col);
        self.out.push(Token { kind, span });
        self.line_had_tokens = true;
    }

    fn err(&mut self, code: DiagCode, msg: impl Into<String>) {
        let span = Span::new(self.file, self.byte(), self.byte(), self.line, self.col);
        self.diags.push(Diagnostic::error(code, span, msg));
    }

    /// Handle indentation at the start of a logical line (bracket_depth == 0).
    fn handle_line_start(&mut self) {
        loop {
            // measure indentation
            let mut indent = 0usize;
            let start = self.pos;
            while let Some(c) = self.peek() {
                if c == ' ' {
                    indent += 1;
                    self.advance();
                } else if c == '\t' {
                    self.err(DiagCode::Layout, "tabs are not allowed for indentation (use spaces, APPENDIX A.2)");
                    self.advance();
                } else {
                    break;
                }
            }
            match self.peek() {
                // blank line or comment-only line: skip, no layout tokens
                Some('\n') | None => {
                    if self.advance().is_none() {
                        return;
                    }
                    continue;
                }
                Some('#') => {
                    while let Some(c) = self.peek() {
                        if c == '\n' {
                            break;
                        }
                        self.advance();
                    }
                    continue;
                }
                _ => {}
            }
            let _ = start;
            // real content: emit INDENT/DEDENT
            let top = *self.indent_stack.last().unwrap();
            if indent > top {
                self.indent_stack.push(indent);
                self.push_layout(TokKind::Indent);
            } else if indent < top {
                while *self.indent_stack.last().unwrap() > indent {
                    self.indent_stack.pop();
                    self.push_layout(TokKind::Dedent);
                }
                if *self.indent_stack.last().unwrap() != indent {
                    self.err(
                        DiagCode::Layout,
                        "inconsistent dedent does not match any outer indentation level",
                    );
                }
            }
            return;
        }
    }

    fn push_layout(&mut self, kind: TokKind) {
        let b = self.byte();
        let span = Span::new(self.file, b, b, self.line, self.col);
        self.out.push(Token { kind, span });
    }

    fn lex_ident_or_kw(&mut self) {
        let lo = self.byte();
        let (line, col) = (self.line, self.col);
        let mut s = String::new();
        while let Some(c) = self.peek() {
            if c == '_' || c.is_alphanumeric() {
                s.push(c);
                self.advance();
            } else {
                break;
            }
        }
        let kind = match keyword(&s) {
            Some(k) => TokKind::Kw(k),
            None => TokKind::Ident(s),
        };
        self.push(kind, lo, line, col);
    }

    fn lex_number(&mut self) {
        let lo = self.byte();
        let (line, col) = (self.line, self.col);
        let mut s = String::new();
        // binary literal 0b...
        if self.peek() == Some('0') && self.peek2() == Some('b') {
            s.push(self.advance().unwrap());
            s.push(self.advance().unwrap());
            while let Some(c) = self.peek() {
                if c == '0' || c == '1' {
                    s.push(c);
                    self.advance();
                } else {
                    break;
                }
            }
            self.push(TokKind::Int(s, None), lo, line, col);
            return;
        }
        while let Some(c) = self.peek() {
            if c.is_ascii_digit() {
                s.push(c);
                self.advance();
            } else {
                break;
            }
        }
        // float: digit+ '.' digit+   (but not '..' range)
        if self.peek() == Some('.') && self.peek2().is_some_and(|c| c.is_ascii_digit()) {
            s.push(self.advance().unwrap()); // '.'
            while let Some(c) = self.peek() {
                if c.is_ascii_digit() {
                    s.push(c);
                    self.advance();
                } else {
                    break;
                }
            }
            // optional f32/f64 suffix
            let suffix = self.try_lex_word_suffix(&["f32", "f64"]);
            if let Some(suf) = suffix {
                s.push_str(&suf);
            }
            self.push(TokKind::Float(s), lo, line, col);
            return;
        }
        // integer suffix
        let suffix = self.try_lex_num_suffix();
        self.push(TokKind::Int(s, suffix), lo, line, col);
    }

    fn try_lex_word_suffix(&mut self, allowed: &[&str]) -> Option<String> {
        // peek an identifier-like run; if it matches an allowed suffix, consume it.
        let mut probe = self.pos;
        let mut w = String::new();
        while let Some(&c) = self.chars.get(probe) {
            if c == '_' || c.is_alphanumeric() {
                w.push(c);
                probe += 1;
            } else {
                break;
            }
        }
        if allowed.contains(&w.as_str()) {
            for _ in 0..w.chars().count() {
                self.advance();
            }
            Some(w)
        } else {
            None
        }
    }

    fn try_lex_num_suffix(&mut self) -> Option<NumSuffix> {
        let mut probe = self.pos;
        let mut w = String::new();
        while let Some(&c) = self.chars.get(probe) {
            if c == '_' || c.is_alphanumeric() {
                w.push(c);
                probe += 1;
            } else {
                break;
            }
        }
        let suf = match w.as_str() {
            "i8" => NumSuffix::I(8),
            "i16" => NumSuffix::I(16),
            "i32" => NumSuffix::I(32),
            "i64" => NumSuffix::I(64),
            "u8" => NumSuffix::U(8),
            "u16" => NumSuffix::U(16),
            "u32" => NumSuffix::U(32),
            "u64" => NumSuffix::U(64),
            "nat" => NumSuffix::Nat,
            _ => return None,
        };
        for _ in 0..w.chars().count() {
            self.advance();
        }
        Some(suf)
    }

    fn lex_string(&mut self) {
        let lo = self.byte();
        let (line, col) = (self.line, self.col);
        self.advance(); // opening quote
        let mut s = String::new();
        while let Some(c) = self.peek() {
            match c {
                '"' => {
                    self.advance();
                    self.push(TokKind::Str(s), lo, line, col);
                    return;
                }
                '\\' => {
                    self.advance();
                    match self.advance() {
                        Some('n') => s.push('\n'),
                        Some('t') => s.push('\t'),
                        Some('\\') => s.push('\\'),
                        Some('"') => s.push('"'),
                        Some('0') => s.push('\0'),
                        Some(other) => s.push(other),
                        None => break,
                    }
                }
                '\n' => break,
                _ => {
                    s.push(c);
                    self.advance();
                }
            }
        }
        self.err(DiagCode::Lex, "unterminated string literal");
        self.push(TokKind::Str(s), lo, line, col);
    }

    fn lex_char(&mut self) {
        let lo = self.byte();
        let (line, col) = (self.line, self.col);
        self.advance(); // '
        let ch = match self.peek() {
            Some('\\') => {
                self.advance();
                match self.advance() {
                    Some('n') => '\n',
                    Some('t') => '\t',
                    Some('\\') => '\\',
                    Some('\'') => '\'',
                    Some('0') => '\0',
                    Some(o) => o,
                    None => {
                        self.err(DiagCode::Lex, "unterminated char literal");
                        '\0'
                    }
                }
            }
            Some(c) => {
                self.advance();
                c
            }
            None => {
                self.err(DiagCode::Lex, "unterminated char literal");
                '\0'
            }
        };
        if self.peek() == Some('\'') {
            self.advance();
        } else {
            self.err(DiagCode::Lex, "expected closing ' in char literal");
        }
        self.push(TokKind::Char(ch), lo, line, col);
    }

    fn sym(&mut self, kind: Sym, n: usize) {
        let lo = self.byte();
        let (line, col) = (self.line, self.col);
        for _ in 0..n {
            self.advance();
        }
        self.push(TokKind::Sym(kind), lo, line, col);
        // track bracket depth for implicit line joining
        match kind {
            Sym::LParen | Sym::LBracket | Sym::LBrace => self.bracket_depth += 1,
            Sym::RParen | Sym::RBracket | Sym::RBrace => {
                self.bracket_depth = (self.bracket_depth - 1).max(0)
            }
            _ => {}
        }
    }

    fn lex_symbol(&mut self) {
        let c = self.peek().unwrap();
        let c2 = self.peek2();
        use Sym::*;
        match c {
            '(' => self.sym(LParen, 1),
            ')' => self.sym(RParen, 1),
            '[' => self.sym(LBracket, 1),
            ']' => self.sym(RBracket, 1),
            '{' => self.sym(LBrace, 1),
            '}' => self.sym(RBrace, 1),
            ':' => self.sym(Colon, 1),
            ',' => self.sym(Comma, 1),
            '@' => self.sym(At, 1),
            '~' => self.sym(Tilde, 1),
            '.' => {
                if c2 == Some('.') {
                    if self.chars.get(self.pos + 2) == Some(&'=') {
                        self.sym(DotDotEq, 3);
                    } else {
                        self.sym(DotDot, 2);
                    }
                } else {
                    self.sym(Dot, 1);
                }
            }
            '-' => match c2 {
                Some('>') => self.sym(Arrow, 2),
                Some('=') => self.sym(MinusEq, 2),
                _ => self.sym(Minus, 1),
            },
            '=' => match c2 {
                Some('=') => self.sym(EqEq, 2),
                Some('>') => self.sym(FatArrow, 2),
                _ => self.sym(Eq, 1),
            },
            '!' => {
                if c2 == Some('=') {
                    self.sym(Ne, 2);
                } else {
                    self.err(DiagCode::Lex, "unexpected '!' (did you mean '!=' or 'not'?)");
                    self.advance();
                }
            }
            '<' => match c2 {
                Some('=') => self.sym(Le, 2),
                Some('<') => self.sym(Shl, 2),
                _ => self.sym(Lt, 1),
            },
            '>' => match c2 {
                Some('=') => self.sym(Ge, 2),
                Some('>') => self.sym(Shr, 2),
                _ => self.sym(Gt, 1),
            },
            '+' => {
                if c2 == Some('=') {
                    self.sym(PlusEq, 2)
                } else {
                    self.sym(Plus, 1)
                }
            }
            '*' => match c2 {
                Some('*') => self.sym(StarStar, 2),
                Some('=') => self.sym(StarEq, 2),
                _ => self.sym(Star, 1),
            },
            '/' => match c2 {
                Some('/') => self.sym(SlashSlash, 2),
                Some('=') => self.sym(SlashEq, 2),
                _ => self.sym(Slash, 1),
            },
            '%' => {
                if c2 == Some('=') {
                    self.sym(PercentEq, 2)
                } else {
                    self.sym(Percent, 1)
                }
            }
            '^' => {
                if c2 == Some('=') {
                    self.sym(CaretEq, 2)
                } else {
                    self.sym(Caret, 1)
                }
            }
            '&' => {
                if c2 == Some('=') {
                    self.sym(AmpEq, 2)
                } else {
                    self.sym(Amp, 1)
                }
            }
            '|' => {
                if c2 == Some('=') {
                    self.sym(PipeEq, 2)
                } else {
                    self.sym(Pipe, 1)
                }
            }
            other => {
                self.err(DiagCode::Lex, format!("unexpected character {other:?}"));
                self.advance();
            }
        }
    }

    fn run(mut self) -> Result<Vec<Token>, Vec<Diagnostic>> {
        // initial line start
        if self.bracket_depth == 0 {
            self.line_had_tokens = false;
            self.handle_line_start();
        }
        while let Some(c) = self.peek() {
            match c {
                ' ' => {
                    self.advance();
                }
                '\t' => {
                    // tab inside a line: treat as space but warn once via error
                    self.err(DiagCode::Layout, "tab character in source (use spaces)");
                    self.advance();
                }
                '\n' => {
                    self.advance();
                    if self.bracket_depth == 0 {
                        if self.line_had_tokens {
                            self.push_layout(TokKind::Newline);
                        }
                        self.line_had_tokens = false;
                        self.handle_line_start();
                    }
                }
                '#' => {
                    while let Some(c) = self.peek() {
                        if c == '\n' {
                            break;
                        }
                        self.advance();
                    }
                }
                '"' => self.lex_string(),
                '\'' => self.lex_char(),
                c if c == '_' || c.is_alphabetic() => self.lex_ident_or_kw(),
                c if c.is_ascii_digit() => self.lex_number(),
                _ => self.lex_symbol(),
            }
        }
        // final newline + dedents
        if self.line_had_tokens {
            self.push_layout(TokKind::Newline);
        }
        while self.indent_stack.len() > 1 {
            self.indent_stack.pop();
            self.push_layout(TokKind::Dedent);
        }
        let b = self.src_len as u32;
        self.out.push(Token {
            kind: TokKind::Eof,
            span: Span::new(self.file, b, b, self.line, self.col),
        });
        if self.diags.iter().any(Diagnostic::is_error) {
            Err(self.diags)
        } else {
            Ok(self.out)
        }
    }
}

/// Lex `src` into a token stream with synthesised layout tokens (APPENDIX A.2).
pub fn lex(src: &str, file: u32) -> Result<Vec<Token>, Vec<Diagnostic>> {
    Lexer::new(src, file).run()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(src: &str) -> Vec<TokKind> {
        lex(src, 0).unwrap().into_iter().map(|t| t.kind).collect()
    }

    #[test]
    fn layout_indent_dedent() {
        let src = "fn f():\n    return 1\n";
        let ks = kinds(src);
        assert!(ks.contains(&TokKind::Indent));
        assert!(ks.contains(&TokKind::Dedent));
        assert_eq!(ks.last(), Some(&TokKind::Eof));
    }

    #[test]
    fn range_vs_float() {
        // 0..=n must be DotDotEq, not a float
        let ks = kinds("0..=n\n");
        assert!(ks.contains(&TokKind::Sym(Sym::DotDotEq)));
        // 3.5 is a float
        let ks2 = kinds("3.5\n");
        assert!(matches!(ks2[0], TokKind::Float(_)));
    }

    #[test]
    fn bracket_continuation_joins_lines() {
        // newline inside parens does not produce NEWLINE/INDENT
        let src = "f(\n  1,\n  2)\n";
        let ks = kinds(src);
        let newlines = ks.iter().filter(|k| **k == TokKind::Newline).count();
        assert_eq!(newlines, 1); // only the final logical newline
    }

    #[test]
    fn reduction_keywords() {
        let ks = kinds("sum i in 0..=n: i\n");
        assert_eq!(ks[0], TokKind::Kw(Keyword::Sum));
        assert!(ks.contains(&TokKind::Kw(Keyword::In)));
    }
}
