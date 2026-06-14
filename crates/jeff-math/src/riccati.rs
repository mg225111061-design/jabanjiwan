//! Continuous-time algebraic Riccati equation (CARE) for LQR. CLAUDE.md 10.2
//! (`riccati_schur`), APPENDIX I.3.
//!
//! Solves `AᵀX + XA − X B R⁻¹ Bᵀ X + Q = 0` for the **stabilizing, PSD** solution
//! via the matrix-sign function on the Hamiltonian `H = [[A, −G],[−Q, −Aᵀ]]`,
//! `G = B R⁻¹ Bᵀ` (Roberts 1971/1980).
//!
//! The two CARE traps (Tier-A brief), handled by the *certificate*, not the solver:
//!   1. CARE has many solutions; a small residual alone is not enough. The cert also
//!      requires `X` PSD **and** the closed loop `A − GX` Hurwitz (checked by a
//!      Lyapunov test). [`solve_care`] returns a candidate; the checker validates it.
//!   2. If `(A,B)` is not stabilizable / `(A,Q)` not detectable, no stabilizing
//!      solution exists — then the residual/stability checks fail and the collapser
//!      **refuses** (never ships a wrong `X`, P0).

use crate::fmat::{solve_dense, FMat};

/// `G = B R⁻¹ Bᵀ`.
fn gain_g(b: &FMat, r: &FMat) -> Option<FMat> {
    let r_inv = r.inverse()?;
    Some(b.matmul(&r_inv).matmul(&b.transpose()))
}

/// CARE residual `‖AᵀX + XA − X G X + Q‖_F`, `G = B R⁻¹ Bᵀ`.
pub fn care_residual(a: &FMat, b: &FMat, q: &FMat, r: &FMat, x: &FMat) -> Option<f64> {
    let g = gain_g(b, r)?;
    let term = a
        .transpose()
        .matmul(x)
        .add(&x.matmul(a))
        .sub(&x.matmul(&g).matmul(x))
        .add(q);
    Some(term.frob_norm())
}

/// Closed-loop matrix `A − B R⁻¹ Bᵀ X` (Hurwitz iff `X` is stabilizing).
pub fn closed_loop(a: &FMat, b: &FMat, r: &FMat, x: &FMat) -> Option<FMat> {
    let g = gain_g(b, r)?;
    Some(a.sub(&g.matmul(x)))
}

/// PSD test up to `tol`: symmetric and every LDLᵀ pivot `≥ −tol`.
pub fn is_psd(x: &FMat, tol: f64) -> bool {
    if !x.is_symmetric(tol.max(1e-9)) {
        return false;
    }
    // symmetrize for the pivot test
    let n = x.rows;
    let mut s = FMat::zeros(n, n);
    for i in 0..n {
        for j in 0..n {
            s.set(i, j, 0.5 * (x.get(i, j) + x.get(j, i)));
        }
    }
    match s.ldlt_diag() {
        Some(d) => d.iter().all(|&v| v >= -tol),
        None => false, // singular pivot: treat as not-strictly-PSD (conservative)
    }
}

/// Is `m` Hurwitz (all eigenvalues Re < 0)? Tested without an eigensolver via the
/// Lyapunov equation `mᵀP + P m = −I`: `m` is Hurwitz **iff** the unique solution
/// `P` exists and is positive-definite. Solved through the Kronecker form
/// `(I⊗mᵀ + mᵀ⊗I) vec(P) = −vec(I)`.
pub fn is_hurwitz(m: &FMat) -> bool {
    let n = m.rows;
    if n != m.cols {
        return false;
    }
    let mt = m.transpose();
    // Build K = I⊗mᵀ + mᵀ⊗I  (n² × n²), column-major vec.
    let n2 = n * n;
    let mut k = FMat::zeros(n2, n2);
    // vec index: column-major, p = (col)*n + row ; entry P[row][col].
    // (I⊗mᵀ): block-diagonal with mᵀ on each n-block.
    // (mᵀ⊗I): (mᵀ)_{ab} * I in (a,b) block.
    for a in 0..n {
        for b in 0..n {
            let mtab = mt.get(a, b);
            for i in 0..n {
                // (mᵀ⊗I): row (a*n+i), col (b*n+i) += mt[a][b]
                if mtab != 0.0 {
                    let r = a * n + i;
                    let c = b * n + i;
                    k.set(r, c, k.get(r, c) + mtab);
                }
            }
        }
    }
    // (I⊗mᵀ): block diagonal, block c has mᵀ
    for blk in 0..n {
        for i in 0..n {
            for j in 0..n {
                let r = blk * n + i;
                let c = blk * n + j;
                k.set(r, c, k.get(r, c) + mt.get(i, j));
            }
        }
    }
    // rhs = -vec(I)
    let mut rhs = vec![0.0; n2];
    for i in 0..n {
        rhs[i * n + i] = -1.0; // I in column-major: P[i][i] at index i*n+i
    }
    let Some(pvec) = solve_dense(&k, &rhs) else {
        return false; // singular ⇒ eigenvalue on imaginary axis ⇒ not Hurwitz
    };
    // reshape (column-major) into P and test PD
    let mut p = FMat::zeros(n, n);
    for col in 0..n {
        for row in 0..n {
            p.set(row, col, pvec[col * n + row]);
        }
    }
    // P must be positive-definite (symmetrize, then all LDLᵀ pivots > 0).
    let mut s = FMat::zeros(n, n);
    for i in 0..n {
        for j in 0..n {
            s.set(i, j, 0.5 * (p.get(i, j) + p.get(j, i)));
        }
    }
    match s.ldlt_diag() {
        Some(d) => d.iter().all(|&v| v > 1e-9),
        None => false,
    }
}

/// Matrix sign function via Newton iteration `Z ← (Z + Z⁻¹)/2`.
fn matrix_sign(h: &FMat, iters: usize) -> Option<FMat> {
    let mut z = h.clone();
    for _ in 0..iters {
        let zi = z.inverse()?;
        let next = z.add(&zi).scale(0.5);
        // convergence check
        let diff = next.sub(&z).frob_norm();
        z = next;
        if diff < 1e-13 {
            break;
        }
    }
    Some(z)
}

/// Solve the CARE for a stabilizing solution candidate via the matrix-sign method.
/// Returns `None` if the iteration breaks down. The caller MUST validate the result
/// (residual + PSD + Hurwitz) — this routine does not guarantee correctness on its own.
pub fn solve_care(a: &FMat, b: &FMat, q: &FMat, r: &FMat, iters: usize) -> Option<FMat> {
    let n = a.rows;
    let g = gain_g(b, r)?;
    // H = [[A, -G], [-Q, -Aᵀ]]
    let mut h = FMat::zeros(2 * n, 2 * n);
    let at = a.transpose();
    for i in 0..n {
        for j in 0..n {
            h.set(i, j, a.get(i, j));
            h.set(i, j + n, -g.get(i, j));
            h.set(i + n, j, -q.get(i, j));
            h.set(i + n, j + n, -at.get(i, j));
        }
    }
    let w = matrix_sign(&h, iters)?;
    // W blocks
    let block = |r0: usize, c0: usize| -> FMat {
        let mut m = FMat::zeros(n, n);
        for i in 0..n {
            for j in 0..n {
                m.set(i, j, w.get(r0 + i, c0 + j));
            }
        }
        m
    };
    let (w11, w12, w21, w22) = (block(0, 0), block(0, n), block(n, 0), block(n, n));
    let id = FMat::identity(n);
    // M = [I-W11 | -W12], N = [-W21 | I-W22]  (n × 2n)
    // X = (N Mᵀ)(M Mᵀ)⁻¹
    let i_w11 = id.sub(&w11);
    let neg_w12 = w12.scale(-1.0);
    let neg_w21 = w21.scale(-1.0);
    let i_w22 = id.sub(&w22);
    // M Mᵀ = (I-W11)(I-W11)ᵀ + (-W12)(-W12)ᵀ
    let mmt = i_w11
        .matmul(&i_w11.transpose())
        .add(&neg_w12.matmul(&neg_w12.transpose()));
    // N Mᵀ = (-W21)(I-W11)ᵀ + (I-W22)(-W12)ᵀ
    let nmt = neg_w21
        .matmul(&i_w11.transpose())
        .add(&i_w22.matmul(&neg_w12.transpose()));
    let mmt_inv = mmt.inverse()?;
    let x = nmt.matmul(&mmt_inv);
    // symmetrize (the true solution is symmetric)
    let mut xs = FMat::zeros(n, n);
    for i in 0..n {
        for j in 0..n {
            xs.set(i, j, 0.5 * (x.get(i, j) + x.get(j, i)));
        }
    }
    Some(xs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scalar_care_has_x_one() {
        // A=0,B=1,R=1,Q=1 ⇒ X²=1, stabilizing X=1 (A-GX = -1 < 0).
        let a = FMat::from_data(1, 1, vec![0.0]);
        let b = FMat::from_data(1, 1, vec![1.0]);
        let q = FMat::from_data(1, 1, vec![1.0]);
        let r = FMat::from_data(1, 1, vec![1.0]);
        let x = solve_care(&a, &b, &q, &r, 60).unwrap();
        assert!((x.get(0, 0) - 1.0).abs() < 1e-9, "X={}", x.get(0, 0));
        assert!(care_residual(&a, &b, &q, &r, &x).unwrap() < 1e-9);
        assert!(is_psd(&x, 1e-9));
        // closed loop A - G X = -1 is Hurwitz
        let g = gain_g(&b, &r).unwrap();
        let acl = a.sub(&g.matmul(&x));
        assert!(is_hurwitz(&acl));
    }

    #[test]
    fn care_2x2_validated() {
        // A=[[0,1],[0,0]] (double integrator), B=[[0],[1]], Q=I, R=[1].
        let a = FMat::from_data(2, 2, vec![0.0, 1.0, 0.0, 0.0]);
        let b = FMat::from_data(2, 1, vec![0.0, 1.0]);
        let q = FMat::identity(2);
        let r = FMat::from_data(1, 1, vec![1.0]);
        let x = solve_care(&a, &b, &q, &r, 80).unwrap();
        assert!(care_residual(&a, &b, &q, &r, &x).unwrap() < 1e-7, "residual too big");
        assert!(is_psd(&x, 1e-7), "X must be PSD");
        let g = gain_g(&b, &r).unwrap();
        let acl = a.sub(&g.matmul(&x));
        assert!(is_hurwitz(&acl), "closed loop must be Hurwitz");
    }

    #[test]
    fn hurwitz_detects_unstable() {
        assert!(is_hurwitz(&FMat::from_data(1, 1, vec![-2.0])));
        assert!(!is_hurwitz(&FMat::from_data(1, 1, vec![0.5]))); // unstable
        // stable 2x2: [[-1,0],[0,-2]]
        assert!(is_hurwitz(&FMat::from_data(2, 2, vec![-1.0, 0.0, 0.0, -2.0])));
    }
}
