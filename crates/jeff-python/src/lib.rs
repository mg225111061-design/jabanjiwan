//! Stage 13 — real CPython embedding (master directive PART F).
//!
//! JEFF embeds the actual CPython interpreter (FFI to `libpython`) and drives it, rather
//! than reimplementing the scientific stack. The defining discipline is the **verification
//! boundary** (P1, layer 8): verification stops at the FFI. JEFF proves its OWN kernels;
//! every value that crosses from Python is a **trusted axiom**, never a verified step. The
//! type system enforces this — Python results are returned as [`Trusted<T>`], and there is
//! no path from a `Trusted` value to a verified certificate.
//!
//! CPython linkage is behind the `embed` feature (off by default) so the default workspace
//! build/CI needs no Python and stays deterministic. Build/test the live interpreter with
//! `cargo test -p jeff-python --features embed`.

/// Where a value came from. `Verified` is reserved for pure-JEFF kernels backed by a
/// machine-checked certificate; anything obtained through foreign code is `Trusted`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Provenance {
    /// Produced and checked entirely inside JEFF (carries a certificate elsewhere).
    Verified,
    /// Obtained through foreign code (Python/NumPy/…) — an axiom, not a proof.
    Trusted,
}

/// A value obtained across the FFI. It can only ever be `Trusted` (P1): constructing one
/// asserts nothing about correctness, and there is deliberately no `into_verified`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Trusted<T> {
    value: T,
}

impl<T> Trusted<T> {
    /// Wrap a foreign value. Always `Trusted` provenance.
    pub fn new(value: T) -> Self {
        Trusted { value }
    }
    /// The provenance is always [`Provenance::Trusted`].
    pub fn provenance(&self) -> Provenance {
        Provenance::Trusted
    }
    /// Read the underlying value (the caller acknowledges it is unverified).
    pub fn get(&self) -> &T {
        &self.value
    }
    /// Consume and return the underlying value.
    pub fn into_inner(self) -> T {
        self.value
    }
}

/// An error raised by the embedded interpreter (the Python exception text), or a marshalling
/// failure at the boundary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PyError(pub String);

impl std::fmt::Display for PyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "python: {}", self.0)
    }
}
impl std::error::Error for PyError {}

#[cfg(feature = "embed")]
mod ffi;

#[cfg(feature = "embed")]
pub use ffi::{eval_f64, eval_i64, eval_string, exec, run, ensure_init};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trusted_values_are_never_verified() {
        // The whole point of layer 8: a foreign value's provenance is Trusted, full stop.
        let t = Trusted::new(42i64);
        assert_eq!(t.provenance(), Provenance::Trusted);
        assert_eq!(*t.get(), 42);
        assert_ne!(t.provenance(), Provenance::Verified);
    }
}
