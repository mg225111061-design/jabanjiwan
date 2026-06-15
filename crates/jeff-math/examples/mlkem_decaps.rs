use jeff_math::mlkem;
fn unhex(s:&str)->Vec<u8>{(0..s.len()/2).map(|i|u8::from_str_radix(&s[2*i..2*i+2],16).unwrap()).collect()}
fn hx(b:&[u8])->String{b.iter().map(|x|format!("{x:02x}")).collect()}
fn main(){
    let a:Vec<String>=std::env::args().collect();
    let dk=unhex(a[1].trim()); let c=unhex(a[2].trim());
    let k=mlkem::decaps(&dk,&c);
    println!("{}",hx(&k));
}
