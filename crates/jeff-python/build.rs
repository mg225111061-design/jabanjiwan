//! Link libpython only when the `embed` feature is on (Stage 13). Without it the crate is
//! a pure-Rust stub, so the default workspace build needs no Python at all.

fn main() {
    // Cargo sets CARGO_FEATURE_<NAME> for each enabled feature. (cfg! doesn't see crate
    // features inside a build script, so check the env var.)
    if std::env::var_os("CARGO_FEATURE_EMBED").is_none() {
        return;
    }
    let cfg = |var: &str| -> Option<String> {
        let out = std::process::Command::new("python3")
            .args([
                "-c",
                &format!("import sysconfig;print(sysconfig.get_config_var('{var}') or '')"),
            ])
            .output()
            .ok()?;
        let s = String::from_utf8(out.stdout).ok()?.trim().to_string();
        if s.is_empty() {
            None
        } else {
            Some(s)
        }
    };
    let ldversion = cfg("LDVERSION").unwrap_or_else(|| "3".to_string());
    let libdir = cfg("LIBDIR").unwrap_or_else(|| "/usr/lib".to_string());
    println!("cargo:rustc-link-search=native={libdir}");
    println!("cargo:rustc-link-lib=python{ldversion}");
    println!("cargo:rerun-if-changed=build.rs");
}
