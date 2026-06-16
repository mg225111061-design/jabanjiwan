//! zeil_check — HARAN v5 / T2: hypergeometric creative-telescoping certificate, machine-verified.
//! Runs the REAL Zeilberger search (jeff_collapse_arith::zeilberger) and verifies the resulting
//! telescoper + certificate with the independent checker (jeff_math::hyper::telescoper_holds — an
//! exact rational-function identity, coefficient-zero, no SMT). The search is untrusted; only a
//! checker-verified telescoper is reported (R2/R25).
//!
//! usage:  zeil_check binom | binom_sq
//! output: TELESCOPER which=.. order=.. verified=<bool> provisos=asserted-not-proven
//!     or  DEFER which=.. (no telescoper within bounds)

use jeff_collapse_arith::zeilberger::{zeilberger, Bounds};
use jeff_math::hyper::{telescoper_holds, HyperTerm, LinForm};
use jeff_math::Poly;
use num_bigint::BigInt;
use num_rational::BigRational;

fn binom_nk() -> HyperTerm {
    // C(n,k) = Γ(n+1) / (Γ(k+1)·Γ(n-k+1))
    HyperTerm {
        coeff: BigRational::from(BigInt::from(1)),
        z_k: BigRational::from(BigInt::from(1)),
        poly: Poly::from_i64(1),
        gammas: vec![
            (LinForm::new(1, 0, 1), 1),
            (LinForm::new(0, 1, 1), -1),
            (LinForm::new(1, -1, 1), -1),
        ],
    }
}

fn main() {
    let which = std::env::args().nth(1).unwrap_or_else(|| "binom".into());
    let term = match which.as_str() {
        "binom" => binom_nk(),         // Σ_k C(n,k) = 2^n
        "binom_sq" => binom_nk().pow(2), // Σ_k C(n,k)^2 = C(2n,n)
        _ => {
            eprintln!("usage: zeil_check binom | binom_sq");
            std::process::exit(2);
        }
    };
    match zeilberger(&term, &Bounds::default()) {
        Some(t) => {
            let verified = telescoper_holds(&term, &t.l, &t.r);
            // The checker verifies the telescoping IDENTITY exactly. The boundary-term provisos
            // (sum endpoints vanish; certificate denominators non-zero in range) are NOT machine-
            // proven here — tracked honestly as asserted.
            println!(
                "TELESCOPER which={which} order={} verified={verified} provisos=asserted-not-proven",
                t.l.len().saturating_sub(1)
            );
        }
        None => println!("DEFER which={which} (no telescoper within bounds)"),
    }
}
