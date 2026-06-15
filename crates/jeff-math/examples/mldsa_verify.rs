//! ML-DSA-44 verify → "true"/"false", for ACVP sigVer comparison.
//!   mp mode: cargo run ... -- mp <pk_hex> <mprime_hex> <sig_hex>
//!   mu mode: cargo run ... -- mu <pk_hex> <mu_hex>     <sig_hex>

fn hx(s: &str) -> Vec<u8> {
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect()
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let mode = &a[1];
    let pk = hx(&a[2]);
    let data = hx(&a[3]);
    let sig = hx(&a[4]);
    let ok = match mode.as_str() {
        "mp" => jeff_math::mldsa::verify_encoded(&pk, &data, &sig),
        "mu" => jeff_math::mldsa::verify_encoded_mu(&pk, &data, &sig),
        _ => panic!("mode must be mp|mu"),
    };
    println!("verify {ok}");
}
