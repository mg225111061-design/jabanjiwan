//! Certificate replay tool (CLAUDE.md R25 / ci/cert_replay.sh).
//!
//! Reads every `*.machine.json` certificate in the given directory, deserializes it
//! to a [`jeff_cert::Certificate`], and **independently re-runs the checker**. Exits
//! nonzero if any certificate fails to re-verify (a certificate that does not
//! actually pass a real check is a P0/P1 violation — DR1).

use jeff_cert::Certificate;
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let Some(dir) = args.next() else {
        eprintln!("usage: replay <dir-of-certificates>");
        return ExitCode::from(2);
    };
    let entries = match std::fs::read_dir(&dir) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("replay: cannot read {dir:?}: {e}");
            return ExitCode::from(2);
        }
    };
    let mut checked = 0usize;
    let mut failed = 0usize;
    let mut paths: Vec<_> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.ends_with(".machine.json"))
        })
        .collect();
    paths.sort(); // deterministic order (R11)
    for path in paths {
        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("replay: read {path:?}: {e}");
                failed += 1;
                continue;
            }
        };
        let cert: Certificate = match serde_json::from_str(&text) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("replay: parse {path:?}: {e}");
                failed += 1;
                continue;
            }
        };
        checked += 1;
        // Independent re-verification from disk (R25).
        match jeff_verify::verify(cert) {
            Some(_) => println!("ok   {}", path.display()),
            None => {
                eprintln!("FAIL {} (certificate did not re-verify)", path.display());
                failed += 1;
            }
        }
    }
    if failed > 0 {
        eprintln!("cert-replay: {failed} of {checked} failed");
        ExitCode::FAILURE
    } else {
        println!("cert-replay OK ({checked} certificates re-verified)");
        ExitCode::SUCCESS
    }
}
