use jeff_backend::simd::gemm_blocked;
use std::time::Instant;
fn main(){
    let n=512usize;
    let a:Vec<f64>=(0..n*n).map(|t|{let i=t/n;let j=t%n; (((i*7+j*3)%11) as f64)-5.0}).collect();
    let b:Vec<f64>=(0..n*n).map(|t|{let i=t/n;let j=t%n; (((i*5+j*2)%13) as f64)-6.0}).collect();
    let mut best=f64::MAX; let mut c=Vec::new();
    for _ in 0..5 { let t=Instant::now(); c=gemm_blocked(&a,&b,n,n,n); let e=t.elapsed().as_secs_f64(); if e<best{best=e;} }
    println!("JEFF_GEMM n={n} time_s={best:.9} C00={} C_last={}", c[0], c[n*n-1]);
}
