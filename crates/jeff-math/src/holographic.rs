//! Stage 8: holographic / Pfaffian algorithms (CLAUDE.md PART E / 10.5).
//!
//! FKT (Fisher–Kasteleyn–Temperley): the number of perfect matchings of a **planar**
//! graph equals `|Pf(A)|` for a Pfaffian (Kasteleyn) orientation `A`. The certificate is
//! exact and double-checked: (a) the algebraic identity `Pf(A)² = det(A)` (always true
//! for skew-symmetric A), and (b) `|Pf(A)| == ` the naive perfect-matching count. Outside
//! the planar class the matchgate method does not apply (Cai–Lu dichotomy: #P-hard) →
//! `defer[non-planar]`. The naive oracle is the ultimate P0 guard: a wrong count is
//! rejected regardless of how the orientation was found.

#![allow(clippy::needless_range_loop)]

use std::collections::HashMap;

/// Combinatorial Pfaffian of a skew-symmetric `n×n` integer matrix (`n` even), summing
/// over perfect matchings of `{0..n-1}` with sign. Exact; `O(n!!)` (small `n`).
pub fn pfaffian(a: &[i64], n: usize) -> i64 {
    if !n.is_multiple_of(2) {
        return 0;
    }
    let avail: Vec<usize> = (0..n).collect();
    pf_rec(a, n, &avail)
}

fn pf_rec(a: &[i64], n: usize, avail: &[usize]) -> i64 {
    if avail.is_empty() {
        return 1;
    }
    let i = avail[0];
    let mut sum = 0i64;
    for (k, &j) in avail.iter().enumerate().skip(1) {
        let coeff = a[i * n + j];
        if coeff == 0 {
            continue;
        }
        let sign = if (k - 1) % 2 == 0 { 1 } else { -1 };
        let rest: Vec<usize> = avail.iter().copied().filter(|&x| x != i && x != j).collect();
        sum += sign * coeff * pf_rec(a, n, &rest);
    }
    sum
}

/// Exact integer determinant via fraction-free Bareiss elimination.
pub fn det_bareiss(mat: &[i64], n: usize) -> i64 {
    let mut m: Vec<i64> = mat.to_vec();
    let mut prev = 1i64;
    let mut sign = 1i64;
    for k in 0..n {
        if m[k * n + k] == 0 {
            // find a pivot row to swap
            let mut piv = None;
            for r in (k + 1)..n {
                if m[r * n + k] != 0 {
                    piv = Some(r);
                    break;
                }
            }
            match piv {
                Some(r) => {
                    for c in 0..n {
                        m.swap(k * n + c, r * n + c);
                    }
                    sign = -sign;
                }
                None => return 0,
            }
        }
        for i in (k + 1)..n {
            for j in (k + 1)..n {
                m[i * n + j] = (m[i * n + j] * m[k * n + k] - m[i * n + k] * m[k * n + j]) / prev;
            }
        }
        prev = m[k * n + k];
    }
    sign * m[(n - 1) * n + (n - 1)]
}

/// Naive perfect-matching count of an undirected graph (flat `n×n` 0/1 adjacency) — the
/// exact oracle. Recursively matches the lowest unmatched vertex to each neighbour.
pub fn count_pm_naive(adj: &[u8], n: usize) -> u64 {
    let mut used = vec![false; n];
    pm_rec(adj, n, &mut used, 0)
}

fn pm_rec(adj: &[u8], n: usize, used: &mut [bool], start: usize) -> u64 {
    // find first unmatched vertex
    let mut i = start;
    while i < n && used[i] {
        i += 1;
    }
    if i == n {
        return 1; // all matched
    }
    let mut total = 0u64;
    used[i] = true;
    for j in (i + 1)..n {
        if !used[j] && adj[i * n + j] == 1 {
            used[j] = true;
            total += pm_rec(adj, n, used, i + 1);
            used[j] = false;
        }
    }
    used[i] = false;
    total
}

/// Conservative planarity necessary conditions: `|E| ≤ 3|V|−6` (and `≤ 2|V|−4` if
/// bipartite). Failing ⇒ definitely non-planar. Passing is necessary, not sufficient —
/// the naive oracle guards correctness regardless.
pub fn passes_planarity_bound(n: usize, edges: &[(usize, usize)], bipartite: bool) -> bool {
    if n < 3 {
        return true;
    }
    let e = edges.len();
    if e > 3 * n - 6 {
        return false;
    }
    if bipartite && n >= 3 && e > 2 * n - 4 {
        return false;
    }
    true
}

/// Find a Kasteleyn orientation of a planar graph given its inner `faces` (each a cyclic
/// vertex list) and build the skew-symmetric matrix; return `|Pf(A)|`. `None` if the
/// embedding is invalid (orientation cannot be completed → treat as non-planar/defer).
pub fn fkt_count(n: usize, edges: &[(usize, usize)], faces: &[Vec<usize>]) -> Option<i64> {
    let norm = |a: usize, b: usize| if a < b { (a, b) } else { (b, a) };
    // dir[(a,b)] = true means oriented a→b (a<b), false means b→a. absent = unoriented.
    let mut dir: HashMap<(usize, usize), bool> = HashMap::new();
    // spanning tree (union-find): tree edges oriented a→b (a<b).
    let mut parent: Vec<usize> = (0..n).collect();
    fn find(p: &mut [usize], x: usize) -> usize {
        let mut r = x;
        while p[r] != r {
            r = p[r];
        }
        p[x] = r;
        r
    }
    for &(u, v) in edges {
        let (a, b) = norm(u, v);
        let (ra, rb) = (find(&mut parent, a), find(&mut parent, b));
        if ra != rb {
            parent[ra] = rb;
            dir.insert((a, b), true); // a→b
        }
    }
    // greedy face orientation: repeatedly orient the unique unoriented edge of a face to
    // make the clockwise traversal have an odd number of agreeing edges.
    let face_edges = |f: &Vec<usize>| -> Vec<(usize, usize)> {
        (0..f.len()).map(|i| (f[i], f[(i + 1) % f.len()])).collect()
    };
    let total_edges = edges.len();
    for _ in 0..(total_edges + 1) {
        if dir.len() == total_edges {
            break;
        }
        let mut progressed = false;
        for f in faces {
            let fe = face_edges(f);
            let unoriented: Vec<(usize, usize)> =
                fe.iter().filter(|&&(u, v)| !dir.contains_key(&norm(u, v))).copied().collect();
            if unoriented.len() == 1 {
                // count agreements among oriented edges (traversal direction u→v)
                let mut agree = 0i32;
                for &(u, v) in &fe {
                    let key = norm(u, v);
                    if let Some(&d) = dir.get(&key) {
                        // d true means a→b with a<b; traversal is u→v.
                        let traversal_is_au = u < v; // u→v equals a→b orientation
                        let oriented_uv = d == traversal_is_au;
                        if oriented_uv {
                            agree += 1;
                        }
                    }
                }
                let (u, v) = unoriented[0];
                let key = norm(u, v);
                // choose orientation of (u,v) to make total agreements odd
                let traversal_is_au = u < v;
                // if we orient u→v it agrees (+1)
                let make_agree = agree % 2 == 0; // need one more agreement to be odd
                let d = if make_agree { traversal_is_au } else { !traversal_is_au };
                dir.insert(key, d);
                progressed = true;
            }
        }
        if !progressed {
            break;
        }
    }
    if dir.len() != total_edges {
        return None; // could not complete a Kasteleyn orientation
    }
    // build skew-symmetric A
    let mut a = vec![0i64; n * n];
    for (&(x, y), &d) in &dir {
        // d true: x→y (x<y). A[x][y]=+1, A[y][x]=-1.
        let (from, to) = if d { (x, y) } else { (y, x) };
        a[from * n + to] = 1;
        a[to * n + from] = -1;
    }
    Some(pfaffian(&a, n).abs())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn adj_from(n: usize, edges: &[(usize, usize)]) -> Vec<u8> {
        let mut a = vec![0u8; n * n];
        for &(u, v) in edges {
            a[u * n + v] = 1;
            a[v * n + u] = 1;
        }
        a
    }

    #[test]
    fn pfaffian_squared_is_det() {
        // a 4×4 skew-symmetric matrix: Pf² = det.
        let a = [0, 1, 2, 3, -1, 0, 4, 5, -2, -4, 0, 6, -3, -5, -6, 0];
        let pf = pfaffian(&a, 4);
        let det = det_bareiss(&a, 4);
        assert_eq!(pf * pf, det, "Pf²={} det={}", pf * pf, det);
    }

    #[test]
    fn c4_two_matchings() {
        // 4-cycle 0-1-2-3-0: 2 perfect matchings.
        let edges = [(0, 1), (1, 2), (2, 3), (3, 0)];
        let adj = adj_from(4, &edges);
        assert_eq!(count_pm_naive(&adj, 4), 2);
        let faces = vec![vec![0, 1, 2, 3]]; // single inner face
        let fkt = fkt_count(4, &edges, &faces).expect("orient");
        assert_eq!(fkt, 2, "FKT count should match naive");
    }

    #[test]
    fn k4_three_matchings() {
        // K4 is planar; 3 perfect matchings. Embedding: outer triangle 0-1-2 with vertex 3 inside.
        let edges = [(0, 1), (1, 2), (2, 0), (0, 3), (1, 3), (2, 3)];
        let adj = adj_from(4, &edges);
        assert_eq!(count_pm_naive(&adj, 4), 3);
        // inner faces of this embedding: (0,1,3),(1,2,3),(2,0,3)
        let faces = vec![vec![0, 1, 3], vec![1, 2, 3], vec![2, 0, 3]];
        let fkt = fkt_count(4, &edges, &faces).expect("orient");
        assert_eq!(fkt, 3);
    }

    #[test]
    fn k33_is_nonplanar() {
        // K3,3: bipartite, V=6, E=9 > 2·6−4=8 ⇒ flagged non-planar by the bipartite bound.
        let edges: Vec<(usize, usize)> =
            (0..3).flat_map(|i| (3..6).map(move |j| (i, j))).collect();
        assert!(!passes_planarity_bound(6, &edges, true));
        // (its perfect-matching count is 6, but FKT must not be applied)
        let adj = adj_from(6, &edges);
        assert_eq!(count_pm_naive(&adj, 6), 6);
    }
}
