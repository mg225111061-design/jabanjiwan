//! Stage-6A Batch-6: property testing / Fourier learning (CLAUDE.md PART B Batch 6).
//!
//! These are mostly CHEAP DETECTORS — precondition checkers the recognizer uses to
//! decide whether a structural assumption (heavy Fourier mass, low degree, linearity, a
//! small junta) holds before dispatching a heavier collapser. On a Boolean function they
//! compute the Walsh–Hadamard spectrum and read off the property. The exception is
//! list decoding (6.5), an exact recovery with an exact agreement certificate.

#![allow(clippy::needless_range_loop)]

use crate::modular::ModInt;

// ---- Walsh–Hadamard transform (shared) ----

/// Fourier coefficients `f̂(S)` of a Boolean function given as a truth table of `±1`
/// values, length `2^num_vars`. Index `S` is the character bitmask. Exact.
pub fn walsh_hadamard(f: &[f64]) -> Vec<f64> {
    let n = f.len();
    let mut a = f.to_vec();
    let mut h = 1;
    while h < n {
        let mut i = 0;
        while i < n {
            for j in i..i + h {
                let (x, y) = (a[j], a[j + h]);
                a[j] = x + y;
                a[j + h] = x - y;
            }
            i += 2 * h;
        }
        h *= 2;
    }
    a.iter().map(|v| v / n as f64).collect()
}

// ---- 6.1 Goldreich–Levin: heavy Fourier coefficients ----

/// Characters `S` with `|f̂(S)| ≥ theta` (heavy coefficients).
pub fn heavy_coefficients(fhat: &[f64], theta: f64) -> Vec<usize> {
    (0..fhat.len()).filter(|&s| fhat[s].abs() >= theta).collect()
}

// ---- 6.2 Linial–Mansour–Nisan: low-degree tail ----

/// High-degree Fourier mass `Σ_{|S|>k} f̂(S)²` (= `‖f − g_{≤k}‖₂²`). Low ⇒ a degree-`k`
/// approximation is adequate; for PARITY (mass at degree n) it is ≈ 1.
pub fn low_degree_tail(fhat: &[f64], k: usize) -> f64 {
    fhat.iter()
        .enumerate()
        .filter(|(s, _)| (s.count_ones() as usize) > k)
        .map(|(_, &c)| c * c)
        .sum()
}

// ---- 6.3 BLR: distance to the nearest linear function ----

/// Distance to the nearest character `min_S Pr[f ≠ ±χ_S] = (1 − max_S |f̂(S)|)/2`.
pub fn distance_to_linear(fhat: &[f64]) -> f64 {
    let max = fhat.iter().fold(0.0_f64, |m, &c| m.max(c.abs()));
    (1.0 - max) / 2.0
}

// ---- 6.4 Junta / influence testing ----

/// Coordinate influences `Inf_i(f) = Σ_{S ∋ i} f̂(S)²`.
pub fn influences(fhat: &[f64], num_vars: usize) -> Vec<f64> {
    let mut inf = vec![0.0; num_vars];
    for (s, &c) in fhat.iter().enumerate() {
        let c2 = c * c;
        for (i, slot) in inf.iter_mut().enumerate() {
            if s & (1 << i) != 0 {
                *slot += c2;
            }
        }
    }
    inf
}

/// Relevant coordinates (influence above a floor) — a small set ⇒ a junta.
pub fn relevant_variables(fhat: &[f64], num_vars: usize, floor: f64) -> Vec<usize> {
    influences(fhat, num_vars)
        .into_iter()
        .enumerate()
        .filter(|(_, v)| *v > floor)
        .map(|(i, _)| i)
        .collect()
}

// ---- 6.6 Noise sensitivity ----

/// Noise sensitivity `NS_ρ(f) = ½(1 − Σ_S ρ^{|S|} f̂(S)²)`. Low ⇒ degree concentration.
pub fn noise_sensitivity(fhat: &[f64], rho: f64) -> f64 {
    let s: f64 = fhat
        .iter()
        .enumerate()
        .map(|(s, &c)| rho.powi(s.count_ones() as i32) * c * c)
        .sum();
    0.5 * (1.0 - s)
}

/// Exact Fourier degree `max{ |S| : f̂(S) ≠ 0 }` — a witness for the sensitivity/degree
/// floor (BBCMdW): if it is `num_vars` (e.g. PARITY), no low-degree representation exists
/// and Ω(N) queries are necessary, so the recognizer emits `SensitivityDegreeFloor`
/// rather than attempting a sublinear collapse.
pub fn exact_degree(fhat: &[f64]) -> usize {
    fhat.iter()
        .enumerate()
        .filter(|(_, &c)| c.abs() > 1e-12)
        .map(|(s, _)| s.count_ones() as usize)
        .max()
        .unwrap_or(0)
}

// ---- 6.5 Reed–Solomon list/unique decoding (Berlekamp–Welch), exact ----

/// Solve a dense linear system `A z = b` over `GF(q)` (q prime) by Gaussian elimination.
fn solve_mod(mut a: Vec<Vec<u64>>, mut b: Vec<u64>, q: u64) -> Option<Vec<u64>> {
    let n = b.len();
    for col in 0..n {
        let piv = (col..n).find(|&r| !a[r][col].is_multiple_of(q))?;
        a.swap(col, piv);
        b.swap(col, piv);
        let inv = ModInt::new(a[col][col], q).inv()?;
        for c in col..n {
            a[col][c] = (ModInt::new(a[col][c], q) * inv).val;
        }
        b[col] = (ModInt::new(b[col], q) * inv).val;
        for r in 0..n {
            if r == col || a[r][col] == 0 {
                continue;
            }
            let f = ModInt::new(a[r][col], q);
            for c in col..n {
                a[r][c] = (ModInt::new(a[r][c], q) - f * ModInt::new(a[col][c], q)).val;
            }
            b[r] = (ModInt::new(b[r], q) - f * ModInt::new(b[col], q)).val;
        }
    }
    Some(b)
}

/// Evaluate a polynomial (coeffs low→high) at `x` over `GF(q)`.
pub fn poly_eval_mod(coeffs: &[u64], x: u64, q: u64) -> u64 {
    let mut acc = ModInt::zero(q);
    for &c in coeffs.iter().rev() {
        acc = acc * ModInt::new(x, q) + ModInt::new(c, q);
    }
    acc.val
}

/// Berlekamp–Welch decoding of a Reed–Solomon codeword: given evaluation points `xs`
/// and received values `ys` over `GF(q)`, find the message polynomial of degree `< k`
/// that agrees on ≥ `n − ⌊(n−k)/2⌋` points (unique decoding). Returns its coefficients
/// (low→high, length `k`), or `None`. The certificate is the **exact agreement count**.
pub fn rs_decode(xs: &[u64], ys: &[u64], k: usize, q: u64) -> Option<Vec<u64>> {
    let n = xs.len();
    if n != ys.len() || k == 0 || k > n {
        return None;
    }
    let tau = (n - k) / 2; // correctable errors
    if tau == 0 {
        // no errors to correct: interpolation must fit exactly; fall through anyway.
    }
    // Unknowns: N_0..N_{k+tau-1} (deg N < k+tau), e_0..e_{tau-1} (E = e(x) + x^tau monic).
    // Equation per point i: Σ_j N_j x_i^j − y_i Σ_j e_j x_i^j = y_i x_i^tau.
    let n_len = k + tau; // # of N coefficients
    let unknowns = n_len + tau; // N coeffs + e coeffs
    if unknowns != n {
        return None; // requires n = k + 2·tau
    }
    let mut a = vec![vec![0u64; unknowns]; n];
    let mut b = vec![0u64; n];
    for i in 0..n {
        let mut xp = 1u64; // x_i^j
        for j in 0..n_len {
            a[i][j] = xp;
            xp = (ModInt::new(xp, q) * ModInt::new(xs[i], q)).val;
        }
        // −y_i x_i^j for e_j
        let mut xq = 1u64;
        for j in 0..tau {
            a[i][n_len + j] = (ModInt::zero(q) - ModInt::new(ys[i], q) * ModInt::new(xq, q)).val;
            xq = (ModInt::new(xq, q) * ModInt::new(xs[i], q)).val;
        }
        // RHS y_i x_i^tau
        b[i] = (ModInt::new(ys[i], q) * ModInt::new(xq, q)).val;
    }
    let sol = solve_mod(a, b, q)?;
    let n_coeffs = &sol[0..n_len];
    let mut e_coeffs: Vec<u64> = sol[n_len..].to_vec();
    e_coeffs.push(1); // monic x^tau
    // p = N / E by polynomial long division over GF(q); require zero remainder.
    let (quot, rem) = poly_divmod_mod(n_coeffs, &e_coeffs, q);
    if rem.iter().any(|&c| !c.is_multiple_of(q)) {
        return None;
    }
    let mut p = quot;
    p.resize(k, 0);
    Some(p)
}

/// Polynomial division `num / den` over `GF(q)` → `(quotient, remainder)` (low→high).
fn poly_divmod_mod(num: &[u64], den: &[u64], q: u64) -> (Vec<u64>, Vec<u64>) {
    let mut r: Vec<u64> = num.to_vec();
    // strip leading zeros of den
    let mut dd = den.len();
    while dd > 1 && den[dd - 1].is_multiple_of(q) {
        dd -= 1;
    }
    let dlead_inv = ModInt::new(den[dd - 1], q).inv().unwrap();
    let mut quot = vec![0u64; num.len().saturating_sub(dd) + 1];
    let mut rlen = r.len();
    while rlen >= dd {
        let coeff = (ModInt::new(r[rlen - 1], q) * dlead_inv).val;
        let shift = rlen - dd;
        quot[shift] = coeff;
        for j in 0..dd {
            r[shift + j] =
                (ModInt::new(r[shift + j], q) - ModInt::new(coeff, q) * ModInt::new(den[j], q)).val;
        }
        while rlen > 0 && r[rlen - 1].is_multiple_of(q) {
            rlen -= 1;
        }
        if rlen == 0 {
            break;
        }
    }
    r.truncate(rlen.max(1));
    (quot, r)
}

/// Hamming agreement count of a polynomial with a received word (the exact certificate).
pub fn agreement_count(coeffs: &[u64], xs: &[u64], ys: &[u64], q: u64) -> usize {
    xs.iter()
        .zip(ys)
        .filter(|(&x, &y)| poly_eval_mod(coeffs, x, q) == y % q)
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;

    // f(x0,x1) = x0 XOR x1 as ±1: parity, all mass at degree 2.
    fn parity2() -> Vec<f64> {
        // truth table over (x0,x1) with index = x0 + 2 x1; value (-1)^{x0+x1}
        vec![1.0, -1.0, -1.0, 1.0]
    }
    // f = x0 (a dictator / linear, degree 1)
    fn dictator2() -> Vec<f64> {
        vec![1.0, -1.0, 1.0, -1.0] // (-1)^{x0}
    }

    #[test]
    fn wht_parity_is_single_coefficient() {
        let fhat = walsh_hadamard(&parity2());
        // f̂({0,1}) = ±1, others 0
        assert!((fhat[3].abs() - 1.0).abs() < 1e-12);
        assert!(fhat[0].abs() < 1e-12 && fhat[1].abs() < 1e-12 && fhat[2].abs() < 1e-12);
    }

    #[test]
    fn parity_has_no_low_degree_concentration() {
        let fhat = walsh_hadamard(&parity2());
        // degree-1 truncation misses all the mass.
        assert!(low_degree_tail(&fhat, 1) > 0.99);
        // a dictator IS degree-1: tail ≈ 0.
        let dhat = walsh_hadamard(&dictator2());
        assert!(low_degree_tail(&dhat, 1) < 1e-9);
    }

    #[test]
    fn blr_distance_dictator_is_linear() {
        assert!(distance_to_linear(&walsh_hadamard(&dictator2())) < 1e-12);
        // majority-ish / constant-bias function is farther from linear
        let f = vec![1.0, 1.0, 1.0, -1.0]; // AND-like
        assert!(distance_to_linear(&walsh_hadamard(&f)) > 0.0);
    }

    #[test]
    fn junta_influences() {
        // f = x0 ignores x1 → Inf_0 > 0, Inf_1 = 0.
        let inf = influences(&walsh_hadamard(&dictator2()), 2);
        assert!(inf[0] > 0.5 && inf[1] < 1e-9);
        assert_eq!(relevant_variables(&walsh_hadamard(&dictator2()), 2, 1e-6), vec![0]);
    }

    #[test]
    fn noise_sensitivity_parity_high() {
        // parity is maximally noise sensitive; a dictator much less so.
        let nsp = noise_sensitivity(&walsh_hadamard(&parity2()), 0.5);
        let nsd = noise_sensitivity(&walsh_hadamard(&dictator2()), 0.5);
        assert!(nsp > nsd);
    }

    #[test]
    fn parity_has_full_degree_floor() {
        // deg̃(PARITY) = n: the sensitivity/degree floor witness (BBCMdW).
        assert_eq!(exact_degree(&walsh_hadamard(&parity2())), 2); // n=2 vars
        assert_eq!(exact_degree(&walsh_hadamard(&dictator2())), 1); // a dictator is degree 1
    }

    #[test]
    fn rs_decode_corrects_errors() {
        // RS over GF(97), message p(x)=3+2x+x^2 (k=3), n=7 points, 2 errors.
        let q = 97;
        let p = [3u64, 2, 1];
        let xs: Vec<u64> = (1..=7).collect();
        let mut ys: Vec<u64> = xs.iter().map(|&x| poly_eval_mod(&p, x, q)).collect();
        // n=7,k=3 ⇒ tau=2 correctable; introduce 2 errors
        ys[1] = (ys[1] + 5) % q;
        ys[4] = (ys[4] + 9) % q;
        let dec = rs_decode(&xs, &ys, 3, q).expect("decode");
        assert_eq!(dec, vec![3, 2, 1]);
        // agreement with the (corrupted) word is ≥ n − tau = 5
        assert!(agreement_count(&dec, &xs, &ys, q) >= 5);
    }

    #[test]
    fn rs_decode_fails_beyond_radius() {
        // too many errors (3 > tau=2) → no degree-<3 polynomial within radius.
        let q = 97;
        let p = [3u64, 2, 1];
        let xs: Vec<u64> = (1..=7).collect();
        let mut ys: Vec<u64> = xs.iter().map(|&x| poly_eval_mod(&p, x, q)).collect();
        ys[0] = (ys[0] + 1) % q;
        ys[2] = (ys[2] + 1) % q;
        ys[5] = (ys[5] + 1) % q;
        // decoder may return None or a wrong poly; either way agreement < n - tau
        if let Some(dec) = rs_decode(&xs, &ys, 3, q) {
            assert!(agreement_count(&dec, &xs, &ys, q) < 5 || dec == vec![3, 2, 1]);
        }
    }
}
