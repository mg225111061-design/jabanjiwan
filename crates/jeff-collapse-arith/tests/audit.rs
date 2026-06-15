//! Stage 9: collapser determinism audit (CLAUDE.md PART F, R11). Every collapser uses
//! fixed seeds / deterministic order, so the same input must yield the same outcome
//! (collapse-vs-defer and, on defer, the same barrier tag) across runs. Non-determinism
//! would be a bug (R11).

use jeff_cert::{BarrierTag, CollapseOutcome};
use jeff_collapse_arith::{fourier, geometry, moments, planted, streaming};
use jeff_math::fmat::Rng;

/// A comparable summary of an outcome: (collapsed?, defer tag).
fn summary(o: &CollapseOutcome) -> (bool, Option<BarrierTag>) {
    match o {
        CollapseOutcome::Collapsed(_) => (true, None),
        CollapseOutcome::Defer(d) => (false, Some(d.tag)),
    }
}

fn assert_deterministic(name: &str, mut run: impl FnMut() -> CollapseOutcome) {
    let a = summary(&run());
    let b = summary(&run());
    let c = summary(&run());
    assert_eq!(a, b, "[{name}] run 1 vs 2 differ (non-determinism, R11)");
    assert_eq!(b, c, "[{name}] run 2 vs 3 differ (non-determinism, R11)");
}

#[test]
fn collapsers_are_deterministic() {
    // planted clique (n=50, k=20) — spectral recovery is seed-fixed.
    let n = 50;
    let k = 20;
    let mut rng = Rng::new(0xC11);
    let mut adj = vec![0u8; n * n];
    for i in 0..n {
        for j in (i + 1)..n {
            let e = if rng.gaussian() > 0.0 { 1 } else { 0 };
            adj[i * n + j] = e;
            adj[j * n + i] = e;
        }
    }
    for a in 0..k {
        for b in (a + 1)..k {
            adj[a * n + b] = 1;
            adj[b * n + a] = 1;
        }
    }
    assert_deterministic("planted_clique", || planted::planted_clique(&adj, n, k));

    // spiked tensor (clean rank-1, efficient regime).
    let p = 6;
    let mut tr = Rng::new(0x7);
    let mut v: Vec<f64> = (0..p).map(|_| tr.gaussian()).collect();
    let nrm = v.iter().map(|x| x * x).sum::<f64>().sqrt();
    v.iter_mut().for_each(|x| *x /= nrm);
    let mut t = vec![0.0; p * p * p];
    for i in 0..p {
        for j in 0..p {
            for kk in 0..p {
                t[(i * p + j) * p + kk] = 40.0 * v[i] * v[j] * v[kk];
            }
        }
    }
    assert_deterministic("spiked_tensor", || planted::spiked_tensor(&t, p, 1e-3));

    // moment mixture (point masses).
    let locs = [2.0_f64, 5.0];
    let w = [0.7_f64, 0.3];
    let moments: Vec<f64> = (0..6).map(|tt| w[0] * locs[0].powi(tt) + w[1] * locs[1].powi(tt)).collect();
    assert_deterministic("point_mass_mixture", || moments::point_mass_mixture(&moments, 2, 1e-6));

    // geometry: persistent homology on two clusters.
    let pts = vec![0.0, 0.0, 0.1, 0.0, 10.0, 10.0, 10.1, 10.0];
    let dist = jeff_math::geometry::pairwise_distances(&pts, 4, 2);
    assert_deterministic("persistent_homology", || geometry::persistent_homology(&dist, 4, 1.0));

    // streaming: F2 on a skewed stream.
    let mut s = vec![7u64; 500];
    s.extend((0..200).map(|i| (i % 50) as u64 + 100));
    assert_deterministic("frequency_moment", || streaming::frequency_moment(&s, 2, 0.5));

    // fourier: low-degree on a dictator.
    let dictator = vec![1.0, -1.0, 1.0, -1.0];
    assert_deterministic("low_degree", || fourier::low_degree(&dictator, 1, 1e-9));
}
