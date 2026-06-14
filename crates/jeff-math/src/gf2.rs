//! GF(2) linear algebra: bit-packed vectors and matrices over the two-element
//! field, for the Layer-2 folder (APPENDIX E.3) and the `Gf2LinearIdentity`
//! certificate (APPENDIX F.3).
//!
//! Soundness note (the key to checking without Z3): an affine map `f` over GF(2)^n
//! is determined by its values on the affine-spanning set `{0, e_1, ..., e_n}`
//! (n+1 points). So, *given* that a circuit is structurally linear+const only
//! (XOR / NOT / copy / const — guaranteed by `partition` cutting at nonlinear
//! gates, APPENDIX 10.4), agreement on the basis implies agreement everywhere.
//! The checker therefore re-evaluates the original circuit on `{0, e_i}` and
//! confirms it reconstructs the certified `(M, b)`. This is finite and exact.

use serde::{Deserialize, Serialize};

/// A GF(2) vector of `n` bits, packed into u64 words (LSB-first within a word).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Gf2Vec {
    pub n: usize,
    pub words: Vec<u64>,
}

impl Gf2Vec {
    pub fn zeros(n: usize) -> Self {
        Gf2Vec {
            n,
            words: vec![0; n.div_ceil(64)],
        }
    }

    /// The i-th standard basis vector e_i.
    pub fn basis(n: usize, i: usize) -> Self {
        let mut v = Gf2Vec::zeros(n);
        v.set(i, true);
        v
    }

    pub fn get(&self, i: usize) -> bool {
        (self.words[i / 64] >> (i % 64)) & 1 == 1
    }

    pub fn set(&mut self, i: usize, b: bool) {
        let w = i / 64;
        let bit = 1u64 << (i % 64);
        if b {
            self.words[w] |= bit;
        } else {
            self.words[w] &= !bit;
        }
    }

    pub fn xor(&self, o: &Gf2Vec) -> Gf2Vec {
        debug_assert_eq!(self.n, o.n);
        Gf2Vec {
            n: self.n,
            words: self
                .words
                .iter()
                .zip(&o.words)
                .map(|(a, b)| a ^ b)
                .collect(),
        }
    }

    pub fn is_zero(&self) -> bool {
        self.words.iter().all(|w| *w == 0)
    }

    /// GF(2) dot product (parity of AND).
    pub fn dot(&self, o: &Gf2Vec) -> bool {
        debug_assert_eq!(self.n, o.n);
        let mut acc = 0u64;
        for (a, b) in self.words.iter().zip(&o.words) {
            acc ^= a & b;
        }
        acc.count_ones() % 2 == 1
    }
}

/// A GF(2) matrix stored by columns (`cols[j]` is the image of e_j). This makes
/// `M·x = XOR over j with x_j=1 of cols[j]` (the linearity identity in F.3).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Gf2Matrix {
    pub rows: usize,
    pub cols_n: usize,
    pub cols: Vec<Gf2Vec>,
}

impl Gf2Matrix {
    pub fn zeros(rows: usize, cols_n: usize) -> Self {
        Gf2Matrix {
            rows,
            cols_n,
            cols: (0..cols_n).map(|_| Gf2Vec::zeros(rows)).collect(),
        }
    }

    /// Build M from its column images (e.g. `circuit(e_j)`).
    pub fn from_columns(rows: usize, cols: Vec<Gf2Vec>) -> Self {
        Gf2Matrix {
            rows,
            cols_n: cols.len(),
            cols,
        }
    }

    pub fn identity(n: usize) -> Self {
        Gf2Matrix::from_columns(n, (0..n).map(|j| Gf2Vec::basis(n, j)).collect())
    }

    /// Matrix–vector product over GF(2): XOR of columns selected by set bits of x.
    pub fn mat_vec(&self, x: &Gf2Vec) -> Gf2Vec {
        debug_assert_eq!(x.n, self.cols_n);
        let mut acc = Gf2Vec::zeros(self.rows);
        for (j, col) in self.cols.iter().enumerate() {
            if x.get(j) {
                acc = acc.xor(col);
            }
        }
        acc
    }

    /// Matrix–matrix product `self * other` (column-wise).
    pub fn mat_mat(&self, other: &Gf2Matrix) -> Gf2Matrix {
        debug_assert_eq!(self.cols_n, other.rows);
        let cols = other.cols.iter().map(|c| self.mat_vec(c)).collect();
        Gf2Matrix::from_columns(self.rows, cols)
    }

    /// The i-th row of `self` as a `cols_n`-bit vector.
    fn row(&self, i: usize) -> Gf2Vec {
        let mut r = Gf2Vec::zeros(self.cols_n);
        for (j, col) in self.cols.iter().enumerate() {
            r.set(j, col.get(i));
        }
        r
    }

    /// Boolean matrix product `self · other` over GF(2) via the **Method of Four
    /// Russians** (M4RM), clean-room (CLAUDE.md R5 / D8 — links no M4RI). Result is
    /// identical to [`mat_mat`]; this is the table-accelerated path (APPENDIX E.3).
    ///
    /// Idea: process `self` in vertical stripes of `k = ⌊log₂ p⌋` columns. For each
    /// stripe, precompute a table `T[s]` = XOR of the `other`-rows selected by the bits
    /// of `s` (all `2ᵏ` subsets), each entry built from the one with its lowest bit
    /// removed in a **single** row-XOR (the Gray-code/Four-Russians recurrence). Then
    /// every output row is one table lookup per stripe instead of `k` row-adds.
    pub fn mat_mat_m4rm(&self, other: &Gf2Matrix) -> Gf2Matrix {
        debug_assert_eq!(self.cols_n, other.rows);
        let m = self.rows;
        let p = self.cols_n;
        let n = other.cols_n;
        if p == 0 {
            return Gf2Matrix::zeros(m, n);
        }
        // precompute other's rows as length-n vectors (word-parallel XOR units).
        let brows: Vec<Gf2Vec> = (0..p).map(|r| other.row(r)).collect();
        // stripe width k (≥1); ⌊log2 p⌋ keeps the table size O(p).
        let k = (usize::BITS - 1 - (p as u32).leading_zeros()).max(1) as usize;

        let mut crows: Vec<Gf2Vec> = (0..m).map(|_| Gf2Vec::zeros(n)).collect();
        let arows: Vec<Gf2Vec> = (0..m).map(|i| self.row(i)).collect();

        let mut base = 0usize;
        while base < p {
            let width = k.min(p - base);
            let size = 1usize << width;
            // table[s] = XOR of brows[base + bit] for bits set in s.
            let mut table: Vec<Gf2Vec> = Vec::with_capacity(size);
            table.push(Gf2Vec::zeros(n)); // s = 0
            for s in 1..size {
                let low = s.trailing_zeros() as usize;
                let prev = s & !(1 << low);
                table.push(table[prev].xor(&brows[base + low]));
            }
            // each output row gets one lookup for this stripe.
            for (i, arow) in arows.iter().enumerate() {
                let mut idx = 0usize;
                for bit in 0..width {
                    if arow.get(base + bit) {
                        idx |= 1 << bit;
                    }
                }
                if idx != 0 {
                    crows[i] = crows[i].xor(&table[idx]);
                }
            }
            base += width;
        }

        // assemble column-major result from the computed rows.
        let mut cols = vec![Gf2Vec::zeros(m); n];
        for (i, crow) in crows.iter().enumerate() {
            for (j, col) in cols.iter_mut().enumerate() {
                col.set(i, crow.get(j));
            }
        }
        Gf2Matrix::from_columns(m, cols)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matvec_via_columns() {
        // M = identity on 4 bits ; M x == x
        let m = Gf2Matrix::identity(4);
        let mut x = Gf2Vec::zeros(4);
        x.set(0, true);
        x.set(2, true);
        assert_eq!(m.mat_vec(&x), x);
    }

    #[test]
    fn four_russians_matches_naive_matmul() {
        // AR-4 / R5: clean-room Four-Russians must equal the naive product exactly,
        // over a range of shapes (including p where k = ⌊log2 p⌋ varies).
        let mk = |rows: usize, cols: usize, seed: u64| {
            let mut s = seed;
            let mut next = || {
                s ^= s << 13;
                s ^= s >> 7;
                s ^= s << 17;
                s
            };
            let columns: Vec<Gf2Vec> = (0..cols)
                .map(|_| {
                    let mut v = Gf2Vec::zeros(rows);
                    for i in 0..rows {
                        v.set(i, next() & 1 == 1);
                    }
                    v
                })
                .collect();
            Gf2Matrix::from_columns(rows, columns)
        };
        for &(m, p, n) in &[(1usize, 1usize, 1usize), (4, 4, 4), (5, 8, 3), (9, 16, 7), (16, 16, 16)] {
            let a = mk(m, p, 0x1234 + m as u64);
            let b = mk(p, n, 0xABCD + n as u64);
            assert_eq!(
                a.mat_mat_m4rm(&b),
                a.mat_mat(&b),
                "M4RM != naive for {m}x{p} * {p}x{n}"
            );
        }
    }

    #[test]
    fn rotate_xor_is_linear_on_basis() {
        // circ(x) = rotl(x,1) ^ x on bv[4]. Columns = circ(e_i). Check M x == circ(x)
        // for all x (4 bits = 16 points) — exhaustive soundness mirror of F.3.
        let n = 4;
        let circ = |x: u8| -> u8 {
            let rot = ((x << 1) | (x >> (n - 1))) & 0x0f;
            rot ^ x
        };
        let to_vec = |x: u8| {
            let mut v = Gf2Vec::zeros(n);
            for i in 0..n {
                v.set(i, (x >> i) & 1 == 1);
            }
            v
        };
        let cols: Vec<Gf2Vec> = (0..n).map(|i| to_vec(circ(1 << i))).collect();
        let m = Gf2Matrix::from_columns(n, cols);
        for x in 0u8..16 {
            assert_eq!(m.mat_vec(&to_vec(x)), to_vec(circ(x)), "x={x}");
        }
    }
}
