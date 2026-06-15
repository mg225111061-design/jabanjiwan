use jeff_math::kyber;
fn main(){
    let mut f: Vec<u64> = (0..kyber::N as u64).map(|i| i % kyber::Q).collect();
    kyber::ntt(&mut f);
    let s: Vec<String> = f.iter().take(16).map(|x| x.to_string()).collect();
    println!("JEFF_NTT_head16 {}", s.join(","));
    // checksum to compare full arrays
    let sum: u64 = f.iter().enumerate().map(|(i,&v)| (i as u64+1).wrapping_mul(v)).fold(0u64,|a,b|(a+b)%1000000007);
    println!("JEFF_NTT_checksum {sum}");
}
