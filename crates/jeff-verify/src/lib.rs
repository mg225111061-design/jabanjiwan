//! Certificate checkers and the verification entry point.
//!
//! Authority: CLAUDE.md PART 6.2 (checker routing), PART 11, APPENDIX F (proof
//! walkthroughs), R2 (checker before collapser), R23/R31 (timeout/Unknown →
//! fallback), DR1 (certificates must *actually* pass a real check).
//!
//! # Honesty note on solvers (DR2/DR3)
//!
//! The constitution names Z3 as the default checker and Lean for holonomic operator
//! induction. Neither is wired in this environment. Instead every evidence kind is
//! discharged by an **exact, in-house, deterministic** check — the quantifier-free
//! coefficient-zero variant the constitution itself endorses as "more robust"
//! (APPENDIX F.1), GF(2) basis evaluation (F.3), Cayley–Hamilton ring identity
//! (F.5), and exact modular/integer replay (F.6). These are *sound* (they only
//! return `Valid` when the identity provably holds) and *terminating* (no external
//! process, no hang — R23). Where a Z3/Lean path would add nothing over the exact
//! check, we say so rather than pretend a solver ran (DR2). See `checker_name`.

use jeff_cert::{
    verify_with, Boundary, Certificate, Checker, Evidence, Obligation, ReplayKind, VerifiedCertificate,
    VerifyResult,
};
use jeff_math::modular::ModInt;
use jeff_math::{ModMatrix, RatMatrix};
use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::Zero;
use std::collections::BTreeMap;

/// Safety cap for exact replay loops, so a checker can never hang (R23). Beyond
/// this, replay returns `Unknown` → the collapse falls back (R31) rather than
/// blocking. Test fixtures stay well under this.
const REPLAY_CAP: u64 = 5_000_000;

/// Exact polynomial-identity checker (the "Z3Checker" role, F.1/F.4/F.5).
/// Discharges `PolynomialIdentity`, `Telescoper` (identity part), and
/// `EigenCharpoly` (Cayley–Hamilton).
pub struct PolyChecker;

impl Checker for PolyChecker {
    fn check(&self, ev: &Evidence, _ob: &Obligation, _b: &[Boundary]) -> VerifyResult {
        match ev {
            Evidence::PolynomialIdentity { poly } => {
                // The difference polynomial must be identically zero (every
                // coefficient cancels). This is exact and sound (F.1).
                if poly.is_zero() {
                    VerifyResult::Valid
                } else {
                    VerifyResult::Invalid
                }
            }
            Evidence::Telescoper { term, l, r } => {
                // Independently recompute the telescoper certificate numerator from
                // (F, L, R) and confirm it is identically zero (F.2). The checker
                // does the work, not the collapser (R2/R25): a wrong (L,R) fails.
                if jeff_math::hyper::telescoper_holds(term, l, r) {
                    VerifyResult::Valid
                } else {
                    VerifyResult::Invalid
                }
            }
            Evidence::EigenCharpoly { matrix } => {
                // Cayley–Hamilton: charpoly(A) annihilates A, exactly (F.5).
                if matrix.satisfies_charpoly() {
                    VerifyResult::Valid
                } else {
                    VerifyResult::Invalid
                }
            }
            _ => VerifyResult::Unknown, // not my evidence kind
        }
    }
}

/// GF(2) linearity checker (F.3). Re-evaluates the captured circuit on the affine
/// basis `{0, e_1, ..., e_n}` and confirms it reconstructs the certified `(M, b)`.
/// Sound because the circuit is structurally linear (only XOR/NOT/const gates), so
/// agreement on an affine-spanning set implies agreement everywhere (E.3 note).
pub struct Gf2Checker;

impl Checker for Gf2Checker {
    fn check(&self, ev: &Evidence, _ob: &Obligation, _b: &[Boundary]) -> VerifyResult {
        let Evidence::Gf2LinearIdentity { circuit, m, b } = ev else {
            return VerifyResult::Unknown;
        };
        let n = circuit.n_inputs;
        if m.cols_n != n || m.rows != circuit.n_outputs() || b.n != circuit.n_outputs() {
            return VerifyResult::Invalid;
        }
        // b must be circuit(0)
        let zero_in = vec![false; n];
        let c0 = circuit.eval(&zero_in);
        for (i, bit) in c0.iter().enumerate() {
            if *bit != b.get(i) {
                return VerifyResult::Invalid;
            }
        }
        // for each input i: circuit(e_i) must equal col_i XOR b
        for col in 0..n {
            let mut ei = vec![false; n];
            ei[col] = true;
            let out = circuit.eval(&ei);
            let column = &m.cols[col];
            for (row, bit) in out.iter().enumerate() {
                let expected = column.get(row) ^ b.get(row);
                if *bit != expected {
                    return VerifyResult::Invalid;
                }
            }
        }
        VerifyResult::Valid
    }
}

/// Exact numeric/Pfaffian replay checker (F.6, E.5). Recomputes the claimed result
/// by an independent exact method and compares. Bounded by [`REPLAY_CAP`] (R23).
pub struct ReplayChecker;

impl Checker for ReplayChecker {
    fn check(&self, ev: &Evidence, _ob: &Obligation, _b: &[Boundary]) -> VerifyResult {
        match ev {
            Evidence::NumericResidual { replay } => self.check_replay(replay),
            Evidence::PfaffianHolant { witness } => {
                // Replay: det(skew) == pfaffian^2 (FKT, E.5). Exact over Q.
                let m = RatMatrix {
                    rows: witness.dim,
                    cols: witness.dim,
                    data: witness
                        .skew
                        .iter()
                        .map(|&v| BigRational::from(BigInt::from(v)))
                        .collect(),
                };
                let det = m.det();
                let pf = BigRational::from(BigInt::from(witness.claimed_pfaffian));
                if det == &pf * &pf {
                    VerifyResult::Valid
                } else {
                    VerifyResult::Invalid
                }
            }
            _ => VerifyResult::Unknown,
        }
    }
}

impl ReplayChecker {
    fn check_replay(&self, replay: &ReplayKind) -> VerifyResult {
        match replay {
            ReplayKind::MatrixPowerMod {
                matrix,
                dim,
                q,
                exp,
                claimed,
            } => {
                if *exp > REPLAY_CAP {
                    return VerifyResult::Unknown; // would hang → fallback (R31)
                }
                let a = ModMatrix::from_u64(*dim, *q, matrix);
                let got = a.pow_naive(*exp); // independent of fast pow
                let want: Vec<u64> = claimed.iter().map(|&v| v % q).collect();
                if got.data == want {
                    VerifyResult::Valid
                } else {
                    VerifyResult::Invalid
                }
            }
            ReplayKind::LinearRecTerm {
                rec,
                init,
                modulus,
                index,
                claimed,
            } => {
                if *index > REPLAY_CAP {
                    return VerifyResult::Unknown;
                }
                let got = unroll_linrec(rec, init, *modulus, *index);
                if got == claimed % modulus {
                    VerifyResult::Valid
                } else {
                    VerifyResult::Invalid
                }
            }
            ReplayKind::Convolution {
                a,
                b,
                q,
                claimed,
                ..
            } => {
                // Independent check: naive cyclic convolution (does not use NTT).
                let n = a.len();
                if n == 0 || b.len() != n || claimed.len() != n {
                    return VerifyResult::Invalid;
                }
                let mut want = vec![0u64; n];
                for (i, &ai) in a.iter().enumerate() {
                    for (j, &bj) in b.iter().enumerate() {
                        let k = (i + j) % n;
                        want[k] = ((want[k] as u128 + ai as u128 * bj as u128) % *q as u128) as u64;
                    }
                }
                let got: Vec<u64> = claimed.iter().map(|&v| v % q).collect();
                if got == want {
                    VerifyResult::Valid
                } else {
                    VerifyResult::Invalid
                }
            }
            ReplayKind::NegacyclicConvolution { a, b, q, claimed } => {
                // Independent oracle (AR-4): the Θ(n²) schoolbook negacyclic product.
                // Does not use the NTT, so a buggy/forged fast result is caught.
                let n = a.len();
                if n == 0 || b.len() != n || claimed.len() != n || *q <= 1 {
                    return VerifyResult::Invalid;
                }
                let want = jeff_math::pqc::schoolbook_negacyclic(a, b, *q);
                let got: Vec<u64> = claimed.iter().map(|&v| v % q).collect();
                if got == want {
                    VerifyResult::Valid
                } else {
                    VerifyResult::Invalid
                }
            }
            ReplayKind::SampleAgreement { samples } => {
                for s in samples {
                    let mut env: BTreeMap<String, BigRational> = BTreeMap::new();
                    for (name, val) in s.var_names.iter().zip(&s.inputs) {
                        env.insert(name.clone(), BigRational::from(BigInt::from(*val)));
                    }
                    let Some(v) = s.closed_form.eval(&env) else {
                        return VerifyResult::Unknown;
                    };
                    if v != BigRational::from(BigInt::from(s.expected)) {
                        return VerifyResult::Invalid;
                    }
                }
                VerifyResult::Valid
            }
        }
    }
}

/// Unroll a linear recurrence `a_n = Σ_i rec[i] * a_{n-1-i}` (mod m) with `init`
/// as `a_0, a_1, ...`. Exact modular arithmetic.
fn unroll_linrec(rec: &[i64], init: &[i64], modulus: u64, index: u64) -> u64 {
    let m = modulus;
    let red = |x: i64| -> u64 { ((x % m as i64 + m as i64) % m as i64) as u64 };
    if (index as usize) < init.len() {
        return red(init[index as usize]);
    }
    let mut window: Vec<u64> = init.iter().map(|&x| red(x)).collect();
    // ensure window length >= rec.len()
    for i in init.len()..=(index as usize) {
        let mut acc = ModInt::zero(m);
        for (j, &c) in rec.iter().enumerate() {
            if i > j {
                let term = ModInt::new(red(c), m) * ModInt::new(window[i - 1 - j], m);
                acc = acc + term;
            }
        }
        window.push(acc.val);
    }
    window[index as usize]
}

/// Stage-3 Tier-S numeric-kernel checker. Exact wherever possible (matrix inverse
/// `=I`, LDLᵀ `=A` + SPD, Freivalds over ℤ); float only via an explicit-tol residual.
pub struct KernelChecker;

impl Checker for KernelChecker {
    fn check(&self, ev: &Evidence, _ob: &Obligation, _b: &[Boundary]) -> VerifyResult {
        match ev {
            Evidence::MatrixInverse { m, inv } => {
                if m.rows != m.cols || inv.rows != inv.cols || m.rows != inv.rows {
                    return VerifyResult::Invalid;
                }
                // exact: m · inv == I
                if m.mul(inv) == RatMatrix::identity(m.rows) {
                    VerifyResult::Valid
                } else {
                    VerifyResult::Invalid
                }
            }
            Evidence::LdltSpd { a, l, d } => {
                if a.rows != a.cols || l.rows != l.cols || a.rows != l.rows || d.len() != a.rows {
                    return VerifyResult::Invalid;
                }
                let recon = RatMatrix::from_ldlt(l, d);
                let spd = d.iter().all(|x| *x > BigRational::zero());
                if spd && recon == *a {
                    VerifyResult::Valid
                } else {
                    VerifyResult::Invalid
                }
            }
            Evidence::FreivaldsProduct { a, b, c, dim, seeds } => {
                let n = *dim;
                if a.len() != n * n || b.len() != n * n || c.len() != n * n || seeds.is_empty() {
                    return VerifyResult::Invalid;
                }
                let am = jeff_math::IntMatrix { n, data: a.clone() };
                let bm = jeff_math::IntMatrix { n, data: b.clone() };
                let cm = jeff_math::IntMatrix { n, data: c.clone() };
                // exact Freivalds over ℤ with the recorded {0,1} vectors (R11 replay)
                if am.freivalds(&bm, &cm, seeds) {
                    VerifyResult::Valid
                } else {
                    VerifyResult::Invalid
                }
            }
            Evidence::FloatResidual { m, inv, dim, tol } => {
                let n = *dim;
                // a NaN or negative tolerance is not a valid contract.
                if m.len() != n * n || inv.len() != n * n || tol.is_nan() || *tol < 0.0 {
                    return VerifyResult::Invalid;
                }
                // ‖m·inv − I‖∞ ≤ tol (soundness relative to the stated tol).
                let mut worst = 0.0f64;
                for i in 0..n {
                    for j in 0..n {
                        let mut s = 0.0f64;
                        for k in 0..n {
                            s += m[i * n + k] * inv[k * n + j];
                        }
                        let target = if i == j { 1.0 } else { 0.0 };
                        worst = worst.max((s - target).abs());
                    }
                }
                if worst <= *tol {
                    VerifyResult::Valid
                } else {
                    VerifyResult::Invalid
                }
            }
            // ---- Tier-A: residual ≤ tol, residual recomputed independently ----
            Evidence::LowRankResidual {
                a,
                approx,
                rows,
                cols,
                tol,
            } => {
                let (r, c) = (*rows, *cols);
                if a.len() != r * c || approx.len() != r * c || !tol_ok(*tol) {
                    return VerifyResult::Invalid;
                }
                let am = jeff_math::fmat::FMat::from_data(r, c, a.clone());
                let bm = jeff_math::fmat::FMat::from_data(r, c, approx.clone());
                if am.sub(&bm).frob_norm() <= *tol {
                    VerifyResult::Valid
                } else {
                    VerifyResult::Invalid
                }
            }
            Evidence::FmmResidual {
                points,
                charges,
                kernel,
                phi,
                tol,
            } => {
                if points.len() != charges.len() || phi.len() != points.len() || !tol_ok(*tol) {
                    return VerifyResult::Invalid;
                }
                // recompute the exact O(N²) direct sum — the ground truth.
                let exact = jeff_math::nbody::direct_sum(points, charges, *kernel);
                if jeff_math::nbody::max_abs_diff(phi, &exact) <= *tol {
                    VerifyResult::Valid
                } else {
                    VerifyResult::Invalid
                }
            }
            Evidence::LinSolveResidual {
                entries,
                dim,
                b,
                x,
                tol,
            } => {
                if b.len() != *dim || x.len() != *dim || !tol_ok(*tol) {
                    return VerifyResult::Invalid;
                }
                let mut a = jeff_math::fmat::Sparse::new(*dim);
                for &(i, j, v) in entries {
                    if i >= *dim || j >= *dim {
                        return VerifyResult::Invalid;
                    }
                    a.push(i, j, v);
                }
                if jeff_math::fmat::lin_residual(&a, x, b) <= *tol {
                    VerifyResult::Valid
                } else {
                    VerifyResult::Invalid
                }
            }
            Evidence::SinkhornPlan {
                cost,
                a,
                b,
                eps,
                f,
                g,
                tol,
            } => {
                let m = a.len();
                let n = b.len();
                if cost.len() != m * n || f.len() != m || g.len() != n || !tol_ok(*tol) || *eps <= 0.0
                {
                    return VerifyResult::Invalid;
                }
                let plan = jeff_math::ot::plan_from_potentials(cost, f, g, *eps, m, n);
                if !plan.iter().all(|x| x.is_finite()) {
                    return VerifyResult::Invalid; // underflow/overflow → not a valid plan
                }
                if jeff_math::ot::marginal_residual(&plan, a, b, m, n) <= *tol {
                    VerifyResult::Valid
                } else {
                    VerifyResult::Invalid
                }
            }
            Evidence::AreStabilizing {
                a,
                b,
                q,
                r,
                x,
                n,
                m,
                tol,
            } => {
                use jeff_math::fmat::FMat;
                let (nn, mm) = (*n, *m);
                if a.len() != nn * nn
                    || q.len() != nn * nn
                    || x.len() != nn * nn
                    || b.len() != nn * mm
                    || r.len() != mm * mm
                    || !tol_ok(*tol)
                {
                    return VerifyResult::Invalid;
                }
                let am = FMat::from_data(nn, nn, a.clone());
                let bm = FMat::from_data(nn, mm, b.clone());
                let qm = FMat::from_data(nn, nn, q.clone());
                let rm = FMat::from_data(mm, mm, r.clone());
                let xm = FMat::from_data(nn, nn, x.clone());
                // (1) residual ≤ tol, (2) X PSD, (3) closed loop Hurwitz — all required.
                let Some(res) = jeff_math::riccati::care_residual(&am, &bm, &qm, &rm, &xm) else {
                    return VerifyResult::Invalid;
                };
                if res > *tol || !jeff_math::riccati::is_psd(&xm, *tol) {
                    return VerifyResult::Invalid;
                }
                let Some(acl) = jeff_math::riccati::closed_loop(&am, &bm, &rm, &xm) else {
                    return VerifyResult::Invalid;
                };
                if jeff_math::riccati::is_hurwitz(&acl) {
                    VerifyResult::Valid
                } else {
                    VerifyResult::Invalid
                }
            }
            Evidence::LatticeCount { cs, qp, n_lo, n_hi } => {
                // coverage: residue classes 0..period partition the parameter line.
                if qp.period < 1 || qp.polys.len() != qp.period as usize || n_hi < n_lo {
                    return VerifyResult::Invalid;
                }
                // F.4 sample enumeration: exact brute force must equal qp.eval(n).
                for n in *n_lo..=*n_hi {
                    match cs.count(n) {
                        Some(c) => {
                            if c != qp.eval(n) {
                                return VerifyResult::Invalid;
                            }
                        }
                        None => return VerifyResult::Unknown, // unbounded/over-cap → fallback
                    }
                }
                VerifyResult::Valid
            }

            // ---- Batch 1: independent residual recompute against the exact oracle ----
            Evidence::SparseRecovery {
                phi,
                rows,
                cols,
                y,
                x,
                k,
                tol,
                ..
            } => {
                if phi.len() != rows * cols || y.len() != *rows || x.len() != *cols || tol.is_nan() {
                    return VerifyResult::Invalid;
                }
                // ‖Φx − y‖₂ recomputed independently
                let mut resid2 = 0.0;
                for i in 0..*rows {
                    let mut ax = 0.0;
                    for j in 0..*cols {
                        ax += phi[i * cols + j] * x[j];
                    }
                    let d = ax - y[i];
                    resid2 += d * d;
                }
                let nnz = x.iter().filter(|v| v.abs() > 1e-9).count();
                if resid2.sqrt() <= *tol && nnz <= *k {
                    VerifyResult::Valid
                } else {
                    VerifyResult::Invalid
                }
            }
            Evidence::SparseSpectrum {
                signal,
                support,
                n,
                k,
                tol,
            } => {
                if signal.len() != *n || support.len() > *k || tol.is_nan() {
                    return VerifyResult::Invalid;
                }
                let recon = jeff_math::recovery::idft_sparse(support, *n);
                let mut resid2 = 0.0;
                let mut sig2 = 0.0;
                for t in 0..*n {
                    let d = signal[t] - recon[t];
                    resid2 += d * d;
                    sig2 += signal[t] * signal[t];
                }
                if resid2.sqrt() <= *tol * sig2.sqrt() {
                    VerifyResult::Valid
                } else {
                    VerifyResult::Invalid
                }
            }
            Evidence::MatrixCompletion {
                observed,
                u,
                v,
                rows,
                cols,
                r,
                tol,
            } => {
                if u.len() != rows * r || v.len() != cols * r || tol.is_nan() {
                    return VerifyResult::Invalid;
                }
                // residual on observed entries: (UVᵀ)_{ij} recomputed
                let mut resid2 = 0.0;
                for &(i, j, val) in observed {
                    if i >= *rows || j >= *cols {
                        return VerifyResult::Invalid;
                    }
                    let mut e = 0.0;
                    for l in 0..*r {
                        e += u[i * r + l] * v[j * r + l];
                    }
                    let d = e - val;
                    resid2 += d * d;
                }
                if resid2.sqrt() <= *tol {
                    VerifyResult::Valid
                } else {
                    VerifyResult::Invalid
                }
            }
            Evidence::PronyRecurrence { samples, a, tol } => {
                if a.len() < 2 || tol.is_nan() {
                    return VerifyResult::Invalid;
                }
                // a_k must be normalized to 1 and the recurrence must hold within tol.
                if (a[a.len() - 1] - 1.0).abs() > 1e-9 {
                    return VerifyResult::Invalid;
                }
                if jeff_math::prony::recurrence_residual(samples, a) <= *tol {
                    VerifyResult::Valid
                } else {
                    VerifyResult::Invalid
                }
            }
            Evidence::SuperResolution {
                lowpass,
                spikes,
                fc,
                tol,
            } => {
                use jeff_math::complex::Complex;
                if lowpass.len() != 2 * fc + 1 || tol.is_nan() || *fc == 0 {
                    return VerifyResult::Invalid;
                }
                let model: Vec<(f64, Complex)> = spikes
                    .iter()
                    .map(|&(t, re, im)| (t, Complex::new(re, im)))
                    .collect();
                // residual: re-evaluate the spike model at every Fourier index
                let mut worst = 0.0_f64;
                for (m, &(re, im)) in lowpass.iter().enumerate() {
                    let got = jeff_math::prony::eval_spike_model(&model, m);
                    worst = worst.max(got.sub(Complex::new(re, im)).abs());
                }
                // separation must meet the Δ ≥ 2/f_c threshold (Candès–FG 2014)
                let locs: Vec<f64> = spikes.iter().map(|s| s.0).collect();
                let sep = jeff_math::prony::min_circular_separation(&locs);
                let threshold = 2.0 / *fc as f64;
                if worst <= *tol && sep >= threshold {
                    VerifyResult::Valid
                } else {
                    VerifyResult::Invalid
                }
            }
            Evidence::EquiangularTightFrame { frame, m, n, tol } => {
                if frame.len() != m * n || *m <= *n || tol.is_nan() {
                    return VerifyResult::Invalid;
                }
                if jeff_math::frame::is_etf(frame, *m, *n, *tol) {
                    VerifyResult::Valid
                } else {
                    VerifyResult::Invalid
                }
            }

            // ---- Batch 2: planted / spiked detection ----
            Evidence::SpikedCovariance { cov, p, threshold, gap } => {
                if cov.len() != p * p || threshold.is_nan() || gap.is_nan() {
                    return VerifyResult::Invalid;
                }
                let (l1, l2) = jeff_math::planted::top_two_eigenvalues(cov, *p);
                if l1 >= *threshold && (l1 - l2) >= *gap {
                    VerifyResult::Valid
                } else {
                    VerifyResult::Invalid
                }
            }
            Evidence::PlantedClique { adj, n, clique, k } => {
                if adj.len() != n * n {
                    return VerifyResult::Invalid;
                }
                // EXACT: the returned set is literally a clique of size ≥ k.
                if clique.len() >= *k && jeff_math::planted::is_clique(adj, *n, clique) {
                    VerifyResult::Valid
                } else {
                    VerifyResult::Invalid
                }
            }
            Evidence::SbmCommunity { adj, n, threshold } => {
                if adj.len() != n * n || threshold.is_nan() {
                    return VerifyResult::Invalid;
                }
                let (l2, _) = jeff_math::planted::sbm_detect(adj, *n);
                if l2 >= *threshold {
                    VerifyResult::Valid
                } else {
                    VerifyResult::Invalid
                }
            }
            Evidence::SpikedTensor { tensor, p, v, beta, threshold, tol } => {
                if tensor.len() != p * p * p || v.len() != *p || tol.is_nan() {
                    return VerifyResult::Invalid;
                }
                let (sigma, _) = jeff_math::planted::tensor_unfold_top(tensor, *p);
                let resid = jeff_math::planted::rank1_tensor_residual(tensor, v, *beta, *p);
                if sigma >= *threshold && resid <= *tol {
                    VerifyResult::Valid
                } else {
                    VerifyResult::Invalid
                }
            }
            Evidence::SparsePca { cov, p, v, k, threshold } => {
                if cov.len() != p * p || v.len() != *p || threshold.is_nan() {
                    return VerifyResult::Invalid;
                }
                let nnz = v.iter().filter(|x| x.abs() > 1e-9).count();
                let qf = jeff_math::planted::quad_form(cov, v, *p);
                if nnz <= *k && qf >= *threshold {
                    VerifyResult::Valid
                } else {
                    VerifyResult::Invalid
                }
            }
            Evidence::XorRefutation { signed_adj, n, m } => {
                if signed_adj.len() != n * n {
                    return VerifyResult::Invalid;
                }
                // refutation witness: spectral max-sat bound strictly below m ⇒ UNSAT.
                let max_sat = jeff_math::planted::xor_spectral_max_sat(signed_adj, *n, *m);
                if max_sat < *m as f64 {
                    VerifyResult::Valid
                } else {
                    VerifyResult::Invalid
                }
            }

            // ---- Batch 3: latent-variable / moment methods ----
            Evidence::TensorDecomp { tensor, p, r, lambdas, factors, tol } => {
                if tensor.len() != p * p * p || lambdas.len() != *r || factors.len() != r * p || tol.is_nan() {
                    return VerifyResult::Invalid;
                }
                let resid = jeff_math::moments::tensor_decomp_residual(tensor, lambdas, factors, *r, *p);
                if resid <= *tol {
                    VerifyResult::Valid
                } else {
                    VerifyResult::Invalid
                }
            }
            Evidence::HmmRank { bigram, rows, cols, m, tol, gap_min } => {
                if bigram.len() != rows * cols || tol.is_nan() {
                    return VerifyResult::Invalid;
                }
                let (resid, gap) = jeff_math::moments::bigram_rank_residual(bigram, *rows, *cols, *m);
                if resid <= *tol && gap >= *gap_min {
                    VerifyResult::Valid
                } else {
                    VerifyResult::Invalid
                }
            }
            Evidence::MomentFactorization { m2, m3, p, k, weights, means, tol } => {
                if m2.len() != p * p || m3.len() != p * p * p || weights.len() != *k || means.len() != k * p || tol.is_nan() {
                    return VerifyResult::Invalid;
                }
                let r2 = jeff_math::moments::moment2_residual(m2, weights, means, *k, *p);
                let r3 = jeff_math::moments::tensor_decomp_residual(m3, weights, means, *k, *p);
                if r2 <= *tol && r3 <= *tol {
                    VerifyResult::Valid
                } else {
                    VerifyResult::Invalid
                }
            }
            Evidence::MomentMixture { moments, k, weights, locations, tol } => {
                if weights.len() != *k || locations.len() != *k || tol.is_nan() {
                    return VerifyResult::Invalid;
                }
                // recompute m_t = Σ w_j x_j^t and compare to the claimed moment sequence
                let mut worst = 0.0_f64;
                for (t, &mt) in moments.iter().enumerate() {
                    let got: f64 = (0..*k).map(|j| weights[j] * locations[j].powi(t as i32)).sum();
                    worst = worst.max((got - mt).abs());
                }
                if worst <= *tol {
                    VerifyResult::Valid
                } else {
                    VerifyResult::Invalid
                }
            }
            Evidence::IcaProjection { data, n, p, direction, threshold } => {
                if data.len() != n * p || direction.len() != *p || threshold.is_nan() {
                    return VerifyResult::Invalid;
                }
                let ek = jeff_math::moments::excess_kurtosis(data, *n, *p, direction);
                if ek.abs() >= *threshold {
                    VerifyResult::Valid
                } else {
                    VerifyResult::Invalid
                }
            }

            // ---- Batch 4: geometry / dimension / topology ----
            Evidence::PersistentHomology { dist, n, band, feature_count } => {
                if dist.len() != n * n || band.is_nan() {
                    return VerifyResult::Invalid;
                }
                let c = jeff_math::geometry::persistent_feature_count(dist, *n, *band);
                if c == *feature_count && c >= 1 {
                    VerifyResult::Valid
                } else {
                    VerifyResult::Invalid
                }
            }
            Evidence::JlProjection { points, proj, n, d, k, eps } => {
                if points.len() != n * d || proj.len() != n * k || eps.is_nan() {
                    return VerifyResult::Invalid;
                }
                if jeff_math::geometry::max_distortion(points, proj, *n, *d, *k) <= *eps {
                    VerifyResult::Valid
                } else {
                    VerifyResult::Invalid
                }
            }
            Evidence::SpectralCluster { w, n, k, gap_min } => {
                if w.len() != n * n || gap_min.is_nan() {
                    return VerifyResult::Invalid;
                }
                if jeff_math::geometry::cluster_eigengap(w, *n, *k) >= *gap_min {
                    VerifyResult::Valid
                } else {
                    VerifyResult::Invalid
                }
            }
            Evidence::DiffusionMap { w, n, m, gap_min } => {
                if w.len() != n * n || gap_min.is_nan() {
                    return VerifyResult::Invalid;
                }
                if jeff_math::geometry::diffusion_gap(w, *n, *m) >= *gap_min {
                    VerifyResult::Valid
                } else {
                    VerifyResult::Invalid
                }
            }
            Evidence::Isomap { dist, n, k_nn, m, tol } => {
                if dist.len() != n * n || tol.is_nan() {
                    return VerifyResult::Invalid;
                }
                let (resid, _) = jeff_math::geometry::isomap_residual(dist, *n, *k_nn, *m);
                if resid <= *tol {
                    VerifyResult::Valid
                } else {
                    VerifyResult::Invalid
                }
            }
            Evidence::IntrinsicDim { points, n, d, k1, k2, claimed_dim, tol } => {
                if points.len() != n * d || tol.is_nan() {
                    return VerifyResult::Invalid;
                }
                let est = jeff_math::geometry::intrinsic_dimension(points, *n, *d, *k1, *k2);
                if (est - claimed_dim).abs() <= *tol && *claimed_dim < *d as f64 {
                    VerifyResult::Valid
                } else {
                    VerifyResult::Invalid
                }
            }

            // ---- Batch 5: streaming sketches (estimate vs exact oracle) ----
            Evidence::StreamF2 { items, estimate, lambda } => {
                if lambda.is_nan() || estimate.is_nan() {
                    return VerifyResult::Invalid;
                }
                let exact = jeff_math::streaming::exact_f2(items) as f64;
                if (estimate - exact).abs() <= *lambda * exact {
                    VerifyResult::Valid
                } else {
                    VerifyResult::Invalid
                }
            }
            Evidence::CountMinQuery { items, key, estimate, eps } => {
                if eps.is_nan() {
                    return VerifyResult::Invalid;
                }
                let exact = jeff_math::streaming::exact_freq(items, *key);
                // Count-Min never underestimates and overshoots by ≤ ε‖f‖₁.
                if *estimate >= exact && (*estimate as f64) <= exact as f64 + eps * items.len() as f64 {
                    VerifyResult::Valid
                } else {
                    VerifyResult::Invalid
                }
            }
            Evidence::DistinctCount { items, estimate, rel_err } => {
                if rel_err.is_nan() || estimate.is_nan() {
                    return VerifyResult::Invalid;
                }
                let exact = jeff_math::streaming::exact_distinct(items) as f64;
                if (estimate - exact).abs() <= *rel_err * exact {
                    VerifyResult::Valid
                } else {
                    VerifyResult::Invalid
                }
            }
            Evidence::HeavyHitters { items, hitters, phi } => {
                if phi.is_nan() || hitters.is_empty() {
                    return VerifyResult::Invalid;
                }
                let n = items.len() as f64;
                // every reported item must be a genuine heavy hitter (freq ≥ φn).
                for &h in hitters {
                    if (jeff_math::streaming::exact_freq(items, h) as f64) < *phi * n {
                        return VerifyResult::Invalid;
                    }
                }
                VerifyResult::Valid
            }
            Evidence::SublinearMean { values, estimate, lambda } => {
                if lambda.is_nan() || estimate.is_nan() {
                    return VerifyResult::Invalid;
                }
                let exact = jeff_math::streaming::exact_mean(values);
                if (estimate - exact).abs() <= *lambda {
                    VerifyResult::Valid
                } else {
                    VerifyResult::Invalid
                }
            }

            // ---- Batch 6: property testing / Fourier learning ----
            Evidence::HeavyFourier { table, theta, coeffs } => {
                if theta.is_nan() || coeffs.is_empty() {
                    return VerifyResult::Invalid;
                }
                let fhat = jeff_math::fourier::walsh_hadamard(table);
                for &s in coeffs {
                    if s >= fhat.len() || fhat[s].abs() < *theta {
                        return VerifyResult::Invalid;
                    }
                }
                VerifyResult::Valid
            }
            Evidence::LowDegree { table, k, tail_bound } => {
                if tail_bound.is_nan() {
                    return VerifyResult::Invalid;
                }
                let fhat = jeff_math::fourier::walsh_hadamard(table);
                if jeff_math::fourier::low_degree_tail(&fhat, *k) <= *tail_bound {
                    VerifyResult::Valid
                } else {
                    VerifyResult::Invalid
                }
            }
            Evidence::Linearity { table, eps } => {
                if eps.is_nan() {
                    return VerifyResult::Invalid;
                }
                let fhat = jeff_math::fourier::walsh_hadamard(table);
                if jeff_math::fourier::distance_to_linear(&fhat) <= *eps {
                    VerifyResult::Valid
                } else {
                    VerifyResult::Invalid
                }
            }
            Evidence::Junta { table, num_vars, relevant, j, floor } => {
                if floor.is_nan() || relevant.len() > *j {
                    return VerifyResult::Invalid;
                }
                let fhat = jeff_math::fourier::walsh_hadamard(table);
                let actual = jeff_math::fourier::relevant_variables(&fhat, *num_vars, *floor);
                // every influential coordinate must be in the claimed relevant set.
                if actual.iter().all(|v| relevant.contains(v)) {
                    VerifyResult::Valid
                } else {
                    VerifyResult::Invalid
                }
            }
            Evidence::ListDecode { xs, ys, q, k, coeffs, tau } => {
                if xs.len() != ys.len() || coeffs.len() > *k || xs.is_empty() {
                    return VerifyResult::Invalid;
                }
                // EXACT: the polynomial agrees on ≥ n − tau positions.
                let agree = jeff_math::fourier::agreement_count(coeffs, xs, ys, *q);
                if agree >= xs.len() - *tau {
                    VerifyResult::Valid
                } else {
                    VerifyResult::Invalid
                }
            }
            Evidence::NoiseSensitivity { table, rho, ns_bound } => {
                if rho.is_nan() || ns_bound.is_nan() {
                    return VerifyResult::Invalid;
                }
                let fhat = jeff_math::fourier::walsh_hadamard(table);
                if jeff_math::fourier::noise_sensitivity(&fhat, *rho) <= *ns_bound {
                    VerifyResult::Valid
                } else {
                    VerifyResult::Invalid
                }
            }
            _ => VerifyResult::Unknown,
        }
    }
}

/// A tolerance contract must be a finite, non-negative number.
fn tol_ok(tol: f64) -> bool {
    !tol.is_nan() && tol >= 0.0 && tol.is_finite()
}

/// Routes evidence to the right checker (PART 6.2 / APPENDIX F.6). Implements
/// [`Checker`] so it plugs straight into `verify_with`.
pub struct DefaultRegistry;

impl Checker for DefaultRegistry {
    fn check(&self, ev: &Evidence, ob: &Obligation, b: &[Boundary]) -> VerifyResult {
        match ev {
            Evidence::PolynomialIdentity { .. }
            | Evidence::Telescoper { .. }
            | Evidence::EigenCharpoly { .. } => PolyChecker.check(ev, ob, b),
            Evidence::Gf2LinearIdentity { .. } => Gf2Checker.check(ev, ob, b),
            Evidence::NumericResidual { .. } | Evidence::PfaffianHolant { .. } => {
                ReplayChecker.check(ev, ob, b)
            }
            Evidence::MatrixInverse { .. }
            | Evidence::LdltSpd { .. }
            | Evidence::FreivaldsProduct { .. }
            | Evidence::FloatResidual { .. }
            | Evidence::LowRankResidual { .. }
            | Evidence::FmmResidual { .. }
            | Evidence::LinSolveResidual { .. }
            | Evidence::SinkhornPlan { .. }
            | Evidence::AreStabilizing { .. }
            | Evidence::LatticeCount { .. }
            | Evidence::SparseRecovery { .. }
            | Evidence::SparseSpectrum { .. }
            | Evidence::MatrixCompletion { .. }
            | Evidence::PronyRecurrence { .. }
            | Evidence::SuperResolution { .. }
            | Evidence::EquiangularTightFrame { .. }
            | Evidence::SpikedCovariance { .. }
            | Evidence::PlantedClique { .. }
            | Evidence::SbmCommunity { .. }
            | Evidence::SpikedTensor { .. }
            | Evidence::SparsePca { .. }
            | Evidence::XorRefutation { .. }
            | Evidence::TensorDecomp { .. }
            | Evidence::HmmRank { .. }
            | Evidence::MomentFactorization { .. }
            | Evidence::MomentMixture { .. }
            | Evidence::IcaProjection { .. }
            | Evidence::PersistentHomology { .. }
            | Evidence::JlProjection { .. }
            | Evidence::SpectralCluster { .. }
            | Evidence::DiffusionMap { .. }
            | Evidence::Isomap { .. }
            | Evidence::IntrinsicDim { .. }
            | Evidence::StreamF2 { .. }
            | Evidence::CountMinQuery { .. }
            | Evidence::DistinctCount { .. }
            | Evidence::HeavyHitters { .. }
            | Evidence::SublinearMean { .. }
            | Evidence::HeavyFourier { .. }
            | Evidence::LowDegree { .. }
            | Evidence::Linearity { .. }
            | Evidence::Junta { .. }
            | Evidence::ListDecode { .. }
            | Evidence::NoiseSensitivity { .. } => KernelChecker.check(ev, ob, b),
        }
    }
}

/// Which checker discharged a given evidence kind — for the certificate record
/// (APPENDIX H.3 `verified.checker`). Honest naming (DR2): we report the *actual*
/// exact checker, not "z3" when no solver ran.
pub fn checker_name(ev: &Evidence) -> &'static str {
    match ev {
        Evidence::PolynomialIdentity { .. } => "exact-coeff-zero",
        Evidence::EigenCharpoly { .. } => "cayley-hamilton",
        Evidence::Telescoper { .. } => "exact-telescoper",
        Evidence::Gf2LinearIdentity { .. } => "gf2-basis",
        Evidence::NumericResidual { .. } => "exact-replay",
        Evidence::PfaffianHolant { .. } => "pfaffian-replay",
        Evidence::MatrixInverse { .. } => "exact-matrix-inverse",
        Evidence::LdltSpd { .. } => "exact-ldlt-spd",
        Evidence::FreivaldsProduct { .. } => "freivalds-exact",
        Evidence::FloatResidual { .. } => "float-residual-tol",
        Evidence::LowRankResidual { .. } => "frobenius-residual-tol",
        Evidence::FmmResidual { .. } => "nbody-residual-tol",
        Evidence::LinSolveResidual { .. } => "l2-residual-tol",
        Evidence::SinkhornPlan { .. } => "sinkhorn-marginal-tol",
        Evidence::AreStabilizing { .. } => "care-stabilizing-psd",
        Evidence::LatticeCount { .. } => "lattice-sample-exact",
        Evidence::SparseRecovery { .. } => "sparse-residual-l0",
        Evidence::SparseSpectrum { .. } => "sparsefft-residual-tol",
        Evidence::MatrixCompletion { .. } => "completion-residual-tol",
        Evidence::PronyRecurrence { .. } => "prony-recurrence-exact",
        Evidence::SuperResolution { .. } => "superres-model-residual",
        Evidence::EquiangularTightFrame { .. } => "welch-etf-exact",
        Evidence::SpikedCovariance { .. } => "bbp-eigen-threshold",
        Evidence::PlantedClique { .. } => "clique-exact",
        Evidence::SbmCommunity { .. } => "sbm-spectral-threshold",
        Evidence::SpikedTensor { .. } => "tensor-unfold-residual",
        Evidence::SparsePca { .. } => "sparsepca-quadform",
        Evidence::XorRefutation { .. } => "xor-spectral-refutation",
        Evidence::TensorDecomp { .. } => "tensor-decomp-residual",
        Evidence::HmmRank { .. } => "hmm-rank-gap",
        Evidence::MomentFactorization { .. } => "moment-reconstruction",
        Evidence::MomentMixture { .. } => "moment-mixture-replay",
        Evidence::IcaProjection { .. } => "ica-kurtosis",
        Evidence::PersistentHomology { .. } => "ph-mst-exact",
        Evidence::JlProjection { .. } => "jl-distortion",
        Evidence::SpectralCluster { .. } => "laplacian-eigengap",
        Evidence::DiffusionMap { .. } => "diffusion-gap",
        Evidence::Isomap { .. } => "isomap-residual",
        Evidence::IntrinsicDim { .. } => "levina-bickel-mle",
        Evidence::StreamF2 { .. } => "ams-f2-vs-exact",
        Evidence::CountMinQuery { .. } => "count-min-bound",
        Evidence::DistinctCount { .. } => "hll-vs-exact",
        Evidence::HeavyHitters { .. } => "heavy-hitter-exact",
        Evidence::SublinearMean { .. } => "mean-vs-exact",
        Evidence::HeavyFourier { .. } => "wht-heavy-coeff",
        Evidence::LowDegree { .. } => "wht-low-degree-tail",
        Evidence::Linearity { .. } => "wht-blr-distance",
        Evidence::Junta { .. } => "wht-influence-junta",
        Evidence::ListDecode { .. } => "rs-agreement-exact",
        Evidence::NoiseSensitivity { .. } => "wht-noise-sensitivity",
    }
}

/// The single public verification entry point (PART 6.2). Produces a
/// `VerifiedCertificate` **iff** the routed checker returns `Valid` (R31).
pub fn verify(c: Certificate) -> Option<VerifiedCertificate> {
    verify_with(c, &DefaultRegistry)
}

/// Same, but reports the routed result without consuming on failure — for
/// diagnostics / `--collapse-report` (does not bypass the gate).
pub fn check_result(c: &Certificate) -> VerifyResult {
    DefaultRegistry.check(&c.evidence, &c.obligation, &c.boundaries)
}

#[cfg(test)]
mod tests {
    use super::*;
    use jeff_cert::gf2circuit::{Gate, LinearCircuit};
    use jeff_cert::{IrRef, Obligation};
    use jeff_math::{Gf2Matrix, Gf2Vec, Poly, RatMatrix};
    use jeff_span::Span;
    use num_bigint::BigInt;
    use num_rational::BigRational;

    fn cert(ev: Evidence) -> Certificate {
        Certificate {
            collapser_id: "test".into(),
            source: IrRef::new(1, Span::dummy()),
            collapsed: IrRef::new(2, Span::dummy()),
            obligation: Obligation::new("test"),
            evidence: ev,
            boundaries: vec![],
            fallback: IrRef::new(1, Span::dummy()),
        }
    }

    #[test]
    fn polynomial_identity_zero_is_valid() {
        // 6*S(n) - n(n+1)(2n+1) with S the i^2 sum closed form ≡ 0 (F.1).
        let n = Poly::var("n");
        let one = Poly::from_i64(1);
        let two = Poly::from_i64(2);
        let sixth = BigRational::new(BigInt::from(1), BigInt::from(6));
        let s = n
            .mul(&n.add(&one))
            .mul(&two.mul(&n).add(&one))
            .scale(&sixth);
        // diff = S(n) - S(n-1) - n^2
        let s_shift = {
            let m = n.sub(&one);
            m.mul(&m.add(&one)).mul(&two.mul(&m).add(&one)).scale(&sixth)
        };
        let diff = s.sub(&s_shift).sub(&n.mul(&n));
        assert!(verify(cert(Evidence::PolynomialIdentity { poly: diff })).is_some());
    }

    #[test]
    fn wrong_polynomial_identity_is_rejected() {
        // A nonzero "identity" must NOT verify (DR7).
        let nonzero = Poly::var("n"); // ≠ 0
        assert!(verify(cert(Evidence::PolynomialIdentity { poly: nonzero })).is_none());
    }

    #[test]
    fn eigen_charpoly_valid() {
        let a = RatMatrix::from_i64(2, 2, &[1, 1, 1, 0]);
        assert!(verify(cert(Evidence::EigenCharpoly { matrix: a })).is_some());
    }

    #[test]
    fn gf2_linear_identity_valid_and_tamper_rejected() {
        // circuit: out0 = in0 ^ in1, out1 = in1. Build (M,b) correctly.
        let circuit = LinearCircuit {
            n_inputs: 2,
            gates: vec![Gate::Xor(0, 1)], // wire 2
            outputs: vec![2, 1],
        };
        // columns = circuit(e_i) (b=0 here since no NOT/const)
        let col0 = {
            let mut v = Gf2Vec::zeros(2);
            v.set(0, true); // e0 -> out0 = 1^0 =1, out1 = 0
            v
        };
        let col1 = {
            let mut v = Gf2Vec::zeros(2);
            v.set(0, true); // e1 -> out0 = 0^1 =1, out1 = 1
            v.set(1, true);
            v
        };
        let m = Gf2Matrix::from_columns(2, vec![col0, col1.clone()]);
        let b = Gf2Vec::zeros(2);
        assert!(verify(cert(Evidence::Gf2LinearIdentity {
            circuit: circuit.clone(),
            m: m.clone(),
            b: b.clone()
        }))
        .is_some());

        // Tamper M -> reject (DR1/DR7).
        let mut bad = m;
        bad.cols[0] = col1;
        assert!(verify(cert(Evidence::Gf2LinearIdentity {
            circuit,
            m: bad,
            b
        }))
        .is_none());
    }

    #[test]
    fn replay_linrec_fibonacci() {
        // a_n = a_{n-1}+a_{n-2}; F0=0,F1=1; F(10)=55.
        let ev = Evidence::NumericResidual {
            replay: ReplayKind::LinearRecTerm {
                rec: vec![1, 1],
                init: vec![0, 1],
                modulus: 1_000_000_007,
                index: 10,
                claimed: 55,
            },
        };
        assert!(verify(cert(ev)).is_some());
        // wrong claim rejected
        let ev_bad = Evidence::NumericResidual {
            replay: ReplayKind::LinearRecTerm {
                rec: vec![1, 1],
                init: vec![0, 1],
                modulus: 1_000_000_007,
                index: 10,
                claimed: 56,
            },
        };
        assert!(verify(cert(ev_bad)).is_none());
    }

    #[test]
    fn replay_negacyclic_ntt_poly_mul() {
        // PQC poly_mul a*b mod (x^4+1) over Z_7681, computed by the fast NTT, certified
        // against the schoolbook oracle in the checker.
        let q = 7681u64;
        let a = vec![1u64, 2, 3, 4];
        let b = vec![5u64, 6, 7, 8];
        let claimed = jeff_math::pqc::negacyclic_convolve(&a, &b, q, 17).unwrap();
        let ev = Evidence::NumericResidual {
            replay: ReplayKind::NegacyclicConvolution {
                a: a.clone(),
                b: b.clone(),
                q,
                claimed,
            },
        };
        assert!(verify(cert(ev)).is_some());
    }

    /// Tripwire `false_ntt_rejected`: a wrong NTT poly-mul result must NOT verify. The
    /// checker recomputes the schoolbook negacyclic product independently (AR-4), so a
    /// forged/incorrect fast result is caught → None → fallback (P0/P1).
    #[test]
    fn false_ntt_rejected() {
        let q = 7681u64;
        let a = vec![1u64, 2, 3, 4];
        let b = vec![5u64, 6, 7, 8];
        let mut claimed = jeff_math::pqc::negacyclic_convolve(&a, &b, q, 17).unwrap();
        claimed[0] = (claimed[0] + 1) % q; // tamper one coefficient
        let ev = Evidence::NumericResidual {
            replay: ReplayKind::NegacyclicConvolution { a, b, q, claimed },
        };
        assert!(
            verify(cert(ev)).is_none(),
            "a tampered NTT result must be rejected by the schoolbook checker"
        );
    }

    // ===== Stage 6A Batch 1: sparse / low-rank recovery certificates =====

    #[test]
    fn sparse_recovery_verifies_and_rejects() {
        use jeff_math::recovery::{apply, omp};
        let m = 12;
        let n = 24;
        let phi = jeff_math::fmat::gaussian_matrix(m, n, 0xABCDEF);
        let mut x = vec![0.0; n];
        x[2] = 3.0;
        x[7] = -1.5;
        x[19] = 2.0;
        let y = apply(&phi, &x);
        let xr = omp(&phi, &y, 3);
        let phi_flat: Vec<f64> = (0..m)
            .flat_map(|i| (0..n).map(move |j| (i, j)))
            .map(|(i, j)| phi.get(i, j))
            .collect();
        let ev = Evidence::SparseRecovery {
            phi: phi_flat.clone(),
            rows: m,
            cols: n,
            y: y.clone(),
            x: xr,
            k: 3,
            tol: 1e-6,
            random_phi: true,
        };
        assert_eq!(ev.cert_class(), jeff_cert::CertClass::RipConditional);
        assert!(verify(cert(ev)).is_some());

        // tripwire: a wrong (dense / off-support) x must be rejected
        let bad = Evidence::SparseRecovery {
            phi: phi_flat,
            rows: m,
            cols: n,
            y,
            x: vec![1.0; n], // dense, won't satisfy Φx≈y
            k: 3,
            tol: 1e-6,
            random_phi: true,
        };
        assert!(verify(cert(bad)).is_none(), "residual-over-tol must be rejected");
    }

    #[test]
    fn sparse_spectrum_verifies_and_rejects() {
        use jeff_math::recovery::sparse_fft;
        let n = 32;
        let two_pi = std::f64::consts::TAU;
        let x: Vec<f64> = (0..n)
            .map(|t| (two_pi * 3.0 * t as f64 / n as f64).cos())
            .collect();
        let support = sparse_fft(&x, 2);
        let ev = Evidence::SparseSpectrum {
            signal: x.clone(),
            support: support.clone(),
            n,
            k: 2,
            tol: 1e-6,
        };
        assert!(verify(cert(ev)).is_some());
        // tripwire: corrupt a coefficient → residual blows past tol
        let mut bad_support = support;
        bad_support[0].1 += 5.0;
        let bad = Evidence::SparseSpectrum {
            signal: x,
            support: bad_support,
            n,
            k: 2,
            tol: 1e-6,
        };
        assert!(verify(cert(bad)).is_none());
    }

    #[test]
    fn matrix_completion_verifies_and_rejects() {
        use jeff_math::recovery::complete;
        let (rows, cols) = (8, 8);
        let u: Vec<f64> = (0..rows).map(|i| 1.0 + i as f64 * 0.1).collect();
        let v: Vec<f64> = (0..cols).map(|j| 2.0 - j as f64 * 0.05).collect();
        let observed: Vec<(usize, usize, f64)> = (0..rows)
            .flat_map(|i| (0..cols).map(move |j| (i, j)))
            .filter(|&(i, j)| (i * 5 + j * 3) % 10 < 7)
            .map(|(i, j)| (i, j, u[i] * v[j]))
            .collect();
        let (uu, vv) = complete(&observed, rows, cols, 1, 40);
        let uf: Vec<f64> = (0..rows).map(|i| uu.get(i, 0)).collect();
        let vf: Vec<f64> = (0..cols).map(|j| vv.get(j, 0)).collect();
        let ev = Evidence::MatrixCompletion {
            observed: observed.clone(),
            u: uf,
            v: vf,
            rows,
            cols,
            r: 1,
            tol: 1e-3,
        };
        assert_eq!(ev.cert_class(), jeff_cert::CertClass::HighProbability);
        assert!(verify(cert(ev)).is_some());
        // tripwire: zero factors cannot reproduce nonzero observed entries
        let bad = Evidence::MatrixCompletion {
            observed,
            u: vec![0.0; rows],
            v: vec![0.0; cols],
            rows,
            cols,
            r: 1,
            tol: 1e-3,
        };
        assert!(verify(cert(bad)).is_none());
    }

    #[test]
    fn prony_verifies_and_rejects() {
        use jeff_math::prony::prony_fit;
        let s: Vec<f64> = (0..10)
            .map(|t| 2.0 * 1.5_f64.powi(t) + 3.0 * 0.5_f64.powi(t))
            .collect();
        let (a, _) = prony_fit(&s, 2).unwrap();
        let ev = Evidence::PronyRecurrence {
            samples: s.clone(),
            a: a.clone(),
            tol: 1e-6,
        };
        assert!(verify(cert(ev)).is_some());
        // tripwire: a wrong recurrence does not annihilate the samples
        let bad = Evidence::PronyRecurrence {
            samples: s,
            a: vec![0.5, -0.3, 1.0],
            tol: 1e-6,
        };
        assert!(verify(cert(bad)).is_none());
    }

    #[test]
    fn superresolution_verifies_and_rejects() {
        use jeff_math::complex::Complex;
        use jeff_math::prony::super_resolve;
        let true_locs = [0.2_f64, 0.7];
        let amps = [Complex::new(1.0, 0.0), Complex::new(0.8, 0.0)];
        let fc = 8usize;
        let mm = 2 * fc + 1;
        let two_pi = std::f64::consts::TAU;
        let lowpass: Vec<Complex> = (0..mm)
            .map(|m| {
                let mut acc = Complex::zero();
                for (k, &t) in true_locs.iter().enumerate() {
                    acc = acc.add(amps[k].mul(Complex::from_angle(-two_pi * m as f64 * t)));
                }
                acc
            })
            .collect();
        let spikes = super_resolve(&lowpass, 2).unwrap();
        let lp: Vec<(f64, f64)> = lowpass.iter().map(|c| (c.re, c.im)).collect();
        let sp: Vec<(f64, f64, f64)> = spikes.iter().map(|s| (s.t, s.amp.re, s.amp.im)).collect();
        let ev = Evidence::SuperResolution {
            lowpass: lp.clone(),
            spikes: sp,
            fc,
            tol: 1e-6,
        };
        assert_eq!(ev.cert_class(), jeff_cert::CertClass::ThresholdConditional);
        assert!(verify(cert(ev)).is_some());
        // tripwire: spikes too close together (below 2/fc) must be rejected even if they
        // happen to fit — the separation threshold is part of the certificate.
        let close = Evidence::SuperResolution {
            lowpass: lp,
            spikes: vec![(0.10, 1.0, 0.0), (0.10 + 0.5 / fc as f64, 0.8, 0.0)],
            fc,
            tol: 1e-6,
        };
        assert!(verify(cert(close)).is_none());
    }

    #[test]
    fn etf_verifies_and_rejects() {
        use jeff_math::frame::mercedes_benz;
        let (v, m, n) = mercedes_benz();
        let ev = Evidence::EquiangularTightFrame {
            frame: v,
            m,
            n,
            tol: 1e-9,
        };
        assert_eq!(ev.cert_class(), jeff_cert::CertClass::Exact);
        assert!(verify(cert(ev)).is_some());
        // tripwire: a non-equiangular frame is not an ETF
        let bad = Evidence::EquiangularTightFrame {
            frame: vec![1.0, 0.0, 0.0, 1.0, 1.0, 1.0],
            m: 3,
            n: 2,
            tol: 1e-9,
        };
        assert!(verify(cert(bad)).is_none());
    }

    // ===== Stage 6A Batch 2: planted / spiked detection certificates =====

    #[test]
    fn false_spike_rejected() {
        // identity covariance has no spike: λ₁≈λ₂≈1, so a claimed BBP detection at a
        // high threshold must be rejected (no detached eigenvalue, no gap).
        let p = 5;
        let mut cov = vec![0.0; p * p];
        for i in 0..p {
            cov[i * p + i] = 1.0;
        }
        let ev = Evidence::SpikedCovariance { cov, p, threshold: 3.0, gap: 0.5 };
        assert!(verify(cert(ev)).is_none(), "no spike ⇒ reject");
    }

    #[test]
    fn false_clique_rejected() {
        // claim a 3-clique on a triangle missing edge (0,2) → not a clique → reject.
        let n = 3;
        let adj = vec![0u8, 1, 0, 1, 0, 1, 0, 1, 0];
        let ev = Evidence::PlantedClique { adj, n, clique: vec![0, 1, 2], k: 3 };
        assert!(verify(cert(ev)).is_none());
    }

    #[test]
    fn planted_clique_exact_cert_valid() {
        // a real triangle (0,1,2) all adjacent → exact clique cert verifies.
        let n = 3;
        let adj = vec![0u8, 1, 1, 1, 0, 1, 1, 1, 0];
        let ev = Evidence::PlantedClique { adj, n, clique: vec![0, 1, 2], k: 3 };
        assert_eq!(ev.cert_class(), jeff_cert::CertClass::Exact);
        assert!(verify(cert(ev)).is_some());
    }

    #[test]
    fn xor_refutation_witness_valid_and_loose_rejected() {
        // dense random 2-XOR: spectral bound bites → valid UNSAT witness.
        let n = 16;
        let mut rng = jeff_math::fmat::Rng::new(0xAA);
        let m = 300;
        let mut cons = Vec::new();
        for _ in 0..m {
            let i = (rng.next_u64() as usize) % n;
            let mut j = (rng.next_u64() as usize) % n;
            if j == i {
                j = (j + 1) % n;
            }
            cons.push((i, j, (rng.next_u64() & 1) as u8));
        }
        let sa = jeff_math::planted::signed_adjacency(&cons, n);
        assert!(verify(cert(Evidence::XorRefutation { signed_adj: sa, n, m })).is_some());
        // a single constraint cannot be refuted → bound = m, not < m → reject.
        let sa1 = jeff_math::planted::signed_adjacency(&[(0, 1, 0)], n);
        assert!(verify(cert(Evidence::XorRefutation { signed_adj: sa1, n, m: 1 })).is_none());
    }

    // ===== Stage 6A Batch 3: latent-variable / moment certificates =====

    #[test]
    fn moment_mixture_valid_and_false_rejected() {
        // m_t = 0.7·2^t + 0.3·5^t
        let moments: Vec<f64> = (0..6).map(|t| 0.7 * 2f64.powi(t) + 0.3 * 5f64.powi(t)).collect();
        let ev = Evidence::MomentMixture {
            moments: moments.clone(),
            k: 2,
            weights: vec![0.7, 0.3],
            locations: vec![2.0, 5.0],
            tol: 1e-9,
        };
        assert!(verify(cert(ev)).is_some());
        // wrong locations don't reproduce the moments → reject
        let bad = Evidence::MomentMixture {
            moments,
            k: 2,
            weights: vec![0.7, 0.3],
            locations: vec![3.0, 4.0],
            tol: 1e-9,
        };
        assert!(verify(cert(bad)).is_none());
    }

    #[test]
    fn false_tensor_decomp_rejected() {
        // claim a rank-1 decomposition of a tensor that isn't rank-1 from those factors.
        let p = 2;
        let tensor = vec![1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 5.0]; // T[0,0,0]=1, T[1,1,1]=5
        let ev = Evidence::TensorDecomp {
            tensor,
            p,
            r: 1,
            lambdas: vec![1.0],
            factors: vec![1.0, 0.0], // a = e0 ⇒ reconstructs only T[0,0,0]=1, misses T[1,1,1]=5
            tol: 1e-6,
        };
        assert!(verify(cert(ev)).is_none());
    }

    // ===== Stage 6A Batch 4: geometry / dimension / topology certificates =====

    #[test]
    fn persistent_homology_valid_and_false_count_rejected() {
        // two clusters: exactly 2 features above band 1.0.
        let pts = vec![0.0, 0.0, 0.1, 0.0, 10.0, 10.0, 10.1, 10.0];
        let dist = jeff_math::geometry::pairwise_distances(&pts, 4, 2);
        let ev = Evidence::PersistentHomology { dist: dist.clone(), n: 4, band: 1.0, feature_count: 2 };
        assert_eq!(ev.cert_class(), jeff_cert::CertClass::Exact);
        assert!(verify(cert(ev)).is_some());
        // a wrong feature count is rejected
        let bad = Evidence::PersistentHomology { dist, n: 4, band: 1.0, feature_count: 5 };
        assert!(verify(cert(bad)).is_none());
    }

    #[test]
    fn false_eigengap_rejected() {
        // uniform similarity has no 2-cluster eigengap → a claimed gap is rejected.
        let n = 6;
        let w = vec![1.0; n * n];
        let ev = Evidence::SpectralCluster { w, n, k: 2, gap_min: 0.3 };
        assert!(verify(cert(ev)).is_none());
    }

    // ===== Stage 6A Batch 5: streaming sketch certificates =====

    #[test]
    fn streaming_estimates_valid_and_false_rejected() {
        let items: Vec<u64> = {
            let mut v = vec![7u64; 300];
            v.extend((0..100).map(|i| i as u64 + 50));
            v
        };
        // F₂ within bound
        let exact_f2 = jeff_math::streaming::exact_f2(&items) as f64;
        assert!(verify(cert(Evidence::StreamF2 { items: items.clone(), estimate: exact_f2, lambda: 0.1 })).is_some());
        // a wildly wrong F₂ is rejected
        assert!(verify(cert(Evidence::StreamF2 { items: items.clone(), estimate: 1.0, lambda: 0.1 })).is_none());

        // a "heavy hitter" that isn't heavy is rejected
        let bad = Evidence::HeavyHitters { items: items.clone(), hitters: vec![50], phi: 0.3 };
        assert!(verify(cert(bad)).is_none());
        // the genuine heavy hitter passes
        let good = Evidence::HeavyHitters { items, hitters: vec![7], phi: 0.3 };
        assert!(verify(cert(good)).is_some());
    }

    #[test]
    fn count_min_underestimate_rejected() {
        // Count-Min never underestimates; a claimed estimate below the truth is invalid.
        let items = vec![3u64; 100];
        let ev = Evidence::CountMinQuery { items, key: 3, estimate: 50, eps: 0.01 };
        assert!(verify(cert(ev)).is_none());
    }

    // ===== Stage 6A Batch 6: property-testing / Fourier certificates =====

    #[test]
    fn listdecode_valid_and_false_rejected() {
        let q = 97;
        let p = vec![3u64, 2, 1];
        let xs: Vec<u64> = (1..=7).collect();
        let ys: Vec<u64> = xs.iter().map(|&x| jeff_math::fourier::poly_eval_mod(&p, x, q)).collect();
        // exact codeword agrees everywhere → valid
        let ev = Evidence::ListDecode { xs: xs.clone(), ys: ys.clone(), q, k: 3, coeffs: p, tau: 2 };
        assert_eq!(ev.cert_class(), jeff_cert::CertClass::Exact);
        assert!(verify(cert(ev)).is_some());
        // a wrong polynomial agrees on too few points → rejected
        let bad = Evidence::ListDecode { xs, ys, q, k: 3, coeffs: vec![0, 0, 0], tau: 2 };
        assert!(verify(cert(bad)).is_none());
    }

    #[test]
    fn parity_low_degree_claim_rejected() {
        // PARITY (mass at degree 2) cannot satisfy a degree-1 low-degree claim.
        let parity = vec![1.0, -1.0, -1.0, 1.0];
        let ev = Evidence::LowDegree { table: parity, k: 1, tail_bound: 0.1 };
        assert!(verify(cert(ev)).is_none());
    }

    // ===== Verifier-integrity tripwires (MIDBUILD_AUDIT §A.1) =====

    /// Tripwire `false_certificate_is_rejected`: a deliberately wrong certificate
    /// must NOT verify (→ None → the collapser falls back to the original). Proves
    /// the checker is real, not a stub that says Valid (DR1/DR7, P0/P1).
    #[test]
    fn false_certificate_is_rejected() {
        // A claimed polynomial identity that is actually nonzero: "n - 1 == 0".
        let bogus = Poly::var("n").sub(&Poly::from_i64(1));
        assert!(verify(cert(Evidence::PolynomialIdentity { poly: bogus })).is_none());

        // A lie about a linear-recurrence term (F(10) is 55, claim 999).
        let lie = Evidence::NumericResidual {
            replay: ReplayKind::LinearRecTerm {
                rec: vec![1, 1],
                init: vec![0, 1],
                modulus: 1_000_000_007,
                index: 10,
                claimed: 999,
            },
        };
        assert!(verify(cert(lie)).is_none());

        // A Cayley–Hamilton claim about a matrix... that is fine for any matrix, so
        // instead tamper a Pfaffian: det(skew) must equal pf^2.
        let bad_pf = Evidence::PfaffianHolant {
            witness: jeff_cert::HolantWitness {
                skew: vec![0, 1, -1, 0], // 2x2 skew, det = 1, pf = 1
                dim: 2,
                claimed_pfaffian: 5, // lie: 5^2 = 25 != 1
            },
        };
        assert!(verify(cert(bad_pf)).is_none());
    }

    /// Tripwire `false_telescoper_rejected` (Stage 1, checker-first): the telescoper
    /// checker accepts the hand-verified certificate for Σ_k C(n,k) = 2^n and
    /// rejects a wrong operator or a wrong rational certificate. Proven here, in the
    /// verify layer, BEFORE the Zeilberger collapser exists (R2).
    #[test]
    fn false_telescoper_rejected() {
        use jeff_math::hyper::{HyperTerm, LinForm};
        use jeff_math::{Poly, RatFunc};
        // C(n,k) = Γ(n+1)/(Γ(k+1)Γ(n-k+1))
        let term = HyperTerm {
            coeff: num_rational::BigRational::from(BigInt::from(1)),
            z_k: num_rational::BigRational::from(BigInt::from(1)),
            poly: Poly::from_i64(1),
            gammas: vec![
                (LinForm::new(1, 0, 1), 1),
                (LinForm::new(0, 1, 1), -1),
                (LinForm::new(1, -1, 1), -1),
            ],
        };
        let good_l = vec![RatFunc::from_i64(-2), RatFunc::from_i64(1)];
        let good_r = RatFunc::new(
            Poly::var("k").neg(),
            Poly::var("n").add(&Poly::from_i64(1)).sub(&Poly::var("k")),
        );
        // positive: the real certificate verifies through the gate.
        assert!(verify(cert(Evidence::Telescoper {
            term: term.clone(),
            l: good_l.clone(),
            r: good_r.clone(),
        }))
        .is_some());
        // negative: wrong operator → reject.
        let bad_l = vec![RatFunc::from_i64(-3), RatFunc::from_i64(1)];
        assert!(verify(cert(Evidence::Telescoper {
            term: term.clone(),
            l: bad_l,
            r: good_r,
        }))
        .is_none());
        // negative: wrong certificate → reject.
        let bad_r = RatFunc::new(Poly::var("k").neg(), Poly::var("n").sub(&Poly::var("k")));
        assert!(verify(cert(Evidence::Telescoper {
            term,
            l: good_l,
            r: bad_r,
        }))
        .is_none());
    }

    /// Tripwire `sorry_yields_fallback`: when the checker cannot discharge the
    /// obligation (the analogue of a Lean `sorry` / Z3 `unknown` / timeout), it
    /// returns `Unknown`, and `verify` yields `None` → fallback (R31/DR8). Here the
    /// exact replay would exceed its safety cap, so it declines rather than hangs
    /// (R23) — and crucially does NOT pretend the result is valid.
    #[test]
    fn sorry_yields_fallback() {
        let beyond_cap = Evidence::NumericResidual {
            replay: ReplayKind::MatrixPowerMod {
                matrix: vec![1, 1, 1, 0],
                dim: 2,
                q: 1_000_000_007,
                exp: super::REPLAY_CAP + 1, // would hang to replay exactly → Unknown
                claimed: vec![0, 0, 0, 0],
            },
        };
        assert_eq!(super::check_result(&cert(beyond_cap.clone())), VerifyResult::Unknown);
        assert!(verify(cert(beyond_cap)).is_none()); // Unknown is NOT Valid (R31)
    }

    // ===== Stage 3 Tier-S tripwires (checker-first) =====

    /// `false_inverse_rejected`: a wrong inverse must not verify (exact `=I`).
    #[test]
    fn false_inverse_rejected() {
        use jeff_math::RatMatrix;
        let m = RatMatrix::from_i64(2, 2, &[4, 3, 6, 3]);
        let good = m.inverse().unwrap();
        assert!(verify(cert(Evidence::MatrixInverse {
            m: m.clone(),
            inv: good.clone(),
        }))
        .is_some());
        // tamper one entry of the inverse → reject.
        let mut bad = good;
        bad.data[0] += num_rational::BigRational::from(BigInt::from(1));
        assert!(verify(cert(Evidence::MatrixInverse { m, inv: bad })).is_none());
    }

    /// `false_product_rejected`: a wrong matrix product must fail Freivalds.
    #[test]
    fn false_product_rejected() {
        use jeff_math::{freivalds_seeds, IntMatrix};
        let a = IntMatrix::from_i64(3, &[1, 2, 3, 4, 5, 6, 7, 8, 9]);
        let b = IntMatrix::from_i64(3, &[9, 8, 7, 6, 5, 4, 3, 2, 1]);
        let c = a.naive_mul(&b);
        let seeds = freivalds_seeds(3, 40, 999);
        let mk = |cc: &IntMatrix| Evidence::FreivaldsProduct {
            a: a.data.clone(),
            b: b.data.clone(),
            c: cc.data.clone(),
            dim: 3,
            seeds: seeds.clone(),
        };
        assert!(verify(cert(mk(&c))).is_some(), "correct product verifies");
        let mut wrong = c.clone();
        wrong.data[4] += BigInt::from(1);
        assert!(verify(cert(mk(&wrong))).is_none(), "wrong product rejected");
    }

    /// Non-SPD matrix must be refused by the LDLᵀ checker (a negative pivot).
    #[test]
    fn non_spd_ldlt_rejected() {
        use jeff_math::RatMatrix;
        let spd = RatMatrix::from_i64(2, 2, &[4, 2, 2, 3]);
        let (l, d) = spd.ldlt().unwrap();
        assert!(verify(cert(Evidence::LdltSpd { a: spd, l, d })).is_some());
        // indefinite matrix: LDLᵀ exists but some D<0 → not SPD → reject.
        let indef = RatMatrix::from_i64(2, 2, &[1, 2, 2, 1]);
        let (l2, d2) = indef.ldlt().unwrap();
        assert!(verify(cert(Evidence::LdltSpd { a: indef, l: l2, d: d2 })).is_none());
    }

    /// Float residual: within tol verifies; beyond tol is rejected (explicit tol).
    #[test]
    fn float_residual_respects_tol() {
        // m = [[2,0],[0,4]], inv = [[0.5,0],[0,0.25]] → residual 0.
        let m = vec![2.0, 0.0, 0.0, 4.0];
        let good = vec![0.5, 0.0, 0.0, 0.25];
        assert!(verify(cert(Evidence::FloatResidual {
            m: m.clone(),
            inv: good,
            dim: 2,
            tol: 1e-9,
        }))
        .is_some());
        // perturb beyond tol → reject.
        let bad = vec![0.6, 0.0, 0.0, 0.25];
        assert!(verify(cert(Evidence::FloatResidual {
            m,
            inv: bad,
            dim: 2,
            tol: 1e-9,
        }))
        .is_none());
    }

    /// The Tier-S evidence kinds serialize → deserialize → re-verify (R25): the
    /// certificates are self-contained and replayable from disk.
    #[test]
    fn kernel_certs_round_trip_and_reverify() {
        use jeff_math::{freivalds_seeds, IntMatrix, RatMatrix};
        let m = RatMatrix::from_i64(2, 2, &[4, 3, 6, 3]);
        let inv = m.inverse().unwrap();
        let a = IntMatrix::from_i64(2, &[1, 2, 3, 4]);
        let b = IntMatrix::from_i64(2, &[5, 6, 7, 8]);
        let c = a.naive_mul(&b);
        let spd = RatMatrix::from_i64(2, 2, &[4, 2, 2, 3]);
        let (l, d) = spd.ldlt().unwrap();
        let evs = vec![
            Evidence::MatrixInverse { m, inv },
            Evidence::FreivaldsProduct {
                a: a.data.clone(),
                b: b.data.clone(),
                c: c.data.clone(),
                dim: 2,
                seeds: freivalds_seeds(2, 24, 7),
            },
            Evidence::LdltSpd { a: spd, l, d },
            Evidence::FloatResidual {
                m: vec![2.0, 0.0, 0.0, 4.0],
                inv: vec![0.5, 0.0, 0.0, 0.25],
                dim: 2,
                tol: 1e-9,
            },
        ];
        for ev in evs {
            let c = cert(ev);
            let json = serde_json::to_string(&c).unwrap();
            let back: jeff_cert::Certificate = serde_json::from_str(&json).unwrap();
            assert!(verify(back).is_some(), "round-tripped cert must re-verify");
        }
    }

    // ===== Tier-A tripwires (checker-first; approximate certs) =====

    /// `false_lowrank_rejected` / `residual_over_tol_rejected`: a low-rank cert whose
    /// approximation does not meet the stated tol must be rejected.
    #[test]
    fn false_lowrank_rejected() {
        // A = diag(1,1) ; a "rank-1" approx that drops one unit → residual 1 > tol.
        let a = vec![1.0, 0.0, 0.0, 1.0];
        let bad_approx = vec![1.0, 0.0, 0.0, 0.0]; // residual_F = 1
        assert!(verify(cert(Evidence::LowRankResidual {
            a: a.clone(),
            approx: bad_approx,
            rows: 2,
            cols: 2,
            tol: 1e-6,
        }))
        .is_none());
        // an exact approx (residual 0) verifies.
        assert!(verify(cert(Evidence::LowRankResidual {
            a: a.clone(),
            approx: a,
            rows: 2,
            cols: 2,
            tol: 1e-6,
        }))
        .is_some());
    }

    /// An N-body fast potential that disagrees with the exact direct sum is rejected
    /// (the checker recomputes the ground truth).
    #[test]
    fn false_fmm_potential_rejected() {
        use jeff_math::nbody::{direct_sum, KernelKind};
        let pts = vec![0.0, 1.0, 2.0, 3.0];
        let chg = vec![1.0, 1.0, 1.0, 1.0];
        let k = KernelKind::Exponential { decay: 1.0 };
        let exact = direct_sum(&pts, &chg, k);
        // correct potential verifies
        assert!(verify(cert(Evidence::FmmResidual {
            points: pts.clone(),
            charges: chg.clone(),
            kernel: k,
            phi: exact.clone(),
            tol: 1e-9,
        }))
        .is_some());
        // wrong potential rejected
        let mut wrong = exact;
        wrong[0] += 1.0;
        assert!(verify(cert(Evidence::FmmResidual {
            points: pts,
            charges: chg,
            kernel: k,
            phi: wrong,
            tol: 1e-9,
        }))
        .is_none());
    }

    /// A linear-solve cert whose `x` does not satisfy `Ax≈b` within tol is rejected.
    #[test]
    fn false_linsolve_rejected() {
        let entries = vec![(0, 0, 4.0), (0, 1, 1.0), (1, 0, 1.0), (1, 1, 3.0)];
        let b = vec![1.0, 2.0];
        // true solution of [[4,1],[1,3]]x=[1,2]: x=(1/11, 7/11)
        let good = vec![1.0 / 11.0, 7.0 / 11.0];
        assert!(verify(cert(Evidence::LinSolveResidual {
            entries: entries.clone(),
            dim: 2,
            b: b.clone(),
            x: good,
            tol: 1e-9,
        }))
        .is_some());
        assert!(verify(cert(Evidence::LinSolveResidual {
            entries,
            dim: 2,
            b,
            x: vec![0.0, 0.0],
            tol: 1e-9,
        }))
        .is_none());
    }

    /// CARE: a non-stabilizing solution (residual small but wrong invariant subspace)
    /// must be rejected — the cert requires PSD + Hurwitz, not just residual.
    #[test]
    fn non_stabilizing_care_rejected() {
        use jeff_math::fmat::FMat;
        // Scalar CARE A=0,B=1,Q=1,R=1: X²=1. X=1 stabilizing (accept); X=-1 anti-
        // stabilizing — same |residual| (0) but closed loop +1 (unstable) → reject.
        let mk = |x: f64| Evidence::AreStabilizing {
            a: vec![0.0],
            b: vec![1.0],
            q: vec![1.0],
            r: vec![1.0],
            x: vec![x],
            n: 1,
            m: 1,
            tol: 1e-9,
        };
        assert!(verify(cert(mk(1.0))).is_some(), "stabilizing X=1 accepted");
        assert!(verify(cert(mk(-1.0))).is_none(), "anti-stabilizing X=-1 rejected");
        let _ = FMat::identity(1);
    }

    /// Sinkhorn: a plan whose marginals miss the target is rejected; the obligation
    /// records the entropic regularization (not exact Wasserstein).
    #[test]
    fn false_sinkhorn_marginals_rejected() {
        use jeff_math::ot::sinkhorn_log;
        let cost = vec![0.0, 1.0, 1.0, 0.0];
        let a = vec![0.5, 0.5];
        let b = vec![0.5, 0.5];
        let (f, g) = sinkhorn_log(&cost, &a, &b, 0.1, 500);
        assert!(verify(cert(Evidence::SinkhornPlan {
            cost: cost.clone(),
            a: a.clone(),
            b: b.clone(),
            eps: 0.1,
            f,
            g,
            tol: 1e-4,
        }))
        .is_some());
        // zero potentials → plan all ones → marginals = 2 ≠ 0.5 → reject.
        assert!(verify(cert(Evidence::SinkhornPlan {
            cost,
            a,
            b,
            eps: 0.1,
            f: vec![0.0, 0.0],
            g: vec![0.0, 0.0],
            tol: 1e-4,
        }))
        .is_none());
    }

    // ===== Stage 4 (Barvinok) tripwires — exact, no tolerance =====

    fn triangle_cs() -> jeff_math::lattice::ConstraintSystem {
        use jeff_math::lattice::{ConstraintSystem, LinIneq, Rel};
        ConstraintSystem {
            n_vars: 2,
            ineqs: vec![
                LinIneq { var: vec![1, 0], param: 0, c: 0, rel: Rel::Ge },
                LinIneq { var: vec![-1, 1], param: 0, c: 0, rel: Rel::Ge },
                LinIneq { var: vec![0, -1], param: 1, c: 0, rel: Rel::Ge },
            ],
            congrs: vec![],
        }
    }

    /// `false_count_rejected`: a quasi-polynomial that disagrees with the exact
    /// brute-force enumeration at any sampled `n` is rejected (F.4).
    #[test]
    fn false_count_rejected() {
        use jeff_math::lattice::ehrhart_interpolate;
        use jeff_math::UniPoly;
        let cs = triangle_cs();
        let qp = ehrhart_interpolate(&cs).unwrap();
        assert!(verify(cert(Evidence::LatticeCount {
            cs: cs.clone(),
            qp: qp.clone(),
            n_lo: 0,
            n_hi: 20,
        }))
        .is_some());
        // tamper the polynomial → mismatches enumeration → reject.
        let mut bad = qp;
        bad.polys[0] = UniPoly::constant(BigRational::from(BigInt::from(999)));
        assert!(verify(cert(Evidence::LatticeCount {
            cs,
            qp: bad,
            n_lo: 0,
            n_hi: 20,
        }))
        .is_none());
    }

    /// `chamber_coverage_check`: a period that does not match the number of residue
    /// polynomials does not partition the parameter line → rejected.
    #[test]
    fn chamber_coverage_rejected() {
        use jeff_math::lattice::{ehrhart_interpolate, QuasiPoly};
        let cs = triangle_cs();
        let good = ehrhart_interpolate(&cs).unwrap();
        // claim period 2 but provide only one residue polynomial → coverage fails.
        let bad = QuasiPoly { period: 2, polys: good.polys.clone() };
        assert!(verify(cert(Evidence::LatticeCount {
            cs,
            qp: bad,
            n_lo: 0,
            n_hi: 5,
        }))
        .is_none());
    }

    /// Tier-A residual certificates serialize → deserialize → re-verify (R25).
    #[test]
    fn tier_a_certs_round_trip() {
        use jeff_math::nbody::{direct_sum, KernelKind};
        let pts = vec![0.0, 1.0, 2.0];
        let chg = vec![1.0, 2.0, 1.0];
        let k = KernelKind::Exponential { decay: 0.7 };
        let evs = vec![
            Evidence::LowRankResidual {
                a: vec![1.0, 0.0, 0.0, 1.0],
                approx: vec![1.0, 0.0, 0.0, 1.0],
                rows: 2,
                cols: 2,
                tol: 1e-9,
            },
            Evidence::FmmResidual {
                points: pts.clone(),
                charges: chg.clone(),
                kernel: k,
                phi: direct_sum(&pts, &chg, k),
                tol: 1e-9,
            },
            Evidence::LinSolveResidual {
                entries: vec![(0, 0, 2.0), (1, 1, 2.0)],
                dim: 2,
                b: vec![2.0, 4.0],
                x: vec![1.0, 2.0],
                tol: 1e-9,
            },
            {
                let cost = vec![0.0, 1.0, 1.0, 0.0];
                let a = vec![0.5, 0.5];
                let b = vec![0.5, 0.5];
                let (f, g) = jeff_math::ot::sinkhorn_log(&cost, &a, &b, 0.1, 400);
                Evidence::SinkhornPlan { cost, a, b, eps: 0.1, f, g, tol: 1e-4 }
            },
            Evidence::AreStabilizing {
                a: vec![0.0],
                b: vec![1.0],
                q: vec![1.0],
                r: vec![1.0],
                x: vec![1.0],
                n: 1,
                m: 1,
                tol: 1e-9,
            },
            {
                let cs = triangle_cs();
                let qp = jeff_math::lattice::ehrhart_interpolate(&cs).unwrap();
                Evidence::LatticeCount { cs, qp, n_lo: 0, n_hi: 12 }
            },
        ];
        for ev in evs {
            let c = cert(ev);
            let json = serde_json::to_string(&c).unwrap();
            let back: jeff_cert::Certificate = serde_json::from_str(&json).unwrap();
            assert!(verify(back).is_some(), "Tier-A cert must re-verify after round-trip");
        }
    }

    #[test]
    fn checker_names_are_honest() {
        assert_eq!(
            checker_name(&Evidence::PolynomialIdentity { poly: Poly::zero() }),
            "exact-coeff-zero"
        );
        assert_eq!(
            checker_name(&Evidence::EigenCharpoly {
                matrix: RatMatrix::identity(2)
            }),
            "cayley-hamilton"
        );
    }
}
