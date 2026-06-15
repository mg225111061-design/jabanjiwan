//! Parametric ML-KEM ACVP driver. Args:
//!   keygen <set> <d_hex> <z_hex>            → ek, dk
//!   encaps <set> <ek_hex> <m_hex>           → c, K
//!   decaps <set> <dk_hex> <c_hex>           → K
//! <set> ∈ {512,768,1024}.

use jeff_math::mlkem::KemParams;

fn hx(s: &str) -> Vec<u8> {
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect()
}
fn a32(s: &str) -> [u8; 32] {
    let mut a = [0u8; 32];
    a.copy_from_slice(&hx(s));
    a
}
fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}
fn params(set: &str) -> KemParams {
    match set {
        "512" => KemParams::ML_KEM_512,
        "768" => KemParams::ML_KEM_768,
        "1024" => KemParams::ML_KEM_1024,
        _ => panic!("set must be 512|768|1024"),
    }
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let (mode, set) = (a[1].as_str(), a[2].as_str());
    let p = params(set);
    match mode {
        "keygen" => {
            let (ek, dk) = jeff_math::mlkem::keygen_with(&a32(&a[3]), &a32(&a[4]), &p);
            println!("ek {}", hex(&ek));
            println!("dk {}", hex(&dk));
        }
        "encaps" => {
            let (k, c) = jeff_math::mlkem::encaps_with(&hx(&a[3]), &a32(&a[4]), &p);
            println!("c {}", hex(&c));
            println!("K {}", hex(&k));
        }
        "decaps" => {
            let k = jeff_math::mlkem::decaps_with(&hx(&a[3]), &hx(&a[4]), &p);
            println!("K {}", hex(&k));
        }
        _ => panic!("mode"),
    }
}
