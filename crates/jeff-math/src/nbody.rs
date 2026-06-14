//! Fast N-body summation for decaying kernels (CLAUDE.md 10.2 fmm; APPENDIX I.3).
//!
//! `φ_j = Σ_{i≠j} q_i · K(|x_j − x_i|)`. The exact O(N²) [`direct_sum`] is the
//! naive-correct oracle (AR-4); [`barnes_hut`] is a tree-code (FMM-family, monopole
//! far-field) that is asymptotically faster when `K` decays with distance. The
//! certificate is the **measured** residual `‖φ_fast − φ_exact‖∞ ≤ ε`. If `K` does
//! not decay (oscillatory), the far-field approximation is invalid and the collapser
//! demotes to constant-factor-only — here is exactly where "no structure" means "no
//! speedup", not "wrong answer" (the residual check still guards correctness, P0).
//!
//! Honesty: this is a *monopole* tree-code (Barnes–Hut), not a full multipole FMM
//! (which adds local expansions for O(N)); O(N log N) average for decaying kernels.

use serde::{Deserialize, Serialize};

/// A radial kernel `K(r)`. Carries its parameters so a certificate can recompute
/// the exact direct sum independently (R25).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum KernelKind {
    /// `exp(-decay·r)` — decays for `decay > 0`.
    Exponential { decay: f64 },
    /// `1/(eps + r)` — decays (softened Coulomb-like).
    InverseSoft { eps: f64 },
    /// `cos(freq·r)` — oscillatory, does **not** decay (used to test demotion).
    Cosine { freq: f64 },
}

impl KernelKind {
    pub fn eval(self, r: f64) -> f64 {
        match self {
            KernelKind::Exponential { decay } => (-decay * r).exp(),
            KernelKind::InverseSoft { eps } => 1.0 / (eps + r),
            KernelKind::Cosine { freq } => (freq * r).cos(),
        }
    }

    /// Structural precondition (R19): is `K` non-increasing and decaying over a test
    /// range? Measured, not assumed. Far-field monopole approximation needs this.
    pub fn is_decaying(self) -> bool {
        let mut prev = self.eval(0.0);
        let mut r = 0.25;
        while r <= 8.0 {
            let cur = self.eval(r);
            if cur > prev + 1e-12 {
                return false; // increased somewhere ⇒ not monotone decaying
            }
            prev = cur;
            r += 0.25;
        }
        // and it must actually have decayed (not flat)
        self.eval(8.0) < self.eval(0.0) - 1e-9
    }
}

/// Exact O(N²) direct summation — the oracle and the certificate's ground truth.
pub fn direct_sum(points: &[f64], charges: &[f64], kernel: KernelKind) -> Vec<f64> {
    let n = points.len();
    let mut phi = vec![0.0; n];
    for j in 0..n {
        let mut s = 0.0;
        for i in 0..n {
            if i != j {
                s += charges[i] * kernel.eval((points[j] - points[i]).abs());
            }
        }
        phi[j] = s;
    }
    phi
}

enum Tree {
    Leaf {
        idx: usize,
    },
    Node {
        x_min: f64,
        x_max: f64,
        x_c: f64,
        q_total: f64,
        left: Box<Tree>,
        right: Box<Tree>,
    },
}

fn build(order: &[usize], points: &[f64], charges: &[f64]) -> Tree {
    if order.len() == 1 {
        return Tree::Leaf { idx: order[0] };
    }
    let mid = order.len() / 2;
    let left = Box::new(build(&order[..mid], points, charges));
    let right = Box::new(build(&order[mid..], points, charges));
    let xs: Vec<f64> = order.iter().map(|&i| points[i]).collect();
    let x_min = xs.iter().cloned().fold(f64::INFINITY, f64::min);
    let x_max = xs.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let q_total: f64 = order.iter().map(|&i| charges[i]).sum();
    Tree::Node {
        x_min,
        x_max,
        x_c: 0.5 * (x_min + x_max),
        q_total,
        left,
        right,
    }
}

fn eval_tree(
    tree: &Tree,
    xj: f64,
    j: usize,
    points: &[f64],
    charges: &[f64],
    kernel: KernelKind,
    theta: f64,
) -> f64 {
    match tree {
        Tree::Leaf { idx } => {
            if *idx == j {
                0.0
            } else {
                charges[*idx] * kernel.eval((xj - points[*idx]).abs())
            }
        }
        Tree::Node {
            x_min,
            x_max,
            x_c,
            q_total,
            left,
            right,
        } => {
            let size = x_max - x_min;
            let dist = (xj - x_c).abs();
            // Far enough (and j cannot be inside, since theta<2): monopole.
            if dist > 0.0 && size / dist < theta {
                q_total * kernel.eval(dist)
            } else {
                eval_tree(left, xj, j, points, charges, kernel, theta)
                    + eval_tree(right, xj, j, points, charges, kernel, theta)
            }
        }
    }
}

/// Barnes–Hut monopole far-field summation. `theta` is the opening angle (use < 2 so
/// the target is never inside a far node; smaller ⇒ more accurate, slower).
pub fn barnes_hut(points: &[f64], charges: &[f64], kernel: KernelKind, theta: f64) -> Vec<f64> {
    let n = points.len();
    if n == 0 {
        return vec![];
    }
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|&a, &b| points[a].partial_cmp(&points[b]).unwrap());
    let tree = build(&order, points, charges);
    (0..n)
        .map(|j| eval_tree(&tree, points[j], j, points, charges, kernel, theta))
        .collect()
}

/// `‖a − b‖∞`.
pub fn max_abs_diff(a: &[f64], b: &[f64]) -> f64 {
    a.iter()
        .zip(b)
        .map(|(x, y)| (x - y).abs())
        .fold(0.0, f64::max)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grid(n: usize) -> (Vec<f64>, Vec<f64>) {
        let points: Vec<f64> = (0..n).map(|i| i as f64).collect();
        let charges: Vec<f64> = (0..n).map(|i| 1.0 + (i % 3) as f64).collect();
        (points, charges)
    }

    #[test]
    fn barnes_hut_converges_for_decaying_kernel() {
        let (pts, chg) = grid(64);
        let k = KernelKind::Exponential { decay: 1.0 };
        let exact = direct_sum(&pts, &chg, k);
        let err_coarse = max_abs_diff(&barnes_hut(&pts, &chg, k, 0.8), &exact);
        let err_fine = max_abs_diff(&barnes_hut(&pts, &chg, k, 0.2), &exact);
        // monopole tree-code converges as θ shrinks (smaller opening angle).
        assert!(err_fine < err_coarse, "smaller θ must reduce residual ({err_fine} vs {err_coarse})");
        // and a tight θ gives a residual a sensible ε can certify against.
        assert!(err_fine < 1e-2, "BH residual at θ=0.2 is {err_fine}");
    }

    #[test]
    fn decay_precondition_detects_oscillatory() {
        assert!(KernelKind::Exponential { decay: 0.5 }.is_decaying());
        assert!(KernelKind::InverseSoft { eps: 0.1 }.is_decaying());
        assert!(!KernelKind::Cosine { freq: 3.0 }.is_decaying());
    }
}
