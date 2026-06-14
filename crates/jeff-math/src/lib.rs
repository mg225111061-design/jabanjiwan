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

pub mod gf2;
pub mod hyper;
pub mod matrix;
pub mod modular;
pub mod poly;
pub mod ratfun;

pub use gf2::{Gf2Matrix, Gf2Vec};
pub use hyper::{HyperTerm, LinForm};
pub use matrix::{bostan_mori, ModMatrix, RatMatrix};
pub use modular::{ModInt, NttCtx};
pub use poly::{Poly, UniPoly};
pub use ratfun::RatFunc;
