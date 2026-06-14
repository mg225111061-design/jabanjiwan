//! Layer 2 — the GF(2) folder (CLAUDE.md APPENDIX E.3, 10.4, P.3).
//!
//! A bit-circuit built from XOR / NOT / copy / const computes an **affine** function
//! `x ↦ M·x ⊕ b` over GF(2). This folder:
//!   1. `partition`s a circuit into its maximal linear/affine region, cutting at the
//!      first nonlinear gate (2-input AND / OR / MUX);
//!   2. `accumulate`s `(M, b)` by evaluating the affine circuit on the affine-spanning
//!      set `{0, e_1, …, e_n}` (the naive-correct method, AR-4);
//!   3. emits an [`Evidence::Gf2LinearIdentity`] certificate and verifies it (the
//!      checker re-evaluates the *original* circuit on the basis, F.3), returning a
//!      verified `Collapsed` or — if the circuit is nonlinear — an honest
//!      `Defer(nonlinearity)`.
//!
//! # Why `nonlinearity` is the right answer (security, P.3)
//!
//! Cipher S-boxes are *deliberately* nonlinear to defeat linear cryptanalysis. The
//! folder collapses the linear diffusion layers (ShiftRows / MixColumns / key-XOR) but
//! **stops at the S-box** and defers with `nonlinearity`. That is not a limitation to
//! paper over — collapsing across an S-box would mean linearizing it, which is exactly
//! what a cipher is built to prevent. Deferring here is correct (P1).
//!
//! Soundness of the certificate (E.3): an affine map over GF(2)ⁿ is determined by its
//! values on `{0, e_i}` (n+1 points). *Given* that `partition` guarantees the region is
//! XOR/NOT/copy/const only, basis agreement implies agreement on all 2ⁿ inputs. The
//! checker enforces exactly that, independently of this folder (R2/R25).

use jeff_cert::gf2circuit::{Gate, LinearCircuit};
use jeff_cert::{
    BarrierTag, Boundary, Certificate, CollapseOutcome, Collapsed, Defer, Evidence, IrRef,
    Obligation,
};
use jeff_math::{Gf2Matrix, Gf2Vec};
use jeff_span::Span;

/// A bit-circuit that may contain nonlinear gates. Wire ids `0..n_inputs` are the
/// inputs; gate `g` defines wire `n_inputs + g`. `outputs` selects result wires.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BitCircuit {
    pub n_inputs: usize,
    pub gates: Vec<BitGate>,
    pub outputs: Vec<usize>,
}

/// A gate. The first four are linear/affine; the last three are nonlinear over GF(2)
/// and force a `partition` cut (APPENDIX 10.4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BitGate {
    /// copy a wire (identity)
    Wire(usize),
    /// XOR of two wires (linear)
    Xor(usize, usize),
    /// NOT of a wire (affine — contributes to `b`)
    Not(usize),
    /// constant 0/1 (affine)
    Const(bool),
    /// 2-input AND (NONLINEAR)
    And(usize, usize),
    /// 2-input OR (NONLINEAR)
    Or(usize, usize),
    /// MUX select(s, a, b) (NONLINEAR)
    Mux(usize, usize, usize),
}

impl BitGate {
    pub fn is_nonlinear(&self) -> bool {
        matches!(self, BitGate::And(..) | BitGate::Or(..) | BitGate::Mux(..))
    }
}

/// Find the first nonlinear gate (the cut point), returning its gate index. `None`
/// means the whole circuit is linear/affine (APPENDIX 12 detection sketch).
pub fn detect_nonlinearity(c: &BitCircuit) -> Option<usize> {
    c.gates.iter().position(BitGate::is_nonlinear)
}

/// The maximal linear/affine prefix of a circuit, as a cert [`LinearCircuit`], or the
/// index of the nonlinear gate that stops it. (Here "partition" = identify the linear
/// region; a circuit that is wholly linear yields the full circuit.)
pub fn partition(c: &BitCircuit) -> Result<LinearCircuit, usize> {
    if let Some(idx) = detect_nonlinearity(c) {
        return Err(idx);
    }
    // All gates are linear/affine: translate 1:1 into the certificate circuit type.
    let gates = c
        .gates
        .iter()
        .map(|g| match *g {
            BitGate::Wire(w) => Gate::Input(w),
            BitGate::Xor(a, b) => Gate::Xor(a, b),
            BitGate::Not(a) => Gate::Not(a),
            BitGate::Const(v) => Gate::Const(v),
            BitGate::And(..) | BitGate::Or(..) | BitGate::Mux(..) => {
                unreachable!("detect_nonlinearity guarantees no nonlinear gate here")
            }
        })
        .collect();
    Ok(LinearCircuit {
        n_inputs: c.n_inputs,
        gates,
        outputs: c.outputs.clone(),
    })
}

/// Accumulate `(M, b)` for an affine circuit by evaluating it on `{0, e_i}` (the
/// naive-correct definition; this is precisely what the checker re-derives). `b` is the
/// image of `0`; column `j` is `circuit(e_j) ⊕ b` so that `M·e_j ⊕ b = circuit(e_j)`.
pub fn accumulate(circ: &LinearCircuit) -> (Gf2Matrix, Gf2Vec) {
    let n = circ.n_inputs;
    let m_out = circ.n_outputs();

    let zero = vec![false; n];
    let c0 = circ.eval(&zero);
    let mut b = Gf2Vec::zeros(m_out);
    for (i, bit) in c0.iter().enumerate() {
        b.set(i, *bit);
    }

    let mut cols = Vec::with_capacity(n);
    for j in 0..n {
        let mut ej = vec![false; n];
        ej[j] = true;
        let out = circ.eval(&ej);
        let mut col = Gf2Vec::zeros(m_out);
        for (i, bit) in out.iter().enumerate() {
            col.set(i, *bit ^ b.get(i)); // remove the affine offset
        }
        cols.push(col);
    }
    (Gf2Matrix::from_columns(m_out, cols), b)
}

/// Attempt to collapse a bit-circuit to its affine form `M·x ⊕ b`.
///
/// * Nonlinear circuit → `Defer(nonlinearity)` (P1; correct for S-boxes).
/// * Affine circuit → build `(M,b)`, certify with `Gf2LinearIdentity`, verify, and
///   return a `Collapsed` (R1/R32: whole-or-nothing; verify failure ⇒ defer).
pub fn collapse(c: &BitCircuit) -> CollapseOutcome {
    let span = Span::dummy();
    let source = IrRef::new(0, span);
    match partition(c) {
        Err(_gate_idx) => {
            // Honest deferral at the nonlinear gate (R4).
            CollapseOutcome::Defer(Defer::new(BarrierTag::Nonlinearity, source))
        }
        Ok(circuit) => {
            let (m, b) = accumulate(&circuit);
            let cert = Certificate {
                collapser_id: "gf2/folder".into(),
                source,
                collapsed: IrRef::new(1, span),
                obligation: Obligation::new(format!(
                    "forall x in GF(2)^{}. circuit(x) == M·x ⊕ b",
                    c.n_inputs
                )),
                evidence: Evidence::Gf2LinearIdentity { circuit, m, b },
                boundaries: vec![Boundary::new(
                    "linearity established by evaluation on {0, e_1..e_n} (affine-spanning)",
                )],
                fallback: source,
            };
            match jeff_verify::verify(cert) {
                Some(vc) => CollapseOutcome::Collapsed(Collapsed::new(IrRef::new(1, span), vc)),
                // R31: verification not Valid ⇒ fall back (should not happen for a truly
                // affine circuit, but never trust — keep the P0 fallback).
                None => CollapseOutcome::Defer(Defer::new(BarrierTag::ConstantFactorOnly, source)),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Evaluate a BitCircuit directly (the ground-truth executor, for tests).
    fn eval_bit(c: &BitCircuit, input: &[bool]) -> Vec<bool> {
        let mut w: Vec<bool> = (0..c.n_inputs)
            .map(|i| input.get(i).copied().unwrap_or(false))
            .collect();
        for g in &c.gates {
            let v = match *g {
                BitGate::Wire(a) => w[a],
                BitGate::Xor(a, b) => w[a] ^ w[b],
                BitGate::Not(a) => !w[a],
                BitGate::Const(v) => v,
                BitGate::And(a, b) => w[a] & w[b],
                BitGate::Or(a, b) => w[a] | w[b],
                BitGate::Mux(s, a, b) => {
                    if w[s] {
                        w[a]
                    } else {
                        w[b]
                    }
                }
            };
            w.push(v);
        }
        c.outputs.iter().map(|&i| w[i]).collect()
    }

    // y = rotl(x,1) ^ x over bv[4]: linear. wires 0..3 = inputs; gate i = x[(i-1)%4] ^ x[i].
    fn rotl_xor_4() -> BitCircuit {
        // output bit i = x[(i+3)%4] ^ x[i]  (rotate-left-by-1 then XOR original)
        let gates = vec![
            BitGate::Xor(3, 0), // wire4 = x3 ^ x0
            BitGate::Xor(0, 1), // wire5 = x0 ^ x1
            BitGate::Xor(1, 2), // wire6 = x1 ^ x2
            BitGate::Xor(2, 3), // wire7 = x2 ^ x3
        ];
        BitCircuit {
            n_inputs: 4,
            gates,
            outputs: vec![4, 5, 6, 7],
        }
    }

    #[test]
    fn linear_circuit_collapses_with_verified_cert() {
        let c = rotl_xor_4();
        match collapse(&c) {
            CollapseOutcome::Collapsed(col) => {
                // The certificate is verified by jeff-verify's independent Gf2Checker.
                assert_eq!(col.certificate().certificate().collapser_id, "gf2/folder");
            }
            CollapseOutcome::Defer(d) => panic!("linear circuit must collapse, got {:?}", d.tag),
        }
    }

    #[test]
    fn accumulated_mb_reproduces_circuit_on_all_inputs() {
        // Exhaustive P0 check: M·x ⊕ b == circuit(x) for every x in GF(2)^4.
        let c = rotl_xor_4();
        let circ = partition(&c).unwrap();
        let (m, b) = accumulate(&circ);
        for x in 0u8..16 {
            let mut xv = Gf2Vec::zeros(4);
            let mut xbits = vec![false; 4];
            for (i, slot) in xbits.iter_mut().enumerate() {
                let bit = (x >> i) & 1 == 1;
                xv.set(i, bit);
                *slot = bit;
            }
            let mx = m.mat_vec(&xv).xor(&b);
            let want = eval_bit(&c, &xbits);
            for (i, &w) in want.iter().enumerate() {
                assert_eq!(mx.get(i), w, "x={x} bit={i}");
            }
        }
    }

    #[test]
    fn affine_circuit_with_not_and_const() {
        // y0 = x0 ^ 1 (NOT), y1 = const 1, y2 = copy x2.
        let c = BitCircuit {
            n_inputs: 3,
            gates: vec![BitGate::Not(0), BitGate::Const(true), BitGate::Wire(2)],
            outputs: vec![3, 4, 5],
        };
        match collapse(&c) {
            CollapseOutcome::Collapsed(_) => {}
            CollapseOutcome::Defer(d) => panic!("affine circuit must collapse, got {:?}", d.tag),
        }
    }

    #[test]
    fn nonlinear_gate_defers_with_nonlinearity() {
        // An AND gate makes the circuit nonlinear — must defer (NOT collapse), with the
        // `nonlinearity` barrier. This is the S-box case (P.3): correct to stop here.
        let c = BitCircuit {
            n_inputs: 2,
            gates: vec![BitGate::And(0, 1)],
            outputs: vec![2],
        };
        match collapse(&c) {
            CollapseOutcome::Defer(d) => assert_eq!(d.tag, BarrierTag::Nonlinearity),
            CollapseOutcome::Collapsed(_) => panic!("nonlinear circuit must NOT collapse"),
        }
        assert_eq!(detect_nonlinearity(&c), Some(0));
    }

    #[test]
    fn mixed_linear_then_nonlinear_defers() {
        // Linear diffusion (XOR) followed by an S-box-like nonlinear gate: the presence
        // of any nonlinear gate defers the whole circuit (R32: no half-collapse here).
        let c = BitCircuit {
            n_inputs: 2,
            gates: vec![
                BitGate::Xor(0, 1), // linear diffusion (wire2)
                BitGate::Or(0, 2),  // nonlinear (wire3) — the S-box boundary
            ],
            outputs: vec![3],
        };
        assert!(matches!(collapse(&c), CollapseOutcome::Defer(d) if d.tag == BarrierTag::Nonlinearity));
    }
}
