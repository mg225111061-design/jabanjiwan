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
