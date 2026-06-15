//! Stage 12 — the sublinear sparse FFT genuinely beats the naive O(n²) DFT (CLAUDE.md
//! PART D). Prony recovery reads only O(K) samples; the naive DFT reads all n and does
//! O(n²) work. Measured (R8/DR2). Correctness checked against the DFT peaks first.

use jeff_backend::measure::compare;
use jeff_math::sparsefft::{fft_peaks, naive_dft_peaks, sinusoid_sample, sparse_fft_prony};

#[test]
#[ignore = "timing benchmark; run with --ignored (correctness is in jeff-math::sparsefft)"]
fn sublinear_sparse_fft_beats_naive_dft() {
    // Prony resolves frequency f over L contiguous samples only when L ≳ n/f (resolution
    // n/L). So pick well-separated mid-range tones and a prefix L = n/16 — still sublinear
    // (1/16 of the signal), O(L·k) work vs the naive O(n²) DFT. (This is contiguous-sample
    // Prony, resolution-limited; HIKP's randomized O(k log n) is the further upgrade, R24.)
    let n = 8192;
    let tones = [256usize, 1024];
    let k = tones.len();
    let prefix_len = n / 16; // 512 ≪ 8192
    let prefix: Vec<f64> = (0..prefix_len).map(|t| sinusoid_sample(&tones, n, t)).collect();

    // correctness: recovered frequencies match the naive DFT peaks (the oracle).
    let full: Vec<f64> = (0..n).map(|t| sinusoid_sample(&tones, n, t)).collect();
    let peaks = naive_dft_peaks(&full, 4);
    let rec = sparse_fft_prony(&prefix, k, n).expect("recover");
    for f in [256usize, 1024, n - 256, n - 1024] {
        assert!(peaks.contains(&f) && rec.contains(&f), "f={f} peaks={peaks:?} rec={rec:?}");
    }

    // measured speedup: sublinear recovery vs the full naive DFT.
    let r = compare(
        3,
        "naive O(n²) DFT",
        || {
            std::hint::black_box(sparse_fft_prony(&prefix, k, n));
        },
        || {
            std::hint::black_box(naive_dft_peaks(&full, 4));
        },
    );
    eprintln!("{}", r.report("sparse-fft(prony, sublinear)"));
    // reading n/16 samples + O(L·k) work must beat the O(n²) DFT decisively.
    assert!(r.speedup() > 10.0, "sublinear sparse FFT should win: {}", r.report("sparse-fft"));

    // the honest test: beat a *fast* O(n log n) FFT too (for k ≪ n).
    let rf = compare(
        20,
        "fast O(n log n) FFT (radix-2, in-house)",
        || {
            std::hint::black_box(sparse_fft_prony(&prefix, k, n));
        },
        || {
            std::hint::black_box(fft_peaks(&full, 4));
        },
    );
    eprintln!("{}", rf.report("sparse-fft vs fast-FFT"));
    assert!(rf.speedup() > 1.0, "should still beat a fast FFT for k≪n: {}", rf.report("vs fft"));
}
