//! Dense matrices: rational matrices with an exact characteristic polynomial
//! (Faddeev–LeVerrier) and a Cayley–Hamilton check, plus modular matrix power.
//!
//! Backs the `EigenCharpoly` certificate (APPENDIX F.5): a linear state transition
//! `A^n` collapses to a closed form justified by the characteristic polynomial
//! annihilating `A` (Cayley–Hamilton); the checker re-derives the charpoly and
//! verifies `p(A) = 0` exactly. Also backs `matrix_power_mod` (fixture A15) in
//! O(log n) via square-and-multiply.

use crate::modular::ModInt;
use crate::poly::UniPoly;
use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::{One, Zero};
use serde::{Deserialize, Serialize};
use std::str::FromStr;

fn parse_rational(s: &str) -> BigRational {
    BigRational::from_str(s)
        .unwrap_or_else(|_| BigRational::from(BigInt::from_str(s).expect("bad rational")))
}

/// Square (or rectangular) matrix over `BigRational`, row-major.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(into = "RatMatrixSer", from = "RatMatrixSer")]
pub struct RatMatrix {
    pub rows: usize,
    pub cols: usize,
    pub data: Vec<BigRational>,
}

#[derive(Serialize, Deserialize)]
struct RatMatrixSer {
    rows: usize,
    cols: usize,
    data: Vec<String>,
}

impl From<RatMatrix> for RatMatrixSer {
    fn from(m: RatMatrix) -> Self {
        RatMatrixSer {
            rows: m.rows,
            cols: m.cols,
            data: m.data.iter().map(|c| c.to_string()).collect(),
        }
    }
}

impl From<RatMatrixSer> for RatMatrix {
    fn from(s: RatMatrixSer) -> Self {
        RatMatrix {
            rows: s.rows,
            cols: s.cols,
            data: s.data.iter().map(|c| parse_rational(c)).collect(),
        }
    }
}

impl RatMatrix {
    pub fn zeros(rows: usize, cols: usize) -> Self {
        RatMatrix {
            rows,
            cols,
            data: vec![BigRational::zero(); rows * cols],
        }
    }

    pub fn identity(n: usize) -> Self {
        let mut m = RatMatrix::zeros(n, n);
        for i in 0..n {
            m.set(i, i, BigRational::one());
        }
        m
    }

    pub fn from_i64(rows: usize, cols: usize, vals: &[i64]) -> Self {
        assert_eq!(vals.len(), rows * cols);
        RatMatrix {
            rows,
            cols,
            data: vals
                .iter()
                .map(|&v| BigRational::from(BigInt::from(v)))
                .collect(),
        }
    }

    pub fn get(&self, i: usize, j: usize) -> &BigRational {
        &self.data[i * self.cols + j]
    }
    pub fn set(&mut self, i: usize, j: usize, v: BigRational) {
        self.data[i * self.cols + j] = v;
    }

    pub fn add(&self, o: &RatMatrix) -> RatMatrix {
        assert_eq!((self.rows, self.cols), (o.rows, o.cols));
        RatMatrix {
            rows: self.rows,
            cols: self.cols,
            data: self
                .data
                .iter()
                .zip(&o.data)
                .map(|(a, b)| a + b)
                .collect(),
        }
    }

    pub fn mul(&self, o: &RatMatrix) -> RatMatrix {
        assert_eq!(self.cols, o.rows, "matmul dimension mismatch");
        let mut out = RatMatrix::zeros(self.rows, o.cols);
        for i in 0..self.rows {
            for k in 0..self.cols {
                let a = self.get(i, k).clone();
                if a.is_zero() {
                    continue;
                }
                for j in 0..o.cols {
                    let prod = &a * o.get(k, j);
                    let cur = out.get(i, j).clone();
                    out.set(i, j, cur + prod);
                }
            }
        }
        out
    }

    pub fn scale(&self, k: &BigRational) -> RatMatrix {
        RatMatrix {
            rows: self.rows,
            cols: self.cols,
            data: self.data.iter().map(|x| x * k).collect(),
        }
    }

    pub fn trace(&self) -> BigRational {
        let mut t = BigRational::zero();
        for i in 0..self.rows.min(self.cols) {
            t += self.get(i, i);
        }
        t
    }

    pub fn is_zero(&self) -> bool {
        self.data.iter().all(Zero::is_zero)
    }

    /// Characteristic polynomial `det(xI - A)` via Faddeev–LeVerrier.
    /// Returns the monic polynomial `c_0 + c_1 x + ... + x^n`.
    ///
    /// FL recurrence: M_1 = I, c_{n-1} = -tr(A M_1); for k=2..n:
    ///   M_k = A M_{k-1} + c_{n-k+1} I,  c_{n-k} = -(1/k) tr(A M_k).
    pub fn charpoly(&self) -> UniPoly {
        assert_eq!(self.rows, self.cols, "charpoly needs a square matrix");
        let n = self.rows;
        if n == 0 {
            return UniPoly::constant(BigRational::one());
        }
        // coeffs[i] = coefficient of x^i ; leading (x^n) = 1.
        let mut coeffs = vec![BigRational::zero(); n + 1];
        coeffs[n] = BigRational::one();
        let mut m = RatMatrix::identity(n); // M_1
        for k in 1..=n {
            let am = self.mul(&m);
            let c = -(am.trace()) / BigRational::from(BigInt::from(k as i64));
            coeffs[n - k] = c.clone();
            if k < n {
                // M_{k+1} = A M_k + c I
                let mut next = am;
                for i in 0..n {
                    let cur = next.get(i, i).clone();
                    next.set(i, i, cur + &c);
                }
                m = next;
            }
        }
        UniPoly::from_coeffs(coeffs)
    }

    /// Evaluate a univariate polynomial at this matrix (`p(A)`), with the constant
    /// term scaling the identity. Used for the Cayley–Hamilton check `p(A)=0`.
    pub fn eval_poly(&self, p: &UniPoly) -> RatMatrix {
        assert_eq!(self.rows, self.cols);
        let n = self.rows;
        let mut acc = RatMatrix::zeros(n, n);
        let mut power = RatMatrix::identity(n);
        for c in &p.coeffs {
            if !c.is_zero() {
                acc = acc.add(&power.scale(c));
            }
            power = power.mul(self);
        }
        acc
    }

    /// Cayley–Hamilton verification: returns `true` iff `charpoly(A)` annihilates
    /// `A`. This is the soundness core of the `EigenCharpoly` certificate (F.5).
    pub fn satisfies_charpoly(&self) -> bool {
        let p = self.charpoly();
        self.eval_poly(&p).is_zero()
    }

    /// Exact determinant via Gaussian elimination over `Q` (O(n^3)). Used by the
    /// Pfaffian replay check `det(A) = Pf(A)^2` (APPENDIX E.5).
    pub fn det(&self) -> BigRational {
        assert_eq!(self.rows, self.cols, "det needs a square matrix");
        let n = self.rows;
        let mut a = self.data.clone();
        let idx = |i: usize, j: usize| i * n + j;
        let mut det = BigRational::one();
        for col in 0..n {
            // find pivot
            let mut pivot = None;
            for row in col..n {
                if !a[idx(row, col)].is_zero() {
                    pivot = Some(row);
                    break;
                }
            }
            let Some(p) = pivot else {
                return BigRational::zero();
            };
            if p != col {
                for k in 0..n {
                    a.swap(idx(col, k), idx(p, k));
                }
                det = -det;
            }
            let pivot_val = a[idx(col, col)].clone();
            det *= &pivot_val;
            for row in (col + 1)..n {
                let factor = &a[idx(row, col)] / &pivot_val;
                if factor.is_zero() {
                    continue;
                }
                for k in col..n {
                    let sub = &factor * &a[idx(col, k)];
                    a[idx(row, k)] -= sub;
                }
            }
        }
        det
    }
}

/// Integer matrix mod q, for `matrix_power_mod` (fixture A15) — exact, O(log e).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModMatrix {
    pub n: usize,
    pub q: u64,
    pub data: Vec<u64>,
}

impl ModMatrix {
    pub fn identity(n: usize, q: u64) -> Self {
        let mut m = ModMatrix {
            n,
            q,
            data: vec![0; n * n],
        };
        for i in 0..n {
            m.data[i * n + i] = 1 % q;
        }
        m
    }

    pub fn from_u64(n: usize, q: u64, vals: &[u64]) -> Self {
        assert_eq!(vals.len(), n * n);
        ModMatrix {
            n,
            q,
            data: vals.iter().map(|&v| v % q).collect(),
        }
    }

    pub fn mul(&self, o: &ModMatrix) -> ModMatrix {
        assert_eq!(self.n, o.n);
        assert_eq!(self.q, o.q);
        let n = self.n;
        let mut out = ModMatrix {
            n,
            q: self.q,
            data: vec![0; n * n],
        };
        for i in 0..n {
            for k in 0..n {
                let a = self.data[i * n + k];
                if a == 0 {
                    continue;
                }
                for j in 0..n {
                    let cur = out.data[i * n + j] as u128;
                    let add = a as u128 * o.data[k * n + j] as u128;
                    out.data[i * n + j] = ((cur + add) % self.q as u128) as u64;
                }
            }
        }
        out
    }

    /// `A^e mod q` in O(log e) — the collapse for fixture A15.
    pub fn pow(&self, mut e: u64) -> ModMatrix {
        let mut acc = ModMatrix::identity(self.n, self.q);
        let mut base = self.clone();
        while e > 0 {
            if e & 1 == 1 {
                acc = acc.mul(&base);
            }
            base = base.mul(&base);
            e >>= 1;
        }
        acc
    }

    /// Naive `A^e` by repeated multiplication, for the `NumericResidual` replay
    /// check against the O(log e) result (small e).
    pub fn pow_naive(&self, e: u64) -> ModMatrix {
        let mut acc = ModMatrix::identity(self.n, self.q);
        for _ in 0..e {
            acc = acc.mul(self);
        }
        acc
    }
}

/// Convenience: `[x^n] P/Q mod q` for a linear recurrence given its characteristic
/// polynomial `Q` (Bostan–Mori, APPENDIX E.4). Implemented here over `ModInt`
/// coefficients. `p_coeffs`/`q_coeffs` are ascending-degree, length matching.
pub fn bostan_mori(p_coeffs: &[u64], q_coeffs: &[u64], mut n: u64, q: u64) -> u64 {
    // Work with Vec<ModInt>; P/Q with Q(0) != 0.
    let to_mi = |v: &[u64]| -> Vec<ModInt> { v.iter().map(|&x| ModInt::new(x, q)).collect() };
    let mut p = to_mi(p_coeffs);
    let mut qq = to_mi(q_coeffs);

    let poly_mul = |a: &[ModInt], b: &[ModInt]| -> Vec<ModInt> {
        if a.is_empty() || b.is_empty() {
            return vec![];
        }
        let mut c = vec![ModInt::zero(q); a.len() + b.len() - 1];
        for (i, &x) in a.iter().enumerate() {
            for (j, &y) in b.iter().enumerate() {
                c[i + j] = c[i + j] + x * y;
            }
        }
        c
    };
    let neg_odd = |a: &[ModInt]| -> Vec<ModInt> {
        a.iter()
            .enumerate()
            .map(|(i, &x)| if i % 2 == 1 { ModInt::zero(q) - x } else { x })
            .collect()
    };
    let even_part = |a: &[ModInt]| -> Vec<ModInt> { a.iter().step_by(2).cloned().collect() };
    let odd_part = |a: &[ModInt]| -> Vec<ModInt> { a.iter().skip(1).step_by(2).cloned().collect() };

    while n > 0 {
        let qm = neg_odd(&qq);
        let u = poly_mul(&p, &qm); // numerator
        let v = poly_mul(&qq, &qm); // = V(x^2)
        p = if n.is_multiple_of(2) {
            even_part(&u)
        } else {
            odd_part(&u)
        };
        qq = even_part(&v);
        n /= 2;
    }
    // result = P(0)/Q(0)
    let p0 = p.first().copied().unwrap_or(ModInt::zero(q));
    let q0 = qq.first().copied().unwrap_or(ModInt::one(q));
    (p0 * q0.inv().expect("Q(0) invertible")).val
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn charpoly_satisfied_2x2() {
        // A = [[1,1],[1,0]] (Fibonacci companion). charpoly x^2 - x - 1.
        let a = RatMatrix::from_i64(2, 2, &[1, 1, 1, 0]);
        let p = a.charpoly();
        // coeffs: [-1, -1, 1]
        assert_eq!(p.coeff(0), BigRational::from(BigInt::from(-1)));
        assert_eq!(p.coeff(1), BigRational::from(BigInt::from(-1)));
        assert_eq!(p.coeff(2), BigRational::from(BigInt::from(1)));
        assert!(a.satisfies_charpoly());
    }

    #[test]
    fn matrix_power_mod_matches_naive() {
        // Fibonacci companion ^ 10 mod 1_000_000_007
        let q = 1_000_000_007u64;
        let a = ModMatrix::from_u64(2, q, &[1, 1, 1, 0]);
        let fast = a.pow(10);
        let slow = a.pow_naive(10);
        assert_eq!(fast.data, slow.data);
        // top-left of A^n is F(n+1): A^10 top-left = F(11) = 89
        assert_eq!(fast.data[0], 89);
    }

    #[test]
    fn bostan_mori_fibonacci() {
        // F: a_n = a_{n-1}+a_{n-2}, F0=0,F1=1. GF = x/(1 - x - x^2).
        // P = [0,1], Q = [1,-1,-1] (use modular negatives).
        let q = 1_000_000_007u64;
        let p = [0u64, 1];
        let qq = [1u64, q - 1, q - 1];
        // F(10) = 55
        assert_eq!(bostan_mori(&p, &qq, 10, q), 55);
        // F(20) = 6765
        assert_eq!(bostan_mori(&p, &qq, 20, q), 6765);
    }
}
