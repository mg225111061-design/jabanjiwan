//! Minimal complex arithmetic + a complex linear solver + Durand–Kerner root finding.
//!
//! Used by spectral kernels that genuinely need the complex plane: super-resolution
//! (spike locations are roots on the unit circle, Batch 1.5), and available for later
//! spectral stages. Kept tiny and dependency-free (R39: own core data structures).
//! These live on the *kernel* side; certificate checkers re-derive residuals by direct
//! evaluation, never by trusting an iterative solver's convergence.

// Index-based loops read more clearly than iterators for these
// matrix / recurrence kernels; allow it module-wide (numeric code).
#![allow(clippy::needless_range_loop)]

/// A complex number.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Complex {
    pub re: f64,
    pub im: f64,
}

impl Complex {
    pub fn new(re: f64, im: f64) -> Self {
        Complex { re, im }
    }
    pub fn zero() -> Self {
        Complex::new(0.0, 0.0)
    }
    pub fn one() -> Self {
        Complex::new(1.0, 0.0)
    }
    /// e^{iθ}.
    pub fn from_angle(theta: f64) -> Self {
        Complex::new(theta.cos(), theta.sin())
    }
    pub fn conj(self) -> Self {
        Complex::new(self.re, -self.im)
    }
    pub fn abs(self) -> f64 {
        self.re.hypot(self.im)
    }
    pub fn abs2(self) -> f64 {
        self.re * self.re + self.im * self.im
    }
    pub fn arg(self) -> f64 {
        self.im.atan2(self.re)
    }
    /// Scalar multiply by a real.
    pub fn scale(self, s: f64) -> Complex {
        Complex::new(self.re * s, self.im * s)
    }
}

// Inherent arithmetic (not `std::ops`): the kernels chain these as methods
// (`a.mul(b).add(c)`), which reads clearly for complex math and — crucially — avoids
// rewriting ~30 expressions into operator form where a precedence slip would be a P0
// correctness bug. The lint that flags add/sub/mul/div names is suppressed here only.
#[allow(clippy::should_implement_trait)]
impl Complex {
    pub fn add(self, o: Complex) -> Complex {
        Complex::new(self.re + o.re, self.im + o.im)
    }
    pub fn sub(self, o: Complex) -> Complex {
        Complex::new(self.re - o.re, self.im - o.im)
    }
    pub fn mul(self, o: Complex) -> Complex {
        Complex::new(self.re * o.re - self.im * o.im, self.re * o.im + self.im * o.re)
    }
    pub fn div(self, o: Complex) -> Complex {
        let d = o.abs2();
        Complex::new(
            (self.re * o.re + self.im * o.im) / d,
            (self.im * o.re - self.re * o.im) / d,
        )
    }
}

/// Solve the (small, dense) complex system `A z = b` by Gaussian elimination with
/// partial pivoting. `a` is row-major `n×n`. Returns `None` if singular.
pub fn solve_complex(a: &[Vec<Complex>], b: &[Complex]) -> Option<Vec<Complex>> {
    let n = b.len();
    let mut m: Vec<Vec<Complex>> = a.to_vec();
    let mut rhs = b.to_vec();
    for col in 0..n {
        // partial pivot on largest magnitude
        let mut piv = col;
        let mut best = m[col][col].abs();
        for r in (col + 1)..n {
            if m[r][col].abs() > best {
                best = m[r][col].abs();
                piv = r;
            }
        }
        if best < 1e-14 {
            return None;
        }
        m.swap(col, piv);
        rhs.swap(col, piv);
        let diag = m[col][col];
        for r in 0..n {
            if r == col {
                continue;
            }
            let factor = m[r][col].div(diag);
            for c in col..n {
                m[r][c] = m[r][c].sub(factor.mul(m[col][c]));
            }
            rhs[r] = rhs[r].sub(factor.mul(rhs[col]));
        }
    }
    Some((0..n).map(|i| rhs[i].div(m[i][i])).collect())
}

/// Evaluate a polynomial (coeffs low→high) at `x` (Horner).
fn poly_eval(coeffs: &[Complex], x: Complex) -> Complex {
    let mut acc = Complex::zero();
    for &c in coeffs.iter().rev() {
        acc = acc.mul(x).add(c);
    }
    acc
}

/// All roots of a polynomial (coeffs low→high) via Durand–Kerner (Weierstrass).
/// Assumes distinct roots (the regime of Prony/super-resolution). Normalizes to monic.
pub fn poly_roots(coeffs: &[Complex]) -> Vec<Complex> {
    // strip leading (highest) zeros, normalize to monic
    let mut c = coeffs.to_vec();
    while c.len() > 1 && c.last().unwrap().abs() < 1e-15 {
        c.pop();
    }
    let deg = c.len() - 1;
    if deg == 0 {
        return Vec::new();
    }
    let lead = *c.last().unwrap();
    for ci in c.iter_mut() {
        *ci = ci.div(lead);
    }
    // initial guesses: spread around a circle (the classic 0.4+0.9i seed powers)
    let seed = Complex::new(0.4, 0.9);
    let mut roots: Vec<Complex> = (0..deg)
        .map(|k| {
            let mut p = Complex::one();
            for _ in 0..k {
                p = p.mul(seed);
            }
            p
        })
        .collect();
    for _ in 0..500 {
        let mut max_delta = 0.0;
        for i in 0..deg {
            let num = poly_eval(&c, roots[i]);
            let mut den = Complex::one();
            for j in 0..deg {
                if j != i {
                    den = den.mul(roots[i].sub(roots[j]));
                }
            }
            let delta = num.div(den);
            roots[i] = roots[i].sub(delta);
            max_delta = f64::max(max_delta, delta.abs());
        }
        if max_delta < 1e-13 {
            break;
        }
    }
    roots
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn complex_ops() {
        let a = Complex::new(1.0, 2.0);
        let b = Complex::new(3.0, -1.0);
        assert_eq!(a.add(b), Complex::new(4.0, 1.0));
        assert_eq!(a.mul(b), Complex::new(5.0, 5.0));
        let q = a.div(b);
        assert!(q.mul(b).sub(a).abs() < 1e-12);
    }

    #[test]
    fn roots_of_known_polynomial() {
        // (x-1)(x+2) = x² + x − 2  → coeffs low→high [−2, 1, 1]
        let coeffs = vec![
            Complex::new(-2.0, 0.0),
            Complex::new(1.0, 0.0),
            Complex::new(1.0, 0.0),
        ];
        let mut roots = poly_roots(&coeffs);
        roots.sort_by(|a, b| a.re.partial_cmp(&b.re).unwrap());
        assert!(roots[0].sub(Complex::new(-2.0, 0.0)).abs() < 1e-9);
        assert!(roots[1].sub(Complex::new(1.0, 0.0)).abs() < 1e-9);
    }

    #[test]
    fn roots_on_unit_circle() {
        // x² + 1 = 0 → roots ±i
        let coeffs = vec![Complex::one(), Complex::zero(), Complex::one()];
        let roots = poly_roots(&coeffs);
        for r in &roots {
            assert!((r.abs() - 1.0).abs() < 1e-9, "root off unit circle: {r:?}");
            assert!(r.re.abs() < 1e-9, "root not purely imaginary");
        }
    }

    #[test]
    fn solve_complex_system() {
        // [[1, i],[1, -i]] z = [1+i, 1-i]  → z = [1, 1]
        let i = Complex::new(0.0, 1.0);
        let a = vec![
            vec![Complex::one(), i],
            vec![Complex::one(), i.scale(-1.0)],
        ];
        let b = vec![Complex::new(1.0, 1.0), Complex::new(1.0, -1.0)];
        let z = solve_complex(&a, &b).unwrap();
        assert!(z[0].sub(Complex::one()).abs() < 1e-12);
        assert!(z[1].sub(Complex::one()).abs() < 1e-12);
    }
}
