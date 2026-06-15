//! ML-DSA-44 keyGen → FIPS 204 pk/sk bytes, for byte-for-byte comparison vs NIST ACVP.
//! usage: cargo run -p jeff-math --example mldsa_keygen -- <seed_hex_32B>

fn hex_to_bytes(s: &str) -> Vec<u8> {
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect()
}
fn to_hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let seed = hex_to_bytes(&args[1]);
    let mut xi = [0u8; 32];
    xi.copy_from_slice(&seed);
    let (pk, sk) = jeff_math::mldsa::keygen_encoded(&xi);
    println!("pk_len {}", pk.len());
    println!("pk {}", to_hex(&pk));
    println!("sk_len {}", sk.len());
    println!("sk {}", to_hex(&sk));
}
