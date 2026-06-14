//! jeffc CLI (CLAUDE.md PART 5.3).
//!
//! Usage:
//!   jeffc build <file> [--collapse-report] [--emit-certificates <dir>]
//!                      [--emit-llvm] [--total] [--const-time-audit] [--opt-level N]
//!   jeffc run   <file> <fn> <arg>...        (execute via the exact evaluator)
//!
//! User-input errors are reported as diagnostics, never panics (R38).

use jeffc::{collapse_report, compile, emit_certificates, emit_llvm, run, Options};
use num_bigint::BigInt;
use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("{}", USAGE);
        return ExitCode::from(2);
    }
    match args[0].as_str() {
        "build" => cmd_build(&args[1..]),
        "run" => cmd_run(&args[1..]),
        "--help" | "-h" => {
            println!("{USAGE}");
            ExitCode::SUCCESS
        }
        other => {
            eprintln!("jeffc: unknown command {other:?}\n{USAGE}");
            ExitCode::from(2)
        }
    }
}

const USAGE: &str = "jeffc — JEFF/GACC compiler driver\n\
\n\
  jeffc build <file> [options]\n\
    --collapse-report          per-function collapse coverage + barrier tags\n\
    --emit-certificates <dir>  write proof objects (manifest + per-fn cert.json)\n\
    --emit-llvm                print LLVM IR text\n\
    --total                    require Total mode\n\
    --const-time-audit         audit secret[T] paths\n\
    --opt-level <0..3>         backend Layer B strength\n\
\n\
  jeffc run <file> <fn> <arg>...   execute a function via the exact evaluator\n";

fn read(file: &str) -> Result<String, ExitCode> {
    std::fs::read_to_string(file).map_err(|e| {
        eprintln!("jeffc: cannot read {file:?}: {e}");
        ExitCode::from(2)
    })
}

fn print_diags(ds: &[jeff_span::Diagnostic]) {
    for d in ds {
        eprintln!("{d}\n");
    }
}

fn cmd_build(args: &[String]) -> ExitCode {
    let mut file = None;
    let mut opts = Options::default();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--collapse-report" => opts.collapse_report = true,
            "--emit-llvm" => opts.emit_llvm = true,
            "--total" => opts.total = true,
            "--const-time-audit" => opts.const_time_audit = true,
            "--emit-certificates" => {
                i += 1;
                let Some(dir) = args.get(i) else {
                    eprintln!("jeffc: --emit-certificates needs a <dir>");
                    return ExitCode::from(2);
                };
                opts.emit_certificates = Some(PathBuf::from(dir));
            }
            "--opt-level" => {
                i += 1;
                opts.opt_level = args.get(i).and_then(|s| s.parse().ok()).unwrap_or(0);
            }
            f if !f.starts_with("--") => file = Some(f.to_string()),
            other => {
                eprintln!("jeffc: unknown build option {other:?}");
                return ExitCode::from(2);
            }
        }
        i += 1;
    }
    let Some(file) = file else {
        eprintln!("jeffc: build needs a <file>");
        return ExitCode::from(2);
    };
    let src = match read(&file) {
        Ok(s) => s,
        Err(c) => return c,
    };
    let art = match compile(&src, &opts) {
        Ok(a) => a,
        Err(ds) => {
            print_diags(&ds);
            return ExitCode::FAILURE;
        }
    };
    if opts.collapse_report {
        print!("{}", collapse_report(&art));
    }
    if opts.emit_llvm {
        match emit_llvm(&art) {
            Ok(s) => print!("{s}"),
            Err(e) => {
                eprintln!("jeffc: emit-llvm: {e}");
                return ExitCode::FAILURE;
            }
        }
    }
    if let Some(dir) = &opts.emit_certificates {
        if let Err(e) = emit_certificates(&art, dir) {
            eprintln!("jeffc: emit-certificates: {e}");
            return ExitCode::FAILURE;
        }
    }
    if opts.const_time_audit {
        // R6: audit secret[T] paths (jeff-types). FAIL is a hard error (CI gate).
        match jeffc::const_time_audit(&src) {
            Ok((report, all_ok)) => {
                if report.is_empty() {
                    println!("const-time-audit: OK (no secret[T] paths in this module)");
                } else {
                    print!("{report}");
                    if !all_ok {
                        eprintln!("const-time-audit: FAIL (R6: secret-dependent branch/index)");
                        return ExitCode::FAILURE;
                    }
                }
            }
            Err(ds) => {
                print_diags(&ds);
                return ExitCode::FAILURE;
            }
        }
    }
    if !opts.collapse_report && !opts.emit_llvm && opts.emit_certificates.is_none() {
        println!("built {} function(s)", art.funcs.len());
    }
    ExitCode::SUCCESS
}

fn cmd_run(args: &[String]) -> ExitCode {
    if args.len() < 2 {
        eprintln!("jeffc: run needs <file> <fn> <arg>...");
        return ExitCode::from(2);
    }
    let file = &args[0];
    let func = &args[1];
    let src = match read(file) {
        Ok(s) => s,
        Err(c) => return c,
    };
    let art = match compile(&src, &Options::default()) {
        Ok(a) => a,
        Err(ds) => {
            print_diags(&ds);
            return ExitCode::FAILURE;
        }
    };
    let Some(cf) = art.func(func) else {
        eprintln!("jeffc: no function named {func:?}");
        return ExitCode::from(2);
    };
    // bind positional args to the function's parameters, in order
    let mut bindings = Vec::new();
    for (k, (pname, _)) in cf.region.params.iter().enumerate() {
        let Some(raw) = args.get(2 + k) else {
            eprintln!("jeffc: function {func:?} expects {} argument(s)", cf.region.params.len());
            return ExitCode::from(2);
        };
        let Ok(v) = raw.parse::<BigInt>() else {
            eprintln!("jeffc: argument {raw:?} is not an integer");
            return ExitCode::from(2);
        };
        bindings.push((pname.clone(), v));
    }
    match run(&art, func, &bindings) {
        Some(v) => {
            println!("{v}");
            ExitCode::SUCCESS
        }
        None => {
            eprintln!("jeffc: evaluation failed");
            ExitCode::FAILURE
        }
    }
}
