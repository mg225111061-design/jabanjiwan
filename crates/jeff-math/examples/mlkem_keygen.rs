use jeff_math::mlkem;
fn unhex(s:&str)->[u8;32]{let mut a=[0u8;32];for i in 0..32{a[i]=u8::from_str_radix(&s[2*i..2*i+2],16).unwrap();}a}
fn hex(b:&[u8])->String{b.iter().map(|x|format!("{x:02x}")).collect()}
fn main(){
    let args:Vec<String>=std::env::args().collect();
    let d=unhex(args[1].trim()); let z=unhex(args[2].trim());
    let (ek,_dk)=mlkem::keygen(&d,&z);
    println!("ek_jeff_len {}",ek.len());
    println!("ek_jeff_head {}",hex(&ek[..24]));
}
