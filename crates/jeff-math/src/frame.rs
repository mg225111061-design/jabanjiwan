//! Batch-1.6 — Welch bound / equiangular tight frames (Welch 1974).
//!
//! For `M` unit vectors in `R^N` (or `C^N`) with `M>N`, the coherence
//! `μ = max_{i≠j} |⟨v_i,v_j⟩|` obeys the Welch bound `μ ≥ √((M−N)/(N(M−1)))`, with
//! equality iff the frame is an **equiangular tight frame** (ETF). ETFs are the
//! optimal (maximally incoherent) frames for sparse recovery. Detecting "is this an
//! ETF?" is an exact, finite Gram-matrix check — the certificate is `exact`.

/// The Welch coherence lower bound for `m` vectors in dimension `n` (`m>n`).
pub fn welch_bound(m: usize, n: usize) -> f64 {
    if m <= n {
        return 0.0;
    }
    (((m - n) as f64) / ((n * (m - 1)) as f64)).sqrt()
}

/// Normalized inner product `⟨v_i,v_j⟩ / (‖v_i‖‖v_j‖)` for a row-major `m×n` frame.
pub fn normalized_gram_entry(frame: &[f64], _m: usize, n: usize, i: usize, j: usize) -> f64 {
    let vi = &frame[i * n..i * n + n];
    let vj = &frame[j * n..j * n + n];
    let dot: f64 = vi.iter().zip(vj).map(|(a, b)| a * b).sum();
    let ni: f64 = vi.iter().map(|a| a * a).sum::<f64>().sqrt();
    let nj: f64 = vj.iter().map(|a| a * a).sum::<f64>().sqrt();
    if ni == 0.0 || nj == 0.0 {
        return 0.0;
    }
    dot / (ni * nj)
}

/// Coherence `μ = max_{i≠j} |normalized ⟨v_i,v_j⟩|` of a row-major `m×n` frame.
pub fn coherence(frame: &[f64], m: usize, n: usize) -> f64 {
    let mut mu = 0.0_f64;
    for i in 0..m {
        for j in (i + 1)..m {
            mu = mu.max(normalized_gram_entry(frame, m, n, i, j).abs());
        }
    }
    mu
}

/// Is this frame equiangular at the Welch value (⇔ ETF)? All off-diagonal magnitudes
/// equal to `welch_bound(m,n)` within `tol`. This IS the certificate check.
pub fn is_etf(frame: &[f64], m: usize, n: usize, tol: f64) -> bool {
    let w = welch_bound(m, n);
    for i in 0..m {
        for j in (i + 1)..m {
            if (normalized_gram_entry(frame, m, n, i, j).abs() - w).abs() > tol {
                return false;
            }
        }
    }
    true
}

/// A known exact ETF for tests: the Mercedes-Benz frame — 3 unit vectors in R² at
/// 120°. Coherence = 1/2 = welch_bound(3,2). Returned row-major (3×2).
pub fn mercedes_benz() -> (Vec<f64>, usize, usize) {
    let two_pi = std::f64::consts::TAU;
    let mut v = Vec::with_capacity(6);
    for k in 0..3 {
        let ang = two_pi * (k as f64) / 3.0;
        v.push(ang.cos());
        v.push(ang.sin());
    }
    (v, 3, 2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mercedes_benz_is_an_etf() {
        let (v, m, n) = mercedes_benz();
        assert!((welch_bound(m, n) - 0.5).abs() < 1e-12);
        assert!((coherence(&v, m, n) - 0.5).abs() < 1e-9);
        assert!(is_etf(&v, m, n, 1e-9));
    }

    #[test]
    fn random_frame_is_not_an_etf() {
        // generic vectors have coherence strictly above the Welch bound → not an ETF.
        let mut rng = crate::fmat::Rng::new(0x5151);
        let (m, n) = (5usize, 2usize);
        let v: Vec<f64> = (0..m * n).map(|_| rng.gaussian()).collect();
        assert!(coherence(&v, m, n) > welch_bound(m, n) + 1e-3);
        assert!(!is_etf(&v, m, n, 1e-3));
    }
}
