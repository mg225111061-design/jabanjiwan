//! Layer B backend (CLAUDE.md PART C, 10.7). The backend changes **speed, never the
//! answer** (P0): every vectorized path is checked bit-for-bit (integers) or within a
//! stated tolerance (floats) against the scalar oracle. Register allocation and
//! instruction selection are delegated to LLVM (via rustc / clang); what JEFF owns here
//! is vectorization of hot inner loops, an honest measurement harness (Amdahl-p +
//! baseline comparison), and PGO cost-model hooks.
//!
//! Honesty (R8/DR2): the harness *measures*; nothing reports a hardcoded speedup. The
//! baseline-comparison rig compares JEFF's fast path to an **in-house naive baseline**
//! (the exact oracle). Wiring the tuned incumbents (FFTW/cuBLAS/LAPACK) is an
//! out-of-process step (license: R5) and is labeled as such where used — we never
//! claim an incumbent comparison we did not run.

pub mod autotune;
pub mod amx;
pub mod cost;
pub mod cpuprobe;
pub mod measure;
pub mod simd;
pub mod spacetime;

pub use cost::CostWeights;
pub use measure::{amdahl_speedup, BenchResult, Timer};
