//! jeffc driver: the Stage-0 compilation pipeline (CLAUDE.md PART 8, PART 19, J.1).
//!
//! `parse → lower → recognize → collapse | defer → verify → JLIR → codegen`, with a
//! fallback at every boundary (R1/P0): if a function does not collapse to a verified
//! residual, its region runs the *original* work. The exact evaluator (jeff-jlir) is
//! the universal executor and the P0 equivalence oracle.

pub mod kernels;

use jeff_cert::{BarrierTag, VerifiedCertificate};
use jeff_core_ir::{lower, CoreFn};
use jeff_jlir::{JlirRegion, Origin};
use jeff_recognizer::{recognize, AsymptoticCost, Collapser, DispatchTag, Recognized, SaturationBudget};
use jeff_span::Diagnostic;
use num_bigint::BigInt;
use std::path::PathBuf;

/// CLI / driver options (PART 5.3, B.12).
#[derive(Clone, Debug, Default)]
pub struct Options {
    pub emit_certificates: Option<PathBuf>,
    pub collapse_report: bool,
    pub total: bool,
    pub const_time_audit: bool,
    pub emit_jlir: bool,
    pub emit_llvm: bool,
    pub opt_level: u8,
    /// Test hook (R1 fallback harness): force every collapse to be discarded, so we
    /// can prove the fallback path computes the same answer (never-miscompile).
    pub force_fallback: bool,
}

/// Per-function compilation status, the source of truth for honest reporting.
#[derive(Clone, Debug)]
pub enum CompileStatus {
    /// Verified collapse.
    Collapsed {
        layer: u8,
        collapser: String,
        checker: String,
        candidate_cost: AsymptoticCost,
    },
    /// Honest deferral with a named barrier (or constant-factor-only).
    Deferred { tag: BarrierTag },
    /// Recognized as collapsible, but the collapser for this class is a later stage
    /// (R24: tracked, not silently skipped, AR-2: still recognize-or-defer).
    Pending { recognized: DispatchTag, stage: &'static str },
}

/// One compiled function.
#[derive(Clone, Debug)]
pub struct CompiledFn {
    pub name: String,
    pub region: JlirRegion,
    pub status: CompileStatus,
    pub original_cost: AsymptoticCost,
}

/// The compilation artifact (B.12).
#[derive(Clone, Debug)]
pub struct Artifact {
    pub funcs: Vec<CompiledFn>,
    /// Surface-wired stdlib kernel calls compiled at comptime (Stage 11).
    pub kernels: Vec<kernels::KernelFn>,
}

impl Artifact {
    pub fn func(&self, name: &str) -> Option<&CompiledFn> {
        self.funcs.iter().find(|f| f.name == name)
    }
    /// Look up a surface-wired kernel function by name.
    pub fn kernel(&self, name: &str) -> Option<&kernels::KernelFn> {
        self.kernels.iter().find(|k| k.name == name)
    }
}

/// Compile source into an artifact (B.12). Diagnostics from any stage are returned
/// (R20); never panics on user input (R38).
pub fn compile(src: &str, opts: &Options) -> Result<Artifact, Vec<Diagnostic>> {
    let program = jeff_syntax::parse(src, 0)?;
    // R6: secret-taint is a *type rule*, not opt-in. A data-dependent branch/index on a
    // secret[T] value is a compile error regardless of `@constant_time` (PART 7.4, C.4).
    // We surface these before lowering so security errors are reported even for functions
    // that use constructs outside the executable core subset.
    let taint_diags = jeff_types::check_secret_taint(&program);
    if taint_diags.iter().any(Diagnostic::is_error) {
        return Err(taint_diags);
    }

    // Stage 11: split out surface-wired kernel-call functions. They carry array/matrix
    // literal data the integer pipeline does not lower, so they are compiled at comptime
    // (the auto collapse trigger) and removed from the program before `lower`.
    let mut kernel_fns = Vec::new();
    let mut rest = program.clone();
    rest.items.retain(|item| match item {
        jeff_syntax::ast::Item::Fn(f) if kernels::is_kernel_fn(f) => {
            if let Some(kf) = kernels::try_kernel_fn(f) {
                kernel_fns.push(kf);
            }
            false // remove from the integer pipeline
        }
        _ => true,
    });

    let ir = lower(&rest)?;
    let budget = SaturationBudget::default();
    let mut funcs = Vec::new();
    for f in &ir.funcs {
        funcs.push(compile_fn(f, budget, opts));
    }
    Ok(Artifact { funcs, kernels: kernel_fns })
}

fn compile_fn(f: &CoreFn, budget: SaturationBudget, opts: &Options) -> CompiledFn {
    let rec = recognize(f, budget);
    let (region, status) = dispatch(f, &rec, opts);
    // R18: check region invariants (debug-time; surfaced as a fallback otherwise).
    debug_assert!(region.verify_invariants().is_ok());
    CompiledFn {
        name: f.name.clone(),
        region,
        status,
        original_cost: rec.original_cost,
    }
}

/// Dispatch a recognized function to a collapser, or defer (PART 7 dispatch).
fn dispatch(f: &CoreFn, rec: &Recognized, opts: &Options) -> (JlirRegion, CompileStatus) {
    let arith = jeff_collapse_arith::Faulhaber;
    if !opts.force_fallback && arith.entry(rec) {
        let ac = jeff_collapse_arith::collapse(f);
        match ac.outcome {
            jeff_cert::CollapseOutcome::Collapsed(c) => {
                let cert: VerifiedCertificate = c.certificate().clone();
                let checker = jeff_verify::checker_name(&cert.certificate().evidence).to_string();
                let residual = ac.residual.expect("collapsed ⇒ residual present");
                let region = JlirRegion::collapsed(f, residual, cert, 1);
                return (
                    region,
                    CompileStatus::Collapsed {
                        layer: 1,
                        collapser: "arith/faulhaber".to_string(),
                        checker,
                        candidate_cost: rec.candidate_cost,
                    },
                );
            }
            jeff_cert::CollapseOutcome::Defer(d) => {
                return (JlirRegion::deferred(f, d.tag), CompileStatus::Deferred { tag: d.tag });
            }
        }
    }

    // Holonomic (Layer 1): definite hypergeometric sums via Zeilberger.
    if !opts.force_fallback && matches!(rec.tag, DispatchTag::Holonomic) {
        if let Some(ac) = jeff_collapse_arith::holonomic::collapse_holonomic(f) {
            match ac.outcome {
                jeff_cert::CollapseOutcome::Collapsed(c) => {
                    let cert: VerifiedCertificate = c.certificate().clone();
                    let checker = jeff_verify::checker_name(&cert.certificate().evidence).to_string();
                    // residual is a closed form when available, else the original sum
                    let residual = ac.residual.unwrap_or_else(|| f.body.clone());
                    let region = JlirRegion::collapsed(f, residual, cert, 1);
                    return (
                        region,
                        CompileStatus::Collapsed {
                            layer: 1,
                            collapser: "arith/holonomic".to_string(),
                            checker,
                            candidate_cost: rec.candidate_cost,
                        },
                    );
                }
                jeff_cert::CollapseOutcome::Defer(d) => {
                    return (JlirRegion::deferred(f, d.tag), CompileStatus::Deferred { tag: d.tag });
                }
            }
        }
        // collapse_holonomic returned None ⇒ not actually a hypergeometric sum.
    }

    // Not dispatched to a built collapser. Decide an honest status.
    let status = match rec.tag {
        DispatchTag::AffineTripCount => {
            // recognised as affine but Faulhaber declined (or forced fallback)
            CompileStatus::Deferred {
                tag: BarrierTag::ConstantFactorOnly,
            }
        }
        DispatchTag::LinearStateTransition => CompileStatus::Pending {
            recognized: rec.tag,
            stage: "Stage 1/3 (holonomic / eigen)",
        },
        DispatchTag::Holonomic => CompileStatus::Pending {
            recognized: rec.tag,
            stage: "Stage 1 (Gosper/Zeilberger)",
        },
        DispatchTag::Convolution => CompileStatus::Pending {
            recognized: rec.tag,
            stage: "Stage 3/5 (NTT/FFT)",
        },
        DispatchTag::Gf2Affine => CompileStatus::Pending {
            recognized: rec.tag,
            stage: "Stage 5 (GF(2) folder)",
        },
        DispatchTag::PlanarCsp => CompileStatus::Pending {
            recognized: rec.tag,
            stage: "Stage 8 (holographic)",
        },
        DispatchTag::BoundedTreewidth => CompileStatus::Pending {
            recognized: rec.tag,
            stage: "Stage 7 (tensor-network)",
        },
        DispatchTag::None => CompileStatus::Deferred {
            tag: BarrierTag::ConstantFactorOnly,
        },
    };
    // Region always runs the original work (P0 fallback).
    let tag = match &status {
        CompileStatus::Deferred { tag } => *tag,
        _ => BarrierTag::ConstantFactorOnly,
    };
    (JlirRegion::deferred(f, tag), status)
}

/// Execute a compiled function with integer arguments via the exact evaluator
/// (jeff-jlir). This runs the collapsed residual or the deferred original — the
/// fallback harness (R1/P0).
pub fn run(art: &Artifact, name: &str, args: &[(String, BigInt)]) -> Option<BigInt> {
    let cf = art.func(name)?;
    jeff_jlir::eval(&cf.region, args).ok()
}

/// Compile a single function to LLVM and run it via clang (PART 19 codegen→run).
pub fn run_llvm(art: &Artifact, name: &str, args: &[i64]) -> Option<BigInt> {
    let cf = art.func(name)?;
    jeff_codegen::compile_and_run(&cf.region, args).ok()
}

/// Human-readable `--collapse-report` (APPENDIX H.2). Deterministic ordering.
pub fn collapse_report(art: &Artifact) -> String {
    let mut out = String::new();
    for cf in &art.funcs {
        let line = match &cf.status {
            CompileStatus::Collapsed {
                layer,
                collapser,
                checker,
                candidate_cost,
            } => format!(
                "fn {:<16} collapsed  layer={}({})  {}  cert=ok({})",
                cf.name,
                layer,
                collapser,
                cost_str(*candidate_cost),
                checker
            ),
            CompileStatus::Deferred { tag } => format!(
                "fn {:<16} deferred   HONEST_DEFER[{}]  ({})",
                cf.name,
                tag.as_str(),
                tag.rationale()
            ),
            CompileStatus::Pending { recognized, stage } => format!(
                "fn {:<16} deferred   recognized={:?}; collapser pending ({})",
                cf.name, recognized, stage
            ),
        };
        out.push_str(&line);
        out.push('\n');
    }
    // Stage 11: surface-wired kernel calls (compiled at comptime).
    for kf in &art.kernels {
        let line = match &kf.status {
            kernels::KernelStatus::Collapsed { checker, cert_class } => format!(
                "fn {:<16} collapsed  kernel={}  cert=ok({}; class={})",
                kf.name,
                kf.kernel,
                checker,
                cert_class.as_str()
            ),
            kernels::KernelStatus::Deferred { tag } => format!(
                "fn {:<16} deferred   HONEST_DEFER[{}]  ({})",
                kf.name,
                tag.as_str(),
                tag.rationale()
            ),
        };
        out.push_str(&line);
        out.push('\n');
    }
    out
}

/// `--const-time-audit` report (APPENDIX H.4). For every function with `secret[T]`
/// inputs, report OK or FAIL with the secret-taint diagnostics. The audit is sound,
/// not complete (jeff-types): FAIL means a secret may reach a branch/index; OK means
/// no such flow at the source/IR level (see jeff-types for the leakage model — what
/// this does and does not cover). Returns the report and whether all functions passed.
pub fn const_time_audit(src: &str) -> Result<(String, bool), Vec<Diagnostic>> {
    let program = jeff_syntax::parse(src, 0)?;
    let mut out = String::new();
    let mut all_ok = true;
    for item in &program.items {
        let jeff_syntax::ast::Item::Fn(f) = item else {
            continue;
        };
        if !jeff_types::has_secret_inputs(f) {
            continue;
        }
        let diags = jeff_types::audit_fn(f);
        let ct = if jeff_types::is_constant_time(f) {
            "@constant_time "
        } else {
            ""
        };
        if diags.is_empty() {
            out.push_str(&format!(
                "fn {:<16} OK    {}no secret-dependent branch/index (source/IR model)\n",
                f.name, ct
            ));
        } else {
            all_ok = false;
            let detail = diags
                .iter()
                .map(|d| format!("{} at {}", d.code.as_str(), d.span))
                .collect::<Vec<_>>()
                .join("; ");
            out.push_str(&format!("fn {:<16} FAIL  {}{}\n", f.name, ct, detail));
        }
    }
    Ok((out, all_ok))
}

/// Machine-readable collapse report (APPENDIX H.2 JSON).
pub fn collapse_report_json(art: &Artifact) -> serde_json::Value {
    let funcs: Vec<serde_json::Value> = art
        .funcs
        .iter()
        .map(|cf| match &cf.status {
            CompileStatus::Collapsed {
                layer,
                collapser,
                checker,
                candidate_cost,
            } => serde_json::json!({
                "name": cf.name, "status": "collapsed", "layer": layer,
                "collapser": collapser, "cost": cost_str(*candidate_cost),
                "certificate": { "verified": true, "checker": checker }
            }),
            CompileStatus::Deferred { tag } => serde_json::json!({
                "name": cf.name, "status": "deferred", "barrier": tag.as_str()
            }),
            CompileStatus::Pending { recognized, stage } => serde_json::json!({
                "name": cf.name, "status": "deferred",
                "recognized": format!("{recognized:?}"), "collapser_pending": stage
            }),
        })
        .collect();
    serde_json::json!({ "functions": funcs })
}

fn cost_str(c: AsymptoticCost) -> &'static str {
    match c {
        AsymptoticCost::Const => "O(1)",
        AsymptoticCost::Log => "O(log n)",
        AsymptoticCost::Sublinear => "o(n)",
        AsymptoticCost::Linear => "O(n)",
        AsymptoticCost::Superlinear(_) => "superlinear",
    }
}

/// Emit certificates to `<dir>` (APPENDIX H.3). Deterministic (R11): a manifest plus
/// one `<fn>.cert.json` per collapsed function, fields in fixed order.
pub fn emit_certificates(art: &Artifact, dir: &std::path::Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let mut manifest_funcs = Vec::new();
    for cf in &art.funcs {
        if let Origin::Collapsed { cert, .. } = &cf.region.origin {
            let file = format!("{}.cert.json", cf.name);
            let checker = jeff_verify::checker_name(&cert.certificate().evidence);
            // human-readable record (APPENDIX H.3)
            let json = cert.certificate().to_json(checker);
            std::fs::write(dir.join(&file), serde_json::to_string_pretty(&json)?)?;
            // machine-replayable certificate (round-trippable serde) for cert-replay
            // (R25): the replay tool deserializes this and re-runs verify().
            let machine = format!("{}.machine.json", cf.name);
            std::fs::write(
                dir.join(&machine),
                serde_json::to_string_pretty(cert.certificate())?,
            )?;
            manifest_funcs.push(
                serde_json::json!({ "name": cf.name, "cert_file": file, "machine_file": machine }),
            );
        }
    }
    let manifest = serde_json::json!({
        "jeffc_version": env!("CARGO_PKG_VERSION"),
        "functions": manifest_funcs,
    });
    std::fs::write(
        dir.join("manifest.json"),
        serde_json::to_string_pretty(&manifest)?,
    )?;
    Ok(())
}

/// Emit LLVM IR text for every function (`--emit-llvm`).
pub fn emit_llvm(art: &Artifact) -> Result<String, String> {
    let mut out = String::new();
    for cf in &art.funcs {
        match jeff_codegen::emit_function(&cf.region) {
            Ok(f) => out.push_str(&f),
            Err(e) => return Err(format!("{}: {e}", cf.name)),
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn n(v: i64) -> Vec<(String, BigInt)> {
        vec![("n".to_string(), BigInt::from(v))]
    }

    #[test]
    fn e2e_triangular_collapses_and_runs() {
        // Stage 0 DoD: triangular parse→collapse→verify→codegen→run, both paths.
        let art = compile(
            "total fn triangular(n: nat) -> nat: sum i in 0..=n: i\n",
            &Options::default(),
        )
        .unwrap();
        let cf = art.func("triangular").unwrap();
        assert!(matches!(cf.status, CompileStatus::Collapsed { .. }));
        assert!(cf.region.is_collapsed());
        // evaluator path
        assert_eq!(run(&art, "triangular", &n(10)).unwrap(), BigInt::from(55));
        assert_eq!(run(&art, "triangular", &n(1000)).unwrap(), BigInt::from(500500));
        // LLVM codegen → clang → run path
        assert_eq!(run_llvm(&art, "triangular", &[10]).unwrap(), BigInt::from(55));
        assert_eq!(
            run_llvm(&art, "triangular", &[1000]).unwrap(),
            BigInt::from(500500)
        );
    }

    #[test]
    fn e2e_fallback_path_matches_collapse_path_r1() {
        // Force fallback (R1): the function defers and runs the ORIGINAL loop, which
        // must produce the SAME answer as the collapse path (never-miscompile, P0).
        let src = "total fn triangular(n: nat) -> nat: sum i in 0..=n: i\n";
        let collapsed = compile(src, &Options::default()).unwrap();
        let fell_back = compile(
            src,
            &Options {
                force_fallback: true,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(matches!(
            fell_back.func("triangular").unwrap().status,
            CompileStatus::Deferred { .. }
        ));
        for v in [0i64, 1, 7, 100, 5000] {
            let a = run(&collapsed, "triangular", &n(v)).unwrap();
            let b = run(&fell_back, "triangular", &n(v)).unwrap();
            assert_eq!(a, b, "collapse vs fallback differ at n={v}");
            // and both equal the naive sum
            let naive: i64 = (0..=v).sum();
            assert_eq!(a, BigInt::from(naive));
        }
        // LLVM fallback loop also agrees
        assert_eq!(
            run_llvm(&fell_back, "triangular", &[100]).unwrap(),
            BigInt::from(5050)
        );
    }

    #[test]
    fn collapse_report_is_honest_and_deterministic() {
        let art = compile(
            "total fn s2(n: nat) -> nat: sum i in 0..=n: i*i\n",
            &Options::default(),
        )
        .unwrap();
        let r1 = collapse_report(&art);
        let r2 = collapse_report(&art);
        assert_eq!(r1, r2); // deterministic (R11)
        assert!(r1.contains("collapsed"));
        assert!(r1.contains("cert=ok(exact-coeff-zero)"));
    }

    #[test]
    fn emit_certificates_deterministic() {
        let art = compile(
            "total fn s2(n: nat) -> nat: sum i in 0..=n: i*i\n",
            &Options::default(),
        )
        .unwrap();
        let d1 = std::env::temp_dir().join(format!("jeffc_cert_a_{}", std::process::id()));
        let d2 = std::env::temp_dir().join(format!("jeffc_cert_b_{}", std::process::id()));
        emit_certificates(&art, &d1).unwrap();
        emit_certificates(&art, &d2).unwrap();
        let a = std::fs::read_to_string(d1.join("s2.cert.json")).unwrap();
        let b = std::fs::read_to_string(d2.join("s2.cert.json")).unwrap();
        assert_eq!(a, b); // R11
        assert!(a.contains("\"result\": \"valid\""));
        let _ = std::fs::remove_dir_all(&d1);
        let _ = std::fs::remove_dir_all(&d2);
    }
}
