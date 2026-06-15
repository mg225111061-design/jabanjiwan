//! Stage 15.2 — the meta-recognizer (algorithm-selection portfolio).
//!
//! Given an integer sequence, run cheap O(N) feature probes, route to discovery engines in
//! increasing expected cost, and stop at the first **verified** fold. If no engine folds
//! within the parameter ceiling, return the union of class-labeled **structure-absence**
//! certificates (the HONEST_DEFER payload). Every result is gated by the verifier: a fold
//! carries a re-checkable presence certificate, an absence carries a (class, Θ) label.

use jeff_cert::{
    recurrence_absence_certificate, recurrence_present_certificate, VerifiedCertificate,
};
use jeff_math::recurrence::{excludes_recurrence, fit_recurrence, surplus};
use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::Zero;

/// Cheap gating probes (O(N·k)).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Features {
    /// Smallest order `k` whose `k`-th finite differences vanish (⇒ polynomial of degree
    /// `k−1`, a C-finite signature). `None` if none up to the order ceiling.
    pub cfinite_order: Option<usize>,
}

/// A labeled structure-absence (class, Θ) plus its verified certificate.
pub struct AbsenceItem {
    pub class: &'static str,
    pub theta: String,
    pub cert: VerifiedCertificate,
}

/// The meta-recognizer's verdict. `Found` carries a `VerifiedCertificate` (large), but a
/// `Discovery` is produced once per `discover_sequence` call and never hot-path-copied, so
/// boxing it would add indirection for no benefit (cf. `jeff_cert::Evidence`).
#[allow(clippy::large_enum_variant)]
pub enum Discovery {
    /// A verified fold: `operator` annihilates the data exactly (integer-scaled), with its
    /// presence certificate.
    Found {
        engine: &'static str,
        order: usize,
        degree: usize,
        operator: Vec<BigInt>,
        cert: VerifiedCertificate,
    },
    /// No fold within the (max_order, max_degree) ceiling; the union of absence proofs.
    Absent { tried: Vec<AbsenceItem> },
}

impl Discovery {
    pub fn is_found(&self) -> bool {
        matches!(self, Discovery::Found { .. })
    }
}

/// Finite-difference probe: smallest `k ≤ max_order` with all `k`-th differences zero.
fn probe(samples: &[BigInt], max_order: usize) -> Features {
    let mut d: Vec<BigInt> = samples.to_vec();
    let mut order = None;
    for k in 1..=max_order {
        if d.len() < 2 {
            break;
        }
        d = d.windows(2).map(|w| &w[1] - &w[0]).collect();
        if d.iter().all(|x| x.is_zero()) {
            order = Some(k);
            break;
        }
    }
    Features { cfinite_order: order }
}

/// gcd of two non-negative BigInts.
fn gcd(a: &BigInt, b: &BigInt) -> BigInt {
    if b.is_zero() {
        a.clone()
    } else {
        gcd(b, &(a % b))
    }
}

/// Scale a rational operator to primitive integer coefficients (denominators are positive in
/// `BigRational`); homogeneity preserves annihilation.
fn clear_denoms(op: &[BigRational]) -> Vec<BigInt> {
    let mut lcm = BigInt::from(1);
    for c in op {
        let den = c.denom().clone();
        let g = gcd(&lcm, &den);
        lcm = &lcm / &g * &den;
    }
    let ints: Vec<BigInt> = op.iter().map(|c| c.numer() * (&lcm / c.denom())).collect();
    // reduce by content (gcd of all coeffs) so the operator is primitive.
    let mut content = BigInt::from(0);
    for v in &ints {
        let a = if v.sign() == num_bigint::Sign::Minus { -v.clone() } else { v.clone() };
        content = gcd(&content, &a);
    }
    if content.is_zero() {
        ints
    } else {
        ints.iter().map(|v| v / &content).collect()
    }
}

/// Cost-ordered (order, degree, engine) schedule, polynomial signature first.
fn schedule(feats: &Features, max_order: usize, max_degree: usize) -> Vec<(usize, usize, &'static str)> {
    let mut sched: Vec<(usize, usize, &'static str)> = Vec::new();
    if let Some(k) = feats.cfinite_order {
        sched.push((k, 0, "polynomial")); // exact C-finite order from the diff probe
    }
    let mut rest: Vec<(usize, usize, &'static str)> = Vec::new();
    for r in 1..=max_order {
        for d in 0..=max_degree {
            let engine = if d == 0 { "c-finite" } else { "d-finite" };
            rest.push((r, d, engine));
        }
    }
    // increasing unknown count (r+1)(d+1), then order, then degree.
    rest.sort_by_key(|&(r, d, _)| ((r + 1) * (d + 1), r, d));
    sched.extend(rest);
    sched
}

/// Run the portfolio: first verified fold, else the union of absence certificates.
pub fn discover_sequence(samples: &[BigInt], max_order: usize, max_degree: usize) -> Discovery {
    let feats = probe(samples, max_order);
    let mut seen = std::collections::BTreeSet::new();
    for (r, d, engine) in schedule(&feats, max_order, max_degree) {
        if !seen.insert((r, d)) {
            continue; // skip duplicates (the probe hint may coincide with the sweep)
        }
        if let Some(op_rat) = fit_recurrence(samples, r, d) {
            if !op_rat.iter().all(|c| c.is_zero()) {
                let op_int = clear_denoms(&op_rat);
                let cert = recurrence_present_certificate(samples.to_vec(), r, d, op_int.clone());
                if let Some(vc) = jeff_verify::verify(cert) {
                    return Discovery::Found { engine, order: r, degree: d, operator: op_int, cert: vc };
                }
            }
        }
    }
    // No fold ⇒ assemble the absence union over the C-finite and D-finite ceiling boxes.
    let mut tried = Vec::new();
    let mut boxes = vec![(max_order, 0usize)];
    if max_degree > 0 {
        boxes.push((max_order, max_degree));
    }
    for (r, d) in boxes {
        if surplus(samples.len(), r, d) >= 0 && excludes_recurrence(samples, r, d) {
            if let Some(vc) = jeff_verify::verify(recurrence_absence_certificate(samples.to_vec(), r, d)) {
                let (class, theta) = vc.certificate().evidence.absence_label().expect("absence labeled");
                tried.push(AbsenceItem { class, theta, cert: vc });
            }
        }
    }
    Discovery::Absent { tried }
}

/// A cached, verified recognition outcome (Stage 15.4 / 16.2): a discovered fold or a
/// labeled absence, both carrying a re-checkable certificate.
pub struct CachedFold {
    pub found: bool,
    pub cert: VerifiedCertificate,
}

/// The self-improving discover→verify→cache→recognize loop (Stage 15.4). The first
/// encounter pays the full portfolio cost; subsequent encounters of the same input are
/// recognized by an O(1) content-addressed lookup that returns the *same verified
/// certificate* — JEFF's recognized family grows over time. (Content-addressed by meaning
/// is also Stage 16.2 universal memoization.)
#[derive(Default)]
pub struct FoldCache {
    map: std::collections::HashMap<String, CachedFold>,
    pub misses: u64,
    pub hits: u64,
}

impl FoldCache {
    pub fn new() -> Self {
        Self::default()
    }

    fn key(samples: &[BigInt], r: usize, d: usize) -> String {
        let mut s = format!("{r}:{d}:");
        for x in samples {
            s.push_str(&x.to_string());
            s.push(',');
        }
        s
    }

    /// Recognize `samples` via the cache; on a miss, run the portfolio and cache the
    /// verified certificate. `None` only if the portfolio produced neither a fold nor any
    /// absence certificate (e.g. an underdetermined ceiling) — nothing verifiable to cache.
    pub fn recognize(&mut self, samples: &[BigInt], r: usize, d: usize) -> Option<&CachedFold> {
        let k = Self::key(samples, r, d);
        if !self.map.contains_key(&k) {
            self.misses += 1;
            let entry = match discover_sequence(samples, r, d) {
                Discovery::Found { cert, .. } => Some(CachedFold { found: true, cert }),
                Discovery::Absent { mut tried } => {
                    tried.pop().map(|item| CachedFold { found: false, cert: item.cert })
                }
            };
            match entry {
                Some(e) => {
                    self.map.insert(k.clone(), e);
                }
                None => return None,
            }
        } else {
            self.hits += 1;
        }
        self.map.get(&k)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seq(v: &[i64]) -> Vec<BigInt> {
        v.iter().map(|&x| BigInt::from(x)).collect()
    }

    #[test]
    fn probe_routes_correctly() {
        // n^2 is a polynomial of degree 2 ⇒ 3rd differences vanish ⇒ probe order 3 ⇒ folded
        // by the polynomial/C-finite engine (degree 0).
        let sq: Vec<BigInt> = (0..16i64).map(|n| BigInt::from(n * n)).collect();
        assert_eq!(probe(&sq, 6).cfinite_order, Some(3));
        match discover_sequence(&sq, 6, 2) {
            Discovery::Found { engine, order, degree, .. } => {
                assert_eq!(degree, 0, "polynomial ⇒ constant coefficients");
                assert_eq!(order, 3);
                assert!(engine == "polynomial" || engine == "c-finite");
            }
            _ => panic!("n^2 must be discovered"),
        }
    }

    #[test]
    fn portfolio_stops_at_first_fold() {
        // Fibonacci folds at the cheapest C-finite (2,0); the portfolio must stop there.
        let fib = seq(&[0, 1, 1, 2, 3, 5, 8, 13, 21, 34, 55, 89, 144, 233, 377, 610]);
        match discover_sequence(&fib, 5, 2) {
            Discovery::Found { order, degree, cert, .. } => {
                assert_eq!((order, degree), (2, 0), "stop at the cheapest fold");
                assert_eq!(
                    jeff_verify::checker_name(&cert.certificate().evidence),
                    "recurrence-annihilation-exact"
                );
            }
            _ => panic!("Fibonacci must be discovered"),
        }
    }

    #[test]
    fn defer_emits_absence_certificate() {
        // A high-entropy sequence folds nowhere ⇒ Absent with labeled absence certificates.
        let mut x = 0xDEAD_BEEF_1234_5678u64;
        let s: Vec<BigInt> = (0..44)
            .map(|_| {
                x ^= x << 13;
                x ^= x >> 7;
                x ^= x << 17;
                BigInt::from((x % 1000) as i64)
            })
            .collect();
        match discover_sequence(&s, 2, 1) {
            Discovery::Absent { tried } => {
                assert!(!tried.is_empty(), "defer must carry absence proofs");
                for item in &tried {
                    assert_eq!(item.class, "D-finite");
                    assert!(item.theta.contains("N=44"));
                    // the certificate re-verifies independently.
                    let c = item.cert.certificate().clone();
                    assert!(jeff_verify::verify(c).is_some());
                }
            }
            _ => panic!("high-entropy sequence must defer with absence proofs"),
        }
    }

    #[test]
    fn cached_rule_carries_certificate() {
        // A cached recognition carries a re-verifiable certificate (fold or absence).
        let mut cache = FoldCache::new();
        let fib = seq(&[0, 1, 1, 2, 3, 5, 8, 13, 21, 34, 55, 89, 144, 233]);
        let entry = cache.recognize(&fib, 5, 1).expect("Fibonacci recognized");
        assert!(entry.found);
        let c = entry.cert.certificate().clone();
        assert!(jeff_verify::verify(c).is_some(), "cached cert must re-verify");
    }

    #[test]
    fn cached_fold_recognized_cheaply() {
        // Second encounter ≥100× faster than rediscovery (O(1) lookup vs exact rref).
        let mut x = 0x0BADC0DE_F00D_1234u64;
        let s: Vec<BigInt> = (0..40)
            .map(|_| {
                x ^= x << 13;
                x ^= x >> 7;
                x ^= x << 17;
                BigInt::from((x % 1009) as i64)
            })
            .collect();
        // cost of rediscovery (fresh portfolio each time), averaged.
        let reps = 5;
        let t0 = std::time::Instant::now();
        for _ in 0..reps {
            let _ = discover_sequence(&s, 2, 1);
        }
        let rediscover = t0.elapsed().as_nanos() as f64 / reps as f64;

        // warm the cache, then time cache hits.
        let mut cache = FoldCache::new();
        let _ = cache.recognize(&s, 2, 1); // miss (populate)
        let hit_reps = 1000;
        let t1 = std::time::Instant::now();
        for _ in 0..hit_reps {
            let _ = cache.recognize(&s, 2, 1);
        }
        let hit = t1.elapsed().as_nanos() as f64 / hit_reps as f64;
        assert_eq!(cache.hits, hit_reps, "all subsequent calls are hits");
        assert!(
            hit * 100.0 < rediscover,
            "cache hit ({hit:.0}ns) must be ≥100× faster than rediscovery ({rediscover:.0}ns)"
        );
    }
}
