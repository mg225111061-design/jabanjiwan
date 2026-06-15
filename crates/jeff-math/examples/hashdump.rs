// Dump JEFF's SHA-3/SHAKE outputs as hex for independent cross-check vs Python hashlib.
use jeff_math::keccak::{sha3_256, sha3_512, shake128, shake256};
fn hex(b: &[u8]) -> String { b.iter().map(|x| format!("{x:02x}")).collect() }
fn main() {
    for msg in ["", "abc", "jeff-external-validation-2026", "The quick brown fox"] {
        println!("MSG {msg:?}");
        println!("  sha3_256 {}", hex(&sha3_256(msg.as_bytes())));
        println!("  sha3_512 {}", hex(&sha3_512(msg.as_bytes())));
        println!("  shake128_32 {}", hex(&shake128(msg.as_bytes(), 32)));
        println!("  shake256_64 {}", hex(&shake256(msg.as_bytes(), 64)));
    }
}
