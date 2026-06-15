// Dump JEFF's ML-KEM-768 deterministic outputs for fixed seeds, for FIPS-203 cross-check.
use jeff_math::mlkem;
fn hex(b: &[u8]) -> String { b.iter().map(|x| format!("{x:02x}")).collect() }
fn main() {
    let d = [0x11u8; 32];
    let z = [0x22u8; 32];
    let m = [0x33u8; 32];
    let (ek, dk) = mlkem::keygen(&d, &z);
    let (k, ct) = mlkem::encaps(&ek, &m);
    println!("d {}", hex(&d));
    println!("z {}", hex(&z));
    println!("m {}", hex(&m));
    println!("ek_len {}", ek.len());
    println!("ek_head {}", hex(&ek[..16]));
    println!("ct_len {}", ct.len());
    println!("K_jeff {}", hex(&k));
    // self-roundtrip sanity
    println!("roundtrip {}", mlkem::decaps(&dk, &ct) == k);
}
