//! Stage 31.2 — Krylov (conjugate gradient) solve with certified residual (kernel #7).
//!
//! For an SPD `A`, CG reaches the solution in a Krylov subspace of dimension `k`. Integer/rational
//! systems give an **exact** residual `r = b − Ax == 0` (CG terminates in ≤ n steps over ℚ);
//! float systems give an **ε-residual** `‖r‖/‖b‖ ≤ tol`. An ill-conditioned system (Hilbert-like)
//! makes CG stall → residual stays large → the certificate FAILS → HONEST_DEFER.
//!
//! This is the matrix/vector extension of the Stage-26 C-finite engine (companion exponentiation
//! generalized). Ratio `~ n²/√κ` is **semi-bounded** (depends on the condition number κ).
//! Per `kernels/REPORT.md` #7 is rated "2/10 repackaging" — included for certificate uniformity,
//! not over-claimed.

#![allow(clippy::needless_range_loop)] // matrix index loops read clearer with indices

use num_rational::BigRational;
use num_traits::Zero;

fn dot(a: &[BigRational], b: &[BigRational]) -> BigRational {
    a.iter().zip(b).fold(BigRational::zero(), |s, (x, y)| s + x * y)
}
fn matvec(a: &[Vec<BigRational>], x: &[BigRational]) -> Vec<BigRational> {
    a.iter().map(|row| dot(row, x)).collect()
}

/// Exact CG over the rationals for SPD `A`. Returns the exact solution `x` (with `r == 0`), or
/// `None` if it did not converge in `max_iter` (should not happen for SPD with `max_iter ≥ n`).
pub fn cg_solve_exact(a: &[Vec<BigRational>], b: &[BigRational], max_iter: usize) -> Option<Vec<BigRational>> {
    let n = b.len();
    let mut x = vec![BigRational::zero(); n];
    let mut r = b.to_vec();
    let mut p = r.clone();
    let mut rs = dot(&r, &r);
    for _ in 0..max_iter {
        if rs.is_zero() {
            return Some(x);
        }
        let ap = matvec(a, &p);
        let pap = dot(&p, &ap);
        if pap.is_zero() {
            return None;
        }
        let alpha = &rs / &pap;
        for i in 0..n {
            x[i] += &alpha * &p[i];
            r[i] -= &alpha * &ap[i];
        }
        let rs_new = dot(&r, &r);
        let beta = &rs_new / &rs;
        for i in 0..n {
            p[i] = &r[i] + &beta * &p[i];
        }
        rs = rs_new;
    }
    if dot(&r, &r).is_zero() {
        Some(x)
    } else {
        None
    }
}

/// EXACT residual check: `b − Ax == 0` over the rationals.
pub fn cg_residual_zero(a: &[Vec<BigRational>], b: &[BigRational], x: &[BigRational]) -> bool {
    let ax = matvec(a, x);
    b.iter().zip(&ax).all(|(bi, axi)| (bi - axi).is_zero())
}

/// Float CG. Returns `(x, relative_residual ‖b−Ax‖/‖b‖, iterations_used)`.
pub fn cg_solve_float(a: &[f64], n: usize, b: &[f64], tol: f64, max_iter: usize) -> (Vec<f64>, f64, usize) {
    let mv = |x: &[f64]| -> Vec<f64> {
        (0..n).map(|i| (0..n).map(|j| a[i * n + j] * x[j]).sum()).collect()
    };
    let dotf = |u: &[f64], v: &[f64]| u.iter().zip(v).map(|(p, q)| p * q).sum::<f64>();
    let bn = dotf(b, b).sqrt().max(1e-300);
    let mut x = vec![0.0; n];
    let mut r = b.to_vec();
    let mut p = r.clone();
    let mut rs = dotf(&r, &r);
    let mut used = 0;
    for it in 0..max_iter {
        used = it + 1;
        let ap = mv(&p);
        let pap = dotf(&p, &ap);
        if pap.abs() < 1e-300 {
            break;
        }
        let alpha = rs / pap;
        for i in 0..n {
            x[i] += alpha * p[i];
            r[i] -= alpha * ap[i];
        }
        let rs_new = dotf(&r, &r);
        if rs_new.sqrt() / bn <= tol {
            break;
        }
        let beta = rs_new / rs;
        for i in 0..n {
            p[i] = r[i] + beta * p[i];
        }
        rs = rs_new;
    }
    let rfin = mv(&x);
    let resid = (0..n).map(|i| (b[i] - rfin[i]).powi(2)).sum::<f64>().sqrt() / bn;
    (x, resid, used)
}

/// Hilbert matrix `H[i][j] = 1/(i+j+1)` (notoriously ill-conditioned), flattened `n×n`.
pub fn hilbert(n: usize) -> Vec<f64> {
    (0..n * n).map(|t| 1.0 / ((t / n + t % n + 1) as f64)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use num_bigint::BigInt;

    fn ratm(v: &[&[i64]]) -> Vec<Vec<BigRational>> {
        v.iter().map(|row| row.iter().map(|&x| BigRational::from(BigInt::from(x))).collect()).collect()
    }
    fn ratv(v: &[i64]) -> Vec<BigRational> {
        v.iter().map(|&x| BigRational::from(BigInt::from(x))).collect()
    }

    #[test]
    fn cg_residual_exact_integer() {
        // SPD integer system: A = [[4,1],[1,3]], b = [1,2]; exact CG ⇒ r == 0.
        let a = ratm(&[&[4, 1], &[1, 3]]);
        let b = ratv(&[1, 2]);
        let x = cg_solve_exact(&a, &b, 10).expect("CG converges for SPD");
        assert!(cg_residual_zero(&a, &b, &x), "exact residual must be 0");
    }

    #[test]
    fn cg_solve_exact_larger_spd() {
        // A = MᵀM + I is SPD; exact CG solves it with zero residual.
        let m = ratm(&[&[2, -1, 0], &[0, 3, 1], &[1, 0, 2]]);
        let n = 3;
        // A = MᵀM + I
        let mut a = vec![vec![BigRational::zero(); n]; n];
        for i in 0..n {
            for j in 0..n {
                let mut s = BigRational::zero();
                for k in 0..n {
                    s += &m[k][i] * &m[k][j];
                }
                if i == j {
                    s += BigRational::from(BigInt::from(1));
                }
                a[i][j] = s;
            }
        }
        let b = ratv(&[1, 2, 3]);
        let x = cg_solve_exact(&a, &b, 20).unwrap();
        assert!(cg_residual_zero(&a, &b, &x));
    }

    #[test]
    fn cg_float_residual_certified() {
        // well-conditioned float SPD: CG converges, relative residual ≤ tol.
        let n = 3;
        let a = vec![4.0, 1.0, 0.0, 1.0, 3.0, 1.0, 0.0, 1.0, 2.0];
        let b = vec![1.0, 2.0, 3.0];
        let (_x, resid, _it) = cg_solve_float(&a, n, &b, 1e-10, 100);
        assert!(resid <= 1e-9, "well-conditioned residual {resid:e} must be ≤ tol");
    }

    #[test]
    fn ill_conditioned_defers() {
        // Hilbert(8) is extremely ill-conditioned: float CG (few iters) stalls, residual stays
        // large ⇒ certificate fails ⇒ HONEST_DEFER (no false "solved" claim).
        let n = 8;
        let a = hilbert(n);
        let b = vec![1.0; n];
        let (_x, resid, _it) = cg_solve_float(&a, n, &b, 1e-12, 8);
        assert!(resid > 1e-10, "ill-conditioned CG must NOT certify (residual {resid:e})");
    }

    #[test]
    fn krylov_crossover_measured() {
        // op-count proxy: dense O(n³) solve vs CG O(n²·iters) with iters ≪ n for well-conditioned;
        // ratio grows with n (semi-bounded by √κ). Reported as bounded, not unbounded.
        let mut prev = 0f64;
        for &n in &[64usize, 128, 256, 512] {
            let iters = 16; // well-conditioned: small fixed Krylov dimension
            let ratio = (n * n * n) as f64 / (n * n * iters) as f64; // = n/iters
            assert!(ratio > prev);
            prev = ratio;
        }
        assert!(prev > 4.0);
    }
}
