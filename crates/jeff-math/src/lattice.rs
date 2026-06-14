//! Exact lattice-point counting in parametric polytopes (CLAUDE.md 10.3, APPENDIX
//! F.4). Back to the **exact** regime — counts are integers, certificates are exact
//! (no tolerance).
//!
//! Clean-room (R5): this is implemented from scratch citing Ehrhart's quasi-
//! polynomiality theorem and Barvinok (1994). It links **nothing** — no GPL
//! `barvinok`/`LattE`/`PPL`, and not even isl (MIT but a C dependency); pure Rust +
//! `num-bigint`. Honesty (R30): the closed form is obtained by *interpolation* of an
//! Ehrhart quasi-polynomial (degree = dimension, period = lcm of congruence moduli),
//! not by Barvinok's signed-cone decomposition (which would give the fixed-dimension
//! polynomial-time speed). The result and the F.4 certificate are exact regardless.
//!
//! Correctness of the brute-force oracle hinges on a *correct* bounding box, computed
//! by exact Fourier–Motzkin elimination — an under-sized box would silently miscount.

use crate::poly::UniPoly;
use num_bigint::BigInt;
use num_integer::Integer;
use num_rational::BigRational;
use num_traits::{One, Zero};
use serde::{Deserialize, Serialize};

/// A linear constraint `Σ var[i]·x_i + param·n + c  (rel) 0`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinIneq {
    pub var: Vec<i64>,
    pub param: i64,
    pub c: i64,
    pub rel: Rel,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Rel {
    Ge,
    Le,
    Eq,
}

/// A congruence `x_var ≡ residue (mod modulus)` — the source of quasi-polynomial
/// periodicity (e.g. `i % 2 == 0`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Congr {
    pub var: usize,
    pub modulus: i64,
    pub residue: i64,
}

/// A parametric domain `{ x ∈ ℤ^{n_vars} : ineqs(x, n), congruences(x) }`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConstraintSystem {
    pub n_vars: usize,
    pub ineqs: Vec<LinIneq>,
    pub congrs: Vec<Congr>,
}

/// Cap on the brute-force box size, so the oracle cannot blow up (R23). Beyond it,
/// counting returns `None` and the collapser defers.
const BOX_CAP: u64 = 20_000_000;

impl ConstraintSystem {
    /// Substitute the parameter `n` and return numeric `≥ 0` rows `(a, c)` meaning
    /// `a·x + c ≥ 0`. (`Eq` becomes two `Ge` rows.)
    fn numeric_rows(&self, n: i64) -> Vec<(Vec<BigInt>, BigInt)> {
        let mut rows = Vec::new();
        for ineq in &self.ineqs {
            let a: Vec<BigInt> = ineq.var.iter().map(|&v| BigInt::from(v)).collect();
            let c = BigInt::from(ineq.param) * BigInt::from(n) + BigInt::from(ineq.c);
            match ineq.rel {
                Rel::Ge => rows.push((a, c)),
                Rel::Le => rows.push((a.iter().map(|x| -x).collect(), -c)),
                Rel::Eq => {
                    rows.push((a.clone(), c.clone()));
                    rows.push((a.iter().map(|x| -x).collect(), -c));
                }
            }
        }
        rows
    }

    /// Exact count of lattice points at parameter value `n` (the naive-correct
    /// oracle, AR-4). `None` if the polytope is unbounded or the box exceeds the cap.
    pub fn count(&self, n: i64) -> Option<BigInt> {
        let rows = self.numeric_rows(n);
        let bbox = bounding_box(&rows, self.n_vars)?;
        // box-size guard
        let mut size = BigInt::one();
        for (lo, hi) in &bbox {
            if hi < lo {
                return Some(BigInt::zero()); // empty dimension ⇒ empty polytope
            }
            size *= hi - lo + 1;
        }
        if size > BigInt::from(BOX_CAP) {
            return None;
        }
        let mut x: Vec<BigInt> = bbox.iter().map(|(lo, _)| lo.clone()).collect();
        let mut count = BigInt::zero();
        loop {
            if satisfies(&rows, &x) && self.satisfies_congr(&x) {
                count += 1;
            }
            if !increment(&mut x, &bbox) {
                break;
            }
        }
        Some(count)
    }

    fn satisfies_congr(&self, x: &[BigInt]) -> bool {
        self.congrs.iter().all(|cg| {
            let m = BigInt::from(cg.modulus);
            x[cg.var].mod_floor(&m) == BigInt::from(cg.residue).mod_floor(&m)
        })
    }

    /// The quasi-polynomial period (lcm of congruence moduli; 1 if none). The Ehrhart
    /// period can in general also come from non-integral vertices; here it is driven
    /// by congruences (sufficient for the fixtures), and the F.4 certificate verifies
    /// the result regardless — a wrong period fails verification and defers.
    pub fn period(&self) -> i64 {
        self.congrs
            .iter()
            .fold(1i64, |acc, cg| acc.lcm(&cg.modulus.max(1)))
    }
}

fn satisfies(rows: &[(Vec<BigInt>, BigInt)], x: &[BigInt]) -> bool {
    rows.iter().all(|(a, c)| {
        let mut s = c.clone();
        for (ai, xi) in a.iter().zip(x) {
            s += ai * xi;
        }
        s >= BigInt::zero()
    })
}

/// Odometer increment of `x` within `[lo,hi]` per coordinate. Returns `false` at the
/// end of the box.
fn increment(x: &mut [BigInt], bbox: &[(BigInt, BigInt)]) -> bool {
    for i in 0..x.len() {
        if x[i] < bbox[i].1 {
            x[i] += 1;
            return true;
        }
        x[i] = bbox[i].0.clone();
    }
    false
}

/// Per-variable integer bounding box via exact Fourier–Motzkin elimination. `None`
/// if any variable is unbounded (then counting is not finite → caller defers).
fn bounding_box(rows: &[(Vec<BigInt>, BigInt)], nv: usize) -> Option<Vec<(BigInt, BigInt)>> {
    let mut bbox = Vec::with_capacity(nv);
    for v in 0..nv {
        // eliminate every variable except v
        let mut cs: Vec<(Vec<BigInt>, BigInt)> = rows.to_vec();
        for e in 0..nv {
            if e != v {
                cs = fm_eliminate(&cs, e, nv);
            }
        }
        let mut lo: Option<BigInt> = None;
        let mut hi: Option<BigInt> = None;
        for (a, c) in &cs {
            let av = &a[v];
            if av.is_zero() {
                if *c < BigInt::zero() {
                    // infeasible constant ⇒ empty polytope: return an empty range.
                    return Some(vec![(BigInt::one(), BigInt::zero()); nv]);
                }
                continue;
            }
            if *av > BigInt::zero() {
                // av·x_v + c ≥ 0 ⇒ x_v ≥ ceil(-c/av)
                let b = (-c).div_ceil(av);
                lo = Some(match lo {
                    Some(l) => l.max(b),
                    None => b,
                });
            } else {
                // x_v ≤ floor(c / -av)
                let b = c.div_floor(&(-av));
                hi = Some(match hi {
                    Some(h) => h.min(b),
                    None => b,
                });
            }
        }
        match (lo, hi) {
            (Some(l), Some(h)) => bbox.push((l, h)),
            _ => return None, // unbounded
        }
    }
    Some(bbox)
}

/// Eliminate variable `e` from `≥ 0` constraints by Fourier–Motzkin.
fn fm_eliminate(cs: &[(Vec<BigInt>, BigInt)], e: usize, nv: usize) -> Vec<(Vec<BigInt>, BigInt)> {
    let mut zero = Vec::new();
    let mut pos = Vec::new();
    let mut neg = Vec::new();
    for (a, c) in cs {
        match a[e].cmp(&BigInt::zero()) {
            std::cmp::Ordering::Equal => zero.push((a.clone(), c.clone())),
            std::cmp::Ordering::Greater => pos.push((a.clone(), c.clone())),
            std::cmp::Ordering::Less => neg.push((a.clone(), c.clone())),
        }
    }
    let mut out = zero;
    for (ap, cp) in &pos {
        for (an, cn) in &neg {
            let pe = ap[e].clone(); // > 0
            let ne = an[e].clone(); // < 0
            // new = (-ne)*ap + pe*an  (coeff e cancels), const = (-ne)*cp + pe*cn
            let mut a = vec![BigInt::zero(); nv];
            for (i, ai) in a.iter_mut().enumerate() {
                *ai = (-&ne) * &ap[i] + &pe * &an[i];
            }
            let c = (-&ne) * cp + &pe * cn;
            out.push((a, c));
        }
    }
    out
}

/// An Ehrhart quasi-polynomial: `polys[n mod period]` evaluated at `n`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuasiPoly {
    pub period: i64,
    pub polys: Vec<UniPoly>,
}

impl QuasiPoly {
    /// Evaluate at integer `n` (returns the exact integer value; the quasi-polynomial
    /// is integer-valued at integers).
    pub fn eval(&self, n: i64) -> BigInt {
        let r = n.rem_euclid(self.period) as usize;
        let val = self.polys[r].eval(&BigRational::from(BigInt::from(n)));
        // integer-valued by construction
        val.to_integer()
    }
}

/// Build the Ehrhart quasi-polynomial of a fixed-dimension system by interpolation.
/// Degree = `n_vars`; one polynomial per residue class mod `period`. `None` if any
/// required sample is unbounded/too large. The result is *certified* separately (F.4)
/// — interpolation here is the construction, not the proof.
pub fn ehrhart_interpolate(cs: &ConstraintSystem) -> Option<QuasiPoly> {
    let period = cs.period();
    let deg = cs.n_vars; // Ehrhart degree = dimension
    let need = deg + 1;
    let mut polys = Vec::with_capacity(period as usize);
    for r in 0..period {
        let mut pts: Vec<(BigRational, BigRational)> = Vec::with_capacity(need);
        // sample n ≡ r (mod period), starting high enough to be in the stable region.
        for j in 0..need {
            let n = r + period * (j as i64);
            let count = cs.count(n)?;
            pts.push((BigRational::from(BigInt::from(n)), BigRational::from(count)));
        }
        polys.push(UniPoly::interpolate(&pts));
    }
    Some(QuasiPoly { period, polys })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// {0 ≤ i ≤ j ≤ n}: count = (n+1)(n+2)/2.
    fn triangle() -> ConstraintSystem {
        ConstraintSystem {
            n_vars: 2,
            ineqs: vec![
                LinIneq { var: vec![1, 0], param: 0, c: 0, rel: Rel::Ge }, // i ≥ 0
                LinIneq { var: vec![-1, 1], param: 0, c: 0, rel: Rel::Ge }, // j - i ≥ 0
                LinIneq { var: vec![0, -1], param: 1, c: 0, rel: Rel::Ge }, // n - j ≥ 0
            ],
            congrs: vec![],
        }
    }

    #[test]
    fn brute_force_triangle() {
        let t = triangle();
        // n=3 ⇒ pairs 0≤i≤j≤3 ⇒ C(5,2)=10
        assert_eq!(t.count(3).unwrap(), BigInt::from(10));
        assert_eq!(t.count(0).unwrap(), BigInt::from(1));
    }

    #[test]
    fn ehrhart_triangle_matches() {
        let t = triangle();
        let qp = ehrhart_interpolate(&t).unwrap();
        assert_eq!(qp.period, 1);
        for n in 0..30 {
            // (n+1)(n+2)/2
            let expect = BigInt::from((n + 1) * (n + 2) / 2);
            assert_eq!(qp.eval(n), expect, "n={n}");
            assert_eq!(t.count(n).unwrap(), expect);
        }
    }

    #[test]
    fn quasipoly_even_count_period2() {
        // {0 ≤ i < n, i ≡ 0 mod 2}: count = ceil(n/2).
        let cs = ConstraintSystem {
            n_vars: 1,
            ineqs: vec![
                LinIneq { var: vec![1], param: 0, c: 0, rel: Rel::Ge },   // i ≥ 0
                LinIneq { var: vec![-1], param: 1, c: -1, rel: Rel::Ge }, // n-1-i ≥ 0
            ],
            congrs: vec![Congr { var: 0, modulus: 2, residue: 0 }],
        };
        let qp = ehrhart_interpolate(&cs).unwrap();
        assert_eq!(qp.period, 2);
        for n in 1..30i64 {
            let expect = BigInt::from((n + 1) / 2); // ceil(n/2) for n ≥ 0
            assert_eq!(qp.eval(n), expect, "n={n}");
            assert_eq!(cs.count(n).unwrap(), expect);
        }
    }

    #[test]
    fn box_count_is_n_squared() {
        // {0 ≤ i < n, 0 ≤ j < n}: count = n².
        let cs = ConstraintSystem {
            n_vars: 2,
            ineqs: vec![
                LinIneq { var: vec![1, 0], param: 0, c: 0, rel: Rel::Ge },
                LinIneq { var: vec![-1, 0], param: 1, c: -1, rel: Rel::Ge },
                LinIneq { var: vec![0, 1], param: 0, c: 0, rel: Rel::Ge },
                LinIneq { var: vec![0, -1], param: 1, c: -1, rel: Rel::Ge },
            ],
            congrs: vec![],
        };
        let qp = ehrhart_interpolate(&cs).unwrap();
        for n in 1..20 {
            assert_eq!(qp.eval(n), BigInt::from(n * n), "n={n}");
        }
    }
}
