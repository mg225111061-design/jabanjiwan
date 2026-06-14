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

    /// Transpose.
    pub fn transpose(&self) -> RatMatrix {
        let mut t = RatMatrix::zeros(self.cols, self.rows);
        for i in 0..self.rows {
            for j in 0..self.cols {
                t.set(j, i, self.get(i, j).clone());
            }
        }
        t
    }

    /// Exact inverse over `Q` via Gauss–Jordan on `[A | I]`. `None` if singular.
    /// This is the naive-correct reference for Sherman–Morrison / Woodbury (AR-4).
    pub fn inverse(&self) -> Option<RatMatrix> {
        assert_eq!(self.rows, self.cols, "inverse needs a square matrix");
        let n = self.rows;
        let mut a = self.data.clone();
        let mut inv = RatMatrix::identity(n).data;
        let idx = |i: usize, j: usize| i * n + j;
        for col in 0..n {
            let pivot = (col..n).find(|&r| !a[idx(r, col)].is_zero())?;
            if pivot != col {
                for k in 0..n {
                    a.swap(idx(col, k), idx(pivot, k));
                    inv.swap(idx(col, k), idx(pivot, k));
                }
            }
            let p = a[idx(col, col)].clone();
            for k in 0..n {
                a[idx(col, k)] /= &p;
                inv[idx(col, k)] /= &p;
            }
            for r in 0..n {
                if r == col {
                    continue;
                }
                let factor = a[idx(r, col)].clone();
                if factor.is_zero() {
                    continue;
                }
                for k in 0..n {
                    let sa = &factor * &a[idx(col, k)];
                    a[idx(r, k)] -= sa;
                    let si = &factor * &inv[idx(col, k)];
                    inv[idx(r, k)] -= si;
                }
            }
        }
        Some(RatMatrix {
            rows: n,
            cols: n,
            data: inv,
        })
    }

    /// Woodbury identity (exact, ℚ): given `A`, its inverse `a_inv = A⁻¹`, and a
    /// rank-k update `U C V` (`U: n×k`, `C: k×k`, `V: k×n`), return `(A + U C V)⁻¹`
    /// in O(n²k + k³) instead of re-inverting in O(n³):
    ///
    /// `(A+UCV)⁻¹ = A⁻¹ − A⁻¹U (C⁻¹ + V A⁻¹ U)⁻¹ V A⁻¹`.
    ///
    /// Sherman–Morrison is the `k=1`, `C=[1]` special case. Returns `None` if the
    /// inner `k×k` system or `C` is singular.
    pub fn woodbury(
        a_inv: &RatMatrix,
        u: &RatMatrix,
        c: &RatMatrix,
        v: &RatMatrix,
    ) -> Option<RatMatrix> {
        let ainv_u = a_inv.mul(u); // n×k
        let v_ainv = v.mul(a_inv); // k×n
        let c_inv = c.inverse()?;
        let inner = c_inv.add(&v.mul(&ainv_u)); // k×k
        let inner_inv = inner.inverse()?;
        let correction = ainv_u.mul(&inner_inv).mul(&v_ainv); // n×n
        Some(a_inv.sub(&correction))
    }

    pub fn sub(&self, o: &RatMatrix) -> RatMatrix {
        assert_eq!((self.rows, self.cols), (o.rows, o.cols));
        RatMatrix {
            rows: self.rows,
            cols: self.cols,
            data: self.data.iter().zip(&o.data).map(|(a, b)| a - b).collect(),
        }
    }

    /// Sqrt-free Cholesky (LDLᵀ) over `Q`: `A = L D Lᵀ` with `L` unit-lower-triangular
    /// and `D` diagonal. Exact (no irrational square roots). Returns `(L, D)`, or
    /// `None` if a pivot is zero (not factorable). `A` is SPD **iff** every `D[j] > 0`
    /// — the caller refuses non-SPD rather than taking a negative square root.
    pub fn ldlt(&self) -> Option<(RatMatrix, Vec<BigRational>)> {
        assert_eq!(self.rows, self.cols, "ldlt needs a square matrix");
        let n = self.rows;
        let mut l = RatMatrix::identity(n);
        let mut d = vec![BigRational::zero(); n];
        for j in 0..n {
            let mut dj = self.get(j, j).clone();
            for (k, dk) in d.iter().enumerate().take(j) {
                dj -= l.get(j, k) * l.get(j, k) * dk;
            }
            if dj.is_zero() {
                return None;
            }
            d[j] = dj.clone();
            for i in (j + 1)..n {
                let mut s = self.get(i, j).clone();
                for (k, dk) in d.iter().enumerate().take(j) {
                    s -= l.get(i, k) * l.get(j, k) * dk;
                }
                l.set(i, j, s / &dj);
            }
        }
        Some((l, d))
    }

    /// Reconstruct `L · diag(d) · Lᵀ` (for checking an LDLᵀ certificate exactly).
    pub fn from_ldlt(l: &RatMatrix, d: &[BigRational]) -> RatMatrix {
        let n = l.rows;
        let mut dm = RatMatrix::zeros(n, n);
        for (i, di) in d.iter().enumerate() {
            dm.set(i, i, di.clone());
        }
        l.mul(&dm).mul(&l.transpose())
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

/// Square integer matrix, for Strassen multiplication + Freivalds verification.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IntMatrix {
    pub n: usize,
    pub data: Vec<BigInt>, // row-major, n×n
}

impl IntMatrix {
    pub fn zeros(n: usize) -> Self {
        IntMatrix {
            n,
            data: vec![BigInt::from(0); n * n],
        }
    }
    pub fn from_i64(n: usize, vals: &[i64]) -> Self {
        assert_eq!(vals.len(), n * n);
        IntMatrix {
            n,
            data: vals.iter().map(|&v| BigInt::from(v)).collect(),
        }
    }
    pub fn get(&self, i: usize, j: usize) -> &BigInt {
        &self.data[i * self.n + j]
    }

    /// O(n³) reference multiplication (the naive-correct oracle, AR-4).
    pub fn naive_mul(&self, b: &IntMatrix) -> IntMatrix {
        assert_eq!(self.n, b.n);
        let n = self.n;
        let mut c = IntMatrix::zeros(n);
        for i in 0..n {
            for k in 0..n {
                let a = &self.data[i * n + k];
                if a == &BigInt::from(0) {
                    continue;
                }
                for j in 0..n {
                    c.data[i * n + j] += a * &b.data[k * n + j];
                }
            }
        }
        c
    }

    fn block_add(a: &[BigInt], b: &[BigInt]) -> Vec<BigInt> {
        a.iter().zip(b).map(|(x, y)| x + y).collect()
    }
    fn block_sub(a: &[BigInt], b: &[BigInt]) -> Vec<BigInt> {
        a.iter().zip(b).map(|(x, y)| x - y).collect()
    }

    /// Strassen multiplication on a power-of-two size; falls back to naive at small
    /// blocks. Pads non-power-of-two inputs. Exact over ℤ.
    pub fn strassen_mul(&self, b: &IntMatrix) -> IntMatrix {
        assert_eq!(self.n, b.n);
        let n = self.n;
        let m = n.next_power_of_two();
        let ap = self.padded(m);
        let bp = b.padded(m);
        let cp = strassen_flat(&ap, &bp, m);
        // crop back to n×n
        let mut c = IntMatrix::zeros(n);
        for i in 0..n {
            for j in 0..n {
                c.data[i * n + j] = cp[i * m + j].clone();
            }
        }
        c
    }

    fn padded(&self, m: usize) -> Vec<BigInt> {
        let n = self.n;
        let mut out = vec![BigInt::from(0); m * m];
        for i in 0..n {
            for j in 0..n {
                out[i * m + j] = self.data[i * n + j].clone();
            }
        }
        out
    }

    /// Exact Freivalds verification that `c == self·b`: for each recorded {0,1}
    /// vector `x`, check `C·x = A·(B·x)` over ℤ. For a wrong product the probability
    /// a single random `x` passes is ≤ 1/2 (Freivalds' lemma, holds over any integral
    /// domain), so `r` rounds bound the false-accept probability by `2^-r`. Exact
    /// integers ⇒ no float round-off and no modular collision. Deterministic replay
    /// (R11): the verifier uses the *recorded* vectors.
    pub fn freivalds(&self, b: &IntMatrix, c: &IntMatrix, seeds: &[Vec<u8>]) -> bool {
        let n = self.n;
        if b.n != n || c.n != n {
            return false;
        }
        for x in seeds {
            if x.len() != n {
                return false;
            }
            // bx = B·x
            let bx = matvec01(&b.data, n, x);
            // abx = A·bx
            let abx = matvec(&self.data, n, &bx);
            // cx = C·x
            let cx = matvec01(&c.data, n, x);
            if abx != cx {
                return false;
            }
        }
        true
    }
}

/// Deterministic pseudorandom {0,1} vectors (splitmix64), recorded in the cert so
/// verification is reproducible (R11). Used to verify the compiler's own (non-
/// adversarial) product; the 2^-r bound assumes randomness, which these approximate.
pub fn freivalds_seeds(n: usize, rounds: usize, salt: u64) -> Vec<Vec<u8>> {
    let mut state = salt ^ 0x9E37_79B9_7F4A_7C15;
    let mut next = || {
        state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    };
    (0..rounds)
        .map(|_| (0..n).map(|_| (next() & 1) as u8).collect())
        .collect()
}

fn matvec(m: &[BigInt], n: usize, x: &[BigInt]) -> Vec<BigInt> {
    let mut y = vec![BigInt::from(0); n];
    for i in 0..n {
        let mut s = BigInt::from(0);
        for j in 0..n {
            if x[j] != BigInt::from(0) {
                s += &m[i * n + j] * &x[j];
            }
        }
        y[i] = s;
    }
    y
}

fn matvec01(m: &[BigInt], n: usize, x: &[u8]) -> Vec<BigInt> {
    let mut y = vec![BigInt::from(0); n];
    for i in 0..n {
        let mut s = BigInt::from(0);
        for j in 0..n {
            if x[j] != 0 {
                s += &m[i * n + j];
            }
        }
        y[i] = s;
    }
    y
}

/// Strassen on a flat power-of-two `m×m` matrix.
fn strassen_flat(a: &[BigInt], b: &[BigInt], m: usize) -> Vec<BigInt> {
    if m <= 64 {
        // naive base case
        let mut c = vec![BigInt::from(0); m * m];
        for i in 0..m {
            for k in 0..m {
                let av = &a[i * m + k];
                if av == &BigInt::from(0) {
                    continue;
                }
                for j in 0..m {
                    c[i * m + j] += av * &b[k * m + j];
                }
            }
        }
        return c;
    }
    let h = m / 2;
    let quad = |src: &[BigInt], r0: usize, c0: usize| -> Vec<BigInt> {
        let mut q = vec![BigInt::from(0); h * h];
        for i in 0..h {
            for j in 0..h {
                q[i * h + j] = src[(r0 + i) * m + (c0 + j)].clone();
            }
        }
        q
    };
    let (a11, a12, a21, a22) = (quad(a, 0, 0), quad(a, 0, h), quad(a, h, 0), quad(a, h, h));
    let (b11, b12, b21, b22) = (quad(b, 0, 0), quad(b, 0, h), quad(b, h, 0), quad(b, h, h));
    let add = IntMatrix::block_add;
    let sub = IntMatrix::block_sub;
    let m1 = strassen_flat(&add(&a11, &a22), &add(&b11, &b22), h);
    let m2 = strassen_flat(&add(&a21, &a22), &b11, h);
    let m3 = strassen_flat(&a11, &sub(&b12, &b22), h);
    let m4 = strassen_flat(&a22, &sub(&b21, &b11), h);
    let m5 = strassen_flat(&add(&a11, &a12), &b22, h);
    let m6 = strassen_flat(&sub(&a21, &a11), &add(&b11, &b12), h);
    let m7 = strassen_flat(&sub(&a12, &a22), &add(&b21, &b22), h);
    let c11 = add(&sub(&add(&m1, &m4), &m5), &m7);
    let c12 = add(&m3, &m5);
    let c21 = add(&m2, &m4);
    let c22 = add(&add(&sub(&m1, &m2), &m3), &m6);
    let mut c = vec![BigInt::from(0); m * m];
    for i in 0..h {
        for j in 0..h {
            c[i * m + j] = c11[i * h + j].clone();
            c[i * m + (j + h)] = c12[i * h + j].clone();
            c[(i + h) * m + j] = c21[i * h + j].clone();
            c[(i + h) * m + (j + h)] = c22[i * h + j].clone();
        }
    }
    c
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
    fn inverse_times_matrix_is_identity() {
        let a = RatMatrix::from_i64(2, 2, &[4, 3, 6, 3]);
        let inv = a.inverse().unwrap();
        assert_eq!(a.mul(&inv), RatMatrix::identity(2));
        // singular matrix has no inverse
        assert!(RatMatrix::from_i64(2, 2, &[1, 2, 2, 4]).inverse().is_none());
    }

    #[test]
    fn woodbury_matches_direct_inverse() {
        // A + u vᵀ, rank-1 update. Woodbury result must equal the direct inverse.
        let a = RatMatrix::from_i64(3, 3, &[5, 0, 0, 0, 4, 0, 0, 0, 3]);
        let a_inv = a.inverse().unwrap();
        let u = RatMatrix::from_i64(3, 1, &[1, 1, 1]);
        let c = RatMatrix::from_i64(1, 1, &[1]);
        let v = RatMatrix::from_i64(1, 3, &[1, 2, 3]);
        let updated = RatMatrix::woodbury(&a_inv, &u, &c, &v).unwrap();
        // (A + u c vᵀ) explicitly:
        let aucv = a.add(&u.mul(&c).mul(&v));
        // cert: (A+UCV)·X̂ = I  (exact)
        assert_eq!(aucv.mul(&updated), RatMatrix::identity(3));
        // and it equals the direct inverse
        assert_eq!(updated, aucv.inverse().unwrap());
    }

    #[test]
    fn ldlt_reconstructs_and_detects_spd() {
        // SPD matrix: [[4,2],[2,3]] → all D>0, reconstruction exact.
        let a = RatMatrix::from_i64(2, 2, &[4, 2, 2, 3]);
        let (l, d) = a.ldlt().unwrap();
        assert!(d.iter().all(|x| *x > BigRational::zero()), "SPD ⇒ D>0");
        assert_eq!(RatMatrix::from_ldlt(&l, &d), a);
        // non-SPD (indefinite) matrix: [[1,2],[2,1]] has a negative pivot.
        let b = RatMatrix::from_i64(2, 2, &[1, 2, 2, 1]);
        let (_, d2) = b.ldlt().unwrap();
        assert!(d2.iter().any(|x| *x < BigRational::zero()), "indefinite ⇒ some D<0");
    }

    #[test]
    fn strassen_matches_naive() {
        // larger than the base case to exercise recursion
        let n = 70;
        let mk = |seed: i64| {
            let data: Vec<i64> = (0..n * n).map(|i| ((i as i64 * 7 + seed) % 11) - 5).collect();
            IntMatrix::from_i64(n, &data)
        };
        let a = mk(1);
        let b = mk(2);
        assert_eq!(a.strassen_mul(&b), a.naive_mul(&b));
    }

    #[test]
    fn freivalds_accepts_correct_rejects_wrong() {
        let a = IntMatrix::from_i64(3, &[1, 2, 3, 4, 5, 6, 7, 8, 9]);
        let b = IntMatrix::from_i64(3, &[9, 8, 7, 6, 5, 4, 3, 2, 1]);
        let c = a.naive_mul(&b);
        let seeds = freivalds_seeds(3, 30, 12345);
        assert!(a.freivalds(&b, &c, &seeds), "correct product must pass");
        // tamper one entry: must be rejected with overwhelming probability.
        let mut wrong = c.clone();
        wrong.data[0] += BigInt::from(1);
        assert!(!a.freivalds(&b, &wrong, &seeds), "wrong product must be rejected");
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
