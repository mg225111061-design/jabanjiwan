//! Stage-6A Batch-4: geometry / dimension / topology (CLAUDE.md PART B Batch 4).
//!
//! ML-preprocessing kernels (dimension reduction, topology, intrinsic-dimension). The
//! honesty model is persistent homology: a feature is real only if its persistence
//! exceeds a noise band; otherwise the diagram is empty above threshold → defer. The
//! geometric kernels (JL, spectral clustering, diffusion maps, Isomap) detect structure
//! via a spectral gap / realized distortion and defer when it is absent.

#![allow(clippy::needless_range_loop)]

use crate::fmat::{jacobi_eig, FMat, Rng};

// ---- shared ----

/// Flat `n×n` Euclidean distance matrix of `n` points in `d` dims (row-major `points`).
pub fn pairwise_distances(points: &[f64], n: usize, d: usize) -> Vec<f64> {
    let mut dist = vec![0.0; n * n];
    for i in 0..n {
        for j in (i + 1)..n {
            let s: f64 = (0..d).map(|k| (points[i * d + k] - points[j * d + k]).powi(2)).sum();
            let v = s.sqrt();
            dist[i * n + j] = v;
            dist[j * n + i] = v;
        }
    }
    dist
}

struct UnionFind {
    parent: Vec<usize>,
}
impl UnionFind {
    fn new(n: usize) -> Self {
        UnionFind { parent: (0..n).collect() }
    }
    fn find(&mut self, x: usize) -> usize {
        let mut r = x;
        while self.parent[r] != r {
            r = self.parent[r];
        }
        let mut c = x;
        while self.parent[c] != r {
            let n = self.parent[c];
            self.parent[c] = r;
            c = n;
        }
        r
    }
    fn union(&mut self, a: usize, b: usize) -> bool {
        let (ra, rb) = (self.find(a), self.find(b));
        if ra == rb {
            return false;
        }
        self.parent[ra] = rb;
        true
    }
}

// ---- 4.1 Persistent homology (H0) via the MST / single-linkage filtration ----

/// H0 persistence death times (births are 0), sorted **descending**. Computed exactly
/// by Kruskal's MST on the distance matrix: each component-merging edge kills one
/// component with persistence equal to its length. The largest deaths separate clusters.
pub fn h0_persistence(dist: &[f64], n: usize) -> Vec<f64> {
    let mut edges: Vec<(f64, usize, usize)> = Vec::new();
    for i in 0..n {
        for j in (i + 1)..n {
            edges.push((dist[i * n + j], i, j));
        }
    }
    edges.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    let mut uf = UnionFind::new(n);
    let mut deaths = Vec::new();
    for (w, i, j) in edges {
        if uf.union(i, j) {
            deaths.push(w); // a component dies (merges) at filtration value w
        }
    }
    deaths.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
    deaths
}

/// Number of H0 features with persistence strictly above `band` (= number of clusters
/// separated by more than the noise band). This is the checkable certificate quantity.
pub fn persistent_feature_count(dist: &[f64], n: usize, band: f64) -> usize {
    // n points → n-1 finite deaths + 1 essential (infinite) component.
    let deaths = h0_persistence(dist, n);
    1 + deaths.iter().filter(|&&d| d > band).count()
}

// ---- 4.2 Johnson–Lindenstrauss random projection ----

/// The JL target dimension `k ≥ 4 ln n / (ε²/2 − ε³/3)` (Dasgupta–Gupta).
pub fn jl_dimension(n: usize, eps: f64) -> usize {
    let denom = eps * eps / 2.0 - eps * eps * eps / 3.0;
    (4.0 * (n as f64).ln() / denom).ceil() as usize
}

/// Project `n×d` points to `k` dims by `(1/√k)·G` with `G` Gaussian (deterministic seed).
pub fn jl_project(points: &[f64], n: usize, d: usize, k: usize, seed: u64) -> Vec<f64> {
    let mut rng = Rng::new(seed);
    let g: Vec<f64> = (0..k * d).map(|_| rng.gaussian()).collect();
    let scale = 1.0 / (k as f64).sqrt();
    let mut proj = vec![0.0; n * k];
    for i in 0..n {
        for a in 0..k {
            let mut acc = 0.0;
            for j in 0..d {
                acc += g[a * d + j] * points[i * d + j];
            }
            proj[i * k + a] = acc * scale;
        }
    }
    proj
}

/// Max pairwise distortion `|‖f(u)−f(v)‖²/‖u−v‖² − 1|` (the JL certificate quantity).
pub fn max_distortion(points: &[f64], proj: &[f64], n: usize, d: usize, k: usize) -> f64 {
    let mut worst = 0.0_f64;
    for i in 0..n {
        for j in (i + 1)..n {
            let orig: f64 = (0..d).map(|t| (points[i * d + t] - points[j * d + t]).powi(2)).sum();
            if orig < 1e-12 {
                continue;
            }
            let projd: f64 = (0..k).map(|t| (proj[i * k + t] - proj[j * k + t]).powi(2)).sum();
            worst = worst.max((projd / orig - 1.0).abs());
        }
    }
    worst
}

// ---- 4.3 Spectral clustering / Cheeger (normalized-Laplacian eigengap) ----

/// Ascending eigenvalues of the normalized Laplacian `L = I − D^{-1/2} W D^{-1/2}` of a
/// flat `n×n` similarity matrix `w` (nonnegative, symmetric).
pub fn normalized_laplacian_spectrum(w: &[f64], n: usize) -> Vec<f64> {
    let mut deg = vec![0.0; n];
    for i in 0..n {
        deg[i] = (0..n).map(|j| w[i * n + j]).sum::<f64>().max(1e-12);
    }
    let mut l = FMat::zeros(n, n);
    for i in 0..n {
        for j in 0..n {
            let norm = (deg[i] * deg[j]).sqrt();
            let val = if i == j { 1.0 } else { 0.0 } - w[i * n + j] / norm;
            l.set(i, j, val);
        }
    }
    let (mut ev, _) = jacobi_eig(&l);
    ev.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    ev
}

/// Eigengap `λ_{k+1} − λ_k` (ascending, 0-indexed: gap after the k-th smallest) — large
/// when there are exactly `k` clusters (k near-zero eigenvalues).
pub fn cluster_eigengap(w: &[f64], n: usize, k: usize) -> f64 {
    let ev = normalized_laplacian_spectrum(w, n);
    if k == 0 || k >= ev.len() {
        return 0.0;
    }
    ev[k] - ev[k - 1]
}

// ---- 4.4 Diffusion maps (Markov spectral gap) ----

/// Descending eigenvalues of the diffusion operator (symmetric conjugate
/// `D^{-1/2} W D^{-1/2}` of the Markov matrix `P = D^{-1}W`). The leading 1 is trivial.
pub fn diffusion_spectrum(w: &[f64], n: usize) -> Vec<f64> {
    let mut deg = vec![0.0; n];
    for i in 0..n {
        deg[i] = (0..n).map(|j| w[i * n + j]).sum::<f64>().max(1e-12);
    }
    let mut s = FMat::zeros(n, n);
    for i in 0..n {
        for j in 0..n {
            s.set(i, j, w[i * n + j] / (deg[i] * deg[j]).sqrt());
        }
    }
    let (mut ev, _) = jacobi_eig(&s);
    ev.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
    ev
}

/// Spectral gap `λ_m − λ_{m+1}` of the diffusion operator (descending; ev[0]=1 trivial),
/// large when an `m`-dimensional diffusion parametrization exists. A difference (not a
/// ratio) is used because cluster structure produces negative anti-mode eigenvalues.
pub fn diffusion_gap(w: &[f64], n: usize, m: usize) -> f64 {
    let ev = diffusion_spectrum(w, n);
    // ev[0] is the trivial eigenvalue (=1); nontrivial start at ev[1].
    if m + 1 >= ev.len() {
        return f64::INFINITY;
    }
    ev[m] - ev[m + 1]
}

// ---- 4.5 Isomap (geodesic MDS) ----

/// All-pairs shortest paths on a kNN graph (Floyd–Warshall) from a distance matrix.
fn knn_geodesics(dist: &[f64], n: usize, k_nn: usize) -> Vec<f64> {
    let inf = f64::INFINITY;
    let mut g = vec![inf; n * n];
    for i in 0..n {
        g[i * n + i] = 0.0;
        // k nearest neighbours of i
        let mut order: Vec<usize> = (0..n).filter(|&j| j != i).collect();
        order.sort_by(|&a, &b| dist[i * n + a].partial_cmp(&dist[i * n + b]).unwrap());
        for &j in order.iter().take(k_nn) {
            g[i * n + j] = dist[i * n + j];
            g[j * n + i] = dist[i * n + j];
        }
    }
    for k in 0..n {
        for i in 0..n {
            if g[i * n + k].is_infinite() {
                continue;
            }
            for j in 0..n {
                let nd = g[i * n + k] + g[k * n + j];
                if nd < g[i * n + j] {
                    g[i * n + j] = nd;
                }
            }
        }
    }
    g
}

/// Isomap residual variance `1 − R²` at embedding dimension `m`: classical MDS on the
/// geodesic distances (eigenvalues of the double-centered squared-distance Gram). Low at
/// the true intrinsic dimension. Returns `(residual_variance, eigengap_ratio)`.
pub fn isomap_residual(dist: &[f64], n: usize, k_nn: usize, m: usize) -> (f64, f64) {
    let geo = knn_geodesics(dist, n, k_nn);
    // B = -1/2 J D² J  (double centering)
    let d2: Vec<f64> = geo.iter().map(|&x| if x.is_finite() { x * x } else { 0.0 }).collect();
    let row_mean: Vec<f64> = (0..n).map(|i| (0..n).map(|j| d2[i * n + j]).sum::<f64>() / n as f64).collect();
    let grand = row_mean.iter().sum::<f64>() / n as f64;
    let mut b = FMat::zeros(n, n);
    for i in 0..n {
        for j in 0..n {
            b.set(i, j, -0.5 * (d2[i * n + j] - row_mean[i] - row_mean[j] + grand));
        }
    }
    let (ev, _) = jacobi_eig(&b); // descending? jacobi returns descending
    let pos: Vec<f64> = ev.iter().map(|&e| e.max(0.0)).collect();
    let total: f64 = pos.iter().sum();
    let captured: f64 = pos.iter().take(m).sum();
    let residual = if total > 1e-12 { 1.0 - captured / total } else { 0.0 };
    let gap = if m < pos.len() && pos[m] > 1e-12 { pos[m - 1] / pos[m] } else { f64::INFINITY };
    (residual, gap)
}

// ---- 4.6 Levina–Bickel intrinsic-dimension MLE ----

/// Maximum-likelihood intrinsic dimension averaged over neighbour counts `k1..=k2`
/// (Levina–Bickel 2004, bias-corrected `1/(k−2)`). Returns the estimate (may equal the
/// ambient dimension when the cloud is space-filling → no compression).
pub fn intrinsic_dimension(points: &[f64], n: usize, d: usize, k1: usize, k2: usize) -> f64 {
    let dist = pairwise_distances(points, n, d);
    let mut acc = 0.0;
    let mut cnt = 0usize;
    for i in 0..n {
        let mut nbr: Vec<f64> = (0..n).filter(|&j| j != i).map(|j| dist[i * n + j]).collect();
        nbr.sort_by(|a, b| a.partial_cmp(b).unwrap());
        for k in k1..=k2 {
            if k < 3 || k > nbr.len() {
                continue;
            }
            let tk = nbr[k - 1];
            if tk <= 1e-12 {
                continue;
            }
            let mut s = 0.0;
            for j in 0..(k - 1) {
                if nbr[j] > 1e-12 {
                    s += (tk / nbr[j]).ln();
                }
            }
            if s > 1e-12 {
                acc += (k as f64 - 2.0) / s;
                cnt += 1;
            }
        }
    }
    if cnt == 0 {
        d as f64
    } else {
        acc / cnt as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn two_clusters() -> (Vec<f64>, usize, usize) {
        // 6 points: a tight cluster near origin and one near (10,10).
        let pts = vec![
            0.0, 0.0, 0.1, 0.0, 0.0, 0.1, // cluster A
            10.0, 10.0, 10.1, 10.0, 10.0, 10.1, // cluster B
        ];
        (pts, 6, 2)
    }

    #[test]
    fn h0_persistence_finds_two_clusters() {
        let (pts, n, d) = two_clusters();
        let dist = pairwise_distances(&pts, n, d);
        // band between within-cluster (~0.1) and between-cluster (~14) distances.
        assert_eq!(persistent_feature_count(&dist, n, 1.0), 2);
        // a huge band sees only the single essential component.
        assert_eq!(persistent_feature_count(&dist, n, 100.0), 1);
    }

    #[test]
    fn jl_preserves_distances() {
        // ambient d must exceed the JL target dimension for the guarantee to bite.
        let n = 24;
        let d = 256;
        let mut rng = Rng::new(0x7);
        let pts: Vec<f64> = (0..n * d).map(|_| rng.gaussian()).collect();
        let eps = 0.5;
        let k = jl_dimension(n, eps);
        assert!(k < d, "need d > JL dimension {k}");
        let proj = jl_project(&pts, n, d, k, 0x1234);
        let dist = max_distortion(&pts, &proj, n, d, k);
        assert!(dist <= eps, "JL distortion {dist} should be ≤ {eps}");
    }

    #[test]
    fn spectral_clustering_eigengap() {
        // block similarity: two well-connected groups, weak between-links.
        let n = 6;
        let mut w = vec![0.0; n * n];
        for i in 0..n {
            for j in 0..n {
                if i == j {
                    continue;
                }
                let same = (i < 3) == (j < 3);
                w[i * n + j] = if same { 1.0 } else { 0.01 };
            }
        }
        assert!(cluster_eigengap(&w, n, 2) > 0.3, "two clusters ⇒ gap after λ₂");
        // a uniform (single-cluster) similarity has no gap at k=2
        let uniform = vec![1.0; n * n];
        assert!(cluster_eigengap(&uniform, n, 2) < 0.3);
    }

    #[test]
    fn diffusion_gap_detects_low_dim() {
        let n = 6;
        let mut w = vec![0.0; n * n];
        for i in 0..n {
            for j in 0..n {
                if i == j {
                    continue;
                }
                let same = (i < 3) == (j < 3);
                w[i * n + j] = if same { 1.0 } else { 0.01 };
            }
        }
        assert!(diffusion_gap(&w, n, 1) > 0.5, "1-dim diffusion structure (gap)");
    }

    #[test]
    fn isomap_low_residual_on_line() {
        // points on a 1-D line embedded in 2-D → residual variance ~0 at m=1.
        let n = 8;
        let d = 2;
        let pts: Vec<f64> = (0..n).flat_map(|i| [i as f64, 0.0]).collect();
        let dist = pairwise_distances(&pts, n, d);
        let (resid, _gap) = isomap_residual(&dist, n, 3, 1);
        assert!(resid < 0.05, "1-D manifold residual {resid} at m=1");
    }

    #[test]
    fn intrinsic_dimension_of_line_is_one() {
        let n = 40;
        let d = 3;
        let mut rng = Rng::new(0x1D);
        // a noisy 1-D curve in 3-D
        let pts: Vec<f64> = (0..n)
            .flat_map(|i| {
                let t = i as f64 * 0.3;
                [t + 0.001 * rng.gaussian(), 0.001 * rng.gaussian(), 0.001 * rng.gaussian()]
            })
            .collect();
        let m = intrinsic_dimension(&pts, n, d, 5, 12);
        assert!((m - 1.0).abs() < 0.5, "intrinsic dim {m} should be ≈ 1");

        // space-filling 3-D cloud → estimate near ambient 3
        let cloud: Vec<f64> = (0..n * d).map(|_| rng.gaussian()).collect();
        let mc = intrinsic_dimension(&cloud, n, d, 5, 12);
        assert!(mc > 2.0, "full 3-D cloud intrinsic dim {mc} should be > 2");
    }
}
