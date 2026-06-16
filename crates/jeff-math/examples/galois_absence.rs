//! galois_absence — JEFF closure-ABSENCE certificates (Stage 37.1, galois.rs) as a CLI for
//! Mr.Jeffrey / HARAN closure classification (Stage V2). Proves IMPOSSIBILITY ("no closed form
//! exists"), not "couldn't find it":
//!   galois_absence quintic <a> <b>   → x^5 + a·x + b solvable by radicals?  (Galois, A5 simple)
//!   galois_absence erf               → ∫ e^{-x^2} dx elementary?            (Liouville)
//! output: RADICAL_ABSENT|ELEMENTARY_ABSENT no_closed_form=<bool> witness="..."

use jeff_math::galois::{erf_elementary_absence, quintic_radical_absence};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.len() < 2 {
        eprintln!("usage: galois_absence quintic <a> <b> | galois_absence erf");
        std::process::exit(2);
    }
    match a[1].as_str() {
        "quintic" => {
            let aa: i64 = a[2].parse().expect("a must be an integer");
            let bb: i64 = a[3].parse().expect("b must be an integer");
            let c = quintic_radical_absence(aa, bb);
            println!(
                "RADICAL_ABSENT no_closed_form={} object=\"{}\" class=\"{}\" witness=\"{}\"",
                c.no_closed_form, c.object, c.closure_class, c.witness
            );
        }
        "erf" => {
            let c = erf_elementary_absence();
            println!(
                "ELEMENTARY_ABSENT no_closed_form={} object=\"{}\" class=\"{}\" witness=\"{}\"",
                c.no_closed_form, c.object, c.closure_class, c.witness
            );
        }
        _ => {
            eprintln!("unknown subcommand '{}'", a[1]);
            std::process::exit(2);
        }
    }
}
