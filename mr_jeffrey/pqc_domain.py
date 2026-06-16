"""
STAGE Y1 — domain selection (PQC) + the production kernel, with grep-verified assets.
====================================================================================
Domain = PQC (post-quantum crypto), JEFF's stated identity (verified numerics + PQC) and the strongest
asset base in the tree: kyber.rs / dilithium.rs / mlkem.rs / mldsa.rs / modular.rs with real `ntt`,
`poly_mul`, `Q=3329`, `N=256`. (DSP — fft.rs/fourier.rs — is also real; PQC chosen for asset depth.)

Production kernel = Kyber NTT-based polynomial multiplication in Z_q[x]/(x^256+1), q=3329:
    poly_mul(a,b) = intt(pointwise(ntt(a), ntt(b)))   — constant-time, a is secret.

This file holds the kernel spec + the sub-computations the kernel is built from, used in Y2 to measure
the REAL (non-toy) fold ratio. Written in this parser's `<>` dialect (secret<T>, mod<q>, Vec<T,n>).
"""
import os

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))

# grep-verified assets (path, symbol-that-must-be-present)
ASSETS = [
    ("crates/jeff-math/src/kyber.rs", "pub fn poly_mul"),
    ("crates/jeff-math/src/kyber.rs", "pub fn ntt"),
    ("crates/jeff-math/src/kyber.rs", "Q: u64 = 3329"),
    ("crates/jeff-math/src/dilithium.rs", "pub fn poly_mul"),
    ("crates/jeff-math/src/mlkem.rs", None),
    ("crates/jeff-math/src/mldsa.rs", None),
    ("crates/jeff-math/src/modular.rs", None),
]


def asset_evidence():
    out = []
    for path, sym in ASSETS:
        full = os.path.join(ROOT, path)
        exists = os.path.isfile(full)
        has = exists and (sym is None or sym in open(full, encoding="utf-8").read())
        out.append((path, sym, bool(has)))
    return out


# --- the production kernel ---
POLY_MUL = """\
fn poly_mul(a: secret<Vec<mod<3329>, 256>>, b: Vec<mod<3329>, 256>) -> secret<Vec<mod<3329>, 256>>
  ensures result = ntt_convolution(a, b)
  effects pure
{ intt(pointwise(ntt(a), ntt(b))) }
"""

# --- the sub-computations the kernel is built from (for the Y2 4-bucket fold-ratio) ---
SUBKERNELS = {
    # setup: twiddle factors ζ^i (ζ=17) — a linear recurrence ⇒ C-finite ⇒ folds O(log i)
    "twiddle (ζ^i, setup)":
        "fn twiddle(i: Nat) -> Nat\n  effects pure\n"
        "{ match i { 0 => 1  _ => 17 * twiddle(i - 1) } }",
    # setup: a parameter polynomial sum ⇒ Faulhaber ⇒ folds O(1)
    "param_sum (setup)":
        "fn param_sum(n: Nat) -> Nat\n  ensures result = n*(n+1)/2\n  effects pure\n"
        "{ fold k in 1..n { k } }",
    # transform: coefficient norm Σ a_i² — data-dependent ⇒ NO_STRUCTURE Ω(N)
    "coeff_norm (transform)":
        "fn coeff_norm(a: Vec<Int, 256>) -> Int\n  effects pure\n"
        "{ fold i in 1..256 { sq(coeff(a, i)) } }",
    # transform: pointwise product — data-dependent ⇒ NO_STRUCTURE Ω(N)
    "pointwise (transform)":
        "fn pointwise(a: Vec<Int, 256>) -> Int\n  effects pure\n"
        "{ fold i in 1..256 { mul(coeff(a, i), coeff(a, i)) } }",
    # transform: NTT butterfly accumulation — data-dependent ⇒ NO_STRUCTURE Ω(N log N)
    "ntt_acc (transform)":
        "fn ntt_acc(a: Vec<Int, 256>) -> Int\n  effects pure\n"
        "{ fold i in 1..256 { bfly(coeff(a, i)) } }",
}
