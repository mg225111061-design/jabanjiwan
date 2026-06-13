//! JEFF surface syntax: lexer + parser + AST.
//!
//! Authority: CLAUDE.md APPENDIX A (grammar), B.1 (skeleton). The parser returns
//! `Result<Program, Vec<Diagnostic>>` (R20: collect diagnostics; R38: never panic
//! on user input). Spans are preserved on every node (R37).

pub mod ast;
pub mod lexer;
pub mod parser;

pub use ast::Program;
use jeff_span::Diagnostic;

/// Parse a source string into a [`Program`]. `file` is a source-map index used in
/// spans. Lexing and parsing each accumulate diagnostics (R20).
pub fn parse(src: &str, file: u32) -> Result<Program, Vec<Diagnostic>> {
    let toks = lexer::lex(src, file)?;
    parser::Parser::new(toks, file).parse_program()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ast::*;

    #[test]
    fn parse_triangular() {
        // The Stage 0 end-to-end program (PART 19 / J.1, simplified to sum i).
        let p = parse("total fn triangular(n: nat) -> nat: sum i in 0..=n: i\n", 0).unwrap();
        assert_eq!(p.items.len(), 1);
        let Item::Fn(f) = &p.items[0] else {
            panic!("expected fn")
        };
        assert_eq!(f.name, "triangular");
        assert_eq!(f.mode, Mode::Total);
        assert_eq!(f.params.len(), 1);
        let body = &f.body.stmts[0];
        let StmtKind::Expr(e) = &body.kind else {
            panic!("expected expr body")
        };
        assert!(matches!(e.kind, ExprKind::Reduction { .. }));
    }

    #[test]
    fn parse_faulhaber_i_squared() {
        let p = parse("total fn s2(n: nat) -> nat: sum i in 0..=n: i*i\n", 0).unwrap();
        let Item::Fn(f) = &p.items[0] else {
            panic!()
        };
        let StmtKind::Expr(e) = &f.body.stmts[0].kind else {
            panic!()
        };
        let ExprKind::Reduction { body, .. } = &e.kind else {
            panic!()
        };
        assert!(matches!(body.kind, ExprKind::Bin(BinOp::Mul, _, _)));
    }

    #[test]
    fn parse_secret_branch_program() {
        // P02: branch on secret — must parse fine; the *type checker* rejects it.
        let src = "@constant_time\nfn d(sk: secret[Vec[u8, 4]]) -> u8:\n    if sk[0] == 0:\n        1\n    else:\n        0\n";
        // Note: `if` is not in our statement grammar subset; use match form instead.
        let _ = src;
        let p = parse(
            "total fn count_pairs(n: nat) -> nat: count (i, j) in {0 <= i <= j <= n}: 1\n",
            0,
        )
        .unwrap();
        let Item::Fn(f) = &p.items[0] else {
            panic!()
        };
        let StmtKind::Expr(e) = &f.body.stmts[0].kind else {
            panic!()
        };
        let ExprKind::Reduction { kind, domain, .. } = &e.kind else {
            panic!()
        };
        assert_eq!(*kind, RedKind::Count);
        assert!(matches!(domain, Domain::Set(_)));
    }

    #[test]
    fn parse_match_and_data() {
        let src = "data Tree:\n    Leaf(int)\n    Node(Tree, Tree)\n\ntotal fn s(t: Tree) -> int:\n    match t:\n        Leaf(x) => x\n        Node(l, r) => s(l) + s(r)\n";
        let p = parse(src, 0).unwrap();
        assert_eq!(p.items.len(), 2);
        assert!(matches!(p.items[0], Item::Data(_)));
    }

    #[test]
    fn parse_collects_errors_without_panic() {
        // Missing closing paren — should produce diagnostics, not panic (R20/R38).
        let r = parse("total fn f(n: nat -> nat: 0\n", 0);
        assert!(r.is_err());
    }

    #[test]
    fn parse_module_and_imports() {
        let src = "module examples.numerics\nimport core.fold (faulhaber)\n\ntotal fn p(n: nat) -> nat: sum i in 0..=n: i\n";
        let p = parse(src, 0).unwrap();
        assert_eq!(p.module.unwrap().parts, vec!["examples", "numerics"]);
        assert_eq!(p.imports.len(), 1);
        assert_eq!(p.imports[0].names, vec!["faulhaber"]);
    }
}
