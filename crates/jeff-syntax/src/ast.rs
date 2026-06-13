//! Surface AST. Authority: CLAUDE.md APPENDIX A (grammar) and B.1 (skeleton).
//!
//! Every node carries a [`Span`] (R37). The constitution's B.1 lists separate
//! `span` fields; here we use the uniform `{ kind, span }` wrapper for expressions,
//! statements, types and patterns, which is equivalent and keeps span handling
//! consistent (DR6 representational note).

use jeff_span::Span;

pub type Ident = String;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DottedName {
    pub parts: Vec<Ident>,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Program {
    pub module: Option<DottedName>,
    pub imports: Vec<Import>,
    pub items: Vec<Item>,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Import {
    pub path: DottedName,
    pub names: Vec<Ident>,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Item {
    Fn(FnDecl),
    Data(DataDecl),
    Codata(CodataDecl),
    TypeAlias(TypeAlias),
    Const(ConstDecl),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Total,
    General,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Attr {
    pub name: Ident,
    pub args: Vec<AttrArg>,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AttrArg {
    Flag(Ident),
    KeyVal(Ident, Ident),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Type,
    Nat,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GenericParam {
    pub name: Ident,
    pub kind: Kind,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Param {
    pub name: Ident,
    pub ty: Type,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Effect {
    IO,
    Div,
    Alloc,
    Rand,
    Unsafe,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct EffectRow {
    pub labels: Vec<Effect>,
    pub span: Option<Span>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FnDecl {
    pub attrs: Vec<Attr>,
    pub mode: Mode,
    pub name: Ident,
    pub generics: Vec<GenericParam>,
    pub params: Vec<Param>,
    pub ret: Option<Type>,
    pub effect: EffectRow,
    pub body: Block,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DataDecl {
    pub name: Ident,
    pub generics: Vec<GenericParam>,
    pub ctors: Vec<Constructor>,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Constructor {
    pub name: Ident,
    pub fields: Vec<Type>,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CodataDecl {
    pub name: Ident,
    pub generics: Vec<GenericParam>,
    pub destructors: Vec<Destructor>,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Destructor {
    pub name: Ident,
    pub ty: Type,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TypeAlias {
    pub name: Ident,
    pub generics: Vec<GenericParam>,
    pub target: Type,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConstDecl {
    pub name: Ident,
    pub ty: Type,
    pub value: Expr,
    pub span: Span,
}

// ---- Types (A.6) ----

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Type {
    pub kind: TypeKind,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TypeKind {
    Base(BaseType),
    /// Type constructor application, e.g. `Vec[T, n]`, `Mat[f64, m, n]`.
    App(Ident, Vec<TypeArg>),
    Ref {
        mutable: bool,
        inner: Box<Type>,
    },
    Own(Box<Type>),
    Secret(Box<Type>),
    /// Refinement `{ var : base | pred }` (Z3-discharged, §7.5).
    Refine {
        var: Ident,
        base: BaseType,
        pred: Box<Expr>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TypeArg {
    Ty(Type),
    /// Type-level term (nat), e.g. the `n` in `Vec[T, n]`.
    Term(Expr),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BaseType {
    I(u8),
    U(u8),
    Int,
    Nat,
    Rat,
    Bool,
    Char,
    Str,
    F32,
    F64,
    Mod(Box<Expr>),
    Bv(Box<Expr>),
    Vec,
    Fin,
    Mat,
}

// ---- Statements (A.7) ----

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Block {
    pub stmts: Vec<Stmt>,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stmt {
    pub kind: StmtKind,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StmtKind {
    Let {
        pat: Pattern,
        ty: Option<Type>,
        init: Expr,
    },
    Var {
        name: Ident,
        ty: Type,
        init: Expr,
    },
    Assign {
        lhs: LValue,
        op: AssignOp,
        rhs: Expr,
    },
    For {
        pat: Pattern,
        iter: Expr,
        body: Block,
    },
    /// General mode only (§7.7); Total mode rejects `while`.
    While {
        cond: Expr,
        body: Block,
    },
    Return(Option<Expr>),
    Expr(Expr),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AssignOp {
    Eq,
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Xor,
    And,
    Or,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LValue {
    Var(Ident),
    Index(Box<LValue>, Box<Expr>),
    Field(Box<LValue>, Ident),
}

// ---- Expressions (A.8/A.9) ----

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExprKind {
    Lit(Lit),
    Var(Ident),
    Paren(Box<Expr>),
    Bin(BinOp, Box<Expr>, Box<Expr>),
    Un(UnOp, Box<Expr>),
    Call(Box<Expr>, Vec<Expr>),
    Index(Box<Expr>, Box<Expr>),
    Field(Box<Expr>, Ident),
    Range {
        lo: Box<Expr>,
        hi: Box<Expr>,
        inclusive: bool,
    },
    /// First-class recognizer input (A.9): `sum/prod/count/fold b in D: e`.
    Reduction {
        kind: RedKind,
        binder: Vec<Pattern>,
        domain: Domain,
        body: Box<Expr>,
    },
    Match {
        scrut: Box<Expr>,
        arms: Vec<Arm>,
    },
    Borrow {
        mutable: bool,
        inner: Box<Expr>,
    },
    Own(Box<Expr>),
    Move(Box<Expr>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinOp {
    Or,
    And,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    BitOr,
    BitXor,
    BitAnd,
    Shl,
    Shr,
    Add,
    Sub,
    Mul,
    Div,
    FloorDiv,
    Rem,
    Pow,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnOp {
    Neg,
    Not,
    BitNot,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RedKind {
    Sum,
    Prod,
    Count,
    Fold,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Domain {
    Range(Box<Expr>),
    /// Affine constraint set `{ 0<=i<=j<=n }` → Barvinok (3b) when affine.
    Set(Vec<Constraint>),
    Expr(Box<Expr>),
}

/// A chained comparison constraint, e.g. `0 <= i <= j <= n`: `head` then a sequence
/// of `(op, rhs)` parts (A.9).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Constraint {
    pub head: Expr,
    pub parts: Vec<(CmpOp, Expr)>,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CmpOp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Arm {
    pub pat: Pattern,
    pub body: Block,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pattern {
    pub kind: PatternKind,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PatternKind {
    Wild,
    Lit(Lit),
    Var(Ident),
    Ctor(Ident, Vec<Pattern>),
    /// `m + k` sized decomposition for termination tracking (A.9 nat_succ_pattern).
    NatSucc(Ident, u64),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Lit {
    Int(String, Option<NumSuffix>),
    Float(String),
    Bool(bool),
    Char(char),
    Str(String),
    Mod(String, String),
    Rat(String, String),
    Bv(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NumSuffix {
    I(u8),
    U(u8),
    Nat,
}
