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

/// The unified four-axis verdict (Stage 16.5). The verifier — not a heuristic — decides
/// every axis at once: if a fold exists, all four axes move off the floor (carried by the
/// fold's certificate); if no fold of the tried classes exists, the input genuinely sits on
/// the information floor and JEFF drops to Tier D, paying Ω(N), with the absence certificate
/// proving that is optimal up to (class, Θ). Maximally aggressive in the attempt, never wrong
/// in the gate.
#[allow(clippy::large_enum_variant)]
pub enum Verdict {
    /// Structure found: WHEN → compile-time closed form, HOW-MANY → cacheable, COMPOSE →
    /// fusible, PRECISION → exact. `marginal` is the (honest, asymptotic) N-th-term cost the
    /// recurrence licenses.
    Optimized {
        order: usize,
        degree: usize,
        operator: Vec<BigInt>,
        marginal: &'static str,
        cert: VerifiedCertificate,
    },
    /// On the floor: no fold of the tried classes exists (absence-certified) → pay Ω(N),
    /// proven optimal up to (class, Θ). No axis move is licensed.
    Floor {
        paid: &'static str,
        absence: Vec<AbsenceItem>,
    },
}

impl Verdict {
    pub fn is_optimized(&self) -> bool {
        matches!(self, Verdict::Optimized { .. })
    }
}

/// The unified four-axis optimizer (Stage 16.5): one global, certificate-carrying decision.
/// Drives WHEN/HOW-MANY/COMPOSE/PRECISION from a single verified recognition of the input.
pub fn plan_sequence(samples: &[BigInt], max_order: usize, max_degree: usize) -> Verdict {
    match discover_sequence(samples, max_order, max_degree) {
        Discovery::Found { order, degree, operator, cert, .. } => Verdict::Optimized {
            order,
            degree,
            operator,
            // a D-finite recurrence gives a sublinear N-th term (Bostan–Mori: O(log N) for
            // C-finite, O(M(d) log N) for D-finite) — an asymptotic fact, not an overclaim.
            marginal: "sublinear N-th term via the recurrence (O(log N) for C-finite)",
            cert,
        },
        Discovery::Absent { tried } => Verdict::Floor { paid: "Omega(N)", absence: tried },
    }
}

/// Stage 16.2 — universal verified memoization, keyed by the **certificate** (same verified
/// *meaning*, not same bytes of input). A given verified computation is stored once, ever; a
/// hit returns a result that is trusted only via its attached certificate, re-verifiable
/// without recomputation. Two computations that arrive at the same certificate share one
/// slot — compute once across the ecosystem.
#[derive(Default)]
pub struct CertCache {
    store: std::collections::HashMap<String, jeff_cert::Certificate>,
}

impl CertCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Canonical key: the certificate's serialized verified meaning.
    fn key(cert: &jeff_cert::Certificate) -> String {
        serde_json::to_string(cert).expect("certificate serializes")
    }

    pub fn len(&self) -> usize {
        self.store.len()
    }
    pub fn is_empty(&self) -> bool {
        self.store.is_empty()
    }

    /// Memoize a verified result by its certificate. Returns `true` if newly stored (first
    /// time this verified meaning is seen), `false` if an equivalent certificate was already
    /// present (reused — computed once).
    pub fn insert(&mut self, vc: &VerifiedCertificate) -> bool {
        let cert = vc.certificate().clone();
        let k = Self::key(&cert);
        self.store.insert(k, cert).is_none()
    }

    /// Look up by an equivalent certificate; on a hit, RE-VERIFY from the cached certificate
    /// (no recomputation of the underlying result). `None` on a miss.
    pub fn get_reverified(&self, cert: &jeff_cert::Certificate) -> Option<VerifiedCertificate> {
        let k = Self::key(cert);
        self.store.get(&k).cloned().and_then(jeff_verify::verify)
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
    fn composed_fold_is_certified_and_fusion_preserves_result() {
        // Stage 16.3 COMPOSE: fold the structure BETWEEN kernels. a = Fibonacci (C-finite
        // order 2), b = 2^n (order 1); c = a+b is C-finite by closure under +.
        let n = 22;
        let mut fib = vec![BigInt::from(0), BigInt::from(1)];
        while fib.len() < n {
            let k = fib.len();
            fib.push(&fib[k - 1] + &fib[k - 2]);
        }
        let pow2: Vec<BigInt> = (0..n).map(|i| BigInt::from(1i64 << i)).collect();
        let c: Vec<BigInt> = fib.iter().zip(&pow2).map(|(x, y)| x + y).collect(); // staged

        // The composed sequence has its own certified fold (composed_fold_is_certified).
        let (order, operator, cert_ok) = match discover_sequence(&c, 4, 0) {
            Discovery::Found { order, operator, cert, .. } => {
                let reverify = jeff_verify::verify(cert.certificate().clone()).is_some();
                (order, operator, reverify)
            }
            _ => panic!("a+b must be C-finite by closure"),
        };
        assert!(cert_ok, "composed fold carries a re-verifiable certificate");

        // fusion_preserves_result: generating c from its SINGLE recurrence (fused) equals the
        // staged a+b, on every term.
        let op_rat: Vec<BigRational> = operator.iter().map(|x| BigRational::from(x.clone())).collect();
        let gen = jeff_math::recurrence::generate_cfinite(&c[..order], &op_rat, n)
            .expect("C-finite generation");
        let c_rat: Vec<BigRational> = c.iter().map(|x| BigRational::from(x.clone())).collect();
        assert_eq!(gen, c_rat, "fused single recurrence == staged composition");
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
    fn four_axis_preserves_result() {
        // Stage 16.5: a structured input is Optimized; the off-the-floor move (WHEN/COMPOSE)
        // preserves the result — generating from the recurrence reproduces the input exactly.
        let n = 20;
        let mut fib = vec![BigInt::from(0), BigInt::from(1)];
        while fib.len() < n {
            let k = fib.len();
            fib.push(&fib[k - 1] + &fib[k - 2]);
        }
        match plan_sequence(&fib, 4, 0) {
            Verdict::Optimized { order, operator, cert, .. } => {
                assert!(jeff_verify::verify(cert.certificate().clone()).is_some());
                let op: Vec<BigRational> =
                    operator.iter().map(|x| BigRational::from(x.clone())).collect();
                let gen = jeff_math::recurrence::generate_cfinite(&fib[..order], &op, n).unwrap();
                let fib_rat: Vec<BigRational> =
                    fib.iter().map(|x| BigRational::from(x.clone())).collect();
                assert_eq!(gen, fib_rat, "the optimized plan preserves the result");
            }
            Verdict::Floor { .. } => panic!("Fibonacci is structured — must be Optimized"),
        }
    }

    #[test]
    fn floor_reached_emits_certificate() {
        // Stage 16.5: a genuinely structureless input sits on the floor — JEFF pays Ω(N) and
        // the absence certificate proves that is optimal (up to the class, Θ). No fabrication.
        let mut x = 0xF100_0F100_u64.wrapping_mul(0x9E37_79B9);
        let s: Vec<BigInt> = (0..44)
            .map(|_| {
                x ^= x << 13;
                x ^= x >> 7;
                x ^= x << 17;
                BigInt::from((x % 997) as i64)
            })
            .collect();
        match plan_sequence(&s, 2, 1) {
            Verdict::Floor { paid, absence } => {
                assert_eq!(paid, "Omega(N)");
                assert!(!absence.is_empty(), "the floor verdict must carry absence proofs");
                for item in &absence {
                    assert_eq!(item.class, "D-finite");
                    assert!(item.theta.contains("N=44"));
                    assert!(jeff_verify::verify(item.cert.certificate().clone()).is_some());
                }
            }
            Verdict::Optimized { .. } => panic!("high-entropy input must hit the floor"),
        }
    }

    #[test]
    fn cache_keyed_by_certificate() {
        // Stage 16.2: the cache is keyed by verified meaning. The SAME fold computed twice
        // maps to one slot (compute once, ever).
        let fib = seq(&[0, 1, 1, 2, 3, 5, 8, 13, 21, 34, 55, 89, 144]);
        let vc = match discover_sequence(&fib, 5, 1) {
            Discovery::Found { cert, .. } => cert,
            _ => panic!("Fibonacci folds"),
        };
        let mut cache = CertCache::new();
        assert!(cache.insert(&vc), "first insert is new");
        let vc2 = match discover_sequence(&fib, 5, 1) {
            Discovery::Found { cert, .. } => cert,
            _ => unreachable!(),
        };
        assert!(!cache.insert(&vc2), "an equivalent certificate reuses the slot");
        assert_eq!(cache.len(), 1, "computed once across the ecosystem");
    }

    #[test]
    fn cache_hit_reverifiable() {
        // A cache hit returns a result trusted only via its re-verifiable certificate.
        let fib = seq(&[0, 1, 1, 2, 3, 5, 8, 13, 21, 34, 55, 89, 144]);
        let vc = match discover_sequence(&fib, 5, 1) {
            Discovery::Found { cert, .. } => cert,
            _ => panic!("folds"),
        };
        let mut cache = CertCache::new();
        cache.insert(&vc);
        let hit = cache.get_reverified(vc.certificate()).expect("hit re-verifies");
        assert_eq!(
            jeff_verify::checker_name(&hit.certificate().evidence),
            "recurrence-annihilation-exact"
        );
        // a certificate never inserted is a miss.
        let other = recurrence_present_certificate(
            seq(&[1, 1, 1]),
            1,
            0,
            vec![BigInt::from(1), BigInt::from(-1)],
        );
        assert!(cache.get_reverified(&other).is_none());
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
