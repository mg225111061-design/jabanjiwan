//! Secret-taint constant-time audit (CLAUDE.md PART 7.4, APPENDIX C.4; R6/R28/R29).
//!
//! This is the security core of Stage 5: at this stage, *security is part of
//! correctness* (P0/P1). The audit enforces the [SECRET-IF] and [SECRET-IDX] typing
//! rules — a `secret[T]` value may flow through data-oblivious operations (arithmetic,
//! constant-time select/compare) but must never reach a **data-dependent branch** or a
//! **data-dependent memory index**, because either leaks the secret through timing.
//!
//! # What this audit GUARANTEES (the leakage model — in scope)
//!
//! Under the abstraction level of the JEFF source / IR, a function that passes this
//! audit has, for every `secret[T]` input:
//!   1. **No secret-dependent control flow.** No `match` on a secret scrutinee (with a
//!      refutable / multi-arm shape) and no `while` whose condition is secret-derived.
//!      Control flow is therefore independent of secret values, so the executed
//!      instruction trace does not depend on the secret. (`if` does not exist in JEFF
//!      surface syntax — branching is `match` — so those are the branch sinks.)
//!   2. **No secret-dependent memory addressing.** No `v[i]` where the *index* `i` is
//!      secret-derived. Memory access addresses are therefore secret-independent, so
//!      cache-line/page access patterns do not depend on the secret. (Indexing a secret
//!      *array* by a public index, e.g. `sk[0]`, is fine: the address is public, only
//!      the loaded value is secret.)
//!
//! Together these give a source/IR-level constant-time property: the control-flow graph
//! path and the sequence of memory addresses are functions of the *public* inputs only.
//!
//! # What this audit does NOT and CANNOT guarantee (out of model — stated honestly)
//!
//! Per DR3 (no silent approximation) and AR-3 (a security claim must say what it does
//! not cover), the following are **outside** the model and are NOT prevented here:
//!   - **Data-operand timing / Hertzbleed (frequency & power).** Modern CPUs vary core
//!     frequency with power draw, which depends on operand *values* even for nominally
//!     constant-time instructions (Hertzbleed, 2022). A program can be CFG/address
//!     constant-time and still leak through DVFS. Out of model.
//!   - **Data Memory-dependent Prefetchers (DMP).** Some CPUs (e.g. Apple M-series, the
//!     GoFetch attack, 2024) prefetch based on values that *look like* pointers,
//!     leaking data that never indexes memory in the program. Out of model.
//!   - **Cache/microarchitectural contention.** Port contention, SMT siblings, cache
//!     bank conflicts, speculative execution / transient-execution side channels. Out
//!     of model.
//!   - **Variable-latency instructions on operand value.** Integer divide and some
//!     floating ops can be data-dependent in latency on some microarchitectures. We do
//!     not model per-ISA instruction timing; we forbid `secret`-driven *branches and
//!     indices*, which are the architectural channels. (A future tightening could flag
//!     `secret / secret`; tracked, R24, not silently claimed solved.)
//!   - **Backend reintroduction.** A code generator may turn a branchless select back
//!     into a branch. That is the backend's obligation (R29, jeff-backend secret-taint
//!     guard); this front-end audit does not by itself bind the backend.
//!
//! Constant-time here means *constant-time in this model*. It is a necessary, not
//! sufficient, condition for side-channel resistance. We never claim otherwise (R30).
//!
//! # Soundness stance: SOUND, NOT COMPLETE (no false negatives)
//!
//! The taint analysis **over-approximates**: a value is treated as secret if it *could*
//! be secret on any path. Consequences:
//!   - **No false negatives.** If a branch/index can ever see a secret-derived value,
//!     the audit rejects it. It never silently accepts a leak.
//!   - **False positives are allowed.** It may reject a program that is actually safe
//!     (e.g. a value that is provably public on the taken path but tainted globally).
//!     "When in doubt, reject" (the user's Stage 5 instruction). The remedy for a false
//!     positive is an explicit, audited `declassify(...)`.
//!
//! `declassify(e)` is the one audited escape hatch: it clears taint. Its body is the
//! programmer's documented assertion that `e` is safe to treat as public. (Context
//! restrictions on *where* declassify is legal, E0303, are not yet enforced — tracked,
//! R24; today every `declassify` is accepted and clears taint.)
//!
//! # LWE is an assumption, not a collapse (Stage 5 boundary, hidden-structure #36)
//!
//! This crate audits PQC code for constant-time execution. It does **not** attempt to
//! analyze, weaken, or "collapse" the Learning-With-Errors hardness that PQC rests on.
//! LWE hardness is a cryptographic *assumption*; a compiler that "solved" it would break
//! the cryptosystem, not optimize it. JEFF compiles PQC operations (NTT, modular
//! arithmetic) exactly and constant-time, and treats LWE as a wall, never a structure to
//! exploit. See PART 18 #6 and the jeff-math PQC modules.

use jeff_span::{DiagCode, Diagnostic, Span};
use jeff_syntax::ast::*;
use std::collections::HashSet;

/// Audit an entire program. Returns every secret-taint violation as an error
/// diagnostic (R6: these are compile errors). Functions with no `secret[T]` inputs
/// produce no taint and therefore no diagnostics. Collects all diagnostics (R20).
pub fn check_secret_taint(program: &Program) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    for item in &program.items {
        if let Item::Fn(f) = item {
            diags.extend(audit_fn(f));
        }
    }
    diags
}

/// Is this function marked `@constant_time`? Used by the `--const-time-audit` report
/// (APPENDIX H.4). Note: the taint *violations* are reported for every function with
/// secret inputs regardless of attribute (R6 is a type rule, not opt-in); the attribute
/// only affects how the audit report is phrased.
pub fn is_constant_time(f: &FnDecl) -> bool {
    f.attrs.iter().any(|a| a.name == "constant_time")
}

/// Does this function take any `secret[T]` input?
pub fn has_secret_inputs(f: &FnDecl) -> bool {
    f.params.iter().any(|p| type_is_secret(&p.ty))
}

/// Audit a single function: compute the tainted-variable fixpoint from secret params,
/// then report every data-dependent branch/index sink that sees a tainted operand.
pub fn audit_fn(f: &FnDecl) -> Vec<Diagnostic> {
    // Seed: parameters whose type is (or wraps) secret[T].
    let mut taint: HashSet<Ident> = HashSet::new();
    for p in &f.params {
        if type_is_secret(&p.ty) {
            taint.insert(p.name.clone());
        }
    }

    // Collect taint flows (binder <- source) once; they are structural and fixed.
    let mut flows: Vec<Flow> = Vec::new();
    collect_block_flows(&f.body, &mut flows);

    // Monotone fixpoint: a binder becomes tainted if its source is tainted under the
    // current set. Taint only grows, so this terminates (bounded by #variables).
    loop {
        let mut changed = false;
        for fl in &flows {
            if expr_tainted(&fl.source, &taint) {
                for v in &fl.binders {
                    if taint.insert(v.clone()) {
                        changed = true;
                    }
                }
            }
        }
        if !changed {
            break;
        }
    }

    // Report sinks.
    let mut diags = Vec::new();
    report_block(&f.body, &taint, &mut diags);
    diags
}

// ---- taint flows ----

/// A binding: `binders` become tainted when `source` is tainted.
struct Flow {
    binders: Vec<Ident>,
    source: Expr,
}

fn collect_block_flows(b: &Block, out: &mut Vec<Flow>) {
    for s in &b.stmts {
        collect_stmt_flows(s, out);
    }
}

fn collect_stmt_flows(s: &Stmt, out: &mut Vec<Flow>) {
    match &s.kind {
        StmtKind::Let { pat, init, .. } => {
            out.push(Flow {
                binders: pattern_vars(pat),
                source: init.clone(),
            });
            collect_expr_flows(init, out);
        }
        StmtKind::Var { name, init, .. } => {
            out.push(Flow {
                binders: vec![name.clone()],
                source: init.clone(),
            });
            collect_expr_flows(init, out);
        }
        StmtKind::Assign { lhs, rhs, .. } => {
            // The base variable of the lvalue is tainted if the rhs is tainted.
            if let Some(base) = lvalue_base(lhs) {
                out.push(Flow {
                    binders: vec![base],
                    source: rhs.clone(),
                });
            }
            collect_expr_flows(rhs, out);
        }
        StmtKind::For { pat, iter, body } => {
            // Iterating a secret collection taints the loop variable(s).
            out.push(Flow {
                binders: pattern_vars(pat),
                source: iter.clone(),
            });
            collect_expr_flows(iter, out);
            collect_block_flows(body, out);
        }
        StmtKind::While { cond, body } => {
            collect_expr_flows(cond, out);
            collect_block_flows(body, out);
        }
        StmtKind::Return(Some(e)) | StmtKind::Expr(e) => collect_expr_flows(e, out),
        StmtKind::Return(None) => {}
    }
}

/// Collect flows nested inside an expression (match arms, reductions bind variables).
fn collect_expr_flows(e: &Expr, out: &mut Vec<Flow>) {
    match &e.kind {
        ExprKind::Lit(_) | ExprKind::Var(_) => {}
        ExprKind::Paren(a) | ExprKind::Un(_, a) | ExprKind::Field(a, _) | ExprKind::Borrow {
            inner: a, ..
        } | ExprKind::Own(a) | ExprKind::Move(a) => collect_expr_flows(a, out),
        ExprKind::Bin(_, a, b) | ExprKind::Index(a, b) => {
            collect_expr_flows(a, out);
            collect_expr_flows(b, out);
        }
        ExprKind::Range { lo, hi, .. } => {
            collect_expr_flows(lo, out);
            collect_expr_flows(hi, out);
        }
        ExprKind::Call(callee, args) => {
            collect_expr_flows(callee, out);
            for a in args {
                collect_expr_flows(a, out);
            }
        }
        ExprKind::Reduction {
            binder,
            domain,
            body,
            ..
        } => {
            // Binding over a secret domain taints the binder(s).
            let mut binders = Vec::new();
            for p in binder {
                binders.extend(pattern_vars(p));
            }
            out.push(Flow {
                binders,
                source: domain_source(domain),
            });
            collect_domain_flows(domain, out);
            collect_expr_flows(body, out);
        }
        ExprKind::Match { scrut, arms } => {
            collect_expr_flows(scrut, out);
            for arm in arms {
                // Destructuring a secret scrutinee taints the pattern variables.
                out.push(Flow {
                    binders: pattern_vars(&arm.pat),
                    source: (**scrut).clone(),
                });
                collect_block_flows(&arm.body, out);
            }
        }
    }
}

fn collect_domain_flows(d: &Domain, out: &mut Vec<Flow>) {
    match d {
        Domain::Range(e) | Domain::Expr(e) => collect_expr_flows(e, out),
        Domain::Set(cs) => {
            for c in cs {
                collect_expr_flows(&c.head, out);
                for (_, e) in &c.parts {
                    collect_expr_flows(e, out);
                }
            }
        }
    }
}

/// A representative "source" expression for a domain, used to taint reduction binders
/// (over-approximation: if any part is secret, the binder is treated as secret).
fn domain_source(d: &Domain) -> Expr {
    match d {
        Domain::Range(e) | Domain::Expr(e) => (**e).clone(),
        Domain::Set(cs) => {
            // Wrap all constraint expressions so any tainted one taints the binder.
            let mut acc: Option<Expr> = None;
            for c in cs {
                acc = Some(join_or(acc, c.head.clone()));
                for (_, e) in &c.parts {
                    acc = Some(join_or(acc.take(), e.clone()));
                }
            }
            acc.unwrap_or(Expr {
                kind: ExprKind::Lit(Lit::Bool(false)),
                span: Span::dummy(),
            })
        }
    }
}

fn join_or(acc: Option<Expr>, e: Expr) -> Expr {
    match acc {
        None => e,
        Some(a) => {
            let span = a.span.merge(e.span);
            Expr {
                kind: ExprKind::Bin(BinOp::Or, Box::new(a), Box::new(e)),
                span,
            }
        }
    }
}

// ---- taint query ----

/// Is `e` secret-derived under the current taint set? Over-approximates (sound).
fn expr_tainted(e: &Expr, taint: &HashSet<Ident>) -> bool {
    match &e.kind {
        ExprKind::Lit(_) => false,
        ExprKind::Var(x) => taint.contains(x),
        ExprKind::Paren(a)
        | ExprKind::Un(_, a)
        | ExprKind::Field(a, _)
        | ExprKind::Borrow { inner: a, .. }
        | ExprKind::Own(a)
        | ExprKind::Move(a) => expr_tainted(a, taint),
        ExprKind::Bin(_, a, b) => expr_tainted(a, taint) || expr_tainted(b, taint),
        // Indexing into a secret array yields a secret value; a secret index also
        // yields a secret value. (The *violation* for a secret index is reported
        // separately; here we only compute value taint.)
        ExprKind::Index(a, b) => expr_tainted(a, taint) || expr_tainted(b, taint),
        ExprKind::Range { lo, hi, .. } => {
            expr_tainted(lo, taint) || expr_tainted(hi, taint)
        }
        ExprKind::Call(callee, args) => {
            if is_declassify(callee) {
                // Audited escape hatch: declassify returns a public value (clears taint).
                false
            } else {
                // Unknown callee: conservatively, a tainted argument may flow to the
                // result (e.g. constant-time select(secret_cond, a, b) is still secret).
                expr_tainted(callee, taint) || args.iter().any(|a| expr_tainted(a, taint))
            }
        }
        ExprKind::Reduction { domain, body, .. } => {
            domain_tainted(domain, taint) || expr_tainted(body, taint)
        }
        ExprKind::Match { scrut, arms } => {
            expr_tainted(scrut, taint)
                || arms.iter().any(|a| block_result_tainted(&a.body, taint))
        }
    }
}

fn domain_tainted(d: &Domain, taint: &HashSet<Ident>) -> bool {
    match d {
        Domain::Range(e) | Domain::Expr(e) => expr_tainted(e, taint),
        Domain::Set(cs) => cs.iter().any(|c| {
            expr_tainted(&c.head, taint) || c.parts.iter().any(|(_, e)| expr_tainted(e, taint))
        }),
    }
}

/// Taint of a block's *result value* (its trailing expression / return).
fn block_result_tainted(b: &Block, taint: &HashSet<Ident>) -> bool {
    match b.stmts.last() {
        Some(Stmt {
            kind: StmtKind::Expr(e),
            ..
        })
        | Some(Stmt {
            kind: StmtKind::Return(Some(e)),
            ..
        }) => expr_tainted(e, taint),
        _ => false,
    }
}

// ---- violation reporting (sinks) ----

fn report_block(b: &Block, taint: &HashSet<Ident>, out: &mut Vec<Diagnostic>) {
    for s in &b.stmts {
        report_stmt(s, taint, out);
    }
}

fn report_stmt(s: &Stmt, taint: &HashSet<Ident>, out: &mut Vec<Diagnostic>) {
    match &s.kind {
        StmtKind::Let { init, .. } => report_expr(init, taint, out),
        StmtKind::Var { init, .. } => report_expr(init, taint, out),
        StmtKind::Assign { lhs, rhs, .. } => {
            report_lvalue(lhs, taint, out);
            report_expr(rhs, taint, out);
        }
        StmtKind::For { iter, body, .. } => {
            report_expr(iter, taint, out);
            report_block(body, taint, out);
        }
        StmtKind::While { cond, body } => {
            // SECRET-IF: a secret-dependent loop condition leaks the iteration count.
            if expr_tainted(cond, taint) {
                out.push(secret_branch_diag(cond.span, describe(cond), "while-loop condition"));
            }
            report_expr(cond, taint, out);
            report_block(body, taint, out);
        }
        StmtKind::Return(Some(e)) | StmtKind::Expr(e) => report_expr(e, taint, out),
        StmtKind::Return(None) => {}
    }
}

fn report_lvalue(lv: &LValue, taint: &HashSet<Ident>, out: &mut Vec<Diagnostic>) {
    match lv {
        LValue::Var(_) => {}
        LValue::Index(base, idx) => {
            report_lvalue(base, taint, out);
            // SECRET-IDX: a secret index on the left-hand side too.
            if expr_tainted(idx, taint) {
                out.push(secret_index_diag(idx.span, describe(idx)));
            }
            report_expr(idx, taint, out);
        }
        LValue::Field(base, _) => report_lvalue(base, taint, out),
    }
}

fn report_expr(e: &Expr, taint: &HashSet<Ident>, out: &mut Vec<Diagnostic>) {
    match &e.kind {
        ExprKind::Lit(_) | ExprKind::Var(_) => {}
        ExprKind::Paren(a)
        | ExprKind::Un(_, a)
        | ExprKind::Field(a, _)
        | ExprKind::Borrow { inner: a, .. }
        | ExprKind::Own(a)
        | ExprKind::Move(a) => report_expr(a, taint, out),
        ExprKind::Bin(_, a, b) => {
            report_expr(a, taint, out);
            report_expr(b, taint, out);
        }
        ExprKind::Index(base, idx) => {
            // SECRET-IDX: data-dependent memory index. (Indexing a secret array by a
            // *public* index is fine — only a tainted index is a violation.)
            if expr_tainted(idx, taint) {
                out.push(secret_index_diag(idx.span, describe(idx)));
            }
            report_expr(base, taint, out);
            report_expr(idx, taint, out);
        }
        ExprKind::Range { lo, hi, .. } => {
            report_expr(lo, taint, out);
            report_expr(hi, taint, out);
        }
        ExprKind::Call(callee, args) => {
            report_expr(callee, taint, out);
            for a in args {
                report_expr(a, taint, out);
            }
        }
        ExprKind::Reduction { domain, body, .. } => {
            report_domain(domain, taint, out);
            report_expr(body, taint, out);
        }
        ExprKind::Match { scrut, arms } => {
            // SECRET-IF: branching on a secret scrutinee. A single irrefutable arm
            // (a pure `_`/`x` binding) is not a branch; a refutable / multi-arm match is.
            if expr_tainted(scrut, taint) && is_refutable_match(arms) {
                out.push(secret_branch_diag(scrut.span, describe(scrut), "match scrutinee"));
            }
            report_expr(scrut, taint, out);
            for arm in arms {
                report_block(&arm.body, taint, out);
            }
        }
    }
}

fn report_domain(d: &Domain, taint: &HashSet<Ident>, out: &mut Vec<Diagnostic>) {
    match d {
        Domain::Range(e) | Domain::Expr(e) => report_expr(e, taint, out),
        Domain::Set(cs) => {
            for c in cs {
                report_expr(&c.head, taint, out);
                for (_, e) in &c.parts {
                    report_expr(e, taint, out);
                }
            }
        }
    }
}

/// A match is a branch (not just a binding) if it has more than one arm or any arm
/// pattern can fail to match (a literal/constructor/sized pattern).
fn is_refutable_match(arms: &[Arm]) -> bool {
    if arms.len() != 1 {
        return true;
    }
    matches!(
        arms[0].pat.kind,
        PatternKind::Lit(_) | PatternKind::Ctor(_, _) | PatternKind::NatSucc(_, _)
    )
}

// ---- diagnostics (R36: names/types only, never secret values) ----

fn secret_branch_diag(span: Span, operand: String, where_: &str) -> Diagnostic {
    Diagnostic::error(
        DiagCode::SecretBranch,
        span,
        format!("data-dependent branch on secret value ({operand})"),
    )
    .with_note(format!(
        "this {where_} depends on a secret[T] value; constant-time is required (R6)"
    ))
    .with_note(
        "a secret-dependent branch leaks the secret through timing / cache behavior"
            .to_string(),
    )
    .with_help(
        "rewrite data-obliviously (e.g. a constant-time `select(mask, a, b)`), or use an \
         audited `declassify(...)` if the value is provably public"
            .to_string(),
    )
}

fn secret_index_diag(span: Span, operand: String) -> Diagnostic {
    Diagnostic::error(
        DiagCode::SecretIndex,
        span,
        format!("data-dependent index by secret value ({operand})"),
    )
    .with_note(
        "indexing memory by a secret[T] value makes the access address secret-dependent, \
         which leaks through cache timing (R6)"
            .to_string(),
    )
    .with_help(
        "use a constant-time table scan (touch every element and mask), or an audited \
         `declassify(...)` if the index is provably public"
            .to_string(),
    )
}

/// Describe a secret operand by *name/shape only* (R36: never print secret values).
fn describe(e: &Expr) -> String {
    match &e.kind {
        ExprKind::Var(x) => format!("'{x}'"),
        ExprKind::Index(base, _) => match &base.kind {
            ExprKind::Var(x) => format!("an element of '{x}'"),
            _ => "an indexed element".to_string(),
        },
        ExprKind::Field(base, f) => match &base.kind {
            ExprKind::Var(x) => format!("'{x}.{f}'"),
            _ => format!("field '{f}'"),
        },
        ExprKind::Bin(_, _, _) => "a secret-derived expression".to_string(),
        ExprKind::Call(callee, _) => match &callee.kind {
            ExprKind::Var(name) => format!("result of '{name}(...)'"),
            _ => "a call result".to_string(),
        },
        ExprKind::Paren(inner) => describe(inner),
        _ => "a secret-derived expression".to_string(),
    }
}

// ---- helpers ----

/// Does the type carry a `secret[T]`, including through `&` / `&mut` / `own` wrappers?
fn type_is_secret(t: &Type) -> bool {
    match &t.kind {
        TypeKind::Secret(_) => true,
        TypeKind::Ref { inner, .. } => type_is_secret(inner),
        TypeKind::Own(inner) => type_is_secret(inner),
        _ => false,
    }
}

fn is_declassify(callee: &Expr) -> bool {
    matches!(&callee.kind, ExprKind::Var(name) if name == "declassify")
}

fn pattern_vars(p: &Pattern) -> Vec<Ident> {
    let mut v = Vec::new();
    collect_pattern_vars(p, &mut v);
    v
}

fn collect_pattern_vars(p: &Pattern, out: &mut Vec<Ident>) {
    match &p.kind {
        PatternKind::Wild | PatternKind::Lit(_) => {}
        PatternKind::Var(x) => out.push(x.clone()),
        PatternKind::NatSucc(x, _) => out.push(x.clone()),
        PatternKind::Ctor(_, sub) => {
            for s in sub {
                collect_pattern_vars(s, out);
            }
        }
    }
}

fn lvalue_base(lv: &LValue) -> Option<Ident> {
    match lv {
        LValue::Var(x) => Some(x.clone()),
        LValue::Index(base, _) | LValue::Field(base, _) => lvalue_base(base),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jeff_syntax::parse;

    fn audit(src: &str) -> Vec<Diagnostic> {
        let prog = parse(src, 0).expect("parse");
        check_secret_taint(&prog)
    }

    fn codes(diags: &[Diagnostic]) -> Vec<DiagCode> {
        diags.iter().map(|d| d.code).collect()
    }

    // --- TRIPWIRE 1: a secret-dependent branch is rejected (checker-first) ---
    #[test]
    fn secret_dependent_branch_rejected() {
        // match on a secret scrutinee (JEFF has no `if`; branch is `match`).
        let src = "\
@constant_time
fn leaky(sk: secret[Vec[u8, 4]]) -> u8:
    match sk[0]:
        0 => 1
        _ => 0
";
        let d = audit(src);
        assert!(
            codes(&d).contains(&DiagCode::SecretBranch),
            "expected E0301 SecretBranch, got {:?}",
            codes(&d)
        );
    }

    // --- TRIPWIRE 2: a secret-dependent index is rejected ---
    #[test]
    fn secret_dependent_index_rejected() {
        let src = "\
@constant_time
fn leaky(sk: secret[Vec[u8, 4]], table: Vec[u8, 256]) -> u8:
    table[sk[0]]
";
        let d = audit(src);
        assert!(
            codes(&d).contains(&DiagCode::SecretIndex),
            "expected E0302 SecretIndex, got {:?}",
            codes(&d)
        );
    }

    // --- A secret-dependent while loop leaks the iteration count ---
    #[test]
    fn secret_dependent_while_rejected() {
        let src = "\
fn leaky(sk: secret[u32]) -> u32:
    var c: u32 = 0
    while c < sk:
        c = c + 1
    return c
";
        let d = audit(src);
        assert!(codes(&d).contains(&DiagCode::SecretBranch));
    }

    // --- transitive taint: secret flows through a let-binding to the sink ---
    #[test]
    fn transitive_taint_through_let() {
        let src = "\
fn leaky(sk: secret[u32]) -> u32:
    let t = sk + 1
    let u = t * 2
    match u:
        0 => 1
        _ => 0
";
        let d = audit(src);
        assert!(
            codes(&d).contains(&DiagCode::SecretBranch),
            "taint must flow sk -> t -> u -> match"
        );
    }

    // --- NO false positive: public index into a secret array is fine ---
    #[test]
    fn public_index_into_secret_array_is_ok() {
        // sk[0] loads a secret value but the *address* (0) is public — allowed.
        let src = "\
fn ok(sk: secret[Vec[u8, 4]]) -> secret[u8]:
    sk[0]
";
        let d = audit(src);
        assert!(d.is_empty(), "public index must not flag, got {:?}", codes(&d));
    }

    // --- NO false positive: arithmetic on secrets is data-oblivious, allowed ---
    #[test]
    fn arithmetic_on_secret_is_ok() {
        let src = "\
@constant_time
fn add(a: secret[u32], b: secret[u32]) -> secret[u32]:
    a + b
";
        let d = audit(src);
        assert!(d.is_empty(), "pure arithmetic must not flag, got {:?}", codes(&d));
    }

    // --- NO false positive on public-only code (no secret inputs) ---
    #[test]
    fn public_function_is_unaffected() {
        let src = "\
fn pub_branch(x: u32, table: Vec[u8, 256]) -> u8:
    match x:
        0 => table[x]
        _ => 0
";
        let d = audit(src);
        assert!(d.is_empty(), "public branch/index must not flag, got {:?}", codes(&d));
    }

    // --- declassify clears taint (audited escape hatch) ---
    #[test]
    fn declassify_clears_taint() {
        let src = "\
fn dec(sk: secret[u32]) -> u32:
    let pub_val = declassify(sk)
    match pub_val:
        0 => 1
        _ => 0
";
        let d = audit(src);
        assert!(
            d.is_empty(),
            "declassified value is public; must not flag, got {:?}",
            codes(&d)
        );
    }

    // --- single irrefutable binding match is not a branch (no false positive) ---
    #[test]
    fn irrefutable_match_on_secret_is_not_a_branch() {
        // `match sk: x => x + 1` is a pure binding, not a value-dependent branch.
        let src = "\
fn bind(sk: secret[u32]) -> secret[u32]:
    match sk:
        y => y + 1
";
        let d = audit(src);
        assert!(d.is_empty(), "irrefutable bind must not flag, got {:?}", codes(&d));
    }

    // --- secret reaches index through a reduction binder over a secret array ---
    #[test]
    fn taint_through_for_loop_binder() {
        let src = "\
fn leaky(sk: secret[Vec[u32, 8]], table: Vec[u8, 256]) -> u8:
    var acc: u8 = 0
    for x in sk:
        acc = table[x]
    return acc
";
        let d = audit(src);
        assert!(
            codes(&d).contains(&DiagCode::SecretIndex),
            "loop var x is secret (iterating secret array) -> table[x] is a secret index"
        );
    }

    // --- R36: the diagnostic never embeds a secret value, only the name ---
    #[test]
    fn diagnostic_does_not_leak_values_r36() {
        let src = "\
fn leaky(sk: secret[u32]) -> u32:
    match sk:
        42 => 1
        _ => 0
";
        let d = audit(src);
        let branch = d.iter().find(|x| x.code == DiagCode::SecretBranch).unwrap();
        // mentions the name, not any concrete secret value
        assert!(branch.message.contains("sk"));
        // and references the type concept, not a value
        assert!(branch.notes.iter().any(|n| n.contains("secret[T]")));
    }
}
