//! Stage 7: tensor-network contraction (CLAUDE.md PART E, 10.6).
//!
//! A tensor network contracts to a scalar (closed network) by summing the product over
//! all shared indices. The naive contraction enumerates every index assignment
//! (exponential in the number of indices) — the exact oracle. A treewidth-aware
//! contraction order keeps intermediate tensors small; the cost is governed by the
//! contraction *width* (≈ treewidth, Markov–Shi). If the width exceeds a budget the
//! intermediate tensors blow up exponentially → defer. The certificate is exact: the
//! ordered contraction equals the naive full contraction over the integers (no tol).

#![allow(clippy::needless_range_loop)]

/// A tensor over labelled indices, each ranging in `0..dim`. `data` is row-major over
/// `indices` (the first index is most-significant). Entries are exact integers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tensor {
    pub indices: Vec<usize>,
    pub data: Vec<i64>,
}

impl Tensor {
    /// Flat offset of an assignment (values for *this* tensor's indices, in order).
    fn offset(&self, vals: &[usize], dim: usize) -> usize {
        let mut off = 0;
        for &v in vals {
            off = off * dim + v;
        }
        off
    }
}

/// Naive full contraction of a **closed** network to a scalar: `Σ_assignment Π_t t[asg]`
/// over all `dim^n_indices` assignments (the exact oracle).
pub fn contract_naive(tensors: &[Tensor], dim: usize, n_indices: usize) -> i64 {
    let total = dim.checked_pow(n_indices as u32).expect("index space fits");
    let mut sum = 0i64;
    let mut asg = vec![0usize; n_indices];
    for code in 0..total {
        // decode `code` into the assignment vector
        let mut c = code;
        for slot in asg.iter_mut() {
            *slot = c % dim;
            c /= dim;
        }
        let mut prod = 1i64;
        for t in tensors {
            let vals: Vec<usize> = t.indices.iter().map(|&ix| asg[ix]).collect();
            prod *= t.data[t.offset(&vals, dim)];
            if prod == 0 {
                break;
            }
        }
        sum += prod;
    }
    sum
}

/// How many tensors in `pool` mention index `ix`.
fn index_count(pool: &[Tensor], ix: usize) -> usize {
    pool.iter().filter(|t| t.indices.contains(&ix)).count()
}

/// Contract two tensors, summing out every shared index that appears *only* in these two
/// (i.e. nowhere else in `pool`). Open indices are kept. Returns the merged tensor.
fn contract_pair(a: &Tensor, b: &Tensor, dim: usize, pool: &[Tensor]) -> Tensor {
    // union of indices
    let mut all: Vec<usize> = a.indices.clone();
    for &ix in &b.indices {
        if !all.contains(&ix) {
            all.push(ix);
        }
    }
    // an index is summed iff it is in both? no: iff all its occurrences are within {a,b}.
    // occurrences elsewhere = index_count(pool, ix) − (in a) − (in b).
    let in_ab = |ix: usize| {
        (a.indices.contains(&ix) as usize) + (b.indices.contains(&ix) as usize)
    };
    let summed: Vec<usize> = all
        .iter()
        .copied()
        .filter(|&ix| index_count(pool, ix) == in_ab(ix) && in_ab(ix) >= 1 && a.indices.contains(&ix) && b.indices.contains(&ix))
        .collect();
    let open: Vec<usize> = all.iter().copied().filter(|ix| !summed.contains(ix)).collect();

    let open_total = dim.pow(open.len() as u32);
    let sum_total = dim.pow(summed.len() as u32);
    let mut data = vec![0i64; open_total.max(1)];
    let mut open_asg = vec![0usize; open.len()];
    for ocode in 0..open_total.max(1) {
        // decode MSB-first so that `ocode == offset(open_asg)` (matches Tensor::offset).
        let mut c = ocode;
        for slot_idx in (0..open.len()).rev() {
            open_asg[slot_idx] = c % dim;
            c /= dim;
        }
        let mut acc = 0i64;
        let mut sum_asg = vec![0usize; summed.len()];
        for scode in 0..sum_total {
            let mut c = scode;
            for slot in sum_asg.iter_mut() {
                *slot = c % dim;
                c /= dim;
            }
            // build value lookups for a and b
            let val = |ix: usize| -> usize {
                if let Some(p) = open.iter().position(|&o| o == ix) {
                    open_asg[p]
                } else {
                    let p = summed.iter().position(|&s| s == ix).unwrap();
                    sum_asg[p]
                }
            };
            let av: Vec<usize> = a.indices.iter().map(|&ix| val(ix)).collect();
            let bv: Vec<usize> = b.indices.iter().map(|&ix| val(ix)).collect();
            acc += a.data[a.offset(&av, dim)] * b.data[b.offset(&bv, dim)];
        }
        data[ocode] = acc;
    }
    Tensor { indices: open, data }
}

/// Contract a closed network by a greedy min-width order. Returns `(scalar, max_width)`
/// where `max_width` is the largest intermediate index count (≈ treewidth). Returns
/// `None` if `max_width` would exceed `budget` (treewidth blow-up → defer).
pub fn contract_ordered(tensors: &[Tensor], dim: usize, budget: usize) -> Option<(i64, usize)> {
    let mut pool: Vec<Tensor> = tensors.to_vec();
    let mut max_width = pool.iter().map(|t| t.indices.len()).max().unwrap_or(0);
    while pool.len() > 1 {
        // pick the pair whose contraction yields the fewest open indices
        let mut best = None;
        let mut best_w = usize::MAX;
        for i in 0..pool.len() {
            for j in (i + 1)..pool.len() {
                if pool[i].indices.iter().all(|ix| !pool[j].indices.contains(ix)) {
                    continue; // no shared index — skip (avoid outer products early)
                }
                let merged = contract_pair(&pool[i], &pool[j], dim, &pool);
                let w = merged.indices.len();
                if w < best_w {
                    best_w = w;
                    best = Some((i, j, merged));
                }
            }
        }
        let (i, j, merged) = match best {
            Some(t) => t,
            // disconnected network: force-contract the first two
            None => {
                let m = contract_pair(&pool[0], &pool[1], dim, &pool);
                (0, 1, m)
            }
        };
        max_width = max_width.max(merged.indices.len());
        if max_width > budget {
            return None; // treewidth blow-up
        }
        // remove j then i (j>i) and push merged
        pool.remove(j);
        pool.remove(i);
        pool.push(merged);
    }
    Some((pool[0].data[0], max_width))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mat(data: [i64; 4]) -> Tensor {
        // a 2×2 tensor; indices set by caller after clone
        Tensor { indices: vec![], data: data.to_vec() }
    }

    #[test]
    fn triangle_ordered_equals_naive() {
        // three 2×2 matrices on indices (a,b),(b,c),(c,a): closed network, 3 indices.
        let dim = 2;
        let mut m_ab = mat([1, 2, 3, 4]);
        m_ab.indices = vec![0, 1];
        let mut m_bc = mat([5, 6, 7, 8]);
        m_bc.indices = vec![1, 2];
        let mut m_ca = mat([1, 0, 1, 1]);
        m_ca.indices = vec![2, 0];
        let net = vec![m_ab, m_bc, m_ca];
        let naive = contract_naive(&net, dim, 3);
        let (ordered, width) = contract_ordered(&net, dim, 8).expect("contract");
        assert_eq!(ordered, naive, "ordered must equal naive");
        assert!(width <= 2, "triangle contraction width {width}");
    }

    #[test]
    fn chain_ordered_equals_naive() {
        // a longer chain (matrix product trace) on 4 indices.
        let dim = 2;
        let make = |d: [i64; 4], ix: Vec<usize>| Tensor { indices: ix, data: d.to_vec() };
        let net = vec![
            make([1, 2, 0, 1], vec![0, 1]),
            make([2, 1, 1, 3], vec![1, 2]),
            make([0, 1, 1, 0], vec![2, 3]),
            make([1, 1, 0, 2], vec![3, 0]),
        ];
        let naive = contract_naive(&net, dim, 4);
        let (ordered, _w) = contract_ordered(&net, dim, 8).expect("contract");
        assert_eq!(ordered, naive);
    }

    #[test]
    fn high_width_network_defers() {
        // a dense network forced under a tiny budget → treewidth blow-up.
        let dim = 2;
        let make = |ix: Vec<usize>| {
            let sz = 1usize << ix.len();
            Tensor { indices: ix, data: vec![1i64; sz] }
        };
        // a 4-index tensor needs width 4; budget 2 forces a defer.
        let net = vec![make(vec![0, 1, 2, 3]), make(vec![0, 1, 2, 3])];
        assert!(contract_ordered(&net, dim, 2).is_none());
    }
}
