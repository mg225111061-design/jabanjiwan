use jeff_math::mlkem;
fn unhex(s:&str)->[u8;32]{let mut a=[0u8;32];for i in 0..32{a[i]=u8::from_str_radix(&s[2*i..2*i+2],16).unwrap();}a}
fn hex(b:&[u8])->String{b.iter().map(|x|format!("{x:02x}")).collect()}
fn main(){
    let a:Vec<String>=std::env::args().collect();
    let (ek,dk)=mlkem::keygen(&unhex(a[1].trim()),&unhex(a[2].trim()));
    println!("{}",hex(&ek));   // line 1 = ek
    eprintln!("{}",hex(&dk));  // stderr = dk
}
