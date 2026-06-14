//! Exact linear algebra over `Q` (reduced row echelon form): a particular solution
//! to `A x = b` and a basis for `null(A)`.
//!
//! Backs the Gosper degree-bounded solve (`A x = c`, APPENDIX E.1) and the
//! parametrized Gosper / Zeilberger solve (a homogeneous system whose null space
//! yields the telescoper, APPENDIX E.2). Exact (`BigRational`), deterministic (R11).

use num_rational::BigRational;
use num_traits::Zero;

type Row = Vec<BigRational>;

/// Reduce `(a | rhs)` to RREF in place; returns the pivot columns (within `a`).
fn rref(a: &mut [Row], rhs: &mut [BigRational]) -> Vec<usize> {
    let m = a.len();
    if m == 0 {
        return Vec::new();
    }
    let n = a[0].len();
    let mut pivots = Vec::new();
    let mut r = 0usize;
    for col in 0..n {
        if r >= m {
            break;
        }
        // find a pivot row at/after r with nonzero entry in `col`
        let Some(pr) = (r..m).find(|&i| !a[i][col].is_zero()) else {
            continue;
        };
        a.swap(r, pr);
        rhs.swap(r, pr);
        // normalize pivot row
        let pivot = a[r][col].clone();
        for x in a[r].iter_mut() {
            *x /= &pivot;
        }
        rhs[r] /= &pivot;
        // eliminate this column from all other rows (clone the pivot row to split
        // the borrow and iterate, avoiding range-indexing)
        let pivot_row = a[r].clone();
        let pivot_rhs = rhs[r].clone();
        for (i, row) in a.iter_mut().enumerate() {
            if i == r || row[col].is_zero() {
                continue;
            }
            let factor = row[col].clone();
            for (cell, pv) in row.iter_mut().zip(pivot_row.iter()) {
                *cell -= &factor * pv;
            }
            rhs[i] -= &factor * &pivot_rhs;
        }
        pivots.push(col);
        r += 1;
    }
    pivots
}

/// Solve `A x = b` exactly. Returns a particular solution (free variables set to 0)
/// or `None` if inconsistent. `a` is row-major `m × n`.
pub fn solve(a: &[Row], b: &[BigRational]) -> Option<Vec<BigRational>> {
    let m = a.len();
    let n = if m == 0 { 0 } else { a[0].len() };
    let mut aa: Vec<Row> = a.to_vec();
    let mut bb: Vec<BigRational> = b.to_vec();
    let pivots = rref(&mut aa, &mut bb);
    // inconsistency: a zero row in A with nonzero rhs
    for i in 0..m {
        if aa[i].iter().all(Zero::is_zero) && !bb[i].is_zero() {
            return None;
        }
    }
    let mut x = vec![BigRational::zero(); n];
    for (r, &col) in pivots.iter().enumerate() {
        x[col] = bb[r].clone();
    }
    Some(x)
}

/// A basis for the null space `{ x : A x = 0 }` (one vector per free column).
pub fn nullspace(a: &[Row]) -> Vec<Vec<BigRational>> {
    let m = a.len();
    let n = if m == 0 { 0 } else { a[0].len() };
    if n == 0 {
        return Vec::new();
    }
    let mut aa: Vec<Row> = a.to_vec();
    let mut rhs = vec![BigRational::zero(); m];
    let pivots = rref(&mut aa, &mut rhs);
    let pivot_set: std::collections::BTreeSet<usize> = pivots.iter().copied().collect();
    let free: Vec<usize> = (0..n).filter(|c| !pivot_set.contains(c)).collect();
    let mut basis = Vec::new();
    for &fc in &free {
        let mut v = vec![BigRational::zero(); n];
        v[fc] = BigRational::from(num_bigint::BigInt::from(1));
        // for each pivot row: x_pivot = - sum_{free} a[r][free] * x_free
        for (r, &pc) in pivots.iter().enumerate() {
            v[pc] = -&aa[r][fc];
        }
        basis.push(v);
    }
    basis
}

#[cfg(test)]
mod tests {
    use super::*;
    use num_bigint::BigInt;

    fn r(n: i64) -> BigRational {
        BigRational::from(BigInt::from(n))
    }

    #[test]
    fn solves_simple_system() {
        // x + y = 3 ; x - y = 1  => x=2, y=1
        let a = vec![vec![r(1), r(1)], vec![r(1), r(-1)]];
        let b = vec![r(3), r(1)];
        let x = solve(&a, &b).unwrap();
        assert_eq!(x, vec![r(2), r(1)]);
    }

    #[test]
    fn detects_inconsistency() {
        // x + y = 1 ; x + y = 2  => no solution
        let a = vec![vec![r(1), r(1)], vec![r(1), r(1)]];
        let b = vec![r(1), r(2)];
        assert!(solve(&a, &b).is_none());
    }

    #[test]
    fn nullspace_of_rank_deficient() {
        // [1 1 0] x = 0 has nullspace spanned by (−1,1,0) and (0,0,1)
        let a = vec![vec![r(1), r(1), r(0)]];
        let ns = nullspace(&a);
        assert_eq!(ns.len(), 2);
        // every basis vector v satisfies a·v = 0
        for v in &ns {
            let dot = &a[0][0] * &v[0] + &a[0][1] * &v[1] + &a[0][2] * &v[2];
            assert!(dot.is_zero());
        }
    }
}
