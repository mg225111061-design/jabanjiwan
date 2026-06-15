//! Exact arithmetic backbone for GACC.
//!
//! Everything here is exact (`BigRational` / `BigInt` / modular integers / GF(2)) —
//! no floats on the collapse path (R33). These primitives back *real* certificate
//! checking (PART 11, APPENDIX F): polynomial identities by coefficient-zero
//! normalisation, GF(2) linearity by basis evaluation, matrix-power closures by
//! Cayley–Hamilton, and numeric residuals by exact modular replay.
//!
//! Licensing (R5 / D9): arbitrary precision via `num-bigint`/`num-rational`
//! (MIT/Apache), never GMP/`rug` (LGPL).

pub mod anytime;
pub mod bbp;
pub mod cfinite;
pub mod cohomology;
pub mod complex;
pub mod dilithium;
pub mod displacement;
pub mod expfit;
pub mod fft;
pub mod fmat;
pub mod galois;
pub mod fourier;
pub mod frame;
pub mod geometry;
pub mod gf2;
pub mod holographic;
pub mod holsum;
pub mod hbfc;
pub mod hyper;
pub mod intrel;
pub mod keccak;
pub mod koopman;
pub mod krylov;
pub mod kyber;
pub mod lattice;
pub mod linsolve;
pub mod matrix;
pub mod mldsa;
pub mod mlkem;
pub mod modular;
pub mod moments;
pub mod nbody;
pub mod ordinal;
pub mod ot;
pub mod planted;
pub mod poly;
pub mod pqc;
pub mod prony;
pub mod recurrence;
pub mod recovery;
pub mod ratfun;
pub mod riccati;
pub mod selector;
pub mod sos;
pub mod sparsefft;
pub mod sparsefft2d;
pub mod streaming;
pub mod tensornet;
pub mod tff;
pub mod tetration;
pub mod tropical;

pub use gf2::{Gf2Matrix, Gf2Vec};
pub use hyper::{HyperTerm, LinForm};
pub use matrix::{bostan_mori, freivalds_seeds, IntMatrix, ModMatrix, RatMatrix};
pub use modular::{ModInt, NttCtx};
pub use poly::{Poly, UniPoly};
pub use ratfun::RatFunc;
