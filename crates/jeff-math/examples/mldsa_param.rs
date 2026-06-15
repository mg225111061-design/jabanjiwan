//! Parametric ML-DSA ACVP driver. Args:
//!   keygen <set> <xi_hex>                         → pk, sk
//!   sign   <set> <mp|mu> <sk_hex> <data_hex> <rnd_hex>  → sig
//!   verify <set> <mp|mu> <pk_hex> <data_hex> <sig_hex>  → true/false
//! <set> ∈ {44,65,87}.

use jeff_math::mldsa::DsaParams;

fn hx(s: &str) -> Vec<u8> {
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect()
}
fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}
fn params(set: &str) -> DsaParams {
    match set {
        "44" => DsaParams::ML_DSA_44,
        "65" => DsaParams::ML_DSA_65,
        "87" => DsaParams::ML_DSA_87,
        _ => panic!("set 44|65|87"),
    }
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let p = params(&a[2]);
    match a[1].as_str() {
        "keygen" => {
            let mut xi = [0u8; 32];
            xi.copy_from_slice(&hx(&a[3]));
            let (pk, sk) = jeff_math::mldsa::keygen_encoded_with(&xi, &p);
            println!("pk {}", hex(&pk));
            println!("sk {}", hex(&sk));
        }
        "sign" => {
            let mode = a[3].as_str();
            let sk = hx(&a[4]);
            let data = hx(&a[5]);
            let mut rnd = [0u8; 32];
            rnd.copy_from_slice(&hx(&a[6]));
            let sig = match mode {
                "mp" => jeff_math::mldsa::sign_encoded_with(&sk, &data, &rnd, &p),
                "mu" => jeff_math::mldsa::sign_encoded_mu_with(&sk, &data, &rnd, &p),
                _ => panic!("mode"),
            };
            println!("sig {}", hex(&sig));
        }
        "verify" => {
            let mode = a[3].as_str();
            let ok = match mode {
                "mp" => jeff_math::mldsa::verify_encoded_with(&hx(&a[4]), &hx(&a[5]), &hx(&a[6]), &p),
                "mu" => jeff_math::mldsa::verify_encoded_mu_with(&hx(&a[4]), &hx(&a[5]), &hx(&a[6]), &p),
                _ => panic!("mode"),
            };
            println!("verify {ok}");
        }
        _ => panic!("cmd"),
    }
}
