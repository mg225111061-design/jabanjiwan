//! JLIR → LLVM IR (textual) + compile/run via the system `clang`.
//!
//! Authority: CLAUDE.md PART 8 (codegen row), PART 19 (e2e: parse→…→codegen→run).
//!
//! Stage-0 scope (AR-5): a region whose body is straight-line integer arithmetic
//! (a collapsed residual) or a single `Sum/Prod/Count` reduction over a range (a
//! deferred loop). This is exactly what the thin vertical slice needs. Codegen uses
//! `i64` (machine integers); the exact `BigInt` evaluator in `jeff-jlir` remains the
//! reference for P0 equivalence, so for machine-sized values the two agree. (Bignum
//! codegen is a later concern; documented, not faked.)
//!
//! Nested control-flow bodies (a reduction inside a reduction) are out of the
//! Stage-0 codegen subset; [`emit_function`] returns an error for them rather than
//! emit something wrong (R24/P0) — callers fall back to the evaluator.

use jeff_core_ir::{BinOp, CoreDomain, CoreExpr, CoreExprKind, RedKind};
use jeff_jlir::JlirRegion;
use num_bigint::BigInt;
use std::collections::HashMap;

const TARGET_TRIPLE: &str = "x86_64-pc-linux-gnu";

#[derive(Debug)]
pub enum CodegenError {
    Unsupported(String),
    Io(String),
    Clang(String),
    Parse(String),
}

impl std::fmt::Display for CodegenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CodegenError::Unsupported(s) => write!(f, "unsupported by Stage-0 codegen: {s}"),
            CodegenError::Io(s) => write!(f, "io error: {s}"),
            CodegenError::Clang(s) => write!(f, "clang error: {s}"),
            CodegenError::Parse(s) => write!(f, "could not parse program output: {s}"),
        }
    }
}

struct Emit {
    body: String,
    reg: usize,
    lbl: usize,
    cur: String,
}

impl Emit {
    fn new() -> Self {
        Emit {
            body: String::new(),
            reg: 0,
            lbl: 0,
            cur: "entry".to_string(),
        }
    }
    fn reg(&mut self) -> String {
        self.reg += 1;
        format!("%t{}", self.reg)
    }
    fn label_name(&mut self, base: &str) -> String {
        self.lbl += 1;
        format!("{base}{}", self.lbl)
    }
    fn line(&mut self, s: impl AsRef<str>) {
        self.body.push_str("  ");
        self.body.push_str(s.as_ref());
        self.body.push('\n');
    }
    fn place_label(&mut self, name: &str) {
        self.body.push_str(name);
        self.body.push_str(":\n");
        self.cur = name.to_string();
    }
}

fn sanitize(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_alphanumeric() || c == '_' { c } else { '_' })
        .collect()
}

/// Emit a `define i64 @<name>(...)` for the region's body. Returns the full
/// function text or a `CodegenError::Unsupported` for out-of-subset constructs.
pub fn emit_function(region: &JlirRegion) -> Result<String, CodegenError> {
    region
        .verify_invariants()
        .map_err(CodegenError::Unsupported)?;
    let mut env: HashMap<String, String> = HashMap::new();
    let params: Vec<String> = region
        .params
        .iter()
        .map(|(n, _)| {
            let arg = format!("%arg_{}", sanitize(n));
            env.insert(n.clone(), arg.clone());
            format!("i64 {arg}")
        })
        .collect();
    let mut e = Emit::new();
    let result = emit_expr(&mut e, &region.body, &mut env)?;
    let mut out = String::new();
    out.push_str(&format!(
        "define i64 @{}({}) {{\nentry:\n",
        sanitize(&region.name),
        params.join(", ")
    ));
    out.push_str(&e.body);
    out.push_str(&format!("  ret i64 {result}\n}}\n"));
    Ok(out)
}

fn emit_expr(
    e: &mut Emit,
    expr: &CoreExpr,
    env: &mut HashMap<String, String>,
) -> Result<String, CodegenError> {
    match &expr.kind {
        CoreExprKind::Int(v) => Ok(i64_literal(v)?),
        CoreExprKind::Var(x) => env
            .get(x)
            .cloned()
            .ok_or_else(|| CodegenError::Unsupported(format!("unbound variable {x}"))),
        CoreExprKind::Neg(a) => {
            let op = emit_expr(e, a, env)?;
            let r = e.reg();
            e.line(format!("{r} = sub i64 0, {op}"));
            Ok(r)
        }
        CoreExprKind::Bin(op, a, b) => emit_bin(e, *op, a, b, env),
        CoreExprKind::Call(name, _) => Err(CodegenError::Unsupported(format!(
            "builtin call '{name}' in codegen (holonomic closed-form codegen is backend work)"
        ))),
        CoreExprKind::Reduction {
            kind,
            binder,
            domain,
            body,
        } => emit_reduction(e, *kind, binder, domain, body, env),
        CoreExprKind::Match { .. } => Err(CodegenError::Unsupported(
            "integer `match` is eval-executable (jeff-jlir) but not yet LLVM-lowered (Stage 23 subset)"
                .to_string(),
        )),
    }
}

fn emit_bin(
    e: &mut Emit,
    op: BinOp,
    a: &CoreExpr,
    b: &CoreExpr,
    env: &mut HashMap<String, String>,
) -> Result<String, CodegenError> {
    // Pow with constant exponent → unrolled multiplication.
    if let BinOp::Pow = op {
        let CoreExprKind::Int(exp) = &b.kind else {
            return Err(CodegenError::Unsupported(
                "non-constant exponent".to_string(),
            ));
        };
        let exp = exp
            .try_into()
            .map_err(|_| CodegenError::Unsupported("huge exponent".to_string()))?;
        let base = emit_expr(e, a, env)?;
        if exp == 0u32 {
            return Ok("1".to_string());
        }
        let mut acc = base.clone();
        for _ in 1..exp {
            let r = e.reg();
            e.line(format!("{r} = mul i64 {acc}, {base}"));
            acc = r;
        }
        return Ok(acc);
    }
    let la = emit_expr(e, a, env)?;
    let lb = emit_expr(e, b, env)?;
    let r = e.reg();
    let instr = match op {
        BinOp::Add => format!("{r} = add i64 {la}, {lb}"),
        BinOp::Sub => format!("{r} = sub i64 {la}, {lb}"),
        BinOp::Mul => format!("{r} = mul i64 {la}, {lb}"),
        // Exact division on the collapse path uses sdiv (truncation == floor when exact).
        BinOp::Div | BinOp::FloorDiv => format!("{r} = sdiv i64 {la}, {lb}"),
        BinOp::Rem => format!("{r} = srem i64 {la}, {lb}"),
        BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => {
            let pred = match op {
                BinOp::Eq => "eq",
                BinOp::Ne => "ne",
                BinOp::Lt => "slt",
                BinOp::Le => "sle",
                BinOp::Gt => "sgt",
                BinOp::Ge => "sge",
                _ => unreachable!(),
            };
            let cmp = e.reg();
            e.line(format!("{cmp} = icmp {pred} i64 {la}, {lb}"));
            format!("{r} = zext i1 {cmp} to i64")
        }
        BinOp::Pow => unreachable!("handled above"),
    };
    e.line(instr);
    Ok(r)
}

fn emit_reduction(
    e: &mut Emit,
    kind: RedKind,
    binder: &str,
    domain: &CoreDomain,
    body: &CoreExpr,
    env: &mut HashMap<String, String>,
) -> Result<String, CodegenError> {
    let CoreDomain::Range { lo, hi, inclusive } = domain;
    let prev = e.cur.clone();
    let lo_op = emit_expr(e, lo, env)?;
    let hi_op = emit_expr(e, hi, env)?;

    let cond_l = e.label_name("cond");
    let body_l = e.label_name("body");
    let end_l = e.label_name("end");

    let i = e.reg();
    let acc = e.reg();
    let inext = e.reg();
    let accnext = e.reg();

    let init = match kind {
        RedKind::Sum | RedKind::Count | RedKind::Fold => "0",
        RedKind::Prod => "1",
    };

    e.line(format!("br label %{cond_l}"));
    e.place_label(&cond_l);
    e.line(format!(
        "{i} = phi i64 [ {lo_op}, %{prev} ], [ {inext}, %{body_l} ]"
    ));
    e.line(format!(
        "{acc} = phi i64 [ {init}, %{prev} ], [ {accnext}, %{body_l} ]"
    ));
    let cmp = e.reg();
    let pred = if *inclusive { "sgt" } else { "sge" }; // exit condition
    e.line(format!("{cmp} = icmp {pred} i64 {i}, {hi_op}"));
    e.line(format!("br i1 {cmp}, label %{end_l}, label %{body_l}"));

    e.place_label(&body_l);
    let saved = env.insert(binder.to_string(), i.clone());
    let body_op = emit_expr(e, body, env)?;
    // Stage-0 codegen does not support nested control flow in a reduction body
    // (the back-edge predecessor would differ from %body_l). Refuse rather than
    // emit wrong IR (R24/P0).
    if e.cur != body_l {
        return Err(CodegenError::Unsupported(
            "nested control flow in a reduction body".to_string(),
        ));
    }
    match saved {
        Some(v) => {
            env.insert(binder.to_string(), v);
        }
        None => {
            env.remove(binder);
        }
    }
    let combine = match kind {
        RedKind::Sum | RedKind::Count | RedKind::Fold => "add",
        RedKind::Prod => "mul",
    };
    e.line(format!("{accnext} = {combine} i64 {acc}, {body_op}"));
    e.line(format!("{inext} = add i64 {i}, 1"));
    e.line(format!("br label %{cond_l}"));
    e.place_label(&end_l);
    Ok(acc)
}

fn i64_literal(v: &BigInt) -> Result<String, CodegenError> {
    use num_traits::ToPrimitive;
    v.to_i64()
        .map(|x| x.to_string())
        .ok_or_else(|| CodegenError::Unsupported(format!("integer literal {v} exceeds i64")))
}

/// A complete, compilable LLVM module: the region's function plus a `main` that
/// calls it with `args` and prints the result. `args` must match `region.params`.
pub fn emit_module(region: &JlirRegion, args: &[i64]) -> Result<String, CodegenError> {
    if args.len() != region.params.len() {
        return Err(CodegenError::Unsupported(format!(
            "expected {} args, got {}",
            region.params.len(),
            args.len()
        )));
    }
    let func = emit_function(region)?;
    let call_args: Vec<String> = args.iter().map(|a| format!("i64 {a}")).collect();
    let name = sanitize(&region.name);
    let mut m = String::new();
    m.push_str(&format!("target triple = \"{TARGET_TRIPLE}\"\n"));
    m.push_str("@.fmt = private unnamed_addr constant [5 x i8] c\"%ld\\0A\\00\"\n");
    m.push_str("declare i32 @printf(ptr, ...)\n");
    m.push_str(&func);
    m.push_str("define i32 @main() {\nentry:\n");
    m.push_str(&format!(
        "  %r = call i64 @{name}({})\n",
        call_args.join(", ")
    ));
    m.push_str("  %p = getelementptr inbounds [5 x i8], ptr @.fmt, i64 0, i64 0\n");
    m.push_str("  call i32 (ptr, ...) @printf(ptr %p, i64 %r)\n");
    m.push_str("  ret i32 0\n}\n");
    Ok(m)
}

/// Compile the module with `clang` and run it, returning the printed integer. This
/// is the real "codegen → run" path of the e2e (PART 19).
pub fn compile_and_run(region: &JlirRegion, args: &[i64]) -> Result<BigInt, CodegenError> {
    use std::process::Command;
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let module = emit_module(region, args)?;
    let dir = std::env::temp_dir();
    let pid = std::process::id();
    // Unique per invocation so parallel calls (and parallel tests) never collide.
    let uniq = COUNTER.fetch_add(1, Ordering::Relaxed);
    let stamp = format!("{}_{}_{}", pid, sanitize(&region.name), uniq);
    let ll = dir.join(format!("jeffc_{stamp}.ll"));
    let bin = dir.join(format!("jeffc_{stamp}.out"));
    std::fs::write(&ll, &module).map_err(|e| CodegenError::Io(e.to_string()))?;
    let out = Command::new("clang")
        .arg(&ll)
        .arg("-O0")
        .arg("-o")
        .arg(&bin)
        .output()
        .map_err(|e| CodegenError::Io(format!("spawning clang: {e}")))?;
    if !out.status.success() {
        return Err(CodegenError::Clang(
            String::from_utf8_lossy(&out.stderr).to_string(),
        ));
    }
    let run = Command::new(&bin)
        .output()
        .map_err(|e| CodegenError::Io(format!("running binary: {e}")))?;
    let _ = std::fs::remove_file(&ll);
    let _ = std::fs::remove_file(&bin);
    if !run.status.success() {
        return Err(CodegenError::Clang("program exited nonzero".to_string()));
    }
    let s = String::from_utf8_lossy(&run.stdout);
    s.trim()
        .parse::<BigInt>()
        .map_err(|e| CodegenError::Parse(format!("{e}: {:?}", s.trim())))
}

#[cfg(test)]
mod tests {
    use super::*;
    use jeff_core_ir::{CoreExprKind, CoreFn, CoreParam, CoreTy};
    use jeff_jlir::JlirRegion;
    use jeff_span::Span;

    fn span() -> Span {
        Span::dummy()
    }
    fn int(v: i64) -> CoreExpr {
        CoreExpr {
            kind: CoreExprKind::Int(BigInt::from(v)),
            span: span(),
        }
    }
    fn var(n: &str) -> CoreExpr {
        CoreExpr {
            kind: CoreExprKind::Var(n.into()),
            span: span(),
        }
    }
    fn bin(op: BinOp, a: CoreExpr, b: CoreExpr) -> CoreExpr {
        CoreExpr {
            kind: CoreExprKind::Bin(op, Box::new(a), Box::new(b)),
            span: span(),
        }
    }

    fn region_from(name: &str, body: CoreExpr) -> JlirRegion {
        let f = CoreFn {
            name: name.into(),
            params: vec![CoreParam {
                name: "n".into(),
                ty: CoreTy::Nat,
            }],
            ret: CoreTy::Nat,
            body,
            total: true,
            span: span(),
        };
        JlirRegion::deferred(&f, jeff_cert::BarrierTag::ConstantFactorOnly)
    }

    #[test]
    fn codegen_closed_form_runs() {
        // residual (n*(n+1))/2
        let body = bin(
            BinOp::Div,
            bin(BinOp::Mul, var("n"), bin(BinOp::Add, var("n"), int(1))),
            int(2),
        );
        let region = region_from("triangular_cf", body);
        let v = compile_and_run(&region, &[10]).unwrap();
        assert_eq!(v, BigInt::from(55));
        let v2 = compile_and_run(&region, &[1000]).unwrap();
        assert_eq!(v2, BigInt::from(500500));
    }

    #[test]
    fn codegen_reduction_loop_runs() {
        // original sum i in 0..=n: i
        let body = CoreExpr {
            kind: CoreExprKind::Reduction {
                kind: RedKind::Sum,
                binder: "i".into(),
                domain: CoreDomain::Range {
                    lo: Box::new(int(0)),
                    hi: Box::new(var("n")),
                    inclusive: true,
                },
                body: Box::new(var("i")),
            },
            span: span(),
        };
        let region = region_from("triangular_loop", body);
        assert_eq!(compile_and_run(&region, &[10]).unwrap(), BigInt::from(55));
        assert_eq!(
            compile_and_run(&region, &[1000]).unwrap(),
            BigInt::from(500500)
        );
    }
}
