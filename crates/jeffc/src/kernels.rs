//! Stage 11 — surface-wiring of stdlib kernels (CLAUDE.md PART G). A `.jeff` function
//! whose body is a call to a known kernel (with literal array/matrix data) is recognized
//! here, its constant data extracted from the AST, and the matching collapser run at
//! compile time — the **auto collapse trigger**. The result is a verified collapse (with
//! the real certificate class) or an honest defer (with a barrier tag). This is what
//! makes the kernels callable from `.jeff` source rather than library-only.

use jeff_cert::{BarrierTag, CertClass, CollapseOutcome};
use jeff_collapse_arith::{fourier, geometry, moments, planted, sparse, streaming};
use jeff_syntax::ast::{Expr, ExprKind, FnDecl, Lit, StmtKind, UnOp};

/// A surface-wired kernel call compiled at comptime.
#[derive(Clone, Debug)]
pub struct KernelFn {
    pub name: String,
    pub kernel: String,
    pub status: KernelStatus,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KernelStatus {
    /// Structure found; verified at this class by this checker (auto-triggered collapse).
    Collapsed { checker: String, cert_class: CertClass },
    /// Structure absent; honest deferral with a named barrier.
    Deferred { tag: BarrierTag },
}

// ---- constant extraction from the AST (compile-time data) ----

fn unwrap_paren(e: &Expr) -> &Expr {
    match &e.kind {
        ExprKind::Paren(a) => unwrap_paren(a),
        _ => e,
    }
}

fn const_f64(e: &Expr) -> Option<f64> {
    match &unwrap_paren(e).kind {
        ExprKind::Lit(Lit::Float(s)) => s.parse().ok(),
        ExprKind::Lit(Lit::Int(s, _)) => s.parse::<i64>().ok().map(|v| v as f64),
        ExprKind::Lit(Lit::Rat(n, d)) => {
            Some(n.parse::<f64>().ok()? / d.parse::<f64>().ok()?)
        }
        ExprKind::Un(UnOp::Neg, a) => const_f64(a).map(|v| -v),
        _ => None,
    }
}

fn const_u64(e: &Expr) -> Option<u64> {
    match &unwrap_paren(e).kind {
        ExprKind::Lit(Lit::Int(s, _)) => s.parse().ok(),
        _ => None,
    }
}

fn const_usize(e: &Expr) -> Option<usize> {
    const_u64(e).map(|v| v as usize)
}

fn as_array(e: &Expr) -> Option<&[Expr]> {
    match &unwrap_paren(e).kind {
        ExprKind::Array(elems) => Some(elems),
        _ => None,
    }
}

fn const_f64_vec(e: &Expr) -> Option<Vec<f64>> {
    as_array(e)?.iter().map(const_f64).collect()
}

fn const_u64_vec(e: &Expr) -> Option<Vec<u64>> {
    as_array(e)?.iter().map(const_u64).collect()
}

/// A row-major `f64` matrix from a nested array; returns `(flat, rows, cols)`.
fn const_f64_matrix(e: &Expr) -> Option<(Vec<f64>, usize, usize)> {
    let rows = as_array(e)?;
    let mut flat = Vec::new();
    let mut cols = None;
    for r in rows {
        let row = const_f64_vec(r)?;
        match cols {
            None => cols = Some(row.len()),
            Some(c) if c != row.len() => return None, // ragged
            _ => {}
        }
        flat.extend(row);
    }
    Some((flat, rows.len(), cols.unwrap_or(0)))
}

/// A square `0/1` adjacency from a nested array; returns `(flat u8, n)`.
fn const_u8_matrix(e: &Expr) -> Option<(Vec<u8>, usize)> {
    let (flat, rows, cols) = const_f64_matrix(e)?;
    if rows != cols {
        return None;
    }
    let u8s: Option<Vec<u8>> = flat
        .iter()
        .map(|&v| if v == 0.0 { Some(0u8) } else if v == 1.0 { Some(1u8) } else { None })
        .collect();
    Some((u8s?, rows))
}

// ---- recognize + dispatch a kernel call ----

/// If `f`'s body is a recognized kernel call with constant data, run the collapser at
/// compile time and return its status. `None` ⇒ not a kernel call (use the normal path).
pub fn try_kernel_fn(f: &FnDecl) -> Option<KernelFn> {
    let body = single_call(f)?;
    let (name, args) = body;
    let (kernel, outcome) = dispatch_kernel(name, args)?;
    Some(KernelFn {
        name: f.name.clone(),
        kernel: kernel.to_string(),
        status: classify(outcome),
    })
}

/// Extract a single-call body `kernel(args...)` from a function (single-expression body).
fn single_call(f: &FnDecl) -> Option<(&str, &[Expr])> {
    let stmt = match f.body.stmts.as_slice() {
        [s] => s,
        _ => return None,
    };
    let expr = match &stmt.kind {
        StmtKind::Expr(e) | StmtKind::Return(Some(e)) => e,
        _ => return None,
    };
    match &unwrap_paren(expr).kind {
        ExprKind::Call(callee, args) => match &callee.kind {
            ExprKind::Var(name) => Some((name.as_str(), args.as_slice())),
            _ => None,
        },
        _ => None,
    }
}

fn dispatch_kernel(name: &str, args: &[Expr]) -> Option<(&'static str, CollapseOutcome)> {
    match name {
        // prony(samples[], modes) — exact recurrence when clean (tol 1e-9)
        "prony" => {
            let samples = const_f64_vec(args.first()?)?;
            let modes = const_usize(args.get(1)?)?;
            Some(("sparse/prony", sparse::prony(&samples, modes, 1e-9)))
        }
        // sparse_fft(signal[], k)
        "sparse_fft" => {
            let signal = const_f64_vec(args.first()?)?;
            let k = const_usize(args.get(1)?)?;
            Some(("sparse/sparse-fft", sparse::sparse_fft(&signal, k, 1e-6)))
        }
        // welch_etf(frame[][]) — m×n frame (m>n)
        "welch_etf" => {
            let (frame, m, n) = const_f64_matrix(args.first()?)?;
            Some(("sparse/welch-etf", sparse::equiangular_tight_frame(&frame, m, n, 1e-6)))
        }
        // planted_clique(adj[][], k)
        "planted_clique" => {
            let (adj, n) = const_u8_matrix(args.first()?)?;
            let k = const_usize(args.get(1)?)?;
            Some(("planted/clique", planted::planted_clique(&adj, n, k)))
        }
        // persistent_homology(dist[][], band)
        "persistent_homology" => {
            let (dist, n) = {
                let (flat, r, c) = const_f64_matrix(args.first()?)?;
                if r != c {
                    return None;
                }
                (flat, r)
            };
            let band = const_f64(args.get(1)?)?;
            Some(("geometry/persistent-homology", geometry::persistent_homology(&dist, n, band)))
        }
        // list_decode(xs[], ys[], k, q)
        "list_decode" => {
            let xs = const_u64_vec(args.first()?)?;
            let ys = const_u64_vec(args.get(1)?)?;
            let k = const_usize(args.get(2)?)?;
            let q = const_u64(args.get(3)?)?;
            Some(("fourier/list-decode", fourier::list_decode(&xs, &ys, k, q)))
        }
        // frequency_moment(items[], k) — AMS F_k (F2 collapses; high moments defer)
        "frequency_moment" => {
            let items = const_u64_vec(args.first()?)?;
            let k = const_usize(args.get(1)?)?;
            Some(("streaming/ams-f2", streaming::frequency_moment(&items, k, 0.5)))
        }
        // heavy_hitters(items[], k, phi)
        "heavy_hitters" => {
            let items = const_u64_vec(args.first()?)?;
            let k = const_usize(args.get(1)?)?;
            let phi = const_f64(args.get(2)?)?;
            Some(("streaming/heavy-hitters", streaming::heavy_hitters(&items, k, phi)))
        }
        // point_mass_mixture(moments[], k) — method of moments via Prony
        "point_mass_mixture" => {
            let m = const_f64_vec(args.first()?)?;
            let k = const_usize(args.get(1)?)?;
            Some(("moments/mixture", moments::point_mass_mixture(&m, k, 1e-6)))
        }
        _ => None,
    }
}

fn classify(o: CollapseOutcome) -> KernelStatus {
    match o {
        CollapseOutcome::Collapsed(c) => {
            let cert = c.certificate().certificate();
            KernelStatus::Collapsed {
                checker: jeff_verify::checker_name(&cert.evidence).to_string(),
                cert_class: cert.evidence.cert_class(),
            }
        }
        CollapseOutcome::Defer(d) => KernelStatus::Deferred { tag: d.tag },
    }
}

/// True if any function in the program is a kernel-call function (so the driver can route
/// it here instead of through the integer pipeline).
pub fn is_kernel_fn(f: &FnDecl) -> bool {
    single_call(f).map(|(name, _)| KERNEL_NAMES.contains(&name)).unwrap_or(false)
}

const KERNEL_NAMES: &[&str] = &[
    "prony",
    "sparse_fft",
    "welch_etf",
    "planted_clique",
    "persistent_homology",
    "list_decode",
    "frequency_moment",
    "heavy_hitters",
    "point_mass_mixture",
];
