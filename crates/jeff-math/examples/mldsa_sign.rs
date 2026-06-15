//! ML-DSA-44 sign → signature bytes, for ACVP comparison.
//!   mprime mode: cargo run ... -- mp <sk_hex> <mprime_hex> <rnd_hex_32B>
//!   mu mode:     cargo run ... -- mu <sk_hex> <mu_hex>     <rnd_hex_32B>

fn hx(s: &str) -> Vec<u8> {
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect()
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let mode = &a[1];
    let sk = hx(&a[2]);
    let data = hx(&a[3]);
    let mut rnd = [0u8; 32];
    rnd.copy_from_slice(&hx(&a[4]));
    let sig = match mode.as_str() {
        "mp" => jeff_math::mldsa::sign_encoded(&sk, &data, &rnd),
        "mu" => jeff_math::mldsa::sign_encoded_mu(&sk, &data, &rnd),
        _ => panic!("mode must be mp|mu"),
    };
    println!("sig_len {}", sig.len());
    let h: String = sig.iter().map(|x| format!("{x:02x}")).collect();
    println!("sig {h}");
}
