//! Stage 37.2 — Čech-cohomology obstruction certificates (sheaf gluing).
//!
//! `n` sensors on a circle `S¹`, each measuring only relative displacement; pairwise data is
//! consistent yet **no global section exists**. The obstruction is `H¹ ≠ 0`: with cochain
//! `g_ij` on overlaps, a global section needs `g_ij = s_j − s_i` (a coboundary), which forces the
//! holonomy `Σ_cycle g_ij = 0`; a nonzero holonomy is the obstruction class. The "no global
//! section" claim is an **exact unsat** (the linear system `g_ij = s_j − s_i` is inconsistent).
//! Niche / domain-edge (algebraic topology), labeled (rule 7). Certificate: **absence (exact)**.

use num_bigint::BigInt;
use num_rational::BigRational;

/// `H¹` dimension of a nerve graph by Euler characteristic: `b₁ = |E| − |V| + components`
/// (the number of independent cycles). For a single cycle on `n` vertices, `b₁ = 1`.
pub fn nerve_b1(num_vertices: usize, num_edges: usize, components: usize) -> usize {
    (num_edges + components).saturating_sub(num_vertices)
}

/// An obstruction (no-global-section) certificate.
#[derive(Clone, Debug, PartialEq)]
pub struct ObstructionCert {
    pub ungluable: bool,
    pub cohomology_dim: usize,
    /// The holonomy `Σ_cycle g_ij` — nonzero is the obstruction class.
    pub holonomy: i64,
    pub witness: String,
}

/// Cyclic sensor gluing: edges `i → (i+1) mod n` carry relative displacements `g[i] = x_{i+1} − x_i`.
/// A global assignment `s_0,…,s_{n-1}` with `g[i] = s_{(i+1)} − s_i` exists **iff** the holonomy
/// `Σ g[i] = 0` (telescoping around the cycle). Nonzero holonomy ⇒ **ungluable** (exact unsat of the
/// linear system) ⇒ obstruction class in `H¹(S¹) ≅ ℝ`.
pub fn cyclic_gluing_obstruction(g: &[i64]) -> ObstructionCert {
    let holonomy: i64 = g.iter().sum();
    let n = g.len();
    let b1 = nerve_b1(n, n, 1); // a single cycle: |V|=|E|=n, 1 component ⇒ b₁ = 1
    ObstructionCert {
        ungluable: holonomy != 0,
        cohomology_dim: b1,
        holonomy,
        witness: format!("Σ g_ij = {holonomy} (≠0 ⇒ no s with g_i=s_{{i+1}}−s_i; H¹ dim={b1})"),
    }
}

/// Exact unsat proof that the cyclic gluing system `g[i] = s_{i+1} − s_i (mod cycle)` has no
/// solution when `Σ g ≠ 0`: summing all equations telescopes the RHS to 0, so `Σ g = 0` is
/// necessary; `Σ g ≠ 0` is a direct contradiction. Returns `true` iff **unsat** (no global section).
pub fn gluing_system_unsat(g: &[i64]) -> bool {
    g.iter().sum::<i64>() != 0
}

/// Exact rank (over ℚ) of an incidence-style matrix (rows = edges, cols = vertices), to compute
/// `dim H¹ = |E| − rank(δ⁰)` directly rather than via Euler characteristic.
pub fn incidence_rank(rows: &[Vec<i64>]) -> usize {
    let m: Vec<Vec<BigRational>> = rows
        .iter()
        .map(|r| r.iter().map(|&x| BigRational::from(BigInt::from(x))).collect())
        .collect();
    crate::displacement::exact_rank(&m)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sensor_gluing_obstruction_h1_nonzero() {
        // three sensors, each reads +120° relative displacement → holonomy 360° ≠ 0 ⇒ ungluable.
        let cert = cyclic_gluing_obstruction(&[120, 120, 120]);
        assert!(cert.ungluable, "360° holonomy ⇒ no global section: {}", cert.witness);
        assert_eq!(cert.holonomy, 360);
        assert_eq!(cert.cohomology_dim, 1, "H¹(S¹) ≅ ℝ (dim 1)");
    }

    #[test]
    fn holonomy_360_obstruction() {
        // consistent pairwise (each +120) but globally inconsistent (Σ = 360 ≠ 0).
        assert!(gluing_system_unsat(&[120, 120, 120]));
        // a coboundary g_i = s_{i+1}−s_i (e.g. s = [0,5,2] → g = [5,-3,-2], Σ=0) IS gluable.
        assert!(!gluing_system_unsat(&[5, -3, -2]));
        assert!(!cyclic_gluing_obstruction(&[5, -3, -2]).ungluable);
    }

    #[test]
    fn gluing_unsat_proven() {
        // the unsat is exact: Σ g ≠ 0 telescopes to a contradiction.
        assert!(gluing_system_unsat(&[1, 1, 1, 1])); // Σ=4≠0
        assert!(!gluing_system_unsat(&[1, -1, 1, -1])); // Σ=0, gluable
    }

    #[test]
    fn nerve_b1_generalizes() {
        // single n-cycle ⇒ b₁ = 1; two disjoint cycles ⇒ b₁ = 2; a tree ⇒ b₁ = 0.
        assert_eq!(nerve_b1(5, 5, 1), 1); // 5-cycle
        assert_eq!(nerve_b1(6, 6, 2), 2); // two triangles
        assert_eq!(nerve_b1(5, 4, 1), 0); // path/tree (no cycle)
        // incidence rank route agrees for the 3-cycle: δ⁰ is 3×3 with rank 2 ⇒ H¹ = 3−2 = 1.
        let inc = vec![vec![-1, 1, 0], vec![0, -1, 1], vec![1, 0, -1]];
        let rank = incidence_rank(&inc);
        assert_eq!(rank, 2, "3-cycle incidence rank = 2");
        assert_eq!(3 - rank, 1, "dim H¹ = |E| − rank = 1");
    }
}
