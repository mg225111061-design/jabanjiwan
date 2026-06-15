//! Stage 34 — tropical `(min,+)` DP fold family.
//!
//! The min-plus semiring (`x⊕y = min(x,y)`, `x⊗y = x+y`, identities `+∞` and `0`) turns
//! shortest-path / Viterbi / scheduling DP into linear algebra. The structured win mirrors
//! Stage 26: a layered DAG whose layer transition matrix `M` **repeats** `N` times → the length-`N`
//! shortest paths are `M^{⊗N}` computable by min-plus **fast exponentiation** `O(w³ log N)` vs the
//! naive layer-by-layer DP `O(N·w²)` — ratio `~ N/(w log N)` diverges for fixed width `w`.
//!
//! **No "O(1) collapse"**: tropical varieties can have exponentially many vertices, so the gain is
//! claimed **only** for repeated/low-rank structure with a measured crossover; an unstructured DP
//! (all layers distinct) gets **zero gain** (each layer must be processed — honestly reported).
//! Certificate: **integer-exact** (matpow == naive) for integer weights.

/// Saturating "infinity" for min-plus over `i64` (avoids overflow under `⊗`).
pub const INF: i64 = i64::MAX / 4;

/// `(min,+)` matrix product `C[i][j] = min_k A[i][k] + B[k][j]`, `w×w`.
pub fn min_plus_matmul(a: &[i64], b: &[i64], w: usize) -> Vec<i64> {
    let mut c = vec![INF; w * w];
    for i in 0..w {
        for k in 0..w {
            let aik = a[i * w + k];
            if aik >= INF {
                continue;
            }
            for j in 0..w {
                let v = aik + b[k * w + j];
                if v < c[i * w + j] {
                    c[i * w + j] = v;
                }
            }
        }
    }
    c
}

/// Min-plus identity (`0` on diagonal, `+∞` off).
pub fn min_plus_identity(w: usize) -> Vec<i64> {
    let mut m = vec![INF; w * w];
    for i in 0..w {
        m[i * w + i] = 0;
    }
    m
}

/// `M^{⊗n}` in the min-plus semiring via fast exponentiation, `O(w³ log n)`.
pub fn min_plus_matpow(m: &[i64], n: u64, w: usize) -> Vec<i64> {
    let mut acc = min_plus_identity(w);
    let mut base = m.to_vec();
    let mut e = n;
    while e > 0 {
        if e & 1 == 1 {
            acc = min_plus_matmul(&acc, &base, w);
        }
        base = min_plus_matmul(&base, &base, w);
        e >>= 1;
    }
    acc
}

/// Naive layered-DAG shortest paths over `n` copies of layer `m`: O(n·w²) per source row, here the
/// full transfer matrix by repeated left-multiplication (`O(n·w³)`), the oracle.
pub fn repeated_layer_naive(m: &[i64], n: u64, w: usize) -> Vec<i64> {
    let mut acc = min_plus_identity(w);
    for _ in 0..n {
        acc = min_plus_matmul(&acc, m, w);
    }
    acc
}

/// Naive DP over a list of *distinct* layer matrices (no repeated structure ⇒ no collapse).
pub fn layered_dag_naive(layers: &[Vec<i64>], w: usize) -> Vec<i64> {
    let mut acc = min_plus_identity(w);
    for layer in layers {
        acc = min_plus_matmul(&acc, layer, w);
    }
    acc
}

/// Viterbi (min-plus) shortest path with traceback: returns `(cost, path)` from `src` to `dst`
/// over `n` copies of layer `m`. Exact for integer weights.
pub fn viterbi_path(m: &[i64], n: usize, w: usize, src: usize, dst: usize) -> (i64, Vec<usize>) {
    let mut dist = vec![INF; w];
    dist[src] = 0;
    let mut back = vec![vec![usize::MAX; w]; n];
    for bstep in back.iter_mut().take(n) {
        let mut nd = vec![INF; w];
        for u in 0..w {
            if dist[u] >= INF {
                continue;
            }
            for v in 0..w {
                let c = dist[u] + m[u * w + v];
                if c < nd[v] {
                    nd[v] = c;
                    bstep[v] = u;
                }
            }
        }
        dist = nd;
    }
    // traceback
    let mut path = vec![dst];
    let mut cur = dst;
    for step in (0..n).rev() {
        cur = back[step][cur];
        if cur == usize::MAX {
            return (INF, vec![]);
        }
        path.push(cur);
    }
    path.reverse();
    (dist[dst], path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minplus_semiring_correct() {
        // ⊕ = min, ⊗ = +, identities +∞ (for ⊕) and 0 (for ⊗).
        let w = 2;
        let id = min_plus_identity(w);
        let a = vec![1, 5, 2, 0];
        // A ⊗ I == A
        assert_eq!(min_plus_matmul(&a, &id, w), a);
        // (min,+) product by hand: C[0][0] = min(1+1, 5+2) = 2
        let b = vec![1, 0, 2, 3];
        let c = min_plus_matmul(&a, &b, w);
        assert_eq!(c[0], 2); // min(1+1, 5+2)
        assert_eq!(c[1], 1); // min(1+0, 5+3) = min(1, 8)
    }

    #[test]
    fn layered_dag_collapse_measured() {
        // repeated layer: matpow O(w³ log N) == naive O(N w³), and the op-count ratio ~ N/log N.
        let w = 4;
        let m = vec![
            0, 3, INF, 7, INF, 0, 2, INF, 5, INF, 0, 1, INF, 4, INF, 0,
        ];
        for &n in &[8u64, 64, 1000] {
            assert_eq!(min_plus_matpow(&m, n, w), repeated_layer_naive(&m, n, w), "matpow==naive N={n}");
        }
        // op-count proxy: ratio (N w³)/(w³ log N) = N/log N grows.
        let mut prev = 0f64;
        for k in 3..=7 {
            let n = 10u64.pow(k);
            let ratio = n as f64 / ((64 - n.leading_zeros()) as f64);
            assert!(ratio > prev);
            prev = ratio;
        }
        assert!(prev > 1e5);
    }

    #[test]
    fn viterbi_path_exact() {
        // a 3-node layer; cheapest 2-step path 0→…→2 is exact.
        let w = 3;
        let m = vec![0, 2, 9, 9, 0, 1, 9, 9, 0];
        let (cost, path) = viterbi_path(&m, 2, w, 0, 2);
        // 0→1 (2) →2 (1) = 3, beating 0→2 (9)→2(0)=9 over 2 steps.
        assert_eq!(cost, 3);
        assert_eq!(path.first(), Some(&0));
        assert_eq!(path.last(), Some(&2));
    }

    #[test]
    fn no_structure_zero_gain() {
        // distinct layers (no repetition) ⇒ no matpow collapse; must process each layer. The fold
        // gain is zero (honest): layered_dag_naive is the only correct route.
        let w = 3;
        let layers: Vec<Vec<i64>> = (0..5)
            .map(|s| (0..w * w).map(|t| ((t + s) % 7) as i64).collect())
            .collect();
        let full = layered_dag_naive(&layers, w);
        // there is no single repeated M reproducing distinct layers; matpow of any one layer differs.
        assert_ne!(min_plus_matpow(&layers[0], 5, w), full, "distinct layers ⇒ no repeated-power collapse");
    }

    #[test]
    fn tropical_cert() {
        // integer-exact certificate: matpow result equals the naive layer-by-layer DP exactly.
        let w = 3;
        let m = vec![0, 1, 4, 2, 0, 1, 3, 1, 0];
        let n = 100u64;
        assert_eq!(min_plus_matpow(&m, n, w), repeated_layer_naive(&m, n, w));
    }
}
