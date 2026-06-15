use jeff_math::dilithium;
fn main(){
    let mut f: Vec<u64> = (0..dilithium::N as u64).map(|i| i % dilithium::Q).collect();
    dilithium::ntt(&mut f);
    println!("JEFF head8: {:?}", &f[..8]);
    let sum: u64 = f.iter().enumerate().map(|(i,&v)| (i as u64+1).wrapping_mul(v)).fold(0u64,|a,b|(a+b)%1000000007);
    println!("JEFF NTT checksum: {sum}");
}
