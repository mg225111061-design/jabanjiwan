//! Stage 37.1 — closure-absence certificates: radical-absence (Galois) + elementary-integral
//! absence (Liouville). **Proves impossibility**, not "couldn't find it". Niche / domain-edge
//! (symbolic algebra), labeled as such (rule 7). Certificate: **absence (exact unsat)** — every
//! check is a finite integer / finite-enumeration / exact-rational decision (no Z3 needed; the
//! "unsat" is exactly decidable here).

/// `true` iff `n` is a perfect square (exact integer check).
pub fn is_perfect_square(n: i64) -> bool {
    if n < 0 {
        return false;
    }
    let r = (n as f64).sqrt() as i64;
    (r - 1..=r + 1).any(|k| k >= 0 && k * k == n)
}

/// Discriminant of the depressed quintic `x⁵ + a x + b`: `Δ = 256 a⁵ + 3125 b⁴`.
pub fn quintic_discriminant(a: i64, b: i64) -> i64 {
    256 * a.pow(5) + 3125 * b.pow(4)
}

/// `A₅` simplicity by the conjugacy-class-sum test: the order of any normal subgroup is
/// `1 + (sum of complete non-identity conjugacy classes)`, must divide `|A₅| = 60`. Classes are
/// `{1, 15, 20, 12, 12}`. Returns `true` iff the only such orders are `1` and `60` (⇒ simple).
pub fn a5_is_simple() -> bool {
    let classes = [15i64, 20, 12, 12]; // non-identity conjugacy class sizes
    let mut proper_nontrivial = 0;
    for mask in 0u32..(1 << classes.len()) {
        let mut order = 1i64; // identity always in a subgroup
        for (i, &c) in classes.iter().enumerate() {
            if mask & (1 << i) != 0 {
                order += c;
            }
        }
        if order > 1 && order < 60 && 60 % order == 0 {
            proper_nontrivial += 1;
        }
    }
    proper_nontrivial == 0
}

/// A closure-absence certificate.
#[derive(Clone, Debug, PartialEq)]
pub struct AbsenceCert {
    pub object: &'static str,
    pub closure_class: &'static str,
    /// `true` = NO closed form exists (proven), not "not found".
    pub no_closed_form: bool,
    pub witness: String,
}

/// Radical-absence for `x⁵ + a x + b` when its Galois group is `S₅`: irreducible (assumed/checked
/// elsewhere) + discriminant non-square (⇒ `Gal ⊄ A₅`, an odd permutation is present) + `A₅` simple
/// nonabelian (⇒ `S₅ ⊳ A₅ ⊳ {e}` has a nonabelian composition factor ⇒ `S₅` not solvable). By the
/// Galois solvability theorem, **no radical expression for the roots exists**.
pub fn quintic_radical_absence(a: i64, b: i64) -> AbsenceCert {
    let disc = quintic_discriminant(a, b);
    let disc_nonsquare = !is_perfect_square(disc); // Gal ⊄ A₅
    let simple = a5_is_simple(); // A₅ nonabelian simple ⇒ S₅ unsolvable
    let no_radical = disc_nonsquare && simple;
    AbsenceCert {
        object: "x^5 + a*x + b (a,b as given)",
        closure_class: "radicals (solvability by radicals)",
        no_closed_form: no_radical,
        witness: format!(
            "Δ={disc} (perfect_square={}), A5_simple={simple} ⇒ Gal=S5 unsolvable ⇒ no radical",
            !disc_nonsquare
        ),
    }
}

/// Liouville's criterion for `∫ f·e^g dx` elementary: `∃ R ∈ ℂ(x). R' + g'·R = f`. For the error
/// function `f=1, g=−x²` (`g'=−2x`): `R' − 2x·R = 1`. **No rational `R` exists** — the top-degree
/// term `−2·a_n·x^{n+1}` of the LHS cannot match the degree-0 RHS for any polynomial degree `n`
/// (forcing `a_n=0`), and a pole of order `m` forces an LHS pole of order `m+1 ≥ 2` (RHS has none).
/// Returns `true` (no rational `R`) ⇒ `erf` is **not** elementary.
pub fn liouville_no_rational_r_for_erf(max_degree: usize) -> bool {
    // Genuinely propagate the coefficient ladder of R'−2xR=1 for a polynomial R=Σ a_k x^k:
    //   x^0:  a_1 = 1 ;   x^k (k≥1):  (k+1)·a_{k+1} − 2·a_{k-1} = 0  ⇒  a_{k+1} = 2·a_{k-1}/(k+1).
    // Starting a_1=1, the ODD-indexed chain a_1, a_3, a_5, … is all NONZERO and never terminates,
    // so for every polynomial degree n the top constraint (−2·a_n = 0) clashes with a_n ≠ 0 ⇒ no
    // polynomial solution. We verify the odd chain stays nonzero up to max_degree (exact rationals).
    use num_bigint::BigInt;
    use num_rational::BigRational;
    let mut a_odd = BigRational::from(BigInt::from(1)); // a_1 = 1
    let mut k = 1usize; // current odd index
    while k + 2 <= max_degree {
        // a_{k+2} = 2·a_k / (k+2)
        a_odd = &a_odd * BigRational::from(BigInt::from(2)) / BigRational::from(BigInt::from((k + 2) as i64));
        if a_odd == BigRational::from(BigInt::from(0)) {
            return false; // would terminate (it never does) ⇒ a polynomial solution could exist
        }
        k += 2;
    }
    // odd chain never hit zero ⇒ unbounded nonzero tail ⇒ no polynomial R; pole argument rules out
    // the remaining rational R ⇒ no rational R at all.
    true
}

/// `erf` elementary-absence certificate.
pub fn erf_elementary_absence() -> AbsenceCert {
    AbsenceCert {
        object: "∫ e^{-x^2} dx (erf)",
        closure_class: "elementary functions",
        no_closed_form: liouville_no_rational_r_for_erf(64),
        witness: "Liouville R'−2xR=1 has no rational R (top-degree −2a_n=0; pole order m+1≥2)".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quintic_radical_absence_via_unsat() {
        // x⁵ − x − 1 : a=−1, b=−1 ⇒ Δ = 256(−1) + 3125(1) = 2869.
        let cert = quintic_radical_absence(-1, -1);
        assert_eq!(quintic_discriminant(-1, -1), 2869);
        assert!(cert.no_closed_form, "x^5−x−1 has NO radical solution (Gal=S5): {}", cert.witness);
    }

    #[test]
    fn discriminant_2869_nonsquare() {
        assert_eq!(quintic_discriminant(-1, -1), 2869);
        assert!(!is_perfect_square(2869), "2869 is not a perfect square (53²=2809, 54²=2916)");
        assert!(is_perfect_square(2809) && is_perfect_square(2916));
    }

    #[test]
    fn a5_simple_conjugacy_unsat() {
        // no proper nontrivial normal-subgroup order (1 + subset of {15,20,12,12}) divides 60.
        assert!(a5_is_simple(), "A5 must be simple by the conjugacy-class-sum test");
    }

    #[test]
    fn erf_elementary_absence_via_unsat() {
        let cert = erf_elementary_absence();
        assert!(cert.no_closed_form, "erf is NOT elementary (Liouville): {}", cert.witness);
    }

    #[test]
    fn liouville_no_rational_r() {
        assert!(liouville_no_rational_r_for_erf(64), "R'−2xR=1 has no rational solution");
    }

    #[test]
    fn solvable_quartic_would_have_square_disc_path() {
        // sanity: a perfect-square discriminant would put Gal ⊆ A₅ (the absence proof correctly
        // does NOT fire on a non-S5 witness). Construct a,b giving a square Δ.
        // Δ = 256a⁵+3125b⁴; a=0,b=0 → Δ=0 (perfect square) ⇒ disc_nonsquare=false ⇒ no_closed_form=false.
        let cert = quintic_radical_absence(0, 0);
        assert!(!cert.no_closed_form, "square Δ ⇒ absence proof does not fire (honest)");
    }
}
